"""APP-073 operator command tests (ADR-007 D7.17, D7.19). No AWS/RDS use.

PureTests need nothing. DatabaseTests start one disposable PostgreSQL 16.13
container on a random loopback port with a throwaway TLS certificate, apply the
reviewed migrations and drive the real command line as genui_migration over
verify-full TLS, exactly as an operator would; they are skipped when Docker or
OpenSSL is absent. Every container they start is removed in tearDownClass.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

import admission_operator as op
import runtime

HERE = Path(__file__).resolve().parent
IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
# Shared with app/src/storage/admission.rs unit tests: the Rust storage digest
# and this operator digest must agree byte for byte.
VECTOR_RAW = bytes(range(32))
VECTOR_CODE = "AAAQ-EAYE-AUDA-OCAJ-BIFQ-YDIO-B4IB-CEQT-CQKR-MFYY-DENB-WHA5-DYPQ"
VECTOR_DIGEST = "sha256:3d6abee22a05d2ade33f3017a45c52630d05311687d4f77a28601bb2343752cc"
HANDLE_RAW = bytes(range(100, 116))
HANDLE = "mrswmz3infvgw3dnnzxxa4lsom"
HANDLE_DIGEST = "sha256:4a08c5c04799367f81a02414698be03c99f175ca70405e354ebd814a672e7c22"
ORG = "00000000-0000-4000-8000-0000000000a1"
ISSUER = "operator-test-issuer"
TENANT = "ten_operator_test"


class PureTests(unittest.TestCase):
    def test_code_encoding_digest_and_normalization_vector(self):
        self.assertEqual(op.encode_code(VECTOR_RAW), VECTOR_CODE)
        self.assertEqual(op.code_digest(VECTOR_RAW), VECTOR_DIGEST)
        for spelling in (VECTOR_CODE, VECTOR_CODE.lower(), VECTOR_CODE.replace("-", ""), " " + VECTOR_CODE.replace("-", " ") + "\n"):
            self.assertEqual(op.normalize_code(spelling), VECTOR_RAW)
        # Independent recomputation of the documented construction.
        expected = "sha256:" + hashlib.sha256(b"forma:access-code:v1\x00" + VECTOR_RAW).hexdigest()
        self.assertEqual(expected, VECTOR_DIGEST)

    def test_malformed_codes_normalize_to_nothing(self):
        compact = VECTOR_CODE.replace("-", "")
        for bad in ("", compact[:-1], compact + "A", compact[:-1] + "R", compact[:-1] + "1", compact.replace("A", "0", 1), "x" * 400):
            with self.subTest(bad=bad[:12]):
                self.assertIsNone(op.normalize_code(bad))

    def test_handle_encoding_and_digest_vector(self):
        self.assertEqual(op.encode_handle(HANDLE_RAW), HANDLE)
        self.assertEqual(op.handle_digest(HANDLE_RAW), HANDLE_DIGEST)
        self.assertEqual(base64.b32decode(HANDLE.upper() + "======"), HANDLE_RAW)

    def test_defaults_are_adr_007_d7_17(self):
        self.assertEqual(op.DEFAULTS, {
            "trial_entitlement_days": 30, "code_expiry_days_default": 14, "code_expiry_days_max": 30,
            "max_redemptions_default": 1, "max_redemptions_max": 10, "presignup_reservation_minutes": 10,
            "presignup_reservations_live_max": 10, "pending_admission_hours": 24, "list_rows_max": 100,
            "code_secret_bytes": 32, "handle_bytes": 16})

    def test_grant_parsing_is_closed_and_refuses_admin_and_review(self):
        app, work, source = (str(uuid.uuid4()) for _ in range(3))
        self.assertEqual(op.parse_grant("discover:" + app), ("discover", app, None, None))
        self.assertEqual(op.parse_grant("record_read:%s:%s" % (app, work)), ("record_read", app, work, None))
        self.assertEqual(op.parse_grant("source_read:" + source), ("source_read", None, None, source))
        for bad in ("admin_policy", "admin_policy:" + app, "review_publish:" + app, "owner:" + app, "record_read:" + app,
                    "discover:%s:%s" % (app, work), "source_read:not-a-uuid", "discover:" + app.upper(), ""):
            with self.subTest(bad=bad[:20]), self.assertRaises(op.Refused):
                op.parse_grant(bad)

    def test_issue_argument_bounds(self):
        parser = op.parser()
        base = ["issue", "--organization", ORG, "--issued-by", "unit", "--grant", "source_read:" + str(uuid.uuid4())]
        for extra in (["--max-redemptions", "0"], ["--max-redemptions", "11"], ["--expires-days", "0"], ["--expires-days", "31"],
                      ["--kind", "admin"], ["--provider-email", "a@example.test"],
                      ["--provider-email", "a@example.test", "--provider-role", "member"]):
            with self.subTest(extra=extra), self.assertRaises(op.Refused):
                op.validate(parser.parse_args(base + extra))
        args = op.validate(parser.parse_args(base))
        self.assertEqual((args.max_redemptions, args.expires_days, args.kind), (1, 14, "trial"))
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(["issue", "--organization", ORG, "--issued-by", "unit"]))
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(["issue", "--organization", ORG.upper(), "--issued-by", "unit", "--grant", "source_read:" + str(uuid.uuid4())]))

    def test_canonical_event_bytes_match_the_contract_encoder(self):
        body = {"b": None, "a": "x", "chain_sequence": 2, "kind": "code.issued"}
        self.assertEqual(op.canonical(body), b'{"a":"x","b":null,"chain_sequence":2,"kind":"code.issued"}')
        contracts = HERE.parents[1] / "contracts"
        sys.path.insert(0, str(contracts))
        try:
            from validate import canonical
        except ImportError:
            self.skipTest("contract validator environment not on the path")
        finally:
            sys.path.remove(str(contracts))
        self.assertEqual(op.canonical(body), canonical(body))

    def test_refusal_output_never_carries_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            env = {k: v for k, v in os.environ.items() if not k.startswith("PG")}
            secret = "AAAQ-EAYE-SECRET"
            p = subprocess.run([sys.executable, str(HERE / "admission_operator.py"), "issue", "--organization", ORG,
                                "--issued-by", secret, "--grant", "admin_policy"], capture_output=True, text=True, timeout=30, env=env, cwd=tmp)
            self.assertEqual(p.returncode, 1)
            self.assertEqual(p.stdout, "")
            self.assertNotIn(secret, p.stderr)
            self.assertNotIn("admin_policy", p.stderr)


def run(args, check=True, **kw):
    p = subprocess.run(args, capture_output=True, text=True, timeout=60, **kw)
    if check and p.returncode:
        raise RuntimeError("local fixture command failed: " + " ".join(args[:3]) + ": " + p.stderr[-800:])
    return p


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class DatabaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import psycopg
        cls.psycopg = psycopg
        cls.directory = tempfile.mkdtemp(prefix="genui-operator-")
        cls.name = "genui-app073-operator-" + uuid.uuid4().hex[:12]
        cls.started = False
        try:
            d = cls.directory
            run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "1",
                 "-subj", "/CN=APP073 operator test CA", "-addext", "basicConstraints=critical,CA:TRUE"], cwd=d)
            run(["openssl", "req", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-out", "server.csr", "-subj", "/CN=localhost"], cwd=d)
            Path(d, "server.ext").write_text("subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\n"
                                             "keyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")
            run(["openssl", "x509", "-req", "-in", "server.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial",
                 "-out", "server.pem", "-days", "1", "-extfile", "server.ext"], cwd=d)
            run(["docker", "run", "--detach", "--rm", "--name", cls.name, "--publish", "127.0.0.1::5432", "--memory=512m", "--cpus=1",
                 "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m", "--env=POSTGRES_HOST_AUTH_METHOD=trust", IMAGE])
            cls.started = True
            cls.port = int(run(["docker", "port", cls.name, "5432/tcp"]).stdout.strip().split(":")[1])
            for _ in range(80):
                if run(["docker", "exec", cls.name, "pg_isready", "--host=127.0.0.1", "--username=postgres"], check=False).returncode == 0:
                    break
                time.sleep(.25)
            else:
                raise RuntimeError("local PostgreSQL startup deadline")
            for f in ("server.key", "server.pem"):
                run(["docker", "cp", str(Path(d, f)), cls.name + ":/var/lib/postgresql/" + f])
            run(["docker", "exec", "--user=0", cls.name, "chown", "postgres:postgres", "/var/lib/postgresql/server.key", "/var/lib/postgresql/server.pem"])
            run(["docker", "exec", "--user=0", cls.name, "chmod", "600", "/var/lib/postgresql/server.key"])
            with cls.connect("postgres", "postgres", autocommit=True) as c:
                assert c.execute("SHOW server_version").fetchone()[0].split()[0] == "16.13"
                c.execute("ALTER SYSTEM SET ssl='on'")
                c.execute("ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'")
                c.execute("ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'")
                c.execute("SELECT pg_reload_conf()")
                c.execute("CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS")
                c.execute("CREATE ROLE rds_iam NOLOGIN")
                c.execute("GRANT rds_iam TO genui_migration WITH ADMIN TRUE")
                c.execute("CREATE DATABASE genui OWNER genui_migration")
            with cls.connect() as c:
                runtime.apply_migrations(c, runtime.migration_directory())
            # Fixture rows an existing (pre-0003) organization would have: no audit heads.
            with cls.connect("postgres") as c:
                c.execute("INSERT INTO genui.organizations VALUES (%s,'Operator test legacy','active')", (ORG,))
                c.execute("INSERT INTO genui.provider_tenant_bindings VALUES (%s,%s,%s,true)", (ORG, ISSUER, TENANT))
                cls.source = str(uuid.uuid4())
                c.execute("INSERT INTO genui.sources VALUES (%s,%s)", (ORG, cls.source))
        except BaseException:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        if cls.started:
            run(["docker", "rm", "--force", "--volumes", cls.name], check=False)
            cls.started = False
        shutil.rmtree(cls.directory, ignore_errors=True)

    @classmethod
    def connect(cls, role="genui_migration", database="genui", autocommit=False):
        # Fixture-side inspection only; the command under test uses verify-full TLS.
        return cls.psycopg.connect(host="127.0.0.1", port=cls.port, dbname=database, user=role, sslmode="disable",
                                   connect_timeout=3, autocommit=autocommit)

    def cli(self, *args, user="genui_migration", expect=0):
        env = {k: v for k, v in os.environ.items() if not k.startswith("PG")}
        env.update(PGHOST="localhost", PGPORT=str(self.port), PGUSER=user, PGDATABASE="genui",
                   PGSSLROOTCERT=str(Path(self.directory, "ca.pem")))
        p = subprocess.run([sys.executable, str(HERE / "admission_operator.py"), *args], capture_output=True, text=True,
                           timeout=60, env=env, cwd=self.directory)
        self.assertEqual(p.returncode, expect, p.stderr[-400:])
        if expect == 0:
            self.assertEqual(p.stderr, "")
            self.assertEqual(p.stdout.count("\n"), 1)
            return json.loads(p.stdout)
        self.assertEqual(p.stdout, "")
        return p

    def query(self, statement, params=()):
        with self.connect("postgres") as c:
            return c.execute(statement, params).fetchall()

    def dump(self):
        raw = run(["docker", "exec", self.name, "pg_dump", "--username=postgres", "--dbname=genui", "--data-only"]).stdout
        # pg_dump 16.10+ wraps output in per-run random \restrict keys.
        return "\n".join(x for x in raw.splitlines() if not x.startswith(("\\restrict ", "\\unrestrict ")))

    def ensure_backfill(self):
        self.cli("backfill", "--organization", ORG)

    def issue(self, *extra):
        self.ensure_backfill()
        return self.cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source, *extra)

    def operator_events(self, org=ORG):
        return self.query("SELECT kind FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator' ORDER BY chain_sequence", (org,))

    def test_backfill_creates_five_genesis_heads_once(self):
        first = self.cli("backfill", "--organization", ORG)
        heads = self.query("SELECT writer_role,chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s ORDER BY writer_role", (ORG,))
        self.assertEqual([h[0] for h in heads], sorted(op.CHAINS))
        for role, sequence, head in heads:
            if role in first["created"]:
                self.assertEqual((sequence, head), (1, op.genesis(ORG, role)))
        again = self.cli("backfill", "--organization", ORG)
        self.assertEqual(again["created"], [])
        self.assertEqual(sorted(again["existing"]), sorted(op.CHAINS))
        self.assertIn("organization_sequences", again["deferred"][0])
        self.cli("backfill", "--organization", str(uuid.uuid4()), expect=1)

    def test_provision_creates_organization_binding_heads_handle_and_event_once(self):
        org = str(uuid.uuid4())
        out = self.cli("provision", "--organization", org, "--display-name", "Operator test trial",
                       "--issuer", ISSUER, "--provider-tenant", "ten_" + org[-12:])
        self.assertEqual(set(out), {"organization_id", "handle", "audit_heads", "deferred"})
        raw = op.normalize_handle(out["handle"])
        self.assertIsNotNone(raw)
        self.assertEqual(self.query("SELECT status FROM genui.organizations WHERE organization_id=%s", (org,)), [("active",)])
        self.assertEqual(self.query("SELECT organization_id::text,retired_at FROM genui_private.handle_index WHERE handle_digest=%s",
                                    (op.handle_digest(raw),)), [(org, None)])
        self.assertNotIn(out["handle"], self.dump())
        self.assertEqual(self.query("SELECT count(*) FROM genui.audit_heads WHERE organization_id=%s", (org,)), [(5,)])
        self.assertEqual(self.operator_events(org), [("organization.provisioned",)])
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)
        before = self.dump()
        self.cli("provision", "--organization", org, "--display-name", "again", "--issuer", ISSUER, "--provider-tenant", "ten_x", expect=1)
        self.assertEqual(self.dump(), before)

    def test_issue_prints_the_secret_once_and_stores_only_its_digest(self):
        out = self.issue("--max-redemptions", "2", "--expires-days", "7")
        code = out["code"]
        self.assertEqual(set(out), {"code_id", "code", "kind", "max_redemptions", "expires_at"})
        raw = op.normalize_code(code)
        self.assertIsNotNone(raw)
        digest = op.code_digest(raw)
        dump = self.dump()
        for spelling in (code, code.replace("-", ""), code.lower(), raw.hex(), base64.b64encode(raw).decode()):
            self.assertNotIn(spelling, dump)
        self.assertIn(digest, dump)
        self.assertEqual(self.query("SELECT organization_id::text FROM genui_private.code_index WHERE code_digest=%s", (digest,)), [(ORG,)])
        row = self.query("""SELECT status,max_redemptions,redeemed_count,trial_days,issued_by,expires_at-issued_at FROM genui.access_codes
                            WHERE organization_id=%s AND code_id=%s""", (ORG, out["code_id"]))[0]
        self.assertEqual(row[:5], ("active", 2, 0, 30, "genui_migration:unit-operator"))
        self.assertEqual(row[5].days, 7)
        self.assertEqual(self.query("SELECT permission,source_id::text FROM genui.code_grant_templates WHERE code_id=%s", (out["code_id"],)),
                         [("source_read", self.source)])
        self.assertEqual(self.operator_events()[-1], ("code.issued",))
        listed = self.cli("list", "--organization", ORG)
        self.assertNotIn(code, json.dumps(listed))
        self.assertNotIn(digest, json.dumps(listed))
        entry = next(item for item in listed["codes"] if item["code_id"] == out["code_id"])
        self.assertEqual(set(entry), {"code_id", "kind", "status", "max_redemptions", "redeemed_count", "live_pending_admissions",
                                      "live_reservations", "issued_at", "expires_at", "revoked_at", "provider_invitations"})
        self.assertEqual((entry["status"], entry["redeemed_count"], entry["live_pending_admissions"]), ("active", 0, 0))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_issue_refusals_write_nothing(self):
        self.ensure_backfill()
        before = self.dump()
        for args in (["--grant", "admin_policy"], ["--grant", "review_publish:" + str(uuid.uuid4())],
                     ["--grant", "discover:" + str(uuid.uuid4())],  # unknown app: foreign-key refusal
                     ["--grant", "source_read:" + self.source, "--max-redemptions", "11"],
                     ["--grant", "source_read:" + self.source, "--expires-days", "31"],
                     ["--grant", "source_read:" + self.source, "--provider-email", "x@example.test", "--provider-issuer", ISSUER,
                      "--provider-tenant", TENANT]):
            with self.subTest(args=args[:2]):
                self.cli("issue", "--organization", ORG, "--issued-by", "unit-operator", *args, expect=1)
        self.cli("issue", "--organization", str(uuid.uuid4()), "--issued-by", "unit-operator", "--grant", "source_read:" + self.source, expect=1)
        self.assertEqual(self.dump(), before)

    def test_provider_invitation_is_recorded_pending_without_the_address(self):
        out = self.issue("--provider-email", "Invitee@Example.test", "--provider-role", "member", "--provider-issuer", ISSUER,
                         "--provider-tenant", TENANT)
        rows = self.query("""SELECT invitation_id::text,issuer,provider_tenant_id,remote_invitation_id,intended_email_digest,provider_role,
                             delivery_mode,operation,state FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s""", (ORG, out["code_id"]))
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][1:4], (ISSUER, TENANT, None))
        self.assertEqual(rows[0][4], op.email_digest("invitee@example.test"))
        self.assertEqual(rows[0][5:], ("member", "server_only", "create", "pending"))
        self.assertNotIn("nvitee@", self.dump())
        listed = self.cli("list", "--organization", ORG)
        entry = next(item for item in listed["codes"] if item["code_id"] == out["code_id"])
        self.assertEqual(entry["provider_invitations"], [{"invitation_id": rows[0][0], "operation": "create", "state": "pending"}])
        revoked = self.cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual(revoked["provider_revocations_pending"], 1)
        self.assertEqual(sorted(r[0] for r in self.query("SELECT operation FROM genui.provider_invitations WHERE code_id=%s", (out["code_id"],))),
                         ["create", "revoke"])

    def test_revoke_and_extend_are_audited_and_bounded(self):
        out = self.issue("--expires-days", "5")
        extended = self.cli("extend", "--organization", ORG, "--code-id", out["code_id"], "--expires-days", "20")
        self.assertEqual(extended["changed"], True)
        self.assertEqual(self.operator_events()[-1], ("code.extended",))
        # Beyond issued_at + 30 days is refused, even though the argument itself is in range.
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.access_codes SET issued_at=issued_at-interval '15 days',expires_at=expires_at-interval '15 days' WHERE code_id=%s", (out["code_id"],))
        self.cli("extend", "--organization", ORG, "--code-id", out["code_id"], "--expires-days", "20", expect=1)
        first = self.cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual((first["changed"], first["status"]), (True, "revoked"))
        events = len(self.operator_events())
        again = self.cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual(again["changed"], False)
        self.assertEqual(len(self.operator_events()), events)
        self.cli("extend", "--organization", ORG, "--code-id", out["code_id"], "--expires-days", "3", expect=1)
        self.cli("revoke", "--organization", ORG, "--code-id", str(uuid.uuid4()), expect=1)
        expired = self.issue("--expires-days", "2")
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.access_codes SET expires_at=issued_at+interval '1 millisecond' WHERE code_id=%s", (expired["code_id"],))
        # An expired code is reissued, never resurrected by extension.
        self.cli("extend", "--organization", ORG, "--code-id", expired["code_id"], "--expires-days", "3", expect=1)
        listed = self.cli("list", "--organization", ORG)
        self.assertEqual(next(i for i in listed["codes"] if i["code_id"] == expired["code_id"])["status"], "expired")
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_entitlement_and_membership_revocation_advance_the_epoch(self):
        self.ensure_backfill()
        principal = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (ORG, principal, ISSUER, TENANT, "usr_" + principal))
            c.execute("INSERT INTO genui.memberships VALUES (%s,%s,'active',1)", (ORG, principal))
            c.execute("INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) "
                      "VALUES (%s,%s,%s,'active','trial',now(),now()+interval '30 days')", (ORG, principal, str(uuid.uuid4())))
        out = self.cli("revoke-entitlement", "--organization", ORG, "--principal", principal)
        self.assertEqual((out["changed"], out["membership_epoch"], out["entitlements_revoked"]), (True, 2, 1))
        self.assertEqual(self.query("SELECT status FROM genui.entitlements WHERE principal_id=%s", (principal,)), [("revoked",)])
        self.assertEqual(self.operator_events()[-1], ("entitlement.revoked",))
        self.assertEqual(self.cli("revoke-entitlement", "--organization", ORG, "--principal", principal)["changed"], False)
        out = self.cli("revoke-membership", "--organization", ORG, "--principal", principal)
        self.assertEqual((out["changed"], out["membership_epoch"]), (True, 3))
        self.assertEqual(self.query("SELECT status,membership_epoch FROM genui.memberships WHERE principal_id=%s", (principal,)), [("revoked", 3)])
        self.assertEqual(self.operator_events()[-1], ("membership.revoked",))
        self.assertEqual(self.cli("revoke-membership", "--organization", ORG, "--principal", principal)["changed"], False)
        self.cli("revoke-membership", "--organization", ORG, "--principal", str(uuid.uuid4()), expect=1)

    def test_forged_operator_identity_and_application_roles_are_refused(self):
        self.ensure_backfill()
        before = self.dump()
        for role in ("genui_workspace", "genui_control", "genui_data", "genui_worker"):
            with self.subTest(role=role):
                self.cli("issue", "--organization", ORG, "--issued-by", "forged", "--grant", "source_read:" + self.source, user=role, expect=1)
                self.cli("list", "--organization", ORG, user=role, expect=1)
                for statement in ("SET ROLE genui_owner", "INSERT INTO genui_private.code_index VALUES ('sha256:%s','%s')" % ("0" * 64, ORG),
                                  "UPDATE genui.access_codes SET status='revoked'", "UPDATE genui.entitlements SET status='revoked'"):
                    with self.connect(role) as c, self.assertRaises(self.psycopg.errors.InsufficientPrivilege):
                        c.execute("SELECT set_config('genui.organization_id',%s,true)", (ORG,))
                        c.execute(statement)
        self.assertEqual(self.dump(), before)

    def test_verify_recomputes_sql_written_admission_chain_and_detects_tamper(self):
        out = self.issue("--max-redemptions", "1")
        digest = op.code_digest(op.normalize_code(out["code"]))
        subject = "usr_" + uuid.uuid4().hex
        with self.connect("genui_workspace") as c:
            c.execute("SELECT set_config('genui.organization_id',%s,true)", (ORG,))
            reservation, _, outcome = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s)", (ORG, digest, "sha256:" + "1" * 64)).fetchone()
            self.assertEqual(outcome, "reserved")
            pending, _, outcome = c.execute("SELECT * FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)",
                                            (ORG, reservation, ISSUER, TENANT, subject)).fetchone()
            self.assertEqual(outcome, "pending")
            row = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                            (ORG, pending, ISSUER, TENANT, subject, "sha256:" + "2" * 64)).fetchone()
            self.assertEqual(row[-1], "admitted")
        result = self.cli("verify", "--organization", ORG)
        self.assertEqual(result["verified"], True)
        self.assertGreaterEqual(result["chains"]["admission"]["events"], 2)
        # Positive control: a committed tamper (only the local superuser can
        # write audit_events at all) fails verification; then it is restored.
        event = self.query("""SELECT event_id::text FROM genui.audit_events WHERE organization_id=%s AND kind='admission.redeemed'
                              ORDER BY chain_sequence DESC LIMIT 1""", (ORG,))[0][0]
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.audit_events SET kind='admission.pending' WHERE organization_id=%s AND event_id=%s", (ORG, event))
        try:
            failed = self.cli("verify", "--organization", ORG, expect=1)
            self.assertIn("audit_chain_mismatch", failed.stderr)
        finally:
            with self.connect("postgres") as c:
                c.execute("UPDATE genui.audit_events SET kind='admission.redeemed' WHERE organization_id=%s AND event_id=%s", (ORG, event))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)


if __name__ == "__main__":
    unittest.main()
