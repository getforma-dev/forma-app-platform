"""APP-075 local-only positive for the managed qualification entry.

Local Docker only: no AWS, RDS, registry push or public DNS. Creates and
removes only resources named with its run label.

The Linux cfg(test) executable is taken from a locally built qualification
image (app/qualification/Dockerfile builds it in rust:1.93.1-bookworm with
`cargo test --no-run --locked --lib`, selecting the path from Cargo's JSON
artifact metadata) and checked against the image's build identity.
PostgreSQL 16.13 runs with ssl=on and an owned synthetic CA/server
certificate whose only SAN is the exact RDS hostname; it is attached to an
internal Docker network under that hostname as a network alias. The current
migrations and validated seed are applied through the locked Python
environment; the four application roles receive distinct random local
passwords (local SCRAM over TLS, explicitly not IAM). A throwaway run image
derived from the qualification image differs only by the synthetic CA at the
fixed CA path. The entry then runs as UID 65532 with the exact five-argument
invocation, production Settings::new and verified TLS.

Positive runs: (1) the executable directly, private files written 0600 by a
loader that then execs it; (2) the same executable through the actual
qualification supervisor with exactly two labeled substitutions: credential
refresh writes the synthetic local-password envelope instead of STS/IAM
tokens, and the reviewed-CA pin is the synthetic CA's hash.

Negative runs: unarmed, root UID, wrong argument list, an untrusted CA, and a
trusted CA whose certificate does not name the hostname.

Usage: python3 app/qualification/local_harness.py --image IMAGE --output REPORT
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import tempfile
import time
import uuid

REPOSITORY = Path(__file__).resolve().parents[2]
APP = REPOSITORY / "app"
PG_IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
HOST = "genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com"
RESOURCE_ID = "db-KW4LRMTPI5YXHZXF4LELQZKTAU"
SCOPE = "APP-008 isolated application-data-read-only qualification"
ORGANIZATIONS = ["00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"]
EXECUTABLE = "/app/qualification/genui-storage-tests"
ENTRY = "storage::managed_qualification::managed_readonly_entry"
ARGUMENTS = ["--exact", ENTRY, "--ignored", "--nocapture", "--test-threads=1"]
ARM = "GENUI_APP008_MANAGED_QUALIFICATION"
ROLES = ("workspace", "control", "data", "worker")
RESULT = "APP008_MANAGED_RESULT "
SUMMARY = "APP075_SUPERVISOR "
FILES = "APP075_HARNESS_FILES "
def confinement(uid=65532):
    """No network beyond the internal one, read-only root, no capabilities;
    the private run directory is a fresh 0700 tmpfs owned by the run UID."""
    return ["--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--pids-limit=128",
            "--memory=1g", "--memory-swap=1g", "--cpus=2",
            "--tmpfs=/run/genui:rw,noexec,nosuid,size=1m,uid=%d,gid=%d,mode=0700" % (uid, uid),
            "--tmpfs=/tmp:rw,noexec,nosuid,size=16m"]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(args, *, input=None, timeout=120, check=True, cwd=None, env=None, reveal=False):
    """`reveal` adds stderr to a failure only for commands that never see
    credentials (image builds); psql/loader stderr could echo a password."""
    completed = subprocess.run(args, input=input, capture_output=True, timeout=timeout, cwd=cwd, env=env)
    if check and completed.returncode != 0:
        detail = (": " + completed.stderr.decode(errors="replace")[-600:]) if reveal else ""
        raise RuntimeError("command refused: " + " ".join(str(a) for a in args[:3]) + " ... exit %d%s" % (completed.returncode, detail))
    return completed


def docker(*args, **kwargs):
    return run(["docker", *args], **kwargs)


def registry_bytes(run_id):
    bindings = [{"issuer": "app008-fixture-only", "provider_tenant_id": "ten_" + org, "organization_id": org} for org in ORGANIZATIONS]
    return json.dumps({"schema_version": 1, "registry_version": run_id, "bindings": bindings},
                      sort_keys=True, separators=(",", ":")).encode()


def entry_table(name):
    """The body of one `const NAME: [...] = [...];` table of the entry source."""
    text = (APP / "src/storage/managed_qualification.rs").read_text()
    start = text.index("const %s:" % name)
    return text[start:text.index("];", start)]


def source_names():
    """The ordered SOURCES list of the entry, read from its source."""
    return re.findall(r'"((?:app|contracts|spikes)/[^"]+|Cargo\.(?:lock|toml))"', entry_table("SOURCES"))


def seed_inputs():
    """(pin field, repository path) pairs of the entry's SEED_INPUTS table:
    the one path list shared by the entry, its unit test and these tools."""
    return re.findall(r'\(\s*"(\w+)",\s*"([^"]+)",\s*"[0-9a-f]{64}",?\s*\)', entry_table("SEED_INPUTS"))


def expected_pins():
    """Independent recomputation of the entry's resource pins from files."""
    binding = b"".join(name.encode() + b"\0" + sha256((REPOSITORY / name).read_bytes()).encode() + b"\n" for name in source_names())
    manifest = json.loads((APP / "migrations/manifest.json").read_text())
    version = manifest["database_version"]
    read = lambda path: sha256((REPOSITORY / path).read_bytes())
    pins = {"source_bundle_sha256": sha256(binding), "catalog_query_sha256": read("app/migrations/catalog-query.json"),
            "database_version": version, "catalog_sha256": read("app/migrations/catalog-v%d.json" % version),
            "migrations_manifest_sha256": read("app/migrations/manifest.json")}
    pins.update((field, read(path)) for field, path in seed_inputs())
    return pins


