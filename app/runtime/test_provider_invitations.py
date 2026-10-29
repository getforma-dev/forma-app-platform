"""APP-073 acceptance 6 and 7: operator provider invitations and reconciliation.

SIMULATED TRANSPORT. Every provider here is fake_fauth.FakeFauth, a loopback
stand-in built from the merged fAuth contract text and the public gatewasm
source at 1bf5d3b; nothing calls the deployed fAuth, and nothing here is hosted
identity evidence (APP-074/076). The database tests reuse test_admission_operator's
disposable PostgreSQL 16.13 container with a throwaway TLS CA and drive the real
command line as genui_migration over verify-full TLS.
"""
import datetime
import json
import os
from pathlib import Path
import shutil
import socket
import stat
import tempfile
import time
import unittest
from unittest import mock
import uuid

import admission_operator as op
import fake_fauth
import provider_management as pm
from test_admission_operator import ISSUER, ORG, TENANT, OperatorDatabase

KEY = "mgmt_" + str(uuid.uuid4())
EMAIL = "Invitee@Example.test"


def private_directory(parent, name):
    path = Path(parent, name)
    path.mkdir(mode=0o700)
    os.chmod(path, 0o700)
    return path


def private_file(path, text, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, "w") as f:
        f.write(text)
    os.chmod(path, mode)
    return path


class ConfigTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="genui-provider-config-")
        self.capabilities = private_directory(self.tmp, "capabilities")
        self.key_file = private_file(Path(self.tmp, "key"), KEY + "\n")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def env(self, **changes):
        base = {pm.ENV_URL: "https://auth.example.test", pm.ENV_KEY_FILE: str(self.key_file), pm.ENV_CAPABILITY_DIR: str(self.capabilities)}
        base.update(changes)
        return {k: v for k, v in base.items() if v is not None}

    def refused(self, env, reason, required=True):
        with self.assertRaises(pm.Refused) as refusal:
            pm.load_config(env, required=required)
        self.assertEqual(str(refusal.exception), reason)
        self.assertNotIn(KEY, repr(refusal.exception))

    def test_complete_configuration_loads_without_echoing_the_key(self):
        config = pm.load_config(self.env(), required=True)
        self.assertEqual((config.scheme, config.host, config.port), ("https", "auth.example.test", 443))
        self.assertEqual(config.capability_dir, self.capabilities)
        self.assertNotIn(KEY, repr(config))
        self.assertNotIn(KEY, str(config))
        config = pm.load_config(self.env(**{pm.ENV_KEY_FILE: None, pm.ENV_KEY: KEY, pm.ENV_URL: "http://127.0.0.1:8123/"}), required=True)
        self.assertEqual((config.scheme, config.host, config.port), ("http", "127.0.0.1", 8123))

    def test_absent_and_partial_configuration(self):
        self.assertIsNone(pm.load_config({}, required=False))
        self.refused({}, "provider_configuration_missing")
        self.refused({pm.ENV_URL: "https://auth.example.test"}, "provider_configuration_incomplete", required=False)
        self.refused(self.env(**{pm.ENV_CAPABILITY_DIR: None}), "provider_configuration_incomplete")

    def test_url_refusals(self):
        for url in ("http://auth.example.test", "https://user:pw@auth.example.test", "https://auth.example.test/?x=1",
                    "https://auth.example.test/#f", "https://auth.example.test/manage", "ftp://auth.example.test",
                    "http://127.0.0.2", "https://", "https://auth.example.test:0", "auth.example.test"):
            with self.subTest(url=url):
                self.refused(self.env(**{pm.ENV_URL: url}), "provider_url_refused")

    def test_management_key_comes_only_from_a_private_file_or_the_environment(self):
        loose = private_file(Path(self.tmp, "loose"), KEY, mode=0o644)
        link = Path(self.tmp, "link")
        link.symlink_to(self.key_file)
        for changes in ({pm.ENV_KEY_FILE: str(loose)}, {pm.ENV_KEY_FILE: str(link)}, {pm.ENV_KEY_FILE: str(Path(self.tmp, "absent"))},
                        {pm.ENV_KEY_FILE: "relative/key"}, {pm.ENV_KEY: KEY},
                        {pm.ENV_KEY_FILE: None, pm.ENV_KEY: "plat_" + KEY[5:]}, {pm.ENV_KEY_FILE: None, pm.ENV_KEY: KEY + " x"},
                        {pm.ENV_KEY_FILE: None, pm.ENV_KEY: "mgmt_"}):
            with self.subTest(changes=sorted(changes)):
                self.refused(self.env(**changes), "provider_key_refused")
        # Never an argument: the closed parser has no key option and refuses without echoing it.
        for command in (["reconcile", "--organization", ORG, "--management-key", KEY],
                        ["issue", "--organization", ORG, "--issued-by", "x", "--grant", "source_read:" + ORG, "--management-key", KEY]):
            with self.assertRaises(op.Refused) as refusal:
                op.parser().parse_args(command)
            self.assertEqual(str(refusal.exception), "usage")

    def test_capability_directory_must_be_private_to_the_operator(self):
        open_dir = Path(self.tmp, "open")
        open_dir.mkdir()
        os.chmod(open_dir, 0o755)
        link = Path(self.tmp, "dirlink")
        link.symlink_to(self.capabilities, target_is_directory=True)
        for path in (str(open_dir), str(link), "relative", str(Path(self.tmp, "absent")), str(self.key_file)):
            with self.subTest(path=path):
                self.refused(self.env(**{pm.ENV_CAPABILITY_DIR: path}), "provider_capability_directory_refused")


class CapabilityTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="genui-provider-capability-")
        self.directory = private_directory(self.tmp, "capabilities")
        self.org, self.invitation = str(uuid.uuid4()), str(uuid.uuid4())

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_capability_file_is_private_exclusive_bounded_and_destroyable(self):
        token, remote = "tinv_" + str(uuid.uuid4()), "tinv_" + str(uuid.uuid4())
        path = pm.write_capability(self.directory, self.org, self.invitation, remote, token, "2026-11-01T00:00:00Z")
        self.assertEqual(path.parent, self.directory)
        self.assertEqual(path.name, "%s.%s.capability" % (self.org, self.invitation))
        info = os.lstat(path)
        self.assertTrue(stat.S_ISREG(info.st_mode))
        self.assertEqual((stat.S_IMODE(info.st_mode), info.st_uid), (0o600, os.geteuid()))
        stored = json.loads(path.read_text())
        self.assertEqual(stored, {"format": "forma:fauth-invite-capability:v1", "organization_id": self.org,
                                  "invitation_id": self.invitation, "remote_invitation_id": remote,
                                  "provider_expires_at": "2026-11-01T00:00:00Z", "invite_token": token})
        self.assertTrue(pm.capability_present(self.directory, self.org, self.invitation))
        with self.assertRaises(pm.Refused):
            pm.write_capability(self.directory, self.org, self.invitation, remote, token, "2026-11-01T00:00:00Z")
        self.assertTrue(pm.destroy_capability(self.directory, self.org, self.invitation))
        self.assertFalse(pm.capability_present(self.directory, self.org, self.invitation))
        self.assertFalse(pm.destroy_capability(self.directory, self.org, self.invitation))
        for bad in (("../x", self.invitation), (self.org, "x/y"), (self.org.upper(), self.invitation)):
            with self.subTest(bad=bad), self.assertRaises(pm.Refused):
                pm.write_capability(self.directory, bad[0], bad[1], remote, token, "2026-11-01T00:00:00Z")
        with self.assertRaises(pm.Refused):
            pm.write_capability(self.directory, self.org, self.invitation, remote, "not-a-token", "2026-11-01T00:00:00Z")


