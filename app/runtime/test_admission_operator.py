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

    def test_permission_commands_are_a_closed_set(self):
        """ADR-007 amendment (APP-010 review M2): grant/revoke take only the permissions the tables allow,
        never admin_policy, share_work or export, and each permission names exactly its resource."""
        app, work, source, principal = (str(uuid.uuid4()) for _ in range(4))
        parser = op.parser()
        base = ["--organization", ORG, "--principal", principal]
        self.assertEqual(op.GRANTABLE, {"discover": "app", "run": "app", "edit_definition": "app", "review_publish": "app",
                                        "record_read": "work", "record_write": "work", "source_read": "source"})
        self.assertEqual(op.OPERATOR_KINDS[-3:], ("permission.granted", "permission.revoked", "creator_authority.revoked"))
        for command in ("grant-permission", "revoke-permission"):
            for good in (["--permission", "run", "--app", app], ["--permission", "review_publish", "--app", app],
                         ["--permission", "record_write", "--app", app, "--work", work], ["--permission", "source_read", "--source", source]):
                with self.subTest(command=command, good=good[1]):
                    op.validate(parser.parse_args([command, *base, *good]))
            for bad in (["--permission", "admin_policy"], ["--permission", "share_work", "--app", app, "--work", work],
                        ["--permission", "export", "--app", app, "--work", work], ["--permission", "owner", "--app", app],
                        ["--permission", "run"], ["--permission", "run", "--app", app, "--work", work],
                        ["--permission", "record_read", "--app", app], ["--permission", "source_read", "--app", app],
                        ["--permission", "run", "--app", app.upper()], ["--permission", "source_read", "--source", source, "--app", app]):
                with self.subTest(command=command, bad=bad[1:3]), self.assertRaises(op.Refused):
                    op.validate(parser.parse_args([command, *base, *bad]))
        op.validate(parser.parse_args(["create-app", "--organization", ORG, "--app", app, "--space", source, "--owner", principal]))
        op.validate(parser.parse_args(["revoke-creator-authority", "--organization", ORG, "--app", app, "--work", work]))
        for bad in (["create-app", "--organization", ORG, "--app", app, "--space", source],
                    ["revoke-creator-authority", "--organization", ORG, "--app", app],
                    ["revoke-creator-authority", "--organization", ORG, "--app", "x", "--work", work]):
            with self.subTest(bad=bad[0]), self.assertRaises(op.Refused):
                op.validate(parser.parse_args(bad))

    def test_source_revision_record_is_closed_and_content_derived(self):
        # APP-012: upload-sources' record; the Northstar identity APP-077 prints.
        northstar = "00000000-0000-4000-8000-000000000001"
        content = "sha256:6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647"
        self.assertEqual(op.revision_id(northstar, content), "9161c1e4-27c0-4cbb-addf-eb7ce504bf45")
        normalized = "sha256:0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082"
        good = {"organization_id": northstar, "source_id": "00000000-0000-4000-8000-00000000006e",
                "revision_id": "9161c1e4-27c0-4cbb-addf-eb7ce504bf45", "content_digest": content,
                "normalized_digest": normalized, "object_digest": normalized, "row_count": 6,
                "contract_digest": "sha256:b92d4b96754e5e9de1863dff09e615880ded8d57de22e7546b1f59f2f9178f64"}
        with tempfile.TemporaryDirectory() as directory:
            def parse(value, organization=northstar):
                path = Path(directory) / "record.json"
                path.write_text(json.dumps(value))
                return op.validate(op.parser().parse_args(["register-source-revision", "--organization", organization, "--record", str(path)]))
            self.assertEqual(parse(good).revision_record, good)
            for change, reason in ((dict(good, extra=1), "record_refused"), (dict(good, row_count="6"), "record_refused"),
                                   (dict(good, object_digest=content), "record_digest_refused"),
                                   (dict(good, revision_id="00000000-0000-4000-8000-0000000000f1"), "record_revision_refused"),
                                   (dict(good, content_digest="sha256:" + "0" * 64), "record_revision_refused"),
                                   (dict(good, normalized_digest="sha256:XYZ"), "record_refused")):
                with self.assertRaises(op.Refused) as refusal:
                    parse(change)
                self.assertEqual(str(refusal.exception), reason)
            with self.assertRaises(op.Refused) as refusal:
                parse(good, organization="00000000-0000-4000-8000-000000000002")
            self.assertEqual(str(refusal.exception), "record_organization_refused")
            # APP-078: the same record inline is checked identically; exactly one form.
            def inline(value, organization=northstar):
                return op.validate(op.parser().parse_args(["register-source-revision", "--organization", organization, "--record-json", value]))
            self.assertEqual(inline(json.dumps(good)).revision_record, good)
            for change in (dict(good, extra=1), dict(good, object_digest=content), dict(good, row_count="6")):
                with self.assertRaises(op.Refused):
                    inline(json.dumps(change))
            duplicate = json.dumps(good)[:-1] + ',"row_count":7}'
            for bad in (duplicate, "", "[]", "x" * 4097, json.dumps(good).replace("6e0f", "ée0f")):
                with self.assertRaises(op.Refused) as refusal:
                    inline(bad)
                self.assertTrue(str(refusal.exception).startswith("record_"), bad[:20])
            with self.assertRaises(op.Refused):
                inline(json.dumps(good), organization="00000000-0000-4000-8000-000000000002")
            path = Path(directory) / "record.json"
            for argv in (["register-source-revision", "--organization", northstar],
                         ["register-source-revision", "--organization", northstar, "--record", str(path), "--record-json", json.dumps(good)]):
                with self.assertRaises(op.Refused) as refusal:
                    op.validate(op.parser().parse_args(argv))
                self.assertEqual(str(refusal.exception), "record_refused")
        parser = op.parser()
        base = ["create-bound-work", "--organization", northstar, "--app", ORG, "--from-work", ORG, "--work", ORG,
                "--creator", ORG, "--input", "events", "--source", ORG, "--revision", ORG]
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(base))  # the new work must differ from its origin
        base[8] = "00000000-0000-4000-8000-0000000000a2"
        op.validate(parser.parse_args(base))
        base[12] = "Events"
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(base))

    def test_issue_argument_bounds(self):
        parser = op.parser()
        base = ["issue", "--organization", ORG, "--issued-by", "unit", "--grant", "source_read:" + str(uuid.uuid4())]
        for extra in (["--max-redemptions", "0"], ["--max-redemptions", "11"], ["--expires-days", "0"], ["--expires-days", "31"],
                      ["--kind", "admin"], ["--provider-email", "a@example.test"],
                      ["--provider-email", "a@example.test", "--provider-role", "user"]):
            with self.subTest(extra=extra), self.assertRaises(op.Refused):
                op.validate(parser.parse_args(base + extra))
        # Review M1: an allowlist of the provider's least-privilege role, never a denylist.
        invitation = ["--provider-email", "a@example.test", "--provider-issuer", ISSUER, "--provider-tenant", TENANT, "--provider-role"]
        for role in ("member", "tenant_admin", "superadmin", "org_owner", "admin", "administrator", "owner", "role_admin", "User", "user "):
            with self.subTest(role=role):
                with self.assertRaises(op.Refused) as refused:
                    op.validate(parser.parse_args(base + invitation + [role]))
                self.assertEqual(str(refused.exception), "provider_role_refused")
        self.assertEqual(op.PROVIDER_ROLES, ("user",))
        self.assertEqual(op.validate(parser.parse_args(base + invitation + ["user"])).provider_role, "user")
        args = op.validate(parser.parse_args(base))
        self.assertEqual((args.max_redemptions, args.expires_days, args.kind), (1, 14, "trial"))
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(["issue", "--organization", ORG, "--issued-by", "unit"]))
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(["issue", "--organization", ORG.upper(), "--issued-by", "unit", "--grant", "source_read:" + str(uuid.uuid4())]))
        # APP-078 managed channel: a pre-minted digest of the exact stored shape, never with an invitation.
        digest = op.code_digest(bytes(32))
        self.assertEqual(op.validate(parser.parse_args(base + ["--code-digest", digest])).code_digest, digest)
        for bad in ("sha256:" + "A" * 64, "sha256:" + "0" * 63, digest + "\n", "0" * 64, op.encode_code(bytes(32))):
            with self.subTest(digest=bad[:12]), self.assertRaises(op.Refused) as refused:
                op.validate(parser.parse_args(base + ["--code-digest", bad]))
            self.assertEqual(str(refused.exception), "malformed_code_digest")
        with self.assertRaises(op.Refused) as refused:
            op.validate(parser.parse_args(base + ["--code-digest", digest] + invitation + ["user"]))
        self.assertEqual(str(refused.exception), "code_digest_with_invitation_refused")

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


