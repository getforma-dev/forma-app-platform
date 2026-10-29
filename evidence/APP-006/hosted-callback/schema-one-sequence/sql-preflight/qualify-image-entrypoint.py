"""Independent complete preflight process trial in the cached schema1 image.

The fixed RDS hostname is an alias of an owned local PostgreSQL container on a
Docker INTERNAL network. No AWS connection, real password or image pull occurs.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "app/runtime"))
from qualify_tenant_schema import LocalPostgres, MIGRATIONS

SOURCE = ROOT / "infra/scripts/auth_schema_preflight.py"
spec = importlib.util.spec_from_file_location("reviewed_preflight", SOURCE)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
IMAGE = "877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-synthetic@sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08"


def command(arguments, *, check=True, timeout=30):
    result = subprocess.run(arguments, capture_output=True, text=True, timeout=timeout)
    if check and result.returncode:
        raise AssertionError("owned local fixture command failed")
    return result


def trial():
    image = json.loads(command(["docker", "image", "inspect", IMAGE]).stdout)[0]
    assert image["Config"]["User"] == "65532:65532"
    suffix = uuid.uuid4().hex[:12]
    network, name = "genui-preflight-review-" + suffix, "genui-preflight-review-process-" + suffix
    report = {"scope": "Independent complete process/bootstrap/UID/TLS trial on an owned INTERNAL local Docker network",
              "managed_acceptance": False, "production_hostname_is_only_a_local_container_alias": True,
              "no_real_credentials": True, "image": IMAGE, "local_image_id": image["Id"],
              "image_original_entrypoint": image["Config"]["Entrypoint"],
              "tested_replacement_entrypoint": ["python3"], "image_user": image["Config"]["User"],
              "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(), "passed": False}
    override = helper.emit_override()
    args = json.loads(override)["containerOverrides"][0]["command"]
    assert len(args) == 2 and args[0] == "-c" and len(override.encode()) <= 8192
    report.update(override_bytes=len(override.encode()), override_sha256=hashlib.sha256(override.encode()).hexdigest())
    pg = LocalPostgres()
    network_created = False
    try:
        with tempfile.TemporaryDirectory(prefix="genui-preflight-review-cert-") as private:
            private = Path(private)
            cert, key = private / "server.pem", private / "server.key"
            command(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                     "-subj", "/CN=genui-preflight-owned-local", "-addext", "subjectAltName=DNS:" + helper.HOST,
                     "-keyout", str(key), "-out", str(cert)])
            cert.chmod(0o644)
            with pg:
                for path in (cert, key):
                    command(["docker", "cp", str(path), pg.name + ":/var/lib/postgresql/" + path.name])
                command(["docker", "exec", "--user", "root", pg.name, "chown", "postgres:postgres",
                         "/var/lib/postgresql/server.pem", "/var/lib/postgresql/server.key"])
                command(["docker", "exec", "--user", "root", pg.name, "chmod", "600", "/var/lib/postgresql/server.key"])
                with pg.connect("postgres", autocommit=True) as connection:
                    for setting in ("ssl='on'", "ssl_cert_file='/var/lib/postgresql/server.pem'", "ssl_key_file='/var/lib/postgresql/server.key'"):
                        connection.execute("ALTER SYSTEM SET " + setting)
                    connection.execute("SELECT pg_reload_conf()")
                with pg.connect() as connection:
                    connection.execute((MIGRATIONS / "0001_role_foundation.sql").read_text())
                before = pg.digest()
                command(["docker", "network", "create", "--internal", network])
                network_created = True
                assert json.loads(command(["docker", "network", "inspect", network]).stdout)[0]["Internal"] is True
                command(["docker", "network", "connect", "--alias", helper.HOST, network, pg.name])
                controls = ["docker", "run", "--rm", "--pull=never", "--name", name, "--network", network,
                            "--read-only", "--cap-drop=ALL", "--security-opt=no-new-privileges", "--cpus=1",
                            "--memory=512m", "--memory-swap=512m", "--pids-limit=64", "--tmpfs=/run/genui:rw,noexec,nosuid,size=1m",
                            "--entrypoint=python3"]
                for field, value in {"PGHOST": helper.HOST, "PGPORT": "5432", "PGDATABASE": "genui", "PGUSER": "genui_migration",
                                     "PGSSLMODE": "verify-full", "PGSSLROOTCERT": helper.CA,
                                     "PGPASSWORD": "SYNTHETIC_LOCAL_TRUST_FIXTURE_ONLY"}.items():
                    controls += ["--env", field + "=" + value]
                success = command([*controls, "--mount", "type=bind,src=" + str(cert) + ",dst=" + helper.CA + ",readonly",
                                   IMAGE, *args], check=False, timeout=40)
                assert success.returncode == 0 and success.stderr == ""
                receipt = json.loads(success.stdout)
                assert receipt["passed"] and receipt["database_version"] == 1
                assert receipt["source_sha256"] == report["source_sha256"]
                assert receipt["assertions"]["tls_verify_full"] and receipt["assertions"]["read_only"]
                assert "SYNTHETIC_LOCAL_TRUST_FIXTURE_ONLY" not in success.stdout
                report["local_process_success_receipt"] = receipt
                # With the image's real RDS CA bundle, this local self-signed
                # server must fail verification. No transport fallback exists.
                untrusted = command([*controls, IMAGE, *args], check=False, timeout=40)
                assert untrusted.returncode == 1 and untrusted.stderr == ""
                assert json.loads(untrusted.stdout) == {"scope": helper.SCOPE, "passed": False, "error": "auth_schema_preflight_refused"}
                assert "SYNTHETIC_LOCAL_TRUST_FIXTURE_ONLY" not in untrusted.stdout
                report["untrusted_local_certificate_refused_without_details"] = True
                assert pg.digest() == before
                report["database_logical_dump_unchanged"] = True
                report["passed"] = True
    finally:
        command(["docker", "rm", "--force", "--volumes", name], check=False)
        report["database_container_removed"] = pg.removed
        if network_created:
            report["internal_network_removed"] = command(["docker", "network", "rm", network], check=False).returncode == 0
        remaining = command(["docker", "ps", "--all", "--quiet", "--filter", "name=^/" + name + "$"]).stdout.strip()
        report["preflight_container_removed"] = not remaining
    assert report["database_container_removed"] and report["internal_network_removed"] and report["preflight_container_removed"]
    assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == report["source_sha256"]
    report["limits"] = ["Local Docker DNS alias and synthetic CA deliberately model the fixed production endpoint; this is not an RDS observation.",
                        "Local PostgreSQL uses trust authentication; no real database secret or IAM credential was read.",
                        "Actual ECS task registration/secret wiring, deployed endpoint identity and managed SQL result remain root-owned gates."]
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    output = parser.parse_args().output
    if output.exists():
        raise ValueError("review evidence output already exists")
    result = trial()
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"passed": result["passed"], "override_bytes": result["override_bytes"],
                      "all_owned_resources_removed": result["database_container_removed"] and result["internal_network_removed"] and result["preflight_container_removed"]}))