class ClientTests(unittest.TestCase):
    """The management client against the loopback stand-in, in process."""

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="genui-provider-client-")
        self.capabilities = private_directory(self.tmp, "capabilities")
        self.fake = fake_fauth.FakeFauth(TENANT, KEY)
        self.fake.start()
        self.client = pm.Client(self.config(self.fake.url))

    def tearDown(self):
        self.fake.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def config(self, url, key=KEY):
        return pm.load_config({pm.ENV_URL: url, pm.ENV_KEY: key, pm.ENV_CAPABILITY_DIR: str(self.capabilities)}, required=True)

    def creates(self):
        return self.fake.count("POST", "/manage/invites")

    def test_tenant_binding_is_verified_before_any_write(self):
        self.assertEqual(self.client.require_tenant(TENANT), TENANT)
        for client, tenant, reason in ((self.client, "ten_other", "provider_tenant_mismatch"),
                                       (pm.Client(self.config(self.fake.url, "mgmt_" + str(uuid.uuid4()))), TENANT, "provider_unauthorized")):
            with self.subTest(reason=reason), self.assertRaises(pm.Refused) as refusal:
                client.require_tenant(tenant)
            self.assertEqual(str(refusal.exception), reason)
        self.fake.signup_mode = "open"
        with self.assertRaises(pm.Refused) as refusal:
            self.client.require_tenant(TENANT)
        self.assertEqual(str(refusal.exception), "provider_tenant_not_invite_only")
        self.assertEqual(self.creates(), 0)

    def test_create_sends_exactly_the_server_only_contract_body(self):
        outcome = self.client.create(" " + EMAIL + " ", "user")
        self.assertEqual((outcome.state, outcome.usable, outcome.detail, outcome.status), ("confirmed", True, "created", 201))
        self.assertRegex(outcome.remote_id, pm.REMOTE_ID)
        self.assertRegex(outcome.token, pm.TOKEN)
        self.assertNotEqual(outcome.remote_id, outcome.token)
        self.assertEqual(self.fake.status_of(outcome.remote_id), "pending")
        self.assertEqual(self.fake.create_bodies, [{"email": "invitee@example.test", "role": "user", "delivery_mode": "server_only"}])
        self.assertNotIn(outcome.token, repr(outcome))
        self.assertEqual(self.fake.register(outcome.token, EMAIL)[:4], "usr_")

    def test_create_outcomes_are_classified_and_never_repeated(self):
        def mutate(**changes):
            def apply(body):
                body = json.loads(json.dumps(body))
                for path, value in changes.items():
                    target, keys = body, path.split(".")
                    for key in keys[:-1]:
                        target = target[key]
                    if value is None:
                        target.pop(keys[-1])
                    else:
                        target[keys[-1]] = value
                return body
            return apply
        cases = [  # (fault, expected state, usable, created remotely)
            (("status", dict(status=400, error="invalid_role")), "failed", False, False),
            (("status", dict(status=401, error="invalid_api_key")), "failed", False, False),
            (("status", dict(status=403, error="tenant_suspended")), "failed", False, False),
            (("status", dict(status=429, error="too_many_requests")), "failed", False, False),
            (("status", dict(status=500)), "unknown", False, False),
            (("status", dict(status=503)), "unknown", False, False),
            (("status_after", dict(status=502)), "unknown", False, True),
            (("drop_before", {}), "unknown", False, False),
            (("drop_after", {}), "unknown", False, True),
            (("body_after", dict(body=mutate(**{"invite.role_name": "admin"}))), "confirmed", False, True),
            (("body_after", dict(body=mutate(email_delivery="sent"))), "confirmed", False, True),
            (("body_after", dict(body=mutate(invite_url="http://x/?invite_token=y"))), "confirmed", False, True),
            (("body_after", dict(body=mutate(**{"invite.email": "other@example.test"}))), "confirmed", False, True),
            (("body_after", dict(body=mutate(invite_token="short"))), "confirmed", False, True),
            (("body_after", dict(body=mutate(**{"invite.id": None}))), "unknown", False, True),
        ]
        for (mode, kwargs), state, usable, created in cases:
            with self.subTest(mode=mode, kwargs=sorted(kwargs)):
                self.fake.fault("create", mode, **kwargs)
                before, remote_before = self.creates(), len(self.fake.invites)
                outcome = self.client.create(EMAIL, "user")
                self.assertEqual(self.creates(), before + 1, "a create is sent once and never retried")
                self.assertEqual((outcome.state, outcome.usable), (state, usable))
                self.assertEqual(len(self.fake.invites) - remote_before, 1 if created else 0)
                if not usable:
                    self.assertIsNone(outcome.token)
                if state == "confirmed":
                    self.assertRegex(outcome.remote_id, pm.REMOTE_ID)
                    self.assertEqual(outcome.detail, "contract_mismatch")
                if state == "failed":
                    self.assertEqual(outcome.detail, "refused")
                    self.assertIn(outcome.provider_error, ("invalid_role", "invalid_api_key", "tenant_suspended", "too_many_requests"))

    def test_a_response_past_the_deadline_is_unknown(self):
        self.fake.fault("create", "delay_after", seconds=2.5)
        with mock.patch.dict(pm.PROVIDER_DEFAULTS, {"request_deadline_seconds": 1}):
            started = time.monotonic()
            outcome = self.client.create(EMAIL, "user")
            elapsed = time.monotonic() - started
        self.assertEqual((outcome.state, outcome.detail), ("unknown", "transport_ambiguous"))
        self.assertLess(elapsed, 2.0)
        self.assertEqual(len(self.fake.invites), 1)

    def test_unreachable_provider_sends_nothing(self):
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        client = pm.Client(self.config("http://127.0.0.1:%d" % port))
        outcome = client.create(EMAIL, "user")
        self.assertEqual((outcome.state, outcome.detail), ("failed", "not_sent"))
        revoked = client.revoke("tinv_" + str(uuid.uuid4()))
        self.assertEqual((revoked.state, revoked.detail), ("pending", "not_sent"))
        with self.assertRaises(pm.Refused) as refusal:
            client.require_tenant(TENANT)
        self.assertEqual(str(refusal.exception), "provider_unavailable")

    def test_revoke_outcomes(self):
        created = self.client.create(EMAIL, "user")
        self.assertEqual(self.client.revoke(created.remote_id).state, "confirmed")
        self.assertEqual(self.fake.status_of(created.remote_id), "revoked")
        again = self.client.revoke(created.remote_id)
        self.assertEqual((again.state, again.detail), ("confirmed", "not_pending"))
        for (mode, kwargs), state in ((("status", dict(status=400)), "failed"), (("status", dict(status=401, error="invalid_api_key")), "failed"),
                                      (("status", dict(status=403)), "failed"), (("status", dict(status=500)), "unknown"),
                                      (("status_after", dict(status=502)), "unknown"), (("drop_after", {}), "unknown"),
                                      (("drop_before", {}), "unknown")):
            with self.subTest(mode=mode, kwargs=sorted(kwargs)):
                fresh = self.client.create("r-%s@example.test" % uuid.uuid4().hex[:8], "user")
                self.fake.fault("revoke", mode, **kwargs)
                self.assertEqual(self.client.revoke(fresh.remote_id).state, state)
        with self.assertRaises(pm.Refused):
            self.client.revoke("../config")

    def test_scan_pages_through_the_window_and_stops(self):
        now = fake_fauth._now()
        for i in range(450):
            self.fake.create_direct("bulk-%03d@example.test" % i, created_at=now - datetime.timedelta(seconds=450 - i))
        for i in range(5):
            self.fake.create_direct("old-%d@example.test" % i, created_at=now - datetime.timedelta(days=3, seconds=i))
        before = self.fake.count("GET", "/manage/invites")
        seen = self.client.scan(now - datetime.timedelta(hours=1))
        self.assertEqual(len(seen), 450)
        self.assertEqual(self.fake.count("GET", "/manage/invites") - before, 3)
        self.assertEqual({i.email for i in seen if i.email.startswith("old-")}, set())
        seen = self.client.scan(now - datetime.timedelta(days=4))
        self.assertEqual(len(seen), 455)
        with mock.patch.dict(pm.PROVIDER_DEFAULTS, {"list_pages_max": 2}), self.assertRaises(pm.Refused) as refusal:
            self.client.scan(now - datetime.timedelta(hours=1))
        self.assertEqual(str(refusal.exception), "provider_inspection_incomplete")
        self.fake.fault("list", "delay_after", seconds=2.5)
        with mock.patch.dict(pm.PROVIDER_DEFAULTS, {"request_deadline_seconds": 1}), self.assertRaises(pm.Refused) as refusal:
            self.client.scan(now - datetime.timedelta(hours=1))
        self.assertEqual(str(refusal.exception), "provider_unavailable")


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class ProviderDatabaseTests(OperatorDatabase, unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.private = tempfile.mkdtemp(prefix="genui-provider-db-")
        cls.capabilities = private_directory(cls.private, "capabilities")
        cls.key_file = private_file(Path(cls.private, "management-key"), KEY + "\n")
        cls.fake = fake_fauth.FakeFauth(TENANT, KEY)
        cls.fake.start()

    @classmethod
    def tearDownClass(cls):
        cls.fake.stop()
        shutil.rmtree(cls.private, ignore_errors=True)
        super().tearDownClass()

    def provider_env(self, **changes):
        env = {pm.ENV_URL: self.fake.url, pm.ENV_KEY_FILE: str(self.key_file), pm.ENV_CAPABILITY_DIR: str(self.capabilities)}
        env.update(changes)
        return {k: v for k, v in env.items() if v is not None}

    def run_cli(self, *args, expect=0, env=None):
        return self.cli(*args, expect=expect, env_extra=self.provider_env() if env is None else env)

    def raw_cli(self, *args, env=None):
        """The command line with its raw streams, for disclosure checks."""
        import subprocess
        import sys
        full = {k: v for k, v in os.environ.items() if not k.startswith(("PG", "GENUI_PROVIDER_"))}
        full.update(PGHOST="localhost", PGPORT=str(self.port), PGUSER="genui_migration", PGDATABASE="genui",
                    PGSSLROOTCERT=str(Path(self.directory, "ca.pem")), **(self.provider_env() if env is None else env))
        return subprocess.run([sys.executable, str(Path(op.__file__).with_name("admission_operator.py")), *args], capture_output=True,
                              text=True, timeout=60, env=full, cwd=self.directory)

    def invite(self, email=EMAIL, *extra, expect=0, env=None):
        self.ensure_backfill()
        return self.run_cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                            "--provider-email", email, "--provider-role", "user", "--provider-issuer", ISSUER,
                            "--provider-tenant", TENANT, *extra, expect=expect, env=env)

    def rows(self, code_id):
        return self.query("""SELECT invitation_id::text,operation,state,remote_invitation_id FROM genui.provider_invitations
                             WHERE organization_id=%s AND code_id=%s ORDER BY operation""", (ORG, code_id))

    def age(self, code_id, seconds=900):
        """Move this code's provider operations past the reconcile grace (fixture SQL only)."""
        with self.connect("postgres") as c:
            c.execute("""UPDATE genui.provider_invitations SET created_at=created_at-make_interval(secs=>%s),
                         updated_at=updated_at-make_interval(secs=>%s) WHERE organization_id=%s AND code_id=%s""", (seconds, seconds, ORG, code_id))

    def reconcile(self, env=None):
        return self.run_cli("reconcile", "--organization", ORG, env=env)

    def capability(self, invitation_id):
        return Path(self.capabilities, "%s.%s.capability" % (ORG, invitation_id))

    def code_status(self, code_id):
        return self.query("SELECT status FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (ORG, code_id))[0][0]

    def test_issue_creates_one_server_only_invitation_and_keeps_its_token_private(self):
        creates = self.fake.count("POST", "/manage/invites")
        email = "Private-%s@Example.test" % uuid.uuid4().hex[:6]
        self.ensure_backfill()
        p = self.raw_cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                         "--provider-email", email, "--provider-role", "user", "--provider-issuer", ISSUER, "--provider-tenant", TENANT)
        self.assertEqual((p.returncode, p.stderr), (0, ""), p.stderr[-300:])
        out = json.loads(p.stdout)
        invitation = out["provider_invitation"]
        self.assertEqual(set(invitation), {"invitation_id", "operation", "state", "remote_invitation_id", "capability_stored", "detail"})
        self.assertEqual((invitation["operation"], invitation["state"], invitation["capability_stored"], invitation["detail"]),
                         ("create", "confirmed", True, "created"))
        self.assertEqual(self.fake.count("POST", "/manage/invites"), creates + 1)
        self.assertEqual(self.fake.create_bodies[-1], {"email": email.lower(), "role": "user", "delivery_mode": "server_only"})
        rows = self.query("""SELECT invitation_id::text,issuer,provider_tenant_id,remote_invitation_id,intended_email_digest,provider_role,
                             delivery_mode,operation,state FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s""", (ORG, out["code_id"]))
        self.assertEqual(rows, [(invitation["invitation_id"], ISSUER, TENANT, invitation["remote_invitation_id"], op.email_digest(email),
                                 "user", "server_only", "create", "confirmed")])
        path = self.capability(invitation["invitation_id"])
        info = os.lstat(path)
        self.assertEqual((stat.S_IMODE(info.st_mode), info.st_uid), (0o600, os.geteuid()))
        self.assertEqual(stat.S_IMODE(os.lstat(self.capabilities).st_mode), 0o700)
        token = json.loads(path.read_text())["invite_token"]
        self.assertRegex(token, pm.TOKEN)
        dump = self.dump()
        for text in (token, email, email.lower(), KEY):
            self.assertNotIn(text, dump)
            self.assertNotIn(text, p.stdout + p.stderr)
        self.assertIn(invitation["remote_invitation_id"], dump)
        listed = self.run_cli("list", "--organization", ORG)
        entry = next(item for item in listed["codes"] if item["code_id"] == out["code_id"])
        self.assertEqual(entry["provider_invitations"], [{"invitation_id": invitation["invitation_id"], "operation": "create", "state": "confirmed",
                                                          "remote_invitation_id": invitation["remote_invitation_id"]}])
        self.assertNotIn(token, json.dumps(listed))
        # The stored capability is the one the provider accepts for this address (hosted registration stand-in).
        self.assertIsNotNone(self.fake.register(token, email))

    def test_issue_refuses_before_any_write_without_a_verified_provider_tenant(self):
        self.ensure_backfill()
        before, creates = self.dump(), self.fake.count("POST", "/manage/invites")
        wrong = private_file(Path(self.private, "wrong-key-" + uuid.uuid4().hex), "mgmt_" + str(uuid.uuid4()))
        loose_dir = Path(self.private, "loose-" + uuid.uuid4().hex)
        loose_dir.mkdir(mode=0o755)
        os.chmod(loose_dir, 0o755)
        for env, reason in (({}, "provider_configuration_missing"),
                            (self.provider_env(**{pm.ENV_KEY_FILE: str(wrong)}), "provider_unauthorized"),
                            (self.provider_env(**{pm.ENV_CAPABILITY_DIR: str(loose_dir)}), "provider_capability_directory_refused"),
                            (self.provider_env(**{pm.ENV_URL: "http://auth.example.test"}), "provider_url_refused")):
            with self.subTest(reason=reason):
                refused = self.invite(expect=1, env=env)
                self.assertEqual(json.loads(refused.stderr), {"refused": reason})
        self.assertEqual(self.dump(), before)
        # The right key for the wrong binding: the provider's tenant must equal the code's bound tenant.
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.provider_tenant_bindings VALUES (%s,%s,'ten_other_binding',true) ON CONFLICT DO NOTHING", (ORG, ISSUER))
        before = self.dump()
        refused = self.run_cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                               "--provider-email", EMAIL, "--provider-role", "user", "--provider-issuer", ISSUER,
                               "--provider-tenant", "ten_other_binding", expect=1)
        self.assertEqual(json.loads(refused.stderr), {"refused": "provider_tenant_mismatch"})
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.fake.count("POST", "/manage/invites"), creates)

    def test_a_second_live_invitation_for_one_address_is_refused(self):
        """fAuth's create revokes earlier pending invitations for the address (management.rs:1722-1733)."""
        email = "Once-%s@example.test" % uuid.uuid4().hex[:6]
        first = self.invite(email)
        self.assertEqual(first["provider_invitation"]["state"], "confirmed")
        before, creates = self.dump(), self.fake.count("POST", "/manage/invites")
        refused = self.invite(email.lower(), expect=1)
        self.assertEqual(json.loads(refused.stderr), {"refused": "provider_invitation_exists"})
        self.assertEqual((self.dump(), self.fake.count("POST", "/manage/invites")), (before, creates))
        self.assertEqual(self.fake.status_of(first["provider_invitation"]["remote_invitation_id"]), "pending")
        # Once the first code no longer admits anyone, the address can be invited again.
        self.run_cli("revoke", "--organization", ORG, "--code-id", first["code_id"])
        self.assertEqual(self.invite(email)["provider_invitation"]["state"], "confirmed")

    def test_definitive_refusal_records_failed_and_the_code_shows_no_usable_invitation(self):
        self.fake.fault("create", "status", status=400, error="invalid_role")
        out = self.invite()
        invitation = out["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["remote_invitation_id"], invitation["capability_stored"], invitation["detail"]),
                         ("failed", None, False, "refused"))
        self.assertEqual([r[1:] for r in self.rows(out["code_id"])], [("create", "failed", None)])
        self.assertFalse(self.capability(invitation["invitation_id"]).exists())
        self.assertIn("code", out)  # the code itself committed and is shown once; the operator revokes or keeps it

    def test_unknown_create_outcomes_are_reconciled_by_inspection_without_a_second_create(self):
        cases = {"drop_after": ("drop_after", {}, "confirmed"), "status_after": ("status_after", {"status": 502}, "confirmed"),
                 "drop_before": ("drop_before", {}, "failed"), "status": ("status", {"status": 500}, "failed")}
        issued = {}
        for name, (mode, kwargs, _) in cases.items():
            self.fake.fault("create", mode, **kwargs)
            out = self.invite("unknown-%s-%s@example.test" % (name.replace("_", "-"), uuid.uuid4().hex[:6]))
            self.assertEqual(out["provider_invitation"]["state"], "unknown", name)
            self.assertIsNone(out["provider_invitation"]["remote_invitation_id"])
            issued[name] = out
        creates = self.fake.count("POST", "/manage/invites")
        # Inside the grace period nothing is judged.
        young = self.reconcile()
        for name, out in issued.items():
            self.assertEqual(self.rows(out["code_id"])[0][2], "unknown", name)
        self.assertGreaterEqual(young["deferred"], len(issued))
        for out in issued.values():
            self.age(out["code_id"])
        result = self.reconcile()
        self.assertEqual(self.fake.count("POST", "/manage/invites"), creates, "reconcile never creates")
        for name, (_, _, expected) in cases.items():
            row = self.rows(issued[name]["code_id"])[0]
            self.assertEqual(row[2], expected, name)
            entry = next(o for o in result["operations"] if o["invitation_id"] == row[0])
            self.assertEqual((entry["operation"], entry["state_before"], entry["state"]), ("create", "unknown", expected))
            if expected == "confirmed":
                self.assertEqual(self.fake.status_of(row[3]), "pending")
                self.assertEqual(entry["capability_stored"], False)  # the lost response's token is unrecoverable: reissue
            else:
                self.assertIsNone(row[3])
        own = {self.rows(out["code_id"])[0][0] for out in issued.values()}
        again = self.reconcile()
        self.assertEqual([o for o in again["operations"] if o["invitation_id"] in own], [])
        self.assertEqual(self.fake.count("POST", "/manage/invites"), creates)

    def test_timed_out_create_is_unknown_within_the_deadline(self):
        self.fake.fault("create", "delay_after", seconds=pm.PROVIDER_DEFAULTS["request_deadline_seconds"] + 3)
        started = time.monotonic()
        out = self.invite("slow-%s@example.test" % uuid.uuid4().hex[:6])
        self.assertLess(time.monotonic() - started, pm.PROVIDER_DEFAULTS["request_deadline_seconds"] + 8)
        self.assertEqual((out["provider_invitation"]["state"], out["provider_invitation"]["detail"]), ("unknown", "transport_ambiguous"))
        self.age(out["code_id"])
        self.reconcile()
        row = self.rows(out["code_id"])[0]
        self.assertEqual(row[2], "confirmed")
        self.assertRegex(row[3], pm.REMOTE_ID)

    def test_ambiguous_remote_match_stays_unknown(self):
        email = "twice-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "drop_after")
        out = self.invite(email)
        self.fake.create_direct(email)  # a second invitation for the same address from another tool
        self.age(out["code_id"])
        result = self.reconcile()
        entry = next(o for o in result["operations"] if o["operation"] == "create" and o["invitation_id"] == self.rows(out["code_id"])[0][0])
        self.assertEqual((entry["state"], entry["detail"]), ("unknown", "ambiguous_remote_match"))
        self.assertEqual(self.rows(out["code_id"])[0][2], "unknown")

    def test_reconcile_skips_an_operation_held_by_another_operator(self):
        self.fake.fault("create", "drop_after")
        out = self.invite("held-%s@example.test" % uuid.uuid4().hex[:6])
        self.age(out["code_id"])
        invitation = self.rows(out["code_id"])[0][0]
        with self.connect("postgres") as holder:
            holder.execute("SELECT 1 FROM genui.provider_invitations WHERE organization_id=%s AND invitation_id=%s FOR UPDATE", (ORG, invitation))
            result = self.reconcile()
            self.assertEqual(self.rows(out["code_id"])[0][2], "unknown")
            self.assertNotIn(invitation, [o["invitation_id"] for o in result["operations"]])
        self.reconcile()
        self.assertEqual(self.rows(out["code_id"])[0][2], "confirmed")

    def test_code_revocation_destroys_the_capability_and_revokes_remotely(self):
        out = self.invite("revoke-%s@example.test" % uuid.uuid4().hex[:6])
        created = out["provider_invitation"]
        self.assertTrue(self.capability(created["invitation_id"]).exists())
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual((revoked["changed"], revoked["provider_revocations_pending"], revoked["capabilities_destroyed"], revoked["provider_phase"]),
                         (True, 1, 1, "completed"))
        self.assertEqual([(r["operation"], r["state"], r["remote_invitation_id"]) for r in revoked["provider_revocations"]],
                         [("revoke", "confirmed", created["remote_invitation_id"])])
        self.assertFalse(self.capability(created["invitation_id"]).exists())
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "revoked")
        self.assertEqual([r[1:] for r in self.rows(out["code_id"])],
                         [("create", "confirmed", created["remote_invitation_id"]), ("revoke", "confirmed", created["remote_invitation_id"])])
        self.assertEqual(self.operator_events()[-1], ("code.revoked",))
        own = {r[0] for r in self.rows(out["code_id"])}
        self.assertEqual([o for o in self.reconcile()["operations"] if o["invitation_id"] in own], [])

    def test_a_failed_remote_revocation_never_restores_local_access(self):
        out = self.invite("fail-%s@example.test" % uuid.uuid4().hex[:6])
        created = out["provider_invitation"]
        digest = op.code_digest(op.normalize_code(out["code"]))
        self.fake.fault("revoke", "status", status=503)
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual(revoked["changed"], True)
        self.assertEqual([r["state"] for r in revoked["provider_revocations"]], ["unknown"])
        self.assertFalse(self.capability(created["invitation_id"]).exists())
        self.assertEqual(self.code_status(out["code_id"]), "revoked")
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "pending")
        with self.workspace() as c:
            self.assertEqual(c.execute("SELECT outcome FROM genui_private.reserve_code(%s,%s,%s)", (ORG, digest, "sha256:" + "c" * 64)).fetchone(),
                             ("revoked",))
        own = {r[0] for r in self.rows(out["code_id"])}
        self.fake.fault("revoke", "status", status=401, error="invalid_api_key")
        result = self.reconcile()
        self.assertEqual([o["state"] for o in result["operations"] if o["invitation_id"] in own], ["failed"])
        self.assertEqual(self.code_status(out["code_id"]), "revoked")
        result = self.reconcile()
        self.assertEqual([o["state"] for o in result["operations"] if o["invitation_id"] in own], ["confirmed"])
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "revoked")
        self.assertEqual(self.code_status(out["code_id"]), "revoked")

    def test_local_revocation_never_waits_for_provider_configuration(self):
        out = self.invite("noconfig-%s@example.test" % uuid.uuid4().hex[:6])
        created = out["provider_invitation"]
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"], env={})
        self.assertEqual((revoked["changed"], revoked["provider_revocations_pending"], revoked["provider_phase"], revoked["capabilities_destroyed"]),
                         (True, 1, "not_configured", None))
        self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "confirmed"), ("revoke", "pending")])
        own = {r[0] for r in self.rows(out["code_id"])}
        result = self.reconcile()
        self.assertEqual([(o["operation"], o["state_before"], o["state"]) for o in result["operations"] if o["invitation_id"] in own],
                         [("revoke", "pending", "confirmed")])
        self.assertGreaterEqual(result["capabilities_destroyed"], 1)
        self.assertFalse(self.capability(created["invitation_id"]).exists())
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "revoked")

    def test_revocation_of_an_unknown_create_waits_for_its_reconciliation(self):
        email = "await-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "drop_after")
        out = self.invite(email)
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.assertEqual([(r["state"], r["detail"]) for r in revoked["provider_revocations"]], [("pending", "awaiting_create_reconciliation")])
        self.age(out["code_id"])
        result = self.reconcile()
        self.assertEqual([(o["operation"], o["state"]) for o in result["operations"] if o["invitation_id"] in [r[0] for r in self.rows(out["code_id"])]],
                         [("create", "confirmed"), ("revoke", "confirmed")])
        remote = self.rows(out["code_id"])[0][3]
        self.assertEqual(self.fake.status_of(remote), "revoked")
        self.assertEqual(self.rows(out["code_id"])[1][3], remote)
        # A create judged failed leaves nothing to revoke remotely.
        self.fake.fault("create", "drop_before")
        out = self.invite("never-%s@example.test" % uuid.uuid4().hex[:6])
        self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.age(out["code_id"])
        self.reconcile()
        self.assertEqual([r[1:] for r in self.rows(out["code_id"])], [("create", "failed", None), ("revoke", "confirmed", None)])

    def test_reconcile_propagates_expiry_and_exhaustion_and_ends_the_capability(self):
        expired = self.invite("expired-%s@example.test" % uuid.uuid4().hex[:6])
        exhausted = self.invite("exhausted-%s@example.test" % uuid.uuid4().hex[:6])
        lapsed = self.invite("lapsed-%s@example.test" % uuid.uuid4().hex[:6])
        live = self.invite("live-%s@example.test" % uuid.uuid4().hex[:6])
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.access_codes SET expires_at=issued_at+interval '1 millisecond' WHERE code_id=%s", (expired["code_id"],))
            c.execute("UPDATE genui.access_codes SET status='exhausted',redeemed_count=max_redemptions WHERE code_id=%s", (exhausted["code_id"],))
        path = self.capability(lapsed["provider_invitation"]["invitation_id"])
        stored = json.loads(path.read_text())
        stored["provider_expires_at"] = "2020-01-01T00:00:00Z"
        os.chmod(path, 0o600)
        path.write_text(json.dumps(stored))
        events = len(self.operator_events())
        result = self.reconcile()
        self.assertGreaterEqual(result["revocations_recorded"], 2)
        for out in (expired, exhausted):
            self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "confirmed"), ("revoke", "confirmed")])
            self.assertEqual(self.fake.status_of(out["provider_invitation"]["remote_invitation_id"]), "revoked")
            self.assertFalse(self.capability(out["provider_invitation"]["invitation_id"]).exists())
        self.assertFalse(path.exists())
        self.assertTrue(self.capability(live["provider_invitation"]["invitation_id"]).exists())
        self.assertEqual([r[1:3] for r in self.rows(live["code_id"])], [("create", "confirmed")])
        self.assertEqual(len(self.operator_events()), events, "provider operation state is not an audit kind (D7.8 is closed)")
        self.assertEqual(self.run_cli("verify", "--organization", ORG)["verified"], True)

    def test_contract_mismatch_is_confirmed_unusable_and_revoked(self):
        def admin(body):
            body = json.loads(json.dumps(body))
            body["invite"]["role_name"] = "admin"
            return body
        self.fake.fault("create", "body_after", body=admin)
        out = self.invite("mismatch-%s@example.test" % uuid.uuid4().hex[:6])
        invitation = out["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["capability_stored"], invitation["detail"]), ("confirmed", False, "contract_mismatch"))
        self.assertFalse(self.capability(invitation["invitation_id"]).exists())
        self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "confirmed"), ("revoke", "confirmed")])
        self.assertEqual(self.fake.status_of(invitation["remote_invitation_id"]), "revoked")

    def test_reconcile_requires_configuration_and_an_operator_identity(self):
        refused = self.run_cli("reconcile", "--organization", ORG, expect=1, env={})
        self.assertEqual(json.loads(refused.stderr), {"refused": "provider_configuration_missing"})
        self.ensure_backfill()
        before, calls = self.dump(), len(self.fake.requests)
        for role in ("genui_workspace", "genui_control", "genui_data", "genui_worker"):
            with self.subTest(role=role):
                for args in (("reconcile", "--organization", ORG),
                             ("issue", "--organization", ORG, "--issued-by", "forged", "--grant", "source_read:" + self.source,
                              "--provider-email", "forged@example.test", "--provider-role", "user", "--provider-issuer", ISSUER,
                              "--provider-tenant", TENANT)):
                    p = self.cli(*args, user=role, expect=1, env_extra=self.provider_env())
                    self.assertEqual(json.loads(p.stderr), {"refused": "operator_identity_refused"})
        # A forged operator reaches neither the database nor the provider.
        self.assertEqual((self.dump(), len(self.fake.requests)), (before, calls))