def manifest_bytes(run_id, ca_sha, pins, lifetime=600):
    now = int(time.time())
    return json.dumps({"schema_version": 1, "scope": SCOPE, "armed": True, "run_id": run_id, "issued_at": now,
                       "expires_at": now + lifetime, "database_host": HOST, "database_port": 5432, "database_name": "genui",
                       "declared_rds_resource_id": RESOURCE_ID, "organizations": ORGANIZATIONS,
                       "registry_sha256": sha256(registry_bytes(run_id)), "ca_sha256": ca_sha, "pins": pins},
                      separators=(",", ":")).encode()


def credential_envelope(passwords, storage):
    now = int(time.time())
    roles = {name: {"username": "genui_" + name, "token": passwords[name], "expires_at": now + 600} for name in ROLES}
    return {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": now, "expires_at": now + 600,
            "database": {"host": HOST, "port": 5432, "name": "genui", "roles": roles}, "storage": storage}


def synthetic_storage():
    now = int(time.time())
    return {"region": "us-east-1", "bucket": "genui-app075-local-synthetic", "tenants": [
        {"organization_id": org, "role_arn": "arn:aws:iam::877573132739:role/app075-local-synthetic-%d" % i,
         "kms_key_arn": "arn:aws:kms:us-east-1:877573132739:key/%036d" % i, "prefix": "o/" + org + "/",
         "access_key_id": "app075-local-synthetic", "secret_access_key": "app075-local-synthetic",
         "session_token": "app075-local-synthetic", "expires_at": now + 600} for i, org in enumerate(ORGANIZATIONS)]}


# Runs inside the run container as UID 65532: writes the private files (0600,
# O_EXCL) into the tmpfs /run/genui, records UID/modes, then execs the exact
# executable with the given argument list. Input arrives on stdin, never argv.
LOADER = r"""
import base64, json, os, stat, sys
d = json.load(sys.stdin)
for name, data in d["files"].items():
    fd = os.open("/run/genui/" + name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    os.write(fd, base64.b64decode(data)); os.close(fd)
facts = {"euid": os.geteuid(), "egid": os.getegid(), "run_directory": oct(stat.S_IMODE(os.stat("/run/genui").st_mode)),
         "run_directory_uid": os.stat("/run/genui").st_uid,
         "files": {n: [oct(stat.S_IMODE(os.lstat("/run/genui/" + n).st_mode)), os.lstat("/run/genui/" + n).st_uid] for n in sorted(d["files"])},
         "ca_mode": oct(stat.S_IMODE(os.stat("/etc/ssl/certs/rds-global-bundle.pem").st_mode)),
         "argv": d["args"]}
sys.stdout.write("APP075_HARNESS_FILES " + json.dumps(facts, sort_keys=True) + "\n"); sys.stdout.flush()
os.execv(d["exe"], [d["exe"], *d["args"]])
"""

