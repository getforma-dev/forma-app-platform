"""APP-077 confined checks of a locally built operator image.

Local Docker only (no AWS, registry or RDS). Every operator container runs with
a read-only root, no capabilities, no new privileges and bounded resources;
all but the database sequence also run with no network. Creates and removes
only resources named with its run label; the image itself is the caller's to
remove.

Binds: the exported operator build context (exactly the allowlist, every
tracked parser/ file, every Dockerfile COPY source inside it); the service and
qualification contexts, byte-for-byte equal to a base revision; the image
configuration (UID 65532, entry point and default command equal to the
documented overrides, no port, only the private scratch volume, no credential
or role in the environment); every packaged file against the repository and
the image's own build identity; the parser bytes against the manifest the build
wrote and that manifest's source digest against a recomputation over parser/;
the installed packages against the lock; the reviewed RDS CA at its fixed
path; every documented override (size, container, command parsed inside the
image); fixed refusals; `upload-sources` through the image's own parser,
DuckDB and boto3 against an in-container S3 stub; and the full database
sequence (migrate twice, verify-catalog, seed twice, admission) against a
disposable PostgreSQL 16.13 with TLS on an internal Docker network.

Usage: python3 -P app/operator/check_image.py --image IMAGE --output REPORT [--base-revision REV]
(-P: this directory's operator.py must never shadow the standard library's `operator`.)
"""
import sys

if __name__ == "__main__" and not sys.flags.safe_path:
    raise SystemExit("run with python3 -P: app/operator/operator.py would shadow the standard library's operator module")

import argparse  # noqa: E402
import ast
import datetime
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import uuid

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
REVIEWED_CA_SHA256 = "90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b"
POSTGRES = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
CONFINED = ["--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=64", "--memory=512m",
            "--memory-swap=512m", "--cpus=1", "--tmpfs=/run/operator:rw,noexec,nosuid,size=16m,uid=65532,gid=65532,mode=0700"]
NORTHSTAR, CANARY = "00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"
KEYS = {NORTHSTAR: "arn:aws:kms:us-east-1:123456789012:key/00000000-0000-4000-8000-0000000000a1",
        CANARY: "arn:aws:kms:us-east-1:123456789012:key/00000000-0000-4000-8000-0000000000a2"}
DB_HOST = "pg.app077.test"
SYNTHETIC_PASSWORD = "app077-synthetic-not-a-password"
PROBE = r"""
import hashlib, json, os, stat, sys
from pathlib import Path
import importlib.metadata as md
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
root = Path("/operator")
files = {str(p.relative_to(root)): sha(p) for p in sorted(root.rglob("*")) if p.is_file()}
modes = {str(p.relative_to(root)): oct(stat.S_IMODE(p.stat().st_mode)) for p in sorted(root.rglob("*"))}
setid = [str(p) for p in Path("/operator").rglob("*") if p.is_file() and os.stat(p).st_mode & (stat.S_ISUID | stat.S_ISGID)]
run = os.stat("/run/operator")
print(json.dumps({"euid": os.geteuid(), "egid": os.getegid(), "files": files, "modes": modes, "setid_files": setid,
  "identity": json.loads(Path("/operator/build-identity.json").read_text()),
  "ca_sha256": sha("/etc/ssl/certs/rds-global-bundle.pem"), "ca_mode": oct(stat.S_IMODE(os.stat("/etc/ssl/certs/rds-global-bundle.pem").st_mode)),
  "parser_manifest": json.loads(Path("/operator/bin/parser-manifest.json").read_text()),
  "packages": sorted("%s==%s" % (d.metadata["Name"].lower().replace("_", "-"), d.version) for d in md.distributions()),
  "python": sys.version.split()[0], "run_directory": [oct(stat.S_IMODE(run.st_mode)), run.st_uid]}, sort_keys=True))
"""
OVERRIDES_PROBE = r"""
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("genui_operator", "/operator/app/operator/operator.py")
op = importlib.util.module_from_spec(spec); spec.loader.exec_module(op)
import admission_operator
document = json.loads(op.OVERRIDES_FILE.read_text())
result = []
for item in document["overrides"]:
    command = item["overrides"]["containerOverrides"][0]["command"]
    args = op.parse(command)
    if args.command == "admission":
        admission_operator.validate(admission_operator.parser().parse_args(args.rest))
    result.append({"name": item["name"], "command": args.command})
print(json.dumps({"parsed": result, "admission_subcommands": sorted(admission_operator.parser()._subparsers._group_actions[0].choices)}))
"""


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(args, **kwargs):
    kwargs.setdefault("capture_output", True)
    kwargs.setdefault("timeout", 600)
    return subprocess.run(args, **kwargs)