class OperatorDatabase:
    """One disposable PostgreSQL 16.13 container with a throwaway TLS CA, shared by a TestCase class.

    A mixin, not a TestCase, so other test modules (test_provider_invitations)
    reuse the fixture without collecting these tests twice.
    """
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

    def cli(self, *args, user="genui_migration", expect=0, env_extra=None):
        env = {k: v for k, v in os.environ.items() if not k.startswith(("PG", "GENUI_PROVIDER_"))}
        env.update(PGHOST="localhost", PGPORT=str(self.port), PGUSER=user, PGDATABASE="genui",
                   PGSSLROOTCERT=str(Path(self.directory, "ca.pem")), **(env_extra or {}))
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

    def workspace(self):
        c = self.connect("genui_workspace")
        c.execute("SELECT set_config('genui.organization_id',%s,true)", (ORG,))
        return c

    def pending_for(self, digest, subject, context):
        """Reserve, then record a pending admission as genui_workspace: (outcome, pending id, expiry, presignup id)."""
        with self.workspace() as c:
            reservation, _, outcome = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s)",
                                                (ORG, digest, "sha256:" + context * 64)).fetchone()
            self.assertEqual(outcome, "reserved")
            pending, expires, outcome = c.execute("SELECT * FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)",
                                                  (ORG, reservation, ISSUER, TENANT, subject)).fetchone()
            return outcome, (str(pending) if pending else None), expires, str(reservation)


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class DatabaseTests(OperatorDatabase, unittest.TestCase):
    def test_backfill_creates_five_genesis_heads_once(self):
        first = self.cli("backfill", "--organization", ORG)
        heads = self.query("SELECT writer_role,chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s ORDER BY writer_role", (ORG,))
        self.assertEqual([h[0] for h in heads], sorted(op.CHAINS))
        for role, sequence, head in heads:
            if role in first["created"]:
                self.assertEqual((sequence, head), (1, op.genesis(ORG, role)))
        # D7.7 (0007): the backfilled row starts at 1 + the highest committed
        # revision sequence (1 when this fixture has none); a second backfill
        # leaves it unchanged.
        following = self.query("SELECT coalesce(max(commit_sequence),0)+1 FROM genui.internal_record_revisions WHERE organization_id=%s", (ORG,))[0][0]
        self.assertEqual(first["sequence"], {"organization_sequences": "created", "next_commit_sequence": following})
        self.assertEqual(self.query("SELECT next_commit_sequence FROM genui.organization_sequences WHERE organization_id=%s", (ORG,)), [(following,)])
        again = self.cli("backfill", "--organization", ORG)
        self.assertEqual(again["created"], [])
        self.assertEqual(sorted(again["existing"]), sorted(op.CHAINS))
        self.assertEqual(again["sequence"], {"organization_sequences": "existing", "next_commit_sequence": following})
        verified = self.cli("verify", "--organization", ORG)
        self.assertEqual((verified["command_outcomes"], verified["next_commit_sequence"]), (0, following))
        self.cli("backfill", "--organization", str(uuid.uuid4()), expect=1)

    def test_provision_creates_organization_binding_heads_handle_and_event_once(self):
        org = str(uuid.uuid4())
        out = self.cli("provision", "--organization", org, "--display-name", "Operator test trial",
                       "--issuer", ISSUER, "--provider-tenant", "ten_" + org[-12:])
        self.assertEqual(set(out), {"organization_id", "handle", "audit_heads", "sequence"})
        self.assertEqual(out["sequence"], {"organization_sequences": "created", "next_commit_sequence": 1})
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

    def test_handles_issue_rotate_and_retire_route_through_resolve_handle(self):
        """APP-009 (ADR-006 §handle): only digests are stored; both handles route
        during a rotation; the last live handle never retires; one handle.rotated
        event per change; refusals echo no value and write nothing."""
        org = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.organizations VALUES (%s,'Handle test','active')", (org,))
        self.cli("backfill", "--organization", org)

        def routes(handle):
            raw = op.normalize_handle(handle)
            with self.connect("genui_workspace") as c:
                found = c.execute("SELECT genui_private.resolve_handle(%s)::text", (op.handle_digest(raw),)).fetchone()[0]
            return found
        fixed = op.encode_handle(bytes(range(1, 17)))
        first = self.cli("issue-handle", "--organization", org, "--handle", fixed)
        self.assertEqual(first, {"organization_id": org, "handle": fixed, "live_handles": 1})
        self.assertEqual(routes(fixed), org)
        self.assertNotIn(fixed, self.dump())
        before = self.dump()
        p = self.cli("issue-handle", "--organization", org, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "handle_exists"})
        p = self.cli("issue-handle", "--organization", str(uuid.uuid4()), "--handle", "NOT-A-HANDLE", expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "malformed_handle"})
        self.assertNotIn("NOT-A-HANDLE", p.stderr)
        self.cli("retire-handle", "--organization", org, "--handle", fixed, expect=1)
        self.assertEqual(self.dump(), before)
        second = self.cli("rotate-handle", "--organization", org)
        self.assertEqual(second["live_handles"], 2)
        self.assertEqual((routes(fixed), routes(second["handle"])), (org, org))
        p = self.cli("rotate-handle", "--organization", org, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "rotation_in_progress"})
        self.assertEqual(self.cli("retire-handle", "--organization", org, "--handle", fixed),
                         {"organization_id": org, "retired": True, "live_handles": 1})
        self.assertIsNone(routes(fixed))
        self.assertEqual(routes(second["handle"]), org)
        p = self.cli("retire-handle", "--organization", org, "--handle", fixed, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "handle_retired"})
        p = self.cli("issue-handle", "--organization", str(uuid.uuid4()), "--handle", fixed, expect=1)
        self.assertEqual(self.operator_events(org), [("handle.rotated",)] * 3)
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)

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
                                      "lapsed_pending_admissions", "denied_for_reconciliation", "live_reservations", "issued_at",
                                      "expires_at", "revoked_at", "provider_invitations"})
        self.assertEqual((entry["status"], entry["redeemed_count"], entry["live_pending_admissions"], entry["lapsed_pending_admissions"],
                          entry["denied_for_reconciliation"]), ("active", 0, 0, 0, 0))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_preminted_issue_stores_the_given_digest_and_prints_no_secret(self):
        # APP-078 managed channel: the operator minted the code; the command sees only its digest.
        raw = os.urandom(32)
        code, digest = op.encode_code(raw), op.code_digest(raw)
        out = self.issue("--code-digest", digest)
        self.assertEqual(set(out), {"code_id", "code_digest", "kind", "max_redemptions", "expires_at"})
        self.assertEqual(out["code_digest"], digest)
        self.assertEqual(self.query("SELECT organization_id::text FROM genui_private.code_index WHERE code_digest=%s", (digest,)), [(ORG,)])
        self.assertEqual(self.query("SELECT code_digest::text,status FROM genui.access_codes WHERE organization_id=%s AND code_id=%s",
                                    (ORG, out["code_id"])), [(digest, "active")])
        self.assertNotIn(code, self.dump())
        self.assertEqual(self.operator_events()[-1], ("code.issued",))
        # The code the operator holds is the one the stored digest admits.
        self.assertEqual(op.code_digest(op.normalize_code(code)), digest)
        # A reused digest refuses whole (code_index primary key) and writes nothing.
        before = self.dump()
        self.cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                 "--code-digest", digest, expect=1)
        self.assertEqual(self.dump(), before)
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

    def test_revoke_and_extend_are_audited_and_bounded(self):
        out = self.issue("--expires-days", "5")
        events = len(self.operator_events())
        expiry = self.query("SELECT expires_at FROM genui.access_codes WHERE code_id=%s", (out["code_id"],))
        # Review m5: an extension that would not move the expiry later is refused and writes nothing.
        for days in ("1", "4"):
            with self.subTest(days=days):
                refused = self.cli("extend", "--organization", ORG, "--code-id", out["code_id"], "--expires-days", days, expect=1)
                self.assertIn("expiry_not_extended", refused.stderr)
        self.assertEqual(len(self.operator_events()), events)
        self.assertEqual(self.query("SELECT expires_at FROM genui.access_codes WHERE code_id=%s", (out["code_id"],)), expiry)
        extended = self.cli("extend", "--organization", ORG, "--code-id", out["code_id"], "--expires-days", "20")
        self.assertEqual(extended["changed"], True)
        self.assertGreater(extended["expires_at"], out["expires_at"])
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

    def member(self, status="active"):
        principal = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (ORG, principal, ISSUER, TENANT, "usr_" + principal))
            c.execute("INSERT INTO genui.memberships VALUES (%s,%s,%s,1)", (ORG, principal, status))
        return principal

    def space(self):
        space = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.spaces VALUES (%s,%s,'Operator test space')", (ORG, space))
        return space

    def last_operator_event(self):
        return self.query("""SELECT kind,actor_principal_id::text,app_id::text,work_id::text FROM genui.audit_events
                             WHERE organization_id=%s AND writer_role='operator' ORDER BY chain_sequence DESC LIMIT 1""", (ORG,))[0]

    def epoch(self, principal):
        return self.query("SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, principal))[0][0]

    def test_create_app_grant_and_revoke_permission_are_operator_events(self):
        """ADR-007 amendment (APP-010 review M2, m7): app shells and their grants are operator actions."""
        self.ensure_backfill()
        owner, other, revoked = self.member(), self.member(), self.member("revoked")
        space, app = self.space(), str(uuid.uuid4())
        out = self.cli("create-app", "--organization", ORG, "--app", app, "--space", space, "--owner", owner)
        self.assertEqual(out, {"app_id": app, "owner_principal_id": owner, "granted": ["edit_definition"]})
        self.assertEqual(self.query("SELECT space_id::text,owner_principal_id::text FROM genui.apps WHERE organization_id=%s AND app_id=%s",
                                    (ORG, app)), [(space, owner)])
        self.assertEqual(self.query("SELECT principal_id::text,permission FROM genui.app_permissions WHERE app_id=%s", (app,)),
                         [(owner, "edit_definition")])
        # D7.8 names no operator app-creation kind: the owner's grant is the audited record.
        self.assertEqual(self.last_operator_event(), ("permission.granted", owner, app, None))
        before = self.dump()
        for args in (["--app", app, "--space", space, "--owner", owner],                            # app exists
                     ["--app", str(uuid.uuid4()), "--space", str(uuid.uuid4()), "--owner", owner],  # unknown space
                     ["--app", str(uuid.uuid4()), "--space", space, "--owner", revoked],            # inactive owner
                     ["--app", str(uuid.uuid4()), "--space", space, "--owner", str(uuid.uuid4())]):
            with self.subTest(args=args[1]):
                self.cli("create-app", "--organization", ORG, *args, expect=1)
        self.assertEqual(self.dump(), before)

        epoch = self.epoch(other)
        granted = self.cli("grant-permission", "--organization", ORG, "--principal", other, "--permission", "run", "--app", app)
        self.assertEqual(granted, {"principal_id": other, "permission": "run", "changed": True, "membership_epoch": epoch})
        self.assertEqual(self.last_operator_event(), ("permission.granted", other, app, None))
        events = len(self.operator_events())
        self.assertEqual(self.cli("grant-permission", "--organization", ORG, "--principal", other, "--permission", "run",
                                  "--app", app)["changed"], False)
        self.assertEqual(len(self.operator_events()), events)
        self.cli("grant-permission", "--organization", ORG, "--principal", other, "--permission", "source_read", "--source", self.source)
        self.assertEqual(self.query("SELECT permission FROM genui.source_permissions WHERE principal_id=%s", (other,)), [("source_read",)])
        self.assertEqual(self.last_operator_event(), ("permission.granted", other, None, None))
        before = self.dump()
        for principal, extra in ((revoked, ["--permission", "run", "--app", app]),
                                 (str(uuid.uuid4()), ["--permission", "run", "--app", app]),
                                 (other, ["--permission", "run", "--app", str(uuid.uuid4())]),
                                 (other, ["--permission", "record_read", "--app", app, "--work", str(uuid.uuid4())]),
                                 (other, ["--permission", "source_read", "--source", str(uuid.uuid4())]),
                                 (other, ["--permission", "admin_policy"])):
            with self.subTest(extra=extra[1]):
                self.cli("grant-permission", "--organization", ORG, "--principal", principal, *extra, expect=1)
        self.cli("grant-permission", "--organization", str(uuid.uuid4()), "--principal", other, "--permission", "run", "--app", app, expect=1)
        self.assertEqual(self.dump(), before)

        revoked_out = self.cli("revoke-permission", "--organization", ORG, "--principal", other, "--permission", "run", "--app", app)
        self.assertEqual(revoked_out, {"principal_id": other, "permission": "run", "changed": True, "membership_epoch": epoch + 1})
        self.assertEqual(self.query("SELECT count(*) FROM genui.app_permissions WHERE app_id=%s AND principal_id=%s", (app, other)), [(0,)])
        self.assertEqual(self.last_operator_event(), ("permission.revoked", other, app, None))
        events = len(self.operator_events())
        again = self.cli("revoke-permission", "--organization", ORG, "--principal", other, "--permission", "run", "--app", app)
        self.assertEqual((again["changed"], again["membership_epoch"]), (False, epoch + 1))
        self.assertEqual(len(self.operator_events()), events)
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_revoke_creator_authority_is_ordered_and_audited(self):
        """ADR-007 amendment (APP-010 review M2): creator authority ends only by the operator command."""
        self.ensure_backfill()
        creator, space, app = self.member(), self.space(), str(uuid.uuid4())
        self.cli("create-app", "--organization", ORG, "--app", app, "--space", space, "--owner", creator)
        revision, publication, policy = (str(uuid.uuid4()) for _ in range(3))
        works = {"created": str(uuid.uuid4()), "legacy": str(uuid.uuid4())}
        digest = "sha256:" + "d" * 64
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.definition_revisions VALUES (%s,%s,%s,1,%s,'\\x7b7d')", (ORG, app, revision, digest))
            c.execute("INSERT INTO genui.publication_policies VALUES (%s,%s,1,'owner_publish')", (ORG, policy))
            c.execute("""INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,
                         policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,
                         published_at,canonical_envelope,publication_sequence)
                         VALUES (%s,%s,%s,%s,%s,%s,1,'owner_publish',%s,%s,1,1,1,%s,now(),'\\x7b7d',1)""",
                      (ORG, app, publication, revision, digest, policy, creator, creator, digest))
            c.execute("""INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by)
                         VALUES (%s,%s,%s,%s,1,'',%s)""", (ORG, app, works["created"], publication, creator))
            c.execute("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context) VALUES (%s,%s,%s,%s,1,'')",
                      (ORG, app, works["legacy"], publication))
        epoch = self.epoch(creator)
        out = self.cli("revoke-creator-authority", "--organization", ORG, "--app", app, "--work", works["created"])
        self.assertEqual(out, {"work_id": works["created"], "creator_principal_id": creator, "changed": True, "membership_epoch": epoch + 1})
        self.assertEqual(self.query("SELECT created_by::text,creator_revoked_at IS NOT NULL FROM genui.saved_work WHERE work_id=%s",
                                    (works["created"],)), [(creator, True)])
        self.assertEqual(self.last_operator_event(), ("creator_authority.revoked", creator, app, works["created"]))
        events = len(self.operator_events())
        again = self.cli("revoke-creator-authority", "--organization", ORG, "--app", app, "--work", works["created"])
        self.assertEqual((again["changed"], again["membership_epoch"]), (False, epoch + 1))
        self.assertEqual(len(self.operator_events()), events)
        before = self.dump()
        for work in (works["legacy"], str(uuid.uuid4())):  # no creator recorded; unknown work
            with self.subTest(work=work):
                self.cli("revoke-creator-authority", "--organization", ORG, "--app", app, "--work", work, expect=1)
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_forged_operator_identity_and_application_roles_are_refused(self):
        self.ensure_backfill()
        before = self.dump()
        for role in ("genui_workspace", "genui_control", "genui_data", "genui_worker"):
            with self.subTest(role=role):
                for args in (("issue", "--organization", ORG, "--issued-by", "forged", "--grant", "source_read:" + self.source),
                             ("list", "--organization", ORG)):
                    self.assertIn("operator_identity_refused", self.cli(*args, user=role, expect=1).stderr)
                for statement in ("SET ROLE genui_owner", "INSERT INTO genui_private.code_index VALUES ('sha256:%s','%s')" % ("0" * 64, ORG),
                                  "UPDATE genui.access_codes SET status='revoked'", "UPDATE genui.entitlements SET status='revoked'"):
                    with self.connect(role) as c, self.assertRaises(self.psycopg.errors.InsufficientPrivilege):
                        c.execute("SELECT set_config('genui.organization_id',%s,true)", (ORG,))
                        c.execute(statement)
        self.assertEqual(self.dump(), before)

    def test_verify_reads_one_snapshot_while_an_admission_appends(self):
        """Review m1: an append committed between verify's head and event reads is not a mismatch."""
        out = self.issue("--max-redemptions", "3")
        digest = op.code_digest(op.normalize_code(out["code"]))
        seen = {}
        test = self

        class Interleaved:
            """Connection proxy: right after verify reads the admission head, another session appends."""
            def __init__(self, inner):
                object.__setattr__(self, "inner", inner)
                object.__setattr__(self, "fired", False)

            def __getattr__(self, name):
                return getattr(self.inner, name)

            def __setattr__(self, name, value):
                setattr(self.inner, name, value)

            def execute(self, statement, params=None):
                cursor = self.inner.execute(statement, params)
                if not self.fired and "FROM genui.audit_heads" in statement and params and params[-1] == "admission":
                    rows = cursor.fetchall()
                    object.__setattr__(self, "fired", True)
                    seen["isolation"] = self.inner.execute("SHOW transaction_isolation").fetchone()[0]
                    seen["read_only"] = self.inner.execute("SHOW transaction_read_only").fetchone()[0]
                    seen["append"] = test.pending_for(digest, "usr_m1_interleaved", "a")[0]

                    class Rows:
                        def fetchone(self):
                            return rows[0] if rows else None
                    return Rows()
                return cursor

        env = {"PGHOST": "localhost", "PGPORT": str(self.port), "PGUSER": "genui_migration", "PGDATABASE": "genui",
               "PGSSLROOTCERT": str(Path(self.directory, "ca.pem"))}
        saved = {k: os.environ.get(k) for k in env}
        os.environ.update(env)
        try:
            with op.connect() as connection:
                result = op.verify(Interleaved(connection), type("Args", (), {"organization": ORG})())
        finally:
            for k, v in saved.items():
                if v is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = v
        self.assertEqual(seen, {"isolation": "repeatable read", "read_only": "on", "append": "pending"})
        self.assertEqual(result["verified"], True)
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_second_pending_admission_of_one_identity_reuses_the_live_one(self):
        """Review m3(a): one identity holds at most one live pending admission per code."""
        out = self.issue("--max-redemptions", "3")
        digest = op.code_digest(op.normalize_code(out["code"]))
        first = self.pending_for(digest, "usr_m3_repeat", "3")
        self.assertEqual(first[0], "pending")
        events = self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (ORG,))
        again = self.pending_for(digest, "usr_m3_repeat", "4")
        self.assertEqual(again[:3], first[:3])
        self.assertEqual(self.query("SELECT state FROM genui.code_reservations WHERE reservation_id=%s", (again[3],)), [("consumed",)])
        self.assertEqual(self.query("""SELECT count(*) FROM genui.code_reservations WHERE organization_id=%s AND code_id=%s
                                       AND kind='pending'""", (ORG, out["code_id"])), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (ORG,)), events)
        # The other two slots stay available to other identities.
        self.assertEqual(self.pending_for(digest, "usr_m3_other", "5")[0], "pending")
        entry = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual(entry["live_pending_admissions"], 2)

    def test_list_reports_lapsed_and_reconciliation_pending_admissions(self):
        """Review m3(b): a lapsed pending admission is visible to the operator and never counted live."""
        out = self.issue("--max-redemptions", "3")
        digest = op.code_digest(op.normalize_code(out["code"]))
        lapsed = self.pending_for(digest, "usr_m3_lapsed", "6")[1]
        denied = self.pending_for(digest, "usr_m3_denied", "7")[1]
        self.assertEqual(self.pending_for(digest, "usr_m3_live", "8")[0], "pending")
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.code_reservations SET expires_at=now()-interval '1 minute' WHERE reservation_id IN (%s,%s)", (lapsed, denied))
        with self.workspace() as c:
            row = c.execute("SELECT outcome FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                            (ORG, denied, ISSUER, TENANT, "usr_m3_denied", "sha256:" + "9" * 64)).fetchone()
        self.assertEqual(row, ("expired",))
        entry = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual((entry["live_pending_admissions"], entry["lapsed_pending_admissions"], entry["denied_for_reconciliation"]),
                         (1, 1, 1))

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
