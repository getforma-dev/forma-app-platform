"""APP-075 confined checks of a locally built qualification image.

Local Docker only (no AWS, registry or RDS); every container runs with no
network, a read-only root, no capabilities and bounded resources. Creates and
removes only resources named with its run label; the image itself is the
caller's to remove.

Binds: the exported qualification build context (exactly the allowlist), the
service build context (root .dockerignore, compared byte-for-byte with a base
revision), the image configuration, the executable against its build
identity, the source-bundle pin the executable reports against an
independent recomputation from the exported context, the reviewed RDS CA at
the fixed path, and the supervisor's refusals in the real image: `serve` and
other argument lists, unarmed runs, an operator password, mounts over the CA
or program files, and a run with no reachable credential source, both on a
tmpfs /run/genui and, like the CDK task, on the image's own /run/genui
volume with no /tmp mount.

Usage: python3 app/qualification/check_image.py --image IMAGE --output REPORT [--base-revision REV]
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent))
import local_harness as harness  # noqa: E402

REPOSITORY = harness.REPOSITORY
REVIEWED_CA_SHA256 = "90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b"
SUPERVISOR = ["python3", "/app/qualification_supervisor.py"]
CONFINED = ["--network=none", "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=64",
            "--memory=512m", "--memory-swap=512m", "--cpus=1"]
# Most runs use a fresh tmpfs /run/genui and /tmp. The CDK task instead has a
# read-only root, no /tmp and a task volume initialized from the image's
# VOLUME; the image-volume supervisor case mirrors that (Fargate itself is
# APP-078's to observe).
TMPFS = ["--tmpfs=/run/genui:rw,noexec,nosuid,size=1m,uid=65532,gid=65532,mode=0700",
         "--tmpfs=/tmp:rw,noexec,nosuid,size=16m"]
PROBE = r"""
import hashlib, json, os, stat
from pathlib import Path
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
identity = json.loads(Path("/app/qualification/build-identity.json").read_text())
exe = os.stat("/app/qualification/genui-storage-tests")
setid = [str(p) for p in Path("/app").rglob("*") if p.is_file() and os.stat(p).st_mode & (stat.S_ISUID | stat.S_ISGID)]
run = os.stat("/run/genui")
print(json.dumps({"euid": os.geteuid(), "egid": os.getegid(), "ca_sha256": sha("/etc/ssl/certs/rds-global-bundle.pem"),
  "ca_mode": oct(stat.S_IMODE(os.stat("/etc/ssl/certs/rds-global-bundle.pem").st_mode)),
  "executable_sha256": sha("/app/qualification/genui-storage-tests"), "executable_mode": oct(stat.S_IMODE(exe.st_mode)),
  "identity": identity, "supervisor_sha256": sha("/app/qualification_supervisor.py"), "runtime_sha256": sha("/app/runtime.py"),
  "app_files": sorted(str(p) for p in Path("/app").rglob("*") if p.is_file() and "site-packages" not in str(p)),
  "setid_files": setid, "run_directory": [oct(stat.S_IMODE(run.st_mode)), run.st_uid]}, sort_keys=True))