# Runs the actual supervisor with the two labeled substitutions.
SUPERVISED = r"""
import json, os, sys, time, uuid
sys.path.insert(0, "/app")
import runtime, qualification_supervisor as supervisor
d = json.load(sys.stdin)
def local_refresh(path):
    # Substitution 1: synthetic local-password envelope instead of STS/IAM.
    now = int(time.time())
    roles = {n: {"username": "genui_" + n, "token": d["passwords"][n], "expires_at": now + 600} for n in ("workspace", "control", "data", "worker")}
    storage = d["storage"]
    for tenant in storage["tenants"]:
        tenant["expires_at"] = now + 600
    runtime.atomic_write(path, {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": now, "expires_at": now + 600,
                                "database": {"host": d["host"], "port": 5432, "name": "genui", "roles": roles}, "storage": storage})
environ = dict(os.environ)
environ[supervisor.MANIFEST_ENV] = d["manifest"]
# Substitution 2: the reviewed-CA pin is the synthetic CA's hash.
sys.exit(supervisor.supervise(["run"], environ, refresh=local_refresh, reviewed_ca=d["ca_sha256"]))
"""


def parse_output(stdout):
    lines = stdout.decode("utf-8", errors="replace").splitlines()
    pick = lambda prefix: [json.loads(line[len(prefix):]) for line in lines if line.startswith(prefix)]
    return {"results": pick(RESULT), "summaries": pick(SUMMARY), "facts": pick(FILES),
            "checks_printed": sum(1 for line in lines if "APP008 check: " in line), "lines": len(lines)}