def files_under(root):
    root = Path(root)
    return {str(path.relative_to(root)): sha256(path.read_bytes()) for path in sorted(root.rglob("*")) if path.is_file()}


def allowlist(text=None):
    """The files the operator ignore file admits. A directory re-include
    (`!dir/`) names no file and must be followed at once by its `dir/**`
    exclusion, so the idiom can never admit a whole directory."""
    if text is None:
        text = (HERE / "Dockerfile.dockerignore").read_text()
    lines = [line for line in text.splitlines() if line and not line.startswith("#")]
    if lines[:1] != ["**"]:
        raise ValueError("allowlist must start by excluding everything")
    for index, line in enumerate(lines):
        if line.startswith("!") and line.endswith("/"):
            if lines[index + 1:index + 2] != [line[1:] + "**"]:
                raise ValueError("directory re-include without its exclusion: " + line)
        elif not line.startswith("!") and line != "**" and not (index and lines[index - 1] == "!" + line[:-2]):
            raise ValueError("unexpected exclusion: " + line)
    return sorted(line[1:] for line in lines if line.startswith("!") and not line.endswith("/"))


def dockerfile_sources(text=None):
    """Context paths named by the final stages' COPY instructions (not --from)."""
    if text is None:
        text = (HERE / "Dockerfile").read_text()
    text = text.replace("\\\n", " ")
    sources = []
    for line in text.splitlines():
        words = line.split()
        if not words or words[0] != "COPY" or any(word.startswith("--from=") for word in words):
            continue
        paths = [word for word in words[1:] if not word.startswith("--")]
        if paths == [".", "/"]:
            continue  # the export-only inputs stage
        sources.extend(paths[:-1])
    return sources


def export_context(dockerfile, destination, scratch=None):
    if dockerfile is None:
        path = Path(scratch) / "service-context.Dockerfile"
        path.write_text("FROM scratch\nCOPY . /\n")
        args = ["docker", "build", "--quiet", "-f", str(path), "--output", "type=local,dest=" + str(destination), "."]
    else:
        args = ["docker", "build", "--quiet", "-f", dockerfile, "--target", "inputs", "--output", "type=local,dest=" + str(destination), "."]
    completed = run(args, cwd=REPOSITORY)
    if completed.returncode != 0:
        raise RuntimeError("context export refused: " + completed.stderr.decode(errors="replace")[-400:])
    return files_under(destination)


def tracked(revision="HEAD"):
    return run(["git", "ls-tree", "-r", "--name-only", revision], cwd=REPOSITORY).stdout.decode().splitlines()