"""


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(args, **kwargs):
    kwargs.setdefault("capture_output", True)
    kwargs.setdefault("timeout", 300)
    return subprocess.run(args, **kwargs)


def files_under(root):
    root = Path(root)
    return {str(path.relative_to(root)): sha256(path.read_bytes()) for path in sorted(root.rglob("*")) if path.is_file()}


def export_qualification_context(destination):
    completed = run(["docker", "build", "--quiet", "-f", "app/qualification/Dockerfile", "--target", "inputs",
                     "--output", "type=local,dest=" + str(destination), "."], cwd=REPOSITORY)
    if completed.returncode != 0:
        raise RuntimeError("qualification context export refused")
    return files_under(destination)


def export_service_context(destination, scratch):
    dockerfile = Path(scratch) / "service-context.Dockerfile"
    dockerfile.write_text("FROM scratch\nCOPY . /\n")
    completed = run(["docker", "build", "--quiet", "-f", str(dockerfile), "--output", "type=local,dest=" + str(destination), "."],
                    cwd=REPOSITORY)
    if completed.returncode != 0:
        raise RuntimeError("service context export refused")
    return files_under(destination)


def allowlist(text=None):
    """The files the qualification ignore file admits. A directory re-include
    (`!dir/`, needed so a nested file can be re-included) names no file, and
    must be followed at once by its `dir/**` exclusion so the idiom can never
    admit a whole directory."""
    if text is None:
        text = (REPOSITORY / "app/qualification/Dockerfile.dockerignore").read_text()
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.startswith("!") and line.endswith("/"):
            if lines[index + 1:index + 2] != [line[1:] + "**"]:
                raise ValueError("directory re-include without its exclusion: " + line)
    return sorted(line[1:] for line in lines if line.startswith("!") and not line.endswith("/"))


def bundle_from(context_root):
    names = harness.source_names()
    binding = b"".join(name.encode() + b"\0" + sha256((Path(context_root) / name).read_bytes()).encode() + b"\n" for name in names)
    return sha256(binding), names


class Checks:
    def __init__(self, image, label):
        self.image, self.label = image, label
        self.containers, self.volumes = [], []
        self.expectations = []
        self.report = {"label": label, "image": image, "scope": "APP-075 local confined qualification-image checks; no AWS/RDS/registry",
                       "passed": False}

    def expect(self, name, condition, detail=None):
        entry = {"expectation": name, "met": bool(condition)}
        if detail is not None:
            entry["detail"] = detail
        self.expectations.append(entry)

    def container(self, name, *args, entrypoint=None, tmpfs=True, remove=True):
        container = "%s-%s" % (self.label, name)
        self.containers.append(container)
        command = ["docker", "run", *(["--rm"] if remove else []), "-i", "--name", container, *CONFINED,
                   *(TMPFS if tmpfs else []), *args]
        if entrypoint:
            command += ["--entrypoint", entrypoint[0]]
        return command, entrypoint[1:] if entrypoint else []

    def docker_run(self, name, *args, entrypoint=None, command=(), input=None, timeout=120, tmpfs=True, remove=True):
        prefix, extra = self.container(name, *args, entrypoint=entrypoint, tmpfs=tmpfs, remove=remove)
        completed = run([*prefix, self.image, *extra, *command], input=input, timeout=timeout)
        text = completed.stdout.decode("utf-8", errors="replace")
        lines = text.splitlines()
        pick = lambda p: [json.loads(line[len(p):]) for line in lines if line.startswith(p)]
        return {"exit_code": completed.returncode, "results": pick(harness.RESULT), "summaries": pick(harness.SUMMARY),
                "stdout": text, "stderr": completed.stderr.decode("utf-8", errors="replace")}

    def contexts(self, scratch, base_revision):
        qualification = export_qualification_context(Path(scratch) / "qualification-context")
        self.report["qualification_context"] = {"count": len(qualification), "files": qualification}
        self.expect("qualification context is exactly the explicit allowlist", sorted(qualification) == allowlist(),
                    {"missing": sorted(set(allowlist()) - set(qualification)), "extra": sorted(set(qualification) - set(allowlist()))})
        rust = sorted(name for name in qualification if name.endswith(".rs"))
        names = harness.source_names()
        self.expect("every compile-time input the entry binds is in the exported context",
                    set(names) <= set(qualification), {"missing": sorted(set(names) - set(qualification))})
        bundle, names = bundle_from(Path(scratch) / "qualification-context")
        self.expect("every compiled Rust source is in the context and in the entry's source bundle",
                    rust == sorted(name for name in names if name.endswith(".rs")))
        self.report["context_source_count"] = len(names)
        self.report["context_source_bundle_sha256"] = bundle
        for name in ("app/certs/rds-global-bundle.pem",):
            self.expect("context carries the reviewed RDS CA bundle", qualification.get(name) == REVIEWED_CA_SHA256)
        service = export_service_context(Path(scratch) / "service-context", scratch)
        self.report["service_context"] = {"count": len(service), "files": service}
        if base_revision:
            differing = []
            for name, digest in service.items():
                shown = run(["git", "show", "%s:%s" % (base_revision, name)], cwd=REPOSITORY)
                if shown.returncode != 0 or sha256(shown.stdout) != digest:
                    differing.append(name)
            base_listing = run(["git", "ls-tree", "-r", "--name-only", base_revision], cwd=REPOSITORY).stdout.decode().splitlines()
            self.report["service_context"]["base_revision"] = base_revision
            self.expect("service context bytes equal the base revision", differing == [], {"differing": differing})
            self.expect("service context has no qualification inputs",
                        not any(name.startswith("app/qualification/") or name.endswith("managed_qualification.rs")
                                or name == "app/runtime/qualification_supervisor.py" for name in service))
            self.report["service_context"]["base_tracked_files"] = len(base_listing)

    def configuration(self):
        info = json.loads(run(["docker", "image", "inspect", self.image]).stdout)[0]
        config = info["Config"]
        self.report["image_identity"] = {"id": info["Id"], "architecture": info["Architecture"], "os": info["Os"],
                                         "size": info["Size"], "user": config["User"], "entrypoint": config["Entrypoint"],
                                         "cmd": config["Cmd"], "volumes": sorted((config.get("Volumes") or {}).keys()),
                                         "exposed_ports": sorted((config.get("ExposedPorts") or {}).keys()),
                                         "env_names": sorted(item.split("=", 1)[0] for item in config.get("Env") or [])}
        identity = self.report["image_identity"]
        self.expect("image runs the supervisor as UID 65532 with only `run`",
                    identity["user"] == "65532:65532" and identity["entrypoint"] == SUPERVISOR and identity["cmd"] == ["run"])
        self.expect("image exposes no port and declares only the private run volume",
                    identity["exposed_ports"] == [] and identity["volumes"] == ["/run/genui"])
        self.expect("image environment carries no credential, endpoint or arming value",
                    not any(name.startswith("PG") or any(word in name for word in ("PASSWORD", "TOKEN", "SECRET", "GENUI_APP008", "GENUI_DATABASE"))
                            for name in identity["env_names"]))

    def probe(self):
        result = self.docker_run("probe", entrypoint=["python3"], command=["-c", PROBE])
        facts = json.loads(result["stdout"].strip().splitlines()[-1]) if result["exit_code"] == 0 else {}
        self.report["probe"] = facts
        repo = lambda path: sha256((REPOSITORY / path).read_bytes())
        self.expect("probe ran as UID 65532", facts.get("euid") == 65532 and facts.get("egid") == 65532)
        self.expect("reviewed RDS CA is pinned at the fixed path, read-only",
                    facts.get("ca_sha256") == REVIEWED_CA_SHA256 and facts.get("ca_mode") == "0o444")
        self.expect("executable matches its build identity, mode 0555",
                    facts.get("executable_sha256") == (facts.get("identity") or {}).get("executable_sha256")
                    and facts.get("executable_mode") == "0o555")
        self.expect("supervisor and runtime helper equal the repository files",
                    facts.get("supervisor_sha256") == repo("app/runtime/qualification_supervisor.py")
                    and facts.get("runtime_sha256") == repo("app/runtime/runtime.py"))
        self.expect("no set-id file under /app", facts.get("setid_files") == [])

    def executable(self):
        result = self.docker_run("entry-unarmed", entrypoint=[harness.EXECUTABLE], command=harness.ARGUMENTS)
        value = result["results"][-1] if result["results"] else {}
        self.report["entry_unarmed"] = {"exit_code": result["exit_code"], "result": value}
        self.expect("executable selected exactly refuses an unarmed run before any store exists",
                    result["exit_code"] != 0 and value.get("error") == "configuration_refused"
                    and value.get("cleanup_applicable") is False and value.get("checks") == []
                    and "refusal" in value and value["refusal"] is None)
        pins = value.get("resource_pins") or {}
        expected = harness.expected_pins()
        self.expect("executable's source-bundle pin equals the recomputation from the exported context",
                    pins.get("source_bundle_sha256") == self.report.get("context_source_bundle_sha256"))
        self.expect("executable's resource pins equal the repository recomputation", pins == expected,
                    {"reported": pins, "expected": expected})
        listing = self.docker_run("entry-list", entrypoint=[harness.EXECUTABLE], command=["--list", "--ignored"])
        ignored = sorted(line[:-len(": test")] for line in listing["stdout"].splitlines() if line.endswith(": test"))
        self.report["ignored_tests"] = ignored
        self.expect("the managed entry is an ignored test of the executable", harness.ENTRY in ignored)

    def supervisor(self):
        manifest = harness.manifest_bytes(str(uuid.uuid4()), REVIEWED_CA_SHA256, harness.expected_pins()).decode()
        armed = ["--env", harness.ARM + "=APP-008", "--env", "GENUI_DATABASE_HOST=" + harness.HOST,
                 "--env", "GENUI_APP008_MANAGED_MANIFEST_JSON=" + manifest]
        volume = self.label + "-mount"
        self.volumes.append(volume)
        cases = [
            ("serve", [], ["serve"], 2, "configuration_refused"),
            ("run-extra-argument", armed, ["run", "--exact"], 2, "configuration_refused"),
            ("default-unarmed", [], [], 2, "configuration_refused"),
            ("operator-password", armed + ["--env", "PGPASSWORD=app075-synthetic-not-a-password"], [], 2, "configuration_refused"),
            ("ca-directory-mount", armed + ["--mount", "type=volume,src=%s,dst=/etc/ssl/certs" % volume], [], 2, "configuration_refused"),
            ("program-directory-mount", armed + ["--tmpfs", "/app/qualification:ro,size=1m"], [], 2, "configuration_refused"),
            ("no-credential-source", armed, [], 3, "credential_refresh_unavailable"),
        ]
        observed = self.report["supervisor_cases"] = []
        for name, args, command, status, error in cases + [("image-volume-no-tmp", armed, [], 3, "credential_refresh_unavailable")]:
            image_volume = name == "image-volume-no-tmp"
            result = self.docker_run("supervisor-" + name, *args, command=command, tmpfs=not image_volume, remove=not image_volume)
            if image_volume:
                mounts = json.loads(run(["docker", "container", "inspect", "%s-supervisor-%s" % (self.label, name)]).stdout)[0]["Mounts"]
                self.volumes.extend(mount["Name"] for mount in mounts if mount.get("Type") == "volume")
                self.report["image_volume_mounts"] = [{key: mount.get(key) for key in ("Type", "Destination", "RW")} for mount in mounts]
                self.expect("image-volume case: the only mount is the image's anonymous /run/genui volume (no tmpfs, no /tmp)",
                            [(mount.get("Type"), mount.get("Destination")) for mount in mounts] == [("volume", "/run/genui")])
            summary = result["summaries"][-1] if result["summaries"] else {}
            observed.append({"case": name, "exit_code": result["exit_code"], "summary": summary,
                             "stderr": result["stderr"].strip().splitlines()[-1:] if result["stderr"].strip() else []})
            self.expect("supervisor %s exits %d with %s and launches nothing" % (name, status, error),
                        result["exit_code"] == status and summary.get("error") == error and summary.get("launched") is False
                        and summary.get("files_removed") is True and result["results"] == []
                        and "app075-synthetic-not-a-password" not in result["stdout"] + result["stderr"])

    def cleanup(self):
        removed = {}
        for container in self.containers:
            run(["docker", "rm", "--force", "--volumes", container])
            removed[container] = run(["docker", "container", "inspect", container]).returncode != 0
        for volume in self.volumes:
            run(["docker", "volume", "rm", "--force", volume])
            removed[volume] = run(["docker", "volume", "inspect", volume]).returncode != 0
        self.report["cleanup"] = removed
        self.report["cleanup_complete"] = all(removed.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--image", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--base-revision", help="revision whose service-context bytes must be unchanged")
    args = parser.parse_args()
    checks = Checks(args.image, "genui-app075-check-" + uuid.uuid4().hex[:10])
    checks.report["recorded_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with tempfile.TemporaryDirectory(prefix="genui-app075-check-") as scratch:
        try:
            checks.contexts(scratch, args.base_revision)
            checks.configuration()
            checks.probe()
            checks.executable()
            checks.supervisor()
        except Exception as error:
            checks.report["error"] = type(error).__name__ + ": " + str(error)
        finally:
            checks.cleanup()
    checks.report["expectations"] = checks.expectations
    checks.report["passed"] = "error" not in checks.report and all(item["met"] for item in checks.expectations)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(checks.report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"passed": checks.report["passed"], "cleanup_complete": checks.report["cleanup_complete"],
                      "output": str(args.output)}))
    raise SystemExit(0 if checks.report["passed"] and checks.report["cleanup_complete"] else 1)


if __name__ == "__main__":
    main()