class Harness:
    def __init__(self, image, python, label):
        self.image, self.python, self.label = image, python, label
        self.directory = Path(tempfile.mkdtemp(prefix="genui-app075-harness-")).resolve()
        os.chmod(self.directory, 0o700)
        self.pg = label + "-pg"
        self.network = label + "-net"
        self.images = []
        self.containers = [self.pg, label + "-extract"]
        self.passwords = {name: secrets.token_urlsafe(24) for name in ROLES}
        self.report = {"label": label, "scope": "APP-075 local-only positive; synthetic CA, local SCRAM, no AWS/RDS", "passed": False}

    def openssl(self, *args):
        run(["openssl", *args], cwd=self.directory)

    def tls(self):
        ext = "basicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n"
        (self.directory / "rds.ext").write_text("subjectAltName=DNS:" + HOST + "\n" + ext)
        (self.directory / "localhost.ext").write_text("subjectAltName=DNS:localhost\n" + ext)
        for name, subject in (("ca", "/CN=APP-075 synthetic local CA"), ("wrong", "/CN=APP-075 untrusted CA")):
            self.openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", name + ".key", "-out", name + ".pem",
                         "-days", "1", "-subj", subject, "-addext", "basicConstraints=critical,CA:TRUE")
        for name in ("rds", "localhost"):
            self.openssl("req", "-newkey", "rsa:2048", "-nodes", "-keyout", name + "-server.key", "-out", name + ".csr", "-subj", "/CN=" + name)
            self.openssl("x509", "-req", "-in", name + ".csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial",
                         "-out", name + "-server.pem", "-days", "1", "-extfile", name + ".ext")
        self.ca_sha = sha256((self.directory / "ca.pem").read_bytes())
        self.wrong_sha = sha256((self.directory / "wrong.pem").read_bytes())
        self.report["synthetic_ca_sha256"] = self.ca_sha
        self.report["untrusted_ca_sha256"] = self.wrong_sha

    def executable(self):
        info = json.loads(docker("image", "inspect", self.image).stdout)[0]
        self.report["qualification_image"] = {"id": info["Id"], "architecture": info["Architecture"], "os": info["Os"],
                                              "user": info["Config"]["User"], "entrypoint": info["Config"]["Entrypoint"],
                                              "cmd": info["Config"]["Cmd"]}
        extract = self.label + "-extract"
        docker("create", "--name", extract, self.image)
        try:
            docker("cp", extract + ":" + EXECUTABLE, str(self.directory / "genui-storage-tests"))
            docker("cp", extract + ":/app/qualification/build-identity.json", str(self.directory / "build-identity.json"))
        finally:
            docker("rm", "--force", "--volumes", extract, check=False)  # its anonymous /run/genui volume too
        identity = json.loads((self.directory / "build-identity.json").read_text())
        digest = sha256((self.directory / "genui-storage-tests").read_bytes())
        if digest != identity["executable_sha256"]:
            raise RuntimeError("executable differs from its build identity")
        self.report["executable"] = {"sha256": digest, "identity": identity}

    def postgres(self):
        docker("run", "--detach", "--rm", "--name", self.pg, "--publish", "127.0.0.1::5432", "--memory=512m", "--cpus=1",
               "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m", "--env=POSTGRES_HOST_AUTH_METHOD=trust", PG_IMAGE)
        port = int(docker("port", self.pg, "5432/tcp").stdout.decode().strip().rsplit(":", 1)[1])
        end = time.time() + 30
        while docker("exec", self.pg, "pg_isready", "--host=127.0.0.1", "--username=postgres", check=False).returncode != 0:
            if time.time() > end:
                raise RuntimeError("PostgreSQL ready deadline")
            time.sleep(0.2)
        # The image initializes, restarts and only then serves on TCP.
        while docker("exec", self.pg, "psql", "-h", "127.0.0.1", "-U", "postgres", "-c", "SELECT 1", check=False).returncode != 0:
            if time.time() > end:
                raise RuntimeError("PostgreSQL TCP deadline")
            time.sleep(0.2)
        version = self.sql("SHOW server_version", "postgres").strip()
        if version.split()[0] != "16.13":
            raise RuntimeError("PostgreSQL version refused")
        self.report["postgres"] = {"image": PG_IMAGE, "server_version": version}
        self.server_certificate("rds")
        self.sql("ALTER SYSTEM SET ssl='on'; ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'; "
                 "ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'; SELECT pg_reload_conf();", "postgres")
        self.sql("CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS; CREATE ROLE rds_iam NOLOGIN; "
                 "GRANT rds_iam TO genui_migration WITH ADMIN TRUE;", "postgres")
        self.sql("CREATE DATABASE genui OWNER genui_migration;", "postgres")
        script = ("import sys\nfrom pathlib import Path\nimport psycopg\nimport runtime\nfrom tenant_seed import make_bundles,seed\n"
                  "with psycopg.connect(host='127.0.0.1',port=int(sys.argv[1]),user='genui_migration',dbname='genui',connect_timeout=3) as c:\n"
                  " runtime.apply_migrations(c,Path(sys.argv[2]))\n seed(c,make_bundles())\n")
        env = dict(os.environ)
        for site in sorted((REPOSITORY / "contracts/.venv/lib").glob("python*/site-packages")):
            env["PYTHONPATH"] = str(site)
        run([self.python, "-c", script, str(port), str(APP / "migrations")], cwd=APP / "runtime", env=env, timeout=180)
        version_row = self.sql("SELECT version FROM genui_meta.schema_version WHERE singleton", "genui").strip()
        self.report["postgres"]["schema_version"] = int(version_row)
        self.report["postgres"]["migrations"] = [m["file"] for m in json.loads((APP / "migrations/manifest.json").read_text())["migrations"]]
        statements = "".join("ALTER ROLE genui_%s PASSWORD '%s';" % (name, self.passwords[name]) for name in ROLES)
        self.sql(statements, "genui")
        docker("exec", "--user=postgres", self.pg, "sh", "-c",
               "printf 'local all all trust\\nhostssl all all all scram-sha-256\\nhost all all all reject\\n' > /var/lib/postgresql/data/pg_hba.conf")
        self.sql("SELECT pg_reload_conf();", "genui")
        self.report["postgres"]["authentication"] = "hostssl scram-sha-256 only; plaintext TCP rejected; distinct random per-role local passwords (not IAM)"
        docker("network", "create", "--internal", self.network)
        docker("network", "connect", "--alias", HOST, self.network, self.pg)
        network = json.loads(docker("network", "inspect", self.network).stdout)[0]
        self.report["network"] = {"internal": network["Internal"], "driver": network["Driver"], "alias": HOST}
        if network["Internal"] is not True:
            raise RuntimeError("network must be internal")

    def server_certificate(self, name):
        for source, target in ((name + "-server.key", "server.key"), (name + "-server.pem", "server.pem")):
            docker("cp", str(self.directory / source), self.pg + ":/var/lib/postgresql/" + target)
        docker("exec", "--user=0", self.pg, "chown", "postgres:postgres", "/var/lib/postgresql/server.key", "/var/lib/postgresql/server.pem")
        docker("exec", "--user=0", self.pg, "chmod", "600", "/var/lib/postgresql/server.key")

    def sql(self, statement, database):
        return docker("exec", "-i", self.pg, "psql", "--no-psqlrc", "--set=ON_ERROR_STOP=1", "--tuples-only", "--no-align",
                      "--username=postgres", "--dbname", database, "--file=-", input=statement.encode()).stdout.decode()

    def run_image(self, ca_file, suffix):
        context = self.directory / ("run-image-" + suffix)
        context.mkdir()
        shutil.copyfile(self.directory / ca_file, context / "ca.pem")
        (context / "Dockerfile").write_text("FROM %s\nCOPY --chmod=0444 ca.pem /etc/ssl/certs/rds-global-bundle.pem\n" % self.image)
        tag = "%s-run-%s:local" % (self.label, suffix)
        self.images.append(tag)
        docker("build", "--quiet", "--tag", tag, str(context), timeout=300, reveal=True)
        # The base must still be the inspected qualification image.
        if json.loads(docker("image", "inspect", self.image).stdout)[0]["Id"] != self.report["qualification_image"]["id"]:
            raise RuntimeError("qualification image changed during the run")
        return tag

    def entry(self, image, *, args=ARGUMENTS, user="65532:65532", armed=True, ca_sha=None, name):
        run_id = str(uuid.uuid4())
        files = {"app008-managed-run.json": manifest_bytes(run_id, ca_sha or self.ca_sha, self.pins),
                 "app008-managed-registry.json": registry_bytes(run_id),
                 "credentials.json": json.dumps(credential_envelope(self.passwords, {}), separators=(",", ":")).encode()}
        payload = json.dumps({"exe": EXECUTABLE, "args": list(args),
                              "files": {k: base64.b64encode(v).decode() for k, v in files.items()}}).encode()
        container = "%s-%s" % (self.label, name)
        self.containers.append(container)
        command = ["run", "--rm", "-i", "--name", container, "--network", self.network, "--user", user,
                   *confinement(int(user.split(":")[0]))]
        if armed:
            command += ["--env", ARM + "=APP-008"]
        command += ["--entrypoint", "python3", image, "-c", LOADER]
        started = time.time()
        completed = docker(*command, input=payload, check=False, timeout=660)
        output = parse_output(completed.stdout)
        return {"name": name, "run_id": run_id, "manifest_sha256": sha256(files["app008-managed-run.json"]),
                "registry_sha256": sha256(files["app008-managed-registry.json"]), "exit_code": completed.returncode,
                "seconds": round(time.time() - started, 2), "user": user, "armed": armed, "arguments": list(args),
                "facts": output["facts"], "result": output["results"][-1] if output["results"] else None,
                "result_lines": len(output["results"]), "checks_printed": output["checks_printed"],
                "stderr_bytes": len(completed.stderr)}

    def supervised(self, image):
        run_id = str(uuid.uuid4())
        manifest = manifest_bytes(run_id, self.ca_sha, self.pins).decode()
        payload = json.dumps({"manifest": manifest, "passwords": self.passwords, "storage": synthetic_storage(),
                              "host": HOST, "ca_sha256": self.ca_sha}).encode()
        container = self.label + "-supervised"
        self.containers.append(container)
        command = ["run", "--rm", "-i", "--name", container, "--network", self.network, "--user", "65532:65532", *confinement(),
                   "--env", ARM + "=APP-008", "--env", "GENUI_DATABASE_HOST=" + HOST, "--entrypoint", "python3", image, "-c", SUPERVISED]
        started = time.time()
        completed = docker(*command, input=payload, check=False, timeout=660)
        output = parse_output(completed.stdout)
        return {"name": "supervised_positive", "run_id": run_id, "manifest_sha256": sha256(manifest.encode()),
                "exit_code": completed.returncode, "seconds": round(time.time() - started, 2),
                "substitutions": ["credential refresh writes the synthetic local-password envelope instead of STS/IAM tokens",
                                  "reviewed-CA pin is the synthetic local CA hash instead of 90a6f1a7..."],
                "result": output["results"][-1] if output["results"] else None, "summary": output["summaries"][-1] if output["summaries"] else None,
                "stderr_bytes": len(completed.stderr)}

    def execute(self):
        self.pins = expected_pins()
        self.report["expected_pins"] = self.pins
        self.tls()
        self.executable()
        self.postgres()
        positive_image = self.run_image("ca.pem", "synthetic-ca")
        untrusted_image = self.run_image("wrong.pem", "untrusted-ca")
        runs = self.report["runs"] = []
        positive = self.entry(positive_image, name="positive")
        runs.append(positive)
        runs.append(self.supervised(positive_image))
        runs.append(self.entry(positive_image, name="unarmed", armed=False))
        runs.append(self.entry(positive_image, name="root-uid", user="0:0"))
        runs.append(self.entry(positive_image, name="wrong-arguments", args=ARGUMENTS[:3] + ARGUMENTS[4:]))
        runs.append(self.entry(untrusted_image, name="untrusted-ca", ca_sha=self.wrong_sha))
        self.server_certificate("localhost")
        self.sql("SELECT pg_reload_conf();", "genui")
        runs.append(self.entry(positive_image, name="hostname-mismatch"))
        self.verdict()

    def verdict(self):
        runs = {item["name"]: item for item in self.report["runs"]}
        expectations = []
        def expect(name, condition):
            expectations.append({"expectation": name, "met": bool(condition)})
        positive = runs["positive"]["result"] or {}
        expect("positive passed with six checks and confirmed cleanup", runs["positive"]["exit_code"] == 0
               and positive.get("passed") is True and len(positive.get("checks", [])) == 6
               and positive.get("cleanup_confirmed") is True and positive.get("run_id") == runs["positive"]["run_id"])
        expect("positive reports exactly the independently recomputed pins", positive.get("resource_pins") == self.pins)
        facts = (runs["positive"]["facts"] or [{}])[0]
        expect("positive ran as UID 65532 with 0700 run directory and 0600 private files",
               facts.get("euid") == 65532 and facts.get("run_directory") == "0o700"
               and all(mode == "0o600" and uid == 65532 for mode, uid in facts.get("files", {}).values()))
        expect("positive used the exact five-argument invocation", facts.get("argv") == ARGUMENTS)
        supervised = runs["supervised_positive"]
        result, summary = supervised["result"] or {}, supervised["summary"] or {}
        expect("supervised positive passed with six checks and confirmed cleanup", supervised["exit_code"] == 0
               and result.get("passed") is True and len(result.get("checks", [])) == 6 and result.get("cleanup_confirmed") is True
               and summary.get("passed") is True and summary.get("files_removed") is True and summary.get("refreshes", 0) >= 1)
        for name, error, applicable in (("unarmed", "configuration_refused", False), ("root-uid", "configuration_refused", False),
                                        ("wrong-arguments", "configuration_refused", False),
                                        ("untrusted-ca", "case_refused", True), ("hostname-mismatch", "case_refused", True)):
            item = runs[name]["result"] or {}
            expect(name + " refused with " + error, runs[name]["exit_code"] != 0 and item.get("passed") is False
                   and item.get("error") == error and item.get("cleanup_applicable") is applicable
                   and item.get("cleanup_confirmed") is applicable and item.get("checks") == [])
        self.report["expectations"] = expectations
        self.report["passed"] = all(item["met"] for item in expectations)

    def cleanup(self):
        removed = {}
        for container in self.containers:
            docker("rm", "--force", "--volumes", container, check=False)
            removed[container] = docker("container", "inspect", container, check=False).returncode != 0
        docker("network", "rm", self.network, check=False)
        removed[self.network] = docker("network", "inspect", self.network, check=False).returncode != 0
        for image in self.images:
            docker("image", "rm", "--force", image, check=False)
            removed[image] = docker("image", "inspect", image, check=False).returncode != 0
        shutil.rmtree(self.directory, ignore_errors=True)
        removed["private_directory"] = not self.directory.exists()
        self.report["cleanup"] = removed
        self.report["cleanup_complete"] = all(removed.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--image", required=True, help="locally built qualification image (tag or id)")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--python", default=str(APP / "runtime/.venv/bin/python"), help="locked runtime Python for migrations and seed")
    args = parser.parse_args()
    label = "genui-app075-" + uuid.uuid4().hex[:10]
    harness = Harness(args.image, args.python, label)
    harness.report["recorded_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    try:
        harness.execute()
    except Exception as error:  # Retain the stage; no command output with credentials is recorded.
        harness.report["error"] = type(error).__name__ + ": " + str(error)
    finally:
        harness.cleanup()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(harness.report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"passed": harness.report["passed"], "cleanup_complete": harness.report.get("cleanup_complete"),
                      "output": str(args.output)}))
    raise SystemExit(0 if harness.report["passed"] and harness.report.get("cleanup_complete") else 1)


if __name__ == "__main__":
    main()