class Checks:
    def __init__(self, image, label):
        self.image, self.label = image, label
        self.containers, self.networks = [], []
        self.expectations = []
        self.report = {"label": label, "image": image, "scope": "APP-077 local confined operator-image checks; no AWS/RDS/registry",
                       "passed": False, "timings_seconds": {}}

    def expect(self, name, condition, detail=None):
        entry = {"expectation": name, "met": bool(condition)}
        if detail is not None:
            entry["detail"] = detail
        self.expectations.append(entry)

    def docker_run(self, name, *args, entrypoint=None, command=(), input=None, network="none", timeout=300):
        container = "%s-%s" % (self.label, name)
        self.containers.append(container)
        argv = ["docker", "run", "--rm", "-i", "--name", container, "--network=" + network, *CONFINED, *args]
        if entrypoint:
            argv += ["--entrypoint", entrypoint[0]]
        argv += [self.image, *(entrypoint[1:] if entrypoint else []), *command]
        started = time.monotonic()
        completed = run(argv, input=input, timeout=timeout)
        return {"exit_code": completed.returncode, "stdout": completed.stdout.decode("utf-8", errors="replace"),
                "stderr": completed.stderr.decode("utf-8", errors="replace"), "seconds": round(time.monotonic() - started, 2)}

    # --- contexts ---------------------------------------------------------------
    def contexts(self, scratch, base_revision):
        started = time.monotonic()
        context = export_context("app/operator/Dockerfile", Path(scratch) / "operator-context")
        names = allowlist()
        self.report["operator_context"] = {"count": len(context), "files": context}
        self.expect("operator context is exactly the explicit allowlist", sorted(context) == names,
                    {"missing": sorted(set(names) - set(context)), "extra": sorted(set(context) - set(names))})
        self.expect("operator context bytes equal the checkout",
                    all(sha256((REPOSITORY / name).read_bytes()) == digest for name, digest in context.items()))
        parser = sorted(name for name in tracked() if name.startswith("parser/"))
        self.expect("operator context carries every tracked parser/ file and no other parser file",
                    sorted(name for name in context if name.startswith("parser/")) == parser)
        sources = dockerfile_sources()
        self.expect("every Dockerfile COPY source is inside the allowlist",
                    all(s in context or any(n.startswith(s.rstrip("/") + "/") for n in context) for s in sources), sources)
        self.expect("operator context carries the reviewed RDS CA", context.get("app/certs/rds-global-bundle.pem") == REVIEWED_CA_SHA256)
        self.expect("operator context carries no test, stub or image-check file",
                    not any(name.endswith(("test_operator.py", "local_s3.py", "check_image.py")) for name in context))
        if base_revision:
            for label, dockerfile in (("service", None), ("qualification", "app/qualification/Dockerfile")):
                files = export_context(dockerfile, Path(scratch) / (label + "-context"), scratch)
                differing = []
                for name, digest in files.items():
                    shown = run(["git", "show", "%s:%s" % (base_revision, name)], cwd=REPOSITORY)
                    if shown.returncode != 0 or sha256(shown.stdout) != digest:
                        differing.append(name)
                self.report[label + "_context"] = {"count": len(files), "base_revision": base_revision}
                self.expect("%s context bytes equal the base revision" % label, differing == [], {"differing": differing})
                self.expect("%s context has no operator inputs" % label, not any(name.startswith("app/operator/") for name in files))
        self.report["timings_seconds"]["contexts"] = round(time.monotonic() - started, 2)

    # --- configuration and files -------------------------------------------------
    def configuration(self):
        info = json.loads(run(["docker", "image", "inspect", self.image]).stdout)[0]
        config = info["Config"]
        document = json.loads((HERE / "overrides.json").read_text())
        identity = self.report["image_identity"] = {
            "id": info["Id"], "architecture": info["Architecture"], "os": info["Os"], "size": info["Size"], "user": config["User"],
            "entrypoint": config["Entrypoint"], "cmd": config["Cmd"], "volumes": sorted((config.get("Volumes") or {}).keys()),
            "exposed_ports": sorted((config.get("ExposedPorts") or {}).keys()),
            "env_names": sorted(item.split("=", 1)[0] for item in config.get("Env") or [])}
        self.expect("image runs as UID 65532 with the documented entry point and default command",
                    identity["user"] == "65532:65532" and identity["entrypoint"] == document["entryPoint"]
                    and identity["cmd"] == document["default_command"])
        self.expect("image exposes no port and declares only the private scratch volume",
                    identity["exposed_ports"] == [] and identity["volumes"] == ["/run/operator"])
        self.expect("image environment carries no credential, endpoint, role or tenant value",
                    not any(name.startswith(("PG", "GENUI_", "AWS_ROLE", "AWS_ACCESS", "AWS_SECRET", "AWS_SESSION", "AWS_ENDPOINT"))
                            or any(word in name for word in ("PASSWORD", "TOKEN", "SECRET")) for name in identity["env_names"]))

    def probe(self):
        result = self.docker_run("probe", entrypoint=["python3", "-I", "-c", PROBE])
        facts = json.loads(result["stdout"].strip().splitlines()[-1]) if result["exit_code"] == 0 else {}
        self.report["probe"] = {k: v for k, v in facts.items() if k not in ("files", "modes")}
        self.report["image_files"] = facts.get("files")
        files = facts.get("files") or {}
        packaged = {name: digest for name, digest in files.items() if not name.startswith("bin/") and name != "build-identity.json"}
        repository = {name: sha256((REPOSITORY / name).read_bytes()) for name in packaged if (REPOSITORY / name).is_file()}
        self.expect("probe ran as UID 65532", facts.get("euid") == 65532 and facts.get("egid") == 65532)
        self.expect("every packaged file equals its repository file", packaged and repository == packaged,
                    {"differing": sorted(name for name in packaged if repository.get(name) != packaged[name])})
        expected = sorted(name for name in allowlist() if not name.startswith(("parser/", "app/operator/Dockerfile", "app/certs/")))
        self.expect("the packaged files are exactly the allowlist's runtime files", sorted(packaged) == expected,
                    {"missing": sorted(set(expected) - set(packaged)), "extra": sorted(set(packaged) - set(expected))})
        identity = (facts.get("identity") or {}).get("files") or {}
        self.expect("build identity names every other packaged file with its digest",
                    identity == {name: digest for name, digest in files.items() if name != "build-identity.json"})
        modes = facts.get("modes") or {}
        directories = self.directories(modes)
        wanted = {name: "0o555" if name in directories or name in ("bin/parser-host", "bin/parser_guest.wasm") else "0o444" for name in modes}
        self.expect("packaged files are read-only: 0444, the two parser binaries and every directory 0555", modes and modes == wanted,
                    {name: mode for name, mode in modes.items() if wanted.get(name) != mode})
        self.expect("no set-id file under /operator", facts.get("setid_files") == [])
        self.expect("private scratch /run/operator is 0700 owned by 65532", facts.get("run_directory") == ["0o700", 65532])
        self.expect("reviewed RDS CA is pinned at the fixed path, read-only",
                    facts.get("ca_sha256") == REVIEWED_CA_SHA256 and facts.get("ca_mode") == "0o444")
        manifest = facts.get("parser_manifest") or {}
        sys.path.insert(0, str(REPOSITORY / "parser"))
        import write_manifest
        tree, count = write_manifest.sources_sha256(REPOSITORY / "parser")
        self.expect("parser bytes equal the manifest the image build wrote",
                    files.get("bin/parser-host") == manifest.get("host", {}).get("sha256")
                    and files.get("bin/parser_guest.wasm") == manifest.get("guest", {}).get("sha256")
                    and manifest.get("host", {}).get("path") == "/operator/bin/parser-host"
                    and manifest.get("guest", {}).get("path") == "/operator/bin/parser_guest.wasm")
        self.expect("parser manifest's source digest equals a recomputation over this checkout's parser/",
                    manifest.get("source", {}).get("sources_sha256") == tree and manifest.get("source", {}).get("source_files") == count)
        self.report["parser"] = {"guest_sha256": manifest.get("guest", {}).get("sha256"), "host_sha256": manifest.get("host", {}).get("sha256"),
                                 "sources_sha256": tree, "rustc": manifest.get("source", {}).get("rustc")}
        locked = sorted(re.findall(r"^([a-z0-9-]+==[0-9][0-9a-z.]*) \\$", (HERE / "requirements.lock").read_text(), re.MULTILINE))
        installed = [p for p in facts.get("packages", []) if p.split("==")[0] not in ("pip", "setuptools", "wheel")]
        self.expect("installed packages are exactly the hash-locked set", installed == locked,
                    {"installed": installed, "locked": locked})

    @staticmethod
    def directories(modes):
        names = set(modes)
        return {name for name in names if any(other.startswith(name + "/") for other in names)}

    def static(self):
        tree = ast.parse((HERE / "operator.py").read_text())
        called = {node.attr for node in ast.walk(tree) if isinstance(node, ast.Attribute)}
        forbidden = {"refresh", "serve", "probe", "scoped_session", "assume_role", "qualification"} & called
        self.expect("operator.py reaches no runtime credential refresh, role assumption or service path", not forbidden, sorted(forbidden))

    # --- overrides and refusals ----------------------------------------------------
    def overrides(self):
        document = json.loads((HERE / "overrides.json").read_text())
        sizes = {item["name"]: len(json.dumps(item["overrides"], separators=(",", ":")).encode()) for item in document["overrides"]}
        self.report["override_bytes"] = sizes
        self.expect("every documented override fits 8,192 bytes", sizes and max(sizes.values()) <= 8192, max(sizes.values()))
        self.expect("every documented override names only the operator container and a command",
                    all(item["overrides"] == {"containerOverrides": [{"name": "operator", "command": item["overrides"]["containerOverrides"][0]["command"]}]}
                        for item in document["overrides"]))
        result = self.docker_run("overrides", entrypoint=["python3", "-I", "-c", OVERRIDES_PROBE])
        parsed = json.loads(result["stdout"].strip().splitlines()[-1]) if result["exit_code"] == 0 else {}
        self.report["overrides_in_image"] = parsed
        self.expect("every documented override's command parses inside the image",
                    [item["name"] for item in parsed.get("parsed", [])] == [item["name"] for item in document["overrides"]], result["stderr"][-300:])

    def refusals(self):
        cases = [
            ("no-arguments", [], [], "usage"),
            ("generic-sql", [], ["sql", "SELECT 1"], "usage"),
            ("service-mode", [], ["serve"], "usage"),
            ("admission-unknown", [], ["admission", "drop-everything"], "usage"),
            ("socket-host", ["--env", "PGHOST=/var/run/postgresql", "--env", "PGUSER=genui_migration", "--env", "PGSSLROOTCERT=/x"], ["migrate"], "socket_host_refused"),
            ("application-identity", ["--env", "PGHOST=localhost", "--env", "PGUSER=genui_workspace", "--env", "PGSSLROOTCERT=/x",
                                      "--env", "PGPASSWORD=" + SYNTHETIC_PASSWORD], ["seed"], "migration_identity_required"),
            ("endpoint-override", ["--env", "GENUI_EVIDENCE_BUCKET=genui-app077-local-stub", "--env", "AWS_REGION=us-east-1",
                                   "--env", "GENUI_OPERATOR_TENANTS_JSON=" + tenants_json(), "--env", "AWS_ENDPOINT_URL_S3=http://127.0.0.1:9"],
             ["upload-sources"], "aws_override_refused"),
        ]
        observed = self.report["refusals"] = []
        for name, args, command, code in cases:
            # An empty command must not fall back to the image's default.
            result = self.docker_run("refuse-" + name, *args, command=command) if command else \
                self.docker_run("refuse-" + name, entrypoint=["python3", "-I", "/operator/app/operator/operator.py"])
            lines = result["stderr"].strip().splitlines()
            got = json.loads(lines[-1]).get("refused") if lines and lines[-1].startswith("{") else None
            observed.append({"case": name, "exit_code": result["exit_code"], "refused": got})
            self.expect("refusal %s exits 1 with %s and prints nothing else" % (name, code),
                        result["exit_code"] == 1 and got == code and result["stdout"] == "" and SYNTHETIC_PASSWORD not in result["stderr"])

    # --- upload-sources through the image's own adapter ----------------------------
    def upload(self):
        driver = (HERE / "local_s3.py").read_text() + "\n" + UPLOAD_DRIVER % {"keys": json.dumps(KEYS), "tenants": tenants_json()}
        result = self.docker_run("upload", entrypoint=["python3", "-I", "-"], input=driver.encode())
        value = json.loads(result["stdout"].strip().splitlines()[-1]) if result["exit_code"] == 0 else {}
        self.report["upload_in_image"] = {k: v for k, v in value.items()} or {"stderr": result["stderr"][-600:]}
        self.report["timings_seconds"]["upload"] = result["seconds"]
        plan = (value.get("plan") or [{}])[0]
        puts = value.get("puts") or []
        keys = ["o/%s/s/%s/r/%s/%s" % (NORTHSTAR, "00000000-0000-4000-8000-00000000006e", "00000000-0000-4000-8000-00000000006f", name)
                for name in ("original", "part.parquet")]
        self.expect("upload-sources in the image writes exactly the two ADR-009 B1 keys, original first",
                    [p["key"] for p in puts] == keys)
        self.expect("every PUT is conditional and carries SSE-KMS with the Northstar tenant key and a matching SHA-256 checksum, signed SigV4",
                    len(puts) == 2 and all(p["sse"] == "aws:kms" and p["kms_key_id"] == KEYS[NORTHSTAR] and p["checksum_matches"]
                                           and p["signed"] and p["conditional"] for p in puts + (value.get("rerun_puts") or [])))
        self.expect("a rerun's conditional PUTs store no second version", value.get("stored_versions") == {key: 1 for key in keys}
                    and [p["key"] for p in value.get("rerun_puts") or []] == keys)
        self.expect("the receipt records the real content and normalized digests (not the seeded placeholders)",
                    plan.get("content_digest") == "sha256:6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647"
                    and plan.get("normalized_digest") not in (None, plan.get("seeded_placeholders", {}).get("normalized_digest"))
                    and value.get("stored_digests") == [o["sha256"] for o in plan.get("objects", [])])
        self.expect("the image's adapter used its manifest-bound parser guest",
                    plan.get("adapter", {}).get("parser_guest_sha256") == "sha256:%s" % (self.report.get("parser") or {}).get("guest_sha256"))
        self.expect("a rerun leaves both objects unchanged", value.get("rerun_states") == ["unchanged", "unchanged"])
        self.expect("the normalized object holds the events_v1 columns, six rows, the 1,140,000-byte S02 window and ZSTD",
                    value.get("parquet") == {"columns": ["time", "destination", "bytes"], "rows": 6, "window_bytes": 1140000, "compression": ["ZSTD"]})

    # --- the database sequence -------------------------------------------------------
    def database(self, scratch):
        started = time.monotonic()
        network = self.label + "-net"
        run(["docker", "network", "create", "--internal", network], timeout=60)
        self.networks.append(network)
        ca = Path(scratch) / "ca"
        ca.mkdir()
        ca.chmod(0o755)
        for args in (["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "1",
                      "-subj", "/CN=APP077 image check CA", "-addext", "basicConstraints=critical,CA:TRUE"],
                     ["openssl", "req", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-out", "server.csr", "-subj", "/CN=" + DB_HOST]):
            run(args, cwd=ca, check=True, timeout=60)
        (ca / "server.ext").write_text("subjectAltName=DNS:%s\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\n"
                                       "extendedKeyUsage=serverAuth\n" % DB_HOST)
        run(["openssl", "x509", "-req", "-in", "server.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial", "-out", "server.pem",
             "-days", "1", "-extfile", "server.ext"], cwd=ca, check=True, timeout=60)
        (ca / "ca.pem").chmod(0o644)
        trust = Path(scratch) / "trust"
        trust.mkdir()
        trust.chmod(0o755)
        (trust / "ca.pem").write_bytes((ca / "ca.pem").read_bytes())
        (trust / "ca.pem").chmod(0o644)
        database = self.label + "-postgres"
        self.containers.append(database)
        run(["docker", "run", "--detach", "--rm", "--name", database, "--network", network, "--network-alias", DB_HOST, "--memory=512m",
             "--cpus=1", "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m", "--env=POSTGRES_HOST_AUTH_METHOD=trust", POSTGRES],
            check=True, timeout=120)
        for _ in range(120):
            if run(["docker", "exec", database, "pg_isready", "--host=127.0.0.1", "--username=postgres"], timeout=30).returncode == 0:
                break
            time.sleep(.25)
        else:
            raise RuntimeError("local PostgreSQL startup deadline")
        for name in ("server.key", "server.pem"):
            run(["docker", "cp", str(ca / name), database + ":/var/lib/postgresql/" + name], check=True)
        run(["docker", "exec", "--user=0", database, "chown", "postgres:postgres", "/var/lib/postgresql/server.key", "/var/lib/postgresql/server.pem"], check=True)
        run(["docker", "exec", "--user=0", database, "chmod", "600", "/var/lib/postgresql/server.key"], check=True)
        setup = ["ALTER SYSTEM SET ssl='on'", "ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'",
                 "ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'", "SELECT pg_reload_conf()",
                 "CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS", "CREATE ROLE rds_iam NOLOGIN",
                 "GRANT rds_iam TO genui_migration WITH ADMIN TRUE", "CREATE DATABASE genui OWNER genui_migration"]
        for statement in setup:
            run(["docker", "exec", database, "psql", "--username=postgres", "--dbname=postgres", "-v", "ON_ERROR_STOP=1", "-c", statement], check=True)
        server = run(["docker", "exec", database, "psql", "--username=postgres", "-At", "-c", "SHOW server_version"], check=True).stdout.decode().strip()
        self.report["database_server_version"] = server
        environment = ["--env", "PGHOST=" + DB_HOST, "--env", "PGUSER=genui_migration", "--env", "PGDATABASE=genui",
                       "--env", "PGSSLROOTCERT=/run/test-ca/ca.pem", "--env", "PGPASSWORD=" + SYNTHETIC_PASSWORD,
                       "--mount", "type=bind,src=%s,dst=/run/test-ca,readonly" % trust]
        steps = [("verify-catalog-before", ["verify-catalog"], 1), ("migrate", ["migrate"], 0), ("migrate-repeat", ["migrate"], 0),
                 ("verify-catalog", ["verify-catalog"], 0), ("seed", ["seed"], 0), ("seed-repeat", ["seed"], 0),
                 ("admission-backfill-northstar", ["admission", "backfill", "--organization", NORTHSTAR], 0),
                 ("admission-backfill-canary", ["admission", "backfill", "--organization", CANARY], 0),
                 ("admission-verify-northstar", ["admission", "verify", "--organization", NORTHSTAR], 0),
                 ("admission-list-northstar", ["admission", "list", "--organization", NORTHSTAR], 0),
                 ("default-command", [], 0)]
        outputs = self.report["database_sequence"] = []
        values = {}
        for name, command, status in steps:
            result = self.docker_run("db-" + name, *environment, command=command, network=network)
            lines = result["stdout"].strip().splitlines()
            value = json.loads(lines[-1]) if result["exit_code"] == 0 and lines else None
            values[name] = value
            refused = result["stderr"].strip().splitlines()[-1:] if result["exit_code"] else []
            outputs.append({"step": name, "command": command, "exit_code": result["exit_code"], "seconds": result["seconds"],
                            "output": value, "stderr": refused})
            self.expect("database step %s exits %d" % (name, status), result["exit_code"] == status, result["stderr"][-300:])
            self.expect("database step %s never prints the synthetic password" % name,
                        SYNTHETIC_PASSWORD not in result["stdout"] + result["stderr"])
        manifest = json.loads((REPOSITORY / "app/migrations/manifest.json").read_text())
        self.expect("migrate applied every landed migration and its repeat is identical",
                    values.get("migrate") and values["migrate"]["database_version"] == manifest["database_version"]
                    and values["migrate"] == values.get("migrate-repeat"))
        self.expect("verify-catalog proves the latest catalog read-only",
                    values.get("verify-catalog") == {"command": "verify-catalog", "catalog_version": manifest["database_version"], "read_only": True}
                    and values.get("default-command") == values.get("verify-catalog"))
        self.expect("seed is exact-match idempotent for both organizations",
                    values.get("seed") and values["seed"] == values.get("seed-repeat") and values["seed"]["organizations"] == [NORTHSTAR, CANARY])
        self.expect("admission passthrough backfills five genesis heads and verifies the chains",
                    sorted((values.get("admission-backfill-northstar") or {}).get("created", [])) == ["admission", "control", "data", "operator", "workspace"]
                    and (values.get("admission-verify-northstar") or {}).get("verified") is True)
        history = run(["docker", "exec", database, "psql", "--username=postgres", "--dbname=genui", "-At", "-c",
                       "SELECT version||':'||sha256 FROM genui_meta.schema_migrations ORDER BY version"]).stdout.decode().split()
        self.expect("migration history equals the manifest", history == ["%d:%s" % (m["version"], m["sha256"]) for m in manifest["migrations"]])
        self.report["timings_seconds"]["database"] = round(time.monotonic() - started, 2)

    def cleanup(self):
        removed = {}
        for container in self.containers:
            run(["docker", "rm", "--force", "--volumes", container], timeout=60)
            removed[container] = run(["docker", "container", "inspect", container], timeout=60).returncode != 0
        for network in self.networks:
            run(["docker", "network", "rm", network], timeout=60)
            removed[network] = run(["docker", "network", "inspect", network], timeout=60).returncode != 0
        self.report["cleanup"] = removed
        self.report["cleanup_complete"] = all(removed.values())


def tenants_json():
    return json.dumps([{"organization_id": org, "kms_key_arn": arn, "prefix": "o/%s/" % org} for org, arn in KEYS.items()])


UPLOAD_DRIVER = r'''
import base64, hashlib, importlib.util, json, tempfile
from pathlib import Path
spec = importlib.util.spec_from_file_location("genui_operator", "/operator/app/operator/operator.py")
op = importlib.util.module_from_spec(spec); spec.loader.exec_module(op)
keys = %(keys)s
tenants = op.load_tenants(%(tenants)r)
plans = op.plan_sources()
with Stub("genui-app077-local-stub", {"o/%%s/" %% org: arn for org, arn in keys.items()}) as stub:
    client = stub.client()
    receipts = op.upload(client, "genui-app077-local-stub", tenants, plans)
    first = len(stub.puts())
    rerun = op.upload(client, "genui-app077-local-stub", tenants, plans)
    stored = {key: versions[-1]["body"] for key, versions in stub.objects.items()}
    versions = {key: len(v) for key, v in stub.objects.items()}
    view = lambda p: {"key": p["key"], "sse": p["headers"].get("x-amz-server-side-encryption"),
                      "kms_key_id": p["headers"].get("x-amz-server-side-encryption-aws-kms-key-id"),
                      "signed": p["headers"].get("authorization", "").startswith("AWS4-HMAC-SHA256 "),
                      "conditional": p["headers"].get("if-none-match") == "*",
                      "checksum_matches": base64.b64decode(p["headers"].get("x-amz-checksum-sha256", "")).hex() == p["body_sha256"]}
    puts = [view(p) for p in stub.puts()[:first]]
    rerun_puts = [view(p) for p in stub.puts()[first:]]
import duckdb
with tempfile.TemporaryDirectory() as directory:
    path = Path(directory) / "part.parquet"
    path.write_bytes(stored[plans[0]["objects"][1]["key"]])
    c = duckdb.connect()
    parquet = {"columns": [r[0] for r in c.execute("DESCRIBE SELECT * FROM read_parquet(?)", [str(path)]).fetchall()],
               "rows": c.execute("SELECT count(*) FROM read_parquet(?)", [str(path)]).fetchone()[0],
               "window_bytes": int(c.execute("SELECT sum(bytes) FROM read_parquet(?) WHERE time >= TIMESTAMP '2026-10-22 10:00:00' AND time < TIMESTAMP '2026-10-22 10:10:00'", [str(path)]).fetchone()[0]),
               "compression": sorted({r[0] for r in c.execute("SELECT compression FROM parquet_metadata(?)", [str(path)]).fetchall()})}
    c.close()
print(json.dumps({"plan": op.public(plans), "receipts": receipts, "rerun_states": [o["state"] for o in rerun[0]["objects"]], "puts": puts,
                  "rerun_puts": rerun_puts, "stored_versions": versions,
                  "stored_digests": ["sha256:" + hashlib.sha256(stored[o["key"]]).hexdigest() for o in plans[0]["objects"]], "parquet": parquet}, sort_keys=True))
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--image", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--base-revision", help="revision whose service and qualification context bytes must be unchanged")
    args = parser.parse_args()
    checks = Checks(args.image, "genui-app077-check-" + uuid.uuid4().hex[:10])
    checks.report["recorded_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with tempfile.TemporaryDirectory(prefix="genui-app077-check-") as scratch:
        try:
            checks.contexts(scratch, args.base_revision)
            checks.configuration()
            checks.probe()
            checks.static()
            checks.overrides()
            checks.refusals()
            checks.upload()
            checks.database(scratch)
        except Exception as error:
            checks.report["error"] = type(error).__name__ + ": " + str(error)
        finally:
            checks.cleanup()
    checks.report["expectations"] = checks.expectations
    checks.report["passed"] = "error" not in checks.report and all(item["met"] for item in checks.expectations)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(checks.report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"passed": checks.report["passed"], "cleanup_complete": checks.report["cleanup_complete"],
                      "met": sum(item["met"] for item in checks.expectations), "expectations": len(checks.expectations),
                      "output": str(args.output)}))
    raise SystemExit(0 if checks.report["passed"] and checks.report["cleanup_complete"] else 1)


if __name__ == "__main__":
    main()
