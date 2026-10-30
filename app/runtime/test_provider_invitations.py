"""APP-073 acceptance 6 and 7: operator provider invitations and reconciliation.

SIMULATED TRANSPORT. Every provider here is fake_fauth.FakeFauth, a loopback
stand-in built from the merged fAuth contract text and the public gatewasm
source at 1bf5d3b; nothing calls the deployed fAuth, and nothing here is hosted
identity evidence (APP-074/076). The capability secret (ADR-003 amendment
2026-10-30) is local_secrets.Stub, a loopback Secrets Manager stand-in spoken to
by the real boto3 client; nothing calls AWS. The database tests reuse
test_admission_operator's disposable PostgreSQL 16.13 container with a throwaway
TLS CA and drive the real command line as genui_migration over verify-full TLS.
"""
import contextlib
import datetime
import io
import json
import os
from pathlib import Path
import shutil
import socket
import tempfile
import threading
import time
import unittest
from unittest import mock
import uuid

import admission_operator as op
import fake_fauth
import local_secrets
import provider_management as pm
import runtime
from test_admission_operator import ISSUER, ORG, TENANT, OperatorDatabase

KEY = "mgmt_" + str(uuid.uuid4())
EMAIL = "Invitee@Example.test"
ARN = local_secrets.LOCAL_SECRET_ARN


class SecretEntry:
    """One capability id in the stand-in secret, read like the former capability file."""

    def __init__(self, stub, key):
        self.stub, self.key = stub, key

    def entries(self):
        return json.loads(self.stub.value())

    def exists(self):
        return self.key in self.entries()

    def read_text(self):
        return json.dumps(self.entries()[self.key])

    def write_text(self, text):
        """Fixture-only replacement of this entry (a new AWSCURRENT version)."""
        entries = self.entries()
        entries[self.key] = json.loads(text)
        self.stub.set_value(json.dumps(entries))


def private_file(path, text, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, "w") as f:
        f.write(text)
    os.chmod(path, mode)
    return path


class ConfigTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="genui-provider-config-")
        self.key_file = private_file(Path(self.tmp, "key"), KEY + "\n")

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def env(self, **changes):
        base = {pm.ENV_URL: "https://auth.example.test", pm.ENV_KEY_FILE: str(self.key_file), pm.ENV_CAPABILITY_SECRET: ARN}
        base.update(changes)
        return {k: v for k, v in base.items() if v is not None}

    def refused(self, env, reason, required=True, secret_id=None):
        with self.assertRaises(pm.Refused) as refusal:
            pm.load_config(env, required=required, secret_id=secret_id)
        self.assertEqual(str(refusal.exception), reason)
        self.assertNotIn(KEY, repr(refusal.exception))

    def test_complete_configuration_loads_without_echoing_the_key(self):
        config = pm.load_config(self.env(), required=True)
        self.assertEqual((config.scheme, config.host, config.port), ("https", "auth.example.test", 443))
        self.assertEqual((config.capability_store.secret_id, config.capability_store.region), (ARN, "us-east-1"))
        self.assertNotIn(KEY, repr(config))
        self.assertNotIn(KEY, str(config))
        config = pm.load_config(self.env(**{pm.ENV_KEY_FILE: None, pm.ENV_KEY: KEY, pm.ENV_URL: "http://127.0.0.1:8123/"}), required=True)
        self.assertEqual((config.scheme, config.host, config.port), ("http", "127.0.0.1", 8123))

    def test_absent_and_partial_configuration(self):
        self.assertIsNone(pm.load_config({}, required=False))
        self.refused({}, "provider_configuration_missing")
        self.refused({pm.ENV_URL: "https://auth.example.test"}, "provider_configuration_incomplete", required=False)
        # ADR-003 amendment 2026-10-30: a provider configuration without the capability secret is incomplete.
        self.refused(self.env(**{pm.ENV_CAPABILITY_SECRET: None}), "provider_configuration_incomplete")

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

    def test_injected_management_configuration_is_the_managed_tasks_only_source(self):
        """ADR-003 amendment 2026-10-30 (managed invitations): the ProviderManagementKey secret, injected whole."""
        managed = json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "https://auth.example.test", "management_key": KEY})
        base = {pm.ENV_CAPABILITY_SECRET: ARN}
        config = pm.load_config(dict(base, **{pm.ENV_MANAGEMENT_JSON: managed}), required=True)
        self.assertEqual((config.scheme, config.host, config.port), ("https", "auth.example.test", 443))
        self.assertEqual(config.authorization(), "Bearer " + KEY)
        self.assertNotIn(KEY, repr(config))
        # `{}` is "not configured": optional callers get None, required ones the usual missing reason.
        self.assertIsNone(pm.load_config(dict(base, **{pm.ENV_MANAGEMENT_JSON: "{}"}), required=False))
        self.refused(dict(base, **{pm.ENV_MANAGEMENT_JSON: "{}"}), "provider_configuration_missing")
        # The plain variables are never merged with or preferred over the injected secret.
        for beside in ({pm.ENV_URL: "https://auth.example.test"}, {pm.ENV_KEY_FILE: str(self.key_file)}, {pm.ENV_KEY: KEY}):
            self.refused(dict(base, **{pm.ENV_MANAGEMENT_JSON: managed}, **beside), "provider_configuration_conflict", required=False)
        for bad, reason in (("[]", "provider_configuration_refused"), ("{", "provider_configuration_refused"), ("x" * 5000, "provider_configuration_refused"),
                            (json.dumps({"url": "https://auth.example.test", "management_key": KEY}), "provider_configuration_refused"),
                            (json.dumps({"format": "other", "url": "https://auth.example.test", "management_key": KEY}), "provider_configuration_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "https://auth.example.test", "management_key": KEY, "x": 1}), "provider_configuration_refused"),
                            ('{"format":"%s","format":"%s","url":"https://a.test","management_key":"%s"}' % (pm.MANAGEMENT_FORMAT, pm.MANAGEMENT_FORMAT, KEY),
                             "provider_configuration_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "https://auth.example.test", "management_key": "plat_" + KEY[5:]}), "provider_key_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "https://auth.example.test", "management_key": KEY + " x"}), "provider_key_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "http://auth.example.test", "management_key": KEY}), "provider_url_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": "", "management_key": KEY}), "provider_url_refused"),
                            (json.dumps({"format": pm.MANAGEMENT_FORMAT, "url": 7, "management_key": KEY}), "provider_url_refused")):
            with self.subTest(bad=bad[:30]):
                self.refused(dict(base, **{pm.ENV_MANAGEMENT_JSON: bad}), reason, required=False)
        # The injected configuration still needs the capability secret's identifier.
        self.refused({pm.ENV_MANAGEMENT_JSON: managed}, "provider_configuration_incomplete")

    def test_capability_secret_is_a_full_arn_and_the_host_directory_is_retired(self):
        """ADR-003 amendment 2026-10-30 (APP-074 review M1): the secret replaces the operator-host directory."""
        for bad in ("genui-provider-capabilities", "arn:aws:s3:::bucket", ARN + "\n", ARN.replace("us-east-1", "US-EAST-1"),
                    "arn:aws:secretsmanager:us-east-1:12345:secret:x", ""):
            with self.subTest(bad=bad):
                self.refused(self.env(**{pm.ENV_CAPABILITY_SECRET: bad}), "capability_secret_refused")
                with self.assertRaises(op.Refused) as refusal:
                    op.validate(op.parser().parse_args(["reconcile", "--organization", ORG, "--capability-secret-id", bad]))
                self.assertEqual(str(refusal.exception), "capability_secret_refused")
        for env in (self.env(**{pm.ENV_RETIRED_CAPABILITY_DIR: self.tmp}), {pm.ENV_RETIRED_CAPABILITY_DIR: self.tmp}):
            self.refused(env, "provider_capability_directory_retired", required=False)
            with self.assertRaises(pm.Refused):
                pm.load_store(env)
        # The argument (an identifier, not a value) takes precedence over the environment.
        other = ARN.replace("local-AbCdEf", "other-GhIjKl").replace("us-east-1", "eu-west-2")
        config = pm.load_config(self.env(), required=True, secret_id=other)
        self.assertEqual((config.capability_store.secret_id, config.capability_store.region), (other, "eu-west-2"))
        self.assertIsNone(pm.load_store({}))
        self.assertEqual(pm.load_store({pm.ENV_CAPABILITY_SECRET: ARN}).secret_id, ARN)
        for command in ("issue", "revoke", "reconcile"):
            self.assertIn("--capability-secret-id", op.parser()._subparsers._group_actions[0].choices[command].format_usage())
        for command in ("list", "extend", "verify"):
            self.assertNotIn("--capability-secret-id", op.parser()._subparsers._group_actions[0].choices[command].format_usage())


class CapabilitySecretTests(unittest.TestCase):
    """The environment's capability secret through the real boto3 client against the loopback stand-in."""

    def setUp(self):
        self.stub = local_secrets.Stub(ARN).start()
        self.store = pm.CapabilitySecret(ARN, client=self.stub.client())
        self.org, self.invitation = str(uuid.uuid4()), str(uuid.uuid4())
        self.token, self.remote = "tinv_" + str(uuid.uuid4()), "tinv_" + str(uuid.uuid4())
        self.digest = "sha256:" + "ab" * 32

    def tearDown(self):
        self.stub.stop()

    def add(self, invitation=None, **changes):
        args = dict(organization=self.org, invitation_id=invitation or self.invitation, remote_id=self.remote, token=self.token,
                    expires_at="2026-11-03T00:00:00Z", code_digest=self.digest)
        args.update(changes)
        return self.store.add(**args)

    def test_a_capability_is_added_once_exactly_and_removed(self):
        key = self.add()
        self.assertEqual(key, "%s.%s" % (self.org, self.invitation))
        stored = json.loads(self.stub.value())
        self.assertEqual(stored, {key: {"format": "forma:fauth-invite-capability:v1", "organization_id": self.org,
                                        "invitation_id": self.invitation, "remote_invitation_id": self.remote,
                                        "provider_expires_at": "2026-11-03T00:00:00Z", "invite_token": self.token,
                                        "code_digest": self.digest}})
        snapshot = self.store.read()
        self.assertTrue(pm.capability_present(snapshot, self.org, self.invitation))
        self.assertEqual(pm.capability_record(snapshot, self.org, self.invitation)["remote_invitation_id"], self.remote)
        self.assertEqual(pm.capability_invitations(snapshot, self.org), [self.invitation])
        self.assertEqual(pm.capability_invitations(snapshot, str(uuid.uuid4())), [])
        # An existing entry is never replaced: refused, nothing written.
        puts = self.stub.count("PutSecretValue")
        with self.assertRaises(pm.Refused) as refusal:
            self.add(remote_id="tinv_" + str(uuid.uuid4()))
        self.assertEqual((str(refusal.exception), self.stub.count("PutSecretValue")), ("capability_unwritable", puts))
        self.assertEqual(self.store.remove([(self.org, self.invitation), (self.org, str(uuid.uuid4()))]), 1)
        self.assertEqual(json.loads(self.stub.value()), {})
        puts = self.stub.count("PutSecretValue")
        self.assertEqual(self.store.remove([(self.org, self.invitation)]), 0)
        self.assertEqual(self.stub.count("PutSecretValue"), puts, "nothing to remove writes nothing")

    def test_only_well_formed_entries_are_written(self):
        for changes, reason in (({"organization": "../x"}, "malformed_identifier"), ({"invitation": "x/y"}, "malformed_identifier"),
                                ({"organization": self.org.upper()}, "malformed_identifier"), ({"token": "not-a-token"}, "provider_contract_mismatch"),
                                ({"token": self.remote}, "provider_contract_mismatch"),
                                ({"expires_at": "2026-11-03T00:00:00.5+00:00"}, "provider_contract_mismatch"),
                                ({"code_digest": "ABCD"}, "malformed_identifier"), ({"code_digest": "sha256:" + "AB" * 32}, "malformed_identifier"),
                                ({"code_digest": 7}, "malformed_identifier")):
            with self.subTest(changes=sorted(changes)), self.assertRaises(pm.Refused) as refusal:
                args = dict(changes)
                organization = args.pop("organization", self.org)
                self.store.add(organization, args.pop("invitation", self.invitation), args.pop("remote_id", self.remote),
                               args.pop("token", self.token), args.pop("expires_at", "2026-11-03T00:00:00Z"), args.pop("code_digest", self.digest))
            self.assertEqual(str(refusal.exception), reason)
        self.assertEqual(self.stub.count("PutSecretValue"), 0)
        # A record without the code digest is still a valid record (the service never hands it off).
        self.add(code_digest=None)
        self.assertNotIn("code_digest", json.loads(self.stub.value())["%s.%s" % (self.org, self.invitation)])

    def test_the_whole_map_is_bounded_and_a_malformed_secret_refuses(self):
        self.stub.set_value(json.dumps({"%s.%s" % (self.org, self.invitation): {"format": "other"}}))
        with self.assertRaises(pm.Refused) as refusal:
            self.store.read()
        self.assertEqual(str(refusal.exception), "capability_secret_malformed")
        with self.assertRaises(pm.Refused):
            self.add()
        self.stub.set_value(json.dumps(dict(pm.capability_entry(self.org, str(uuid.uuid4()), self.remote, self.token, "2026-11-03T00:00:00Z")
                                            for _ in range(pm.CAPABILITIES_MAX))))
        with self.assertRaises(pm.Refused) as refusal:
            self.add()
        self.assertEqual(str(refusal.exception), "capability_secret_full")
        self.assertEqual(self.stub.count("PutSecretValue"), 0)

    def test_store_failures_answer_fixed_reasons_and_never_the_value(self):
        self.stub.deny_puts = True
        with self.assertRaises(pm.Refused) as refusal:
            self.add()
        self.assertEqual(str(refusal.exception), "capability_secret_unwritten")
        self.assertIsNone(refusal.exception.__cause__)
        self.assertNotIn(self.token, repr(refusal.exception) + str(refusal.exception.__context__))
        self.assertEqual(json.loads(self.stub.value()), {})
        self.stub.deny_puts = False
        # An ambiguous 500 after the write: the SDK retries with the same token, which is idempotent.
        self.stub.fail_after_put = 1
        self.add()
        self.assertEqual(self.stub.count("PutSecretValue"), 3)  # the denied one, the ambiguous one, its retry
        self.assertEqual(len(self.stub.versions), 1)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        import boto3
        from botocore.config import Config
        unreachable = boto3.session.Session(aws_access_key_id="x", aws_secret_access_key="y", region_name="us-east-1").client(
            "secretsmanager", endpoint_url="http://127.0.0.1:%d" % port,
            config=Config(retries={"total_max_attempts": 1}, connect_timeout=1, read_timeout=1))
        with self.assertRaises(pm.Refused) as refusal:
            pm.CapabilitySecret(ARN, client=unreachable).read()
        self.assertEqual(str(refusal.exception), "capability_secret_unavailable")
        # The identity policy: a caller without PutSecretValue cannot write it.
        self.stub.grants = {local_secrets.LOCAL_ACCESS_KEY_ID: {"secretsmanager:GetSecretValue"}}
        with self.assertRaises(pm.Refused) as refusal:
            self.add(invitation=str(uuid.uuid4()))
        self.assertEqual(str(refusal.exception), "capability_secret_unwritten")

    def test_what_the_operator_writes_is_what_the_service_supervisor_materializes(self):
        """The handoff in one process: operator add -> secret -> runtime.write_capabilities -> the service's file."""
        self.add()
        other = str(uuid.uuid4())
        self.add(invitation=other, token="tinv_" + str(uuid.uuid4()))
        target = Path(tempfile.mkdtemp(prefix="genui-capability-handoff-"))
        try:
            target.chmod(0o700)
            names = runtime.write_capabilities(str(target), self.stub.value())
            self.assertEqual(sorted(names), sorted("%s.%s.capability" % (self.org, i) for i in (self.invitation, other)))
            record = json.loads(Path(target, "%s.%s.capability" % (self.org, self.invitation)).read_text())
            self.assertEqual(record, json.loads(self.stub.value())["%s.%s" % (self.org, self.invitation)])
        finally:
            shutil.rmtree(target, ignore_errors=True)


class ClientTests(unittest.TestCase):
    """The management client against the loopback stand-in, in process."""

    def setUp(self):
        self.fake = fake_fauth.FakeFauth(TENANT, KEY)
        self.fake.start()
        self.client = pm.Client(self.config(self.fake.url))

    def tearDown(self):
        self.fake.stop()

    def config(self, url, key=KEY):
        return pm.load_config({pm.ENV_URL: url, pm.ENV_KEY: key, pm.ENV_CAPABILITY_SECRET: ARN}, required=True)

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
        cls.key_file = private_file(Path(cls.private, "management-key"), KEY + "\n")
        cls.fake = fake_fauth.FakeFauth(TENANT, KEY)
        cls.fake.start()
        cls.secret = local_secrets.Stub(ARN).start()

    @classmethod
    def tearDownClass(cls):
        cls.fake.stop()
        cls.secret.stop()
        shutil.rmtree(cls.private, ignore_errors=True)
        super().tearDownClass()

    def provider_env(self, **changes):
        env = dict(local_secrets.environment(self.secret.endpoint), **{pm.ENV_URL: self.fake.url, pm.ENV_KEY_FILE: str(self.key_file)})
        env.update(changes)
        return {k: v for k, v in env.items() if v is not None}

    def run_cli(self, *args, expect=0, env=None):
        return self.cli(*args, expect=expect, env_extra=self.provider_env() if env is None else env)

    def raw_cli(self, *args, env=None):
        """The command line with its raw streams, for disclosure checks."""
        import subprocess
        import sys
        full = {k: v for k, v in os.environ.items() if not k.startswith(("PG", "GENUI_PROVIDER_", "AWS_"))}
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

    def age(self, *code_ids, seconds=900):
        """Move these codes' provider operations, and the stand-in's invitations for their addresses,
        `seconds` into the past: past the reconcile grace, keeping each invitation inside its create's
        window (fixture SQL and stand-in clock travel only)."""
        digests = {r[0] for r in self.query("""SELECT intended_email_digest::text FROM genui.provider_invitations
                                               WHERE organization_id=%s AND code_id=ANY(%s::uuid[])""", (ORG, list(code_ids)))}
        with self.connect("postgres") as c:
            c.execute("""UPDATE genui.provider_invitations SET created_at=created_at-make_interval(secs=>%s),
                         updated_at=updated_at-make_interval(secs=>%s) WHERE organization_id=%s AND code_id=ANY(%s::uuid[])""",
                      (seconds, seconds, ORG, list(code_ids)))
        self.fake.shift(lambda invite: op.email_digest(invite["email"]) in digests, seconds)

    def remote_ids(self, email):
        return [i["id"] for i in self.fake.invites.values() if i["email"] == email.strip().lower()]

    def entry(self, result, invitation_id):
        return next(o for o in result["operations"] if o["invitation_id"] == invitation_id)

    def pg_env(self):
        return {"PGHOST": "localhost", "PGPORT": str(self.port), "PGUSER": "genui_migration", "PGDATABASE": "genui",
                "PGSSLROOTCERT": str(Path(self.directory, "ca.pem"))}

    def reconcile(self, env=None):
        return self.run_cli("reconcile", "--organization", ORG, env=env)

    def capability(self, invitation_id):
        """This invitation's entry in the environment's capability secret (the stand-in's AWSCURRENT)."""
        return SecretEntry(self.secret, "%s.%s" % (ORG, invitation_id))

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
        # ADR-003 amendment 2026-10-30: the token is in the environment's secret only; the
        # operator host keeps no copy (the working directory holds nothing new).
        self.assertTrue(path.exists())
        self.assertEqual(sorted(os.listdir(self.private)), ["management-key"])
        self.assertNotIn(KEY, self.secret.value())
        record = json.loads(path.read_text())
        token = record["invite_token"]
        self.assertRegex(token, pm.TOKEN)
        # APP-074: the capability records the code's stored digest (never the code),
        # which is how the service's invitation entry finds it for a validated code.
        self.assertEqual(record["code_digest"], op.code_digest(op.normalize_code(out["code"])))
        self.assertEqual(set(record), {"format", "organization_id", "invitation_id", "remote_invitation_id",
                                       "provider_expires_at", "invite_token", "code_digest"})
        self.assertNotIn(out["code"], path.read_text())
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

    def managed_env(self, invitations=None, management=None):
        """The managed operator task's environment: the two injected secrets, no plain provider variable."""
        env = {k: v for k, v in self.provider_env().items() if k not in (pm.ENV_URL, pm.ENV_KEY_FILE, pm.ENV_KEY)}
        env[pm.ENV_MANAGEMENT_JSON] = management if management is not None else json.dumps(
            {"format": pm.MANAGEMENT_FORMAT, "url": self.fake.url, "management_key": KEY})
        if invitations is not None:
            env[op.INVITATION_INPUT_ENV] = invitations if isinstance(invitations, str) else json.dumps(
                {"format": op.INVITATION_INPUT_FORMAT, "invitations": invitations})
        return env

    def managed_issue(self, digest, env):
        return self.raw_cli("issue", "--organization", ORG, "--issued-by", "app078-operator", "--grant", "source_read:" + self.source,
                            "--code-digest", digest, "--provider-input-from-secret", env=env)

    def test_managed_invitation_takes_the_address_from_the_input_secret_and_prints_identifiers_only(self):
        """ADR-003 amendment 2026-10-30 (managed invitations; APP-076 G2): the managed form of `issue`."""
        self.ensure_backfill()
        raw = os.urandom(32)
        code, digest = op.encode_code(raw), op.code_digest(raw)
        email = "Managed-%s@Example.test" % uuid.uuid4().hex[:6]
        entry = {"organization_id": ORG, "code_digest": digest, "email": email, "role": "user", "issuer": ISSUER, "tenant": TENANT}
        creates = self.fake.count("POST", "/manage/invites")
        p = self.managed_issue(digest, self.managed_env([entry]))
        self.assertEqual((p.returncode, p.stderr), (0, ""), p.stderr[-300:])
        out = json.loads(p.stdout)
        self.assertEqual(set(out), {"code_id", "code_digest", "kind", "max_redemptions", "expires_at", "provider_invitation"})
        self.assertEqual(out["code_digest"], digest)
        invitation = out["provider_invitation"]
        self.assertEqual((invitation["operation"], invitation["state"], invitation["capability_stored"], invitation["detail"]),
                         ("create", "confirmed", True, "created"))
        # Exactly one server-only create, for the input's address; the output and the database never carry it.
        self.assertEqual(self.fake.count("POST", "/manage/invites"), creates + 1)
        self.assertEqual(self.fake.create_bodies[-1], {"email": email.lower(), "role": "user", "delivery_mode": "server_only"})
        dump = self.dump()
        for text in (email, email.lower(), KEY, code):
            self.assertNotIn(text, p.stdout + p.stderr)
            self.assertNotIn(text, dump)
        rows = self.query("""SELECT issuer,provider_tenant_id,remote_invitation_id,intended_email_digest,provider_role,delivery_mode,operation,state
                             FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s""", (ORG, out["code_id"]))
        self.assertEqual(rows, [(ISSUER, TENANT, invitation["remote_invitation_id"], op.email_digest(email), "user", "server_only", "create", "confirmed")])
        # The capability names the pre-minted digest, so the service's invitation entry finds it for the minted code.
        record = json.loads(self.capability(invitation["invitation_id"]).read_text())
        self.assertEqual(record["code_digest"], digest)
        self.assertRegex(record["invite_token"], pm.TOKEN)
        self.assertNotIn(record["invite_token"], p.stdout + p.stderr + dump)
        self.assertEqual(self.query("SELECT organization_id::text FROM genui_private.code_index WHERE code_digest=%s", (digest,)), [(ORG,)])
        self.assertIsNotNone(self.fake.register(record["invite_token"], email))
        # A reused digest refuses whole: no second code, no second create.
        before = self.dump()
        again = self.managed_issue(digest, self.managed_env([entry]))
        self.assertEqual(again.returncode, 1)
        self.assertEqual((self.dump(), self.fake.count("POST", "/manage/invites")), (before, creates + 1))

    def test_managed_invitation_refuses_a_missing_mismatched_or_malformed_entry_before_any_write(self):
        self.ensure_backfill()
        digest, other = op.code_digest(os.urandom(32)), op.code_digest(os.urandom(32))
        email = "unused-%s@example.test" % uuid.uuid4().hex[:6]
        entry = {"organization_id": ORG, "code_digest": digest, "email": email, "role": "user", "issuer": ISSUER, "tenant": TENANT}
        before, creates, secret_before = self.dump(), self.fake.count("POST", "/manage/invites"), self.secret.value()
        cases = [(self.managed_env(), "invitation_input_missing"),                                   # no input injected
                 (self.managed_env("{}"), "invitation_input_missing"),                               # the emptied document
                 (self.managed_env([dict(entry, code_digest=other)]), "invitation_input_missing"),   # another code's entry
                 (self.managed_env([dict(entry, organization_id="00000000-0000-4000-8000-0000000000a2")]), "invitation_input_missing"),
                 (self.managed_env("[not json"), "invitation_input_malformed"),
                 (self.managed_env([dict(entry, role="admin")]), "provider_role_refused"),
                 (self.managed_env([entry], management="{}"), "provider_configuration_missing"),     # the key not yet written (A12)
                 (dict(self.managed_env([entry]), **{pm.ENV_URL: self.fake.url}), "provider_configuration_conflict"),
                 ({k: v for k, v in self.managed_env([entry]).items() if k != pm.ENV_CAPABILITY_SECRET}, "provider_configuration_incomplete")]
        for env, reason in cases:
            with self.subTest(reason=reason):
                p = self.managed_issue(digest, env)
                self.assertEqual((p.returncode, p.stdout), (1, ""))
                self.assertEqual(json.loads(p.stderr), {"refused": reason})
                self.assertNotIn(email, p.stderr)
        # The address in argv beside the digest is refused however it is combined.
        for extra in (["--provider-email", email], ["--provider-email", email, "--provider-role", "user", "--provider-issuer", ISSUER, "--provider-tenant", TENANT]):
            p = self.raw_cli("issue", "--organization", ORG, "--issued-by", "x", "--grant", "source_read:" + self.source, "--code-digest", digest,
                             "--provider-input-from-secret", *extra, env=self.managed_env([entry]))
            self.assertEqual((p.returncode, json.loads(p.stderr)), (1, {"refused": "invitation_input_with_flags_refused"}))
        self.assertEqual((self.dump(), self.fake.count("POST", "/manage/invites"), self.secret.value()), (before, creates, secret_before))

    def test_reconcile_and_revoke_run_with_the_injected_management_configuration(self):
        """The managed task's reconcile and revoke need only the injected secrets (no plain provider variable)."""
        out = self.invite("managedrevoke-%s@example.test" % uuid.uuid4().hex[:6])
        created = out["provider_invitation"]
        result = self.run_cli("reconcile", "--organization", ORG, env=self.managed_env())
        self.assertEqual((result["provider_phase"], result["capability_phase"], result["capability_store"]), ("completed", "completed", "secrets_manager"))
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"], env=self.managed_env())
        self.assertEqual((revoked["changed"], revoked["provider_phase"], revoked["capability_phase"], revoked["capabilities_destroyed"]),
                         (True, "completed", "completed", 1))
        self.assertEqual([r["state"] for r in revoked["provider_revocations"]], ["confirmed"])
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "revoked")
        self.assertFalse(self.capability(created["invitation_id"]).exists())
        # With the key not yet written, reconcile refuses and revoke still ends the capability locally.
        refused = self.run_cli("reconcile", "--organization", ORG, env=self.managed_env(management="{}"), expect=1)
        self.assertEqual(json.loads(refused.stderr), {"refused": "provider_configuration_missing"})
        other = self.invite("managedlocal-%s@example.test" % uuid.uuid4().hex[:6])
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", other["code_id"], env=self.managed_env(management="{}"))
        self.assertEqual((revoked["changed"], revoked["provider_phase"], revoked["capability_phase"], revoked["capabilities_destroyed"]),
                         (True, "not_configured", "completed", 1))

    def test_bind_provider_tenant_from_the_auth_configuration_secret_puts_no_identifier_on_argv(self):
        """B1 review m5: the pair comes from the AuthConfiguration secret the operator role reads."""
        self.ensure_backfill()
        saved = self.secret.value()
        document = {"format": runtime.AUTH_CONFIGURATION_FORMAT,
                    "auth_config": {"schema_version": 1, "auth_origin": "https://auth-synthetic.invalid", "app_origin": "https://beta.getforma.dev",
                                    "issuer": "secret-iss", "audience": "secret-aud", "max_access_lifetime_seconds": 900,
                                    "organizations": [{"organization_id": ORG, "tenant_slug": "legacy", "client_id": "legacy-platform"}]},
                    "identity_registry": {"schema_version": 1, "registry_version": str(uuid.uuid4()),
                                          "bindings": [{"issuer": "secret-iss", "provider_tenant_id": "ten_from_secret", "organization_id": ORG}]}}
        env = dict(local_secrets.environment(self.secret.endpoint), **{pm.ENV_AUTH_CONFIGURATION_SECRET: ARN})
        env.pop(pm.ENV_CAPABILITY_SECRET, None)
        rows = lambda: self.query("SELECT issuer,provider_tenant_id,active FROM genui.provider_tenant_bindings WHERE organization_id=%s AND issuer='secret-iss'", (ORG,))
        try:
            self.secret.set_value(json.dumps(document))
            p = self.raw_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", env=env)
            self.assertEqual((p.returncode, p.stderr), (0, ""), p.stderr[-300:])
            self.assertEqual(json.loads(p.stdout), {"organization_id": ORG, "binding": "created", "active_bindings": 2, "audit_event": None,
                                                    "source": "auth_configuration"})
            self.assertEqual(rows(), [("secret-iss", "ten_from_secret", True)])
            for identifier in ("secret-iss", "ten_from_secret", "legacy-platform"):
                self.assertNotIn(identifier, p.stdout)
            again = self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", env=env)
            self.assertEqual((again["binding"], again["source"]), ("existing", "auth_configuration"))
            # The ARN may come as an argument instead of the environment.
            self.assertEqual(self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", "--auth-configuration-secret-id", ARN,
                                          env={k: v for k, v in env.items() if k != pm.ENV_AUTH_CONFIGURATION_SECRET})["binding"], "existing")
            # Refusals before any write: an organization the secret does not route, no ARN, a malformed ARN.
            unrouted = self.run_cli("bind-provider-tenant", "--organization", "00000000-0000-4000-8000-0000000000a2", "--from-auth-configuration", env=env, expect=1)
            self.assertEqual(json.loads(unrouted.stderr), {"refused": "organization_not_routed"})
            missing = self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration",
                                   env={k: v for k, v in env.items() if k != pm.ENV_AUTH_CONFIGURATION_SECRET}, expect=1)
            self.assertEqual(json.loads(missing.stderr), {"refused": "auth_configuration_secret_missing"})
            malformed = self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", "--auth-configuration-secret-id", "not-an-arn",
                                     env=env, expect=1)
            self.assertEqual(json.loads(malformed.stderr), {"refused": "auth_configuration_secret_refused"})
            # The secret still `{}` (5.9 not yet written): nothing to bind; a value the supervisor refuses: malformed.
            self.secret.set_value("{}")
            unconfigured = self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", env=env, expect=1)
            self.assertEqual(json.loads(unconfigured.stderr), {"refused": "auth_configuration_unconfigured"})
            self.secret.set_value(json.dumps(dict(document, format="other")))
            bad = self.run_cli("bind-provider-tenant", "--organization", ORG, "--from-auth-configuration", env=env, expect=1)
            self.assertEqual(json.loads(bad.stderr), {"refused": "auth_configuration_malformed"})
            self.assertEqual(rows(), [("secret-iss", "ten_from_secret", True)])
        finally:
            self.secret.set_value(saved)
            with self.connect("postgres", autocommit=True) as c:
                c.execute("DELETE FROM genui.provider_tenant_bindings WHERE organization_id=%s AND issuer='secret-iss'", (ORG,))

    def test_issue_refuses_before_any_write_without_a_verified_provider_tenant(self):
        self.ensure_backfill()
        before, creates = self.dump(), self.fake.count("POST", "/manage/invites")
        wrong = tempfile.mkdtemp(prefix="genui-wrong-key-")
        wrong = private_file(Path(wrong, "key"), "mgmt_" + str(uuid.uuid4()))
        secret_before = self.secret.value()
        for env, reason in (({}, "provider_configuration_missing"),
                            (self.provider_env(**{pm.ENV_KEY_FILE: str(wrong)}), "provider_unauthorized"),
                            (self.provider_env(**{pm.ENV_CAPABILITY_SECRET: None}), "provider_configuration_incomplete"),
                            (self.provider_env(**{pm.ENV_CAPABILITY_SECRET: "genui-capabilities"}), "capability_secret_refused"),
                            (self.provider_env(**{pm.ENV_RETIRED_CAPABILITY_DIR: self.private}), "provider_capability_directory_retired"),
                            (self.provider_env(**{pm.ENV_URL: "http://auth.example.test"}), "provider_url_refused")):
            with self.subTest(reason=reason):
                refused = self.invite(expect=1, env=env)
                self.assertEqual(json.loads(refused.stderr), {"refused": reason})
        self.assertEqual((self.dump(), self.secret.value()), (before, secret_before))
        shutil.rmtree(wrong.parent, ignore_errors=True)
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
                self.assertEqual(entry["capability_stored"], False)  # the lost response's token is unrecoverable: reissue
                # Review m2: an invitation nobody holds a token for is revoked at once, as create_remote does.
                self.assertEqual(self.fake.status_of(row[3]), "revoked")
                self.assertEqual([r[1:] for r in self.rows(issued[name]["code_id"])], [("create", "confirmed", row[3]), ("revoke", "confirmed", row[3])])
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

    def test_revocation_with_the_secret_alone_ends_the_capability(self):
        """ADR-003 amendment 2026-10-30: the managed operator task has the secret's id and no provider
        settings; its revoke still removes the code's capability from the secret at once."""
        out = self.invite("secretonly-%s@example.test" % uuid.uuid4().hex[:6])
        created = out["provider_invitation"]
        other = self.invite("bystander-%s@example.test" % uuid.uuid4().hex[:6])
        env = {k: v for k, v in self.provider_env().items() if not k.startswith("GENUI_PROVIDER_") or k == pm.ENV_CAPABILITY_SECRET}
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"], env=env)
        self.assertEqual((revoked["provider_phase"], revoked["capability_phase"], revoked["capabilities_destroyed"]),
                         ("not_configured", "completed", 1))
        self.assertFalse(self.capability(created["invitation_id"]).exists())
        self.assertTrue(self.capability(other["provider_invitation"]["invitation_id"]).exists(), "another code's entry is untouched")
        self.assertEqual(self.fake.status_of(created["remote_invitation_id"]), "pending")  # the remote revocation is reconcile's
        # The same through the argument instead of the environment.
        third = self.invite("argument-%s@example.test" % uuid.uuid4().hex[:6])
        env = {k: v for k, v in env.items() if k != pm.ENV_CAPABILITY_SECRET}
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", third["code_id"], "--capability-secret-id", ARN, env=env)
        self.assertEqual((revoked["capability_phase"], revoked["capabilities_destroyed"]), ("completed", 1))
        self.assertFalse(self.capability(third["provider_invitation"]["invitation_id"]).exists())

    def test_an_unwritable_secret_stores_nothing_and_revokes_the_invitation(self):
        self.secret.deny_puts = True
        try:
            out = self.invite("unwritable-%s@example.test" % uuid.uuid4().hex[:6])
        finally:
            self.secret.deny_puts = False
        invitation = out["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["capability_stored"], invitation["detail"]), ("confirmed", False, "capability_unwritable"))
        self.assertEqual(invitation["revocation"]["state"], "confirmed")
        self.assertFalse(self.capability(invitation["invitation_id"]).exists())
        self.assertEqual(self.fake.status_of(invitation["remote_invitation_id"]), "revoked")
        self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "confirmed"), ("revoke", "confirmed")])

    def test_concurrent_operator_writes_never_lose_an_entry(self):
        """Every change is a read and a whole-map write; the operator's advisory lock serializes them."""
        self.ensure_backfill()
        self.secret.get_delay = 0.4
        boxes = [{} for _ in range(3)]
        try:
            runners = [threading.Thread(target=lambda box=box, i=i: box.update(p=self.raw_cli(
                "issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                "--provider-email", "race-%d-%s@example.test" % (i, uuid.uuid4().hex[:6]), "--provider-role", "user",
                "--provider-issuer", ISSUER, "--provider-tenant", TENANT))) for i, box in enumerate(boxes)]
            for runner in runners:
                runner.start()
            for runner in runners:
                runner.join()
        finally:
            self.secret.get_delay = 0
        for box in boxes:
            self.assertEqual(box["p"].returncode, 0, box["p"].stderr)
            invitation = json.loads(box["p"].stdout)["provider_invitation"]
            self.assertEqual((invitation["state"], invitation["capability_stored"]), ("confirmed", True))
            self.assertTrue(self.capability(invitation["invitation_id"]).exists())

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
        path.write_text(json.dumps(stored))
        events = len(self.operator_events())
        result = self.reconcile()
        self.assertEqual((result["capability_store"], result["capability_phase"]), ("secrets_manager", "completed"))
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

    # --- review fixes (2026-10-29): M1, m1-m7 ------------------------------------------

    def unknown_pair(self, email):
        """Review H2's sequence: code 1's create is refused with 500 before any write (unknown, never
        applied); the operator revokes code 1 and reissues; code 2's create is applied and its
        response lost (unknown). One remote invitation exists, code 2's."""
        self.fake.fault("create", "status", status=500)
        first = self.invite(email)
        self.assertEqual(first["provider_invitation"]["state"], "unknown")
        self.run_cli("revoke", "--organization", ORG, "--code-id", first["code_id"])
        self.fake.fault("create", "drop_after")
        second = self.invite(email)
        self.assertEqual(second["provider_invitation"]["state"], "unknown")
        remote = self.remote_ids(email)
        self.assertEqual(len(remote), 1)
        return first, second, remote[0]

    def test_reconcile_never_attributes_one_invitation_to_two_creates(self):
        """Review M1/H2: the invitation lies in both unresolved creates' windows, so neither is judged."""
        email = "h2-%s@example.test" % uuid.uuid4().hex[:6]
        first, second, remote = self.unknown_pair(email)
        self.age(first["code_id"], second["code_id"])
        result = self.reconcile()
        for out in (first, second):
            create = self.rows(out["code_id"])[0]
            self.assertEqual(create[1:], ("create", "unknown", None))
            self.assertEqual(self.entry(result, create[0])["detail"], "ambiguous_remote_match")
        self.assertEqual(self.fake.status_of(remote), "pending", "code 2's invitation is never revoked as code 1's")
        self.assertEqual(self.code_status(second["code_id"]), "active")
        self.assertEqual([r[1:3] for r in self.rows(first["code_id"])], [("create", "unknown"), ("revoke", "pending")])

    def test_reconcile_windows_are_bounded_above_by_the_create_deadline(self):
        """Review M1/H2 with separable windows: one-to-one attribution to the create whose window holds it."""
        email = "h2w-%s@example.test" % uuid.uuid4().hex[:6]
        first, second, remote = self.unknown_pair(email)
        self.age(first["code_id"], second["code_id"])
        with self.connect("postgres") as c:  # code 1's create ended an hour before code 2's (fixture SQL only)
            c.execute("""UPDATE genui.provider_invitations SET created_at=created_at-interval '1 hour',updated_at=updated_at-interval '1 hour'
                         WHERE organization_id=%s AND code_id=%s""", (ORG, first["code_id"]))
        result = self.reconcile()
        first_create, second_create = self.rows(first["code_id"])[0], self.rows(second["code_id"])[0]
        self.assertEqual((first_create[2:], self.entry(result, first_create[0])["detail"]), (("failed", None), "not_found"))
        self.assertEqual((second_create[2:], self.entry(result, second_create[0])["detail"]), (("confirmed", remote), "found"))
        # Review m2: nobody holds that invitation's token, so it is revoked; code 1 had nothing to revoke.
        self.assertEqual(self.fake.status_of(remote), "revoked")
        self.assertEqual([r[1:] for r in self.rows(second["code_id"])], [("create", "confirmed", remote), ("revoke", "confirmed", remote)])
        self.assertEqual([r[1:] for r in self.rows(first["code_id"])], [("create", "failed", None), ("revoke", "confirmed", None)])

    def test_reconcile_takes_the_remote_id_from_a_stored_capability(self):
        """Review M1/H2b: the crash between the capability file and the outcome UPDATE never costs the capability."""
        email = "h2b-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "status", status=500)
        first = self.invite(email)
        self.run_cli("revoke", "--organization", ORG, "--code-id", first["code_id"])
        second = self.invite(email)
        invitation = second["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["capability_stored"]), ("confirmed", True))
        with self.connect("postgres") as c:  # the crash between the file and the UPDATE (fixture SQL only)
            c.execute("UPDATE genui.provider_invitations SET state='pending',remote_invitation_id=NULL WHERE organization_id=%s AND invitation_id=%s",
                      (ORG, invitation["invitation_id"]))
        self.age(first["code_id"], second["code_id"])
        result = self.reconcile()
        entry = self.entry(result, invitation["invitation_id"])
        self.assertEqual((entry["state"], entry["remote_invitation_id"], entry["detail"], entry["capability_stored"]),
                         ("confirmed", invitation["remote_invitation_id"], "capability_record", True))
        self.reconcile()
        self.assertTrue(self.capability(invitation["invitation_id"]).exists())
        self.assertEqual(self.fake.status_of(invitation["remote_invitation_id"]), "pending")
        self.assertEqual([r[1:] for r in self.rows(second["code_id"])], [("create", "confirmed", invitation["remote_invitation_id"])])
        self.assertEqual([r[1:] for r in self.rows(first["code_id"])], [("create", "failed", None), ("revoke", "confirmed", None)])
        self.assertEqual(self.code_status(second["code_id"]), "active")

    def test_invitations_created_by_a_signed_in_user_are_never_attributed(self):
        """Review M1(c): management-key creates carry no inviter (gatewasm management.rs:1678,1744); console ones do."""
        email = "console-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "drop_after")
        out = self.invite(email)
        ours = self.remote_ids(email)
        self.fake.create_direct(email, invited_by="usr_" + uuid.uuid4().hex)
        self.age(out["code_id"])
        result = self.reconcile()
        create = self.rows(out["code_id"])[0]
        self.assertEqual((create[2:], self.entry(result, create[0])["detail"]), (("confirmed", ours[0]), "found"))

    def test_only_one_reconcile_runs_per_provider_tenant(self):
        """Review M1(b): a second reconcile, and invitation-bearing issuance, refuse while one runs."""
        self.ensure_backfill()
        before, creates = self.dump(), self.fake.count("POST", "/manage/invites")
        with self.connect("postgres") as holder:  # what a running reconcile holds
            holder.execute("SELECT 1 FROM genui.provider_tenant_bindings WHERE organization_id=%s FOR NO KEY UPDATE", (ORG,))
            refused = self.run_cli("reconcile", "--organization", ORG, expect=1)
            self.assertEqual(json.loads(refused.stderr), {"refused": "reconcile_in_progress"})
            refused = self.invite("busy-%s@example.test" % uuid.uuid4().hex[:6], expect=1)
            self.assertEqual(json.loads(refused.stderr), {"refused": "provider_tenant_busy"})
        self.assertEqual((self.dump(), self.fake.count("POST", "/manage/invites")), (before, creates))
        # Two real processes: the second refuses while the first inspects the provider.
        self.fake.fault("create", "drop_after")
        out = self.invite("race-%s@example.test" % uuid.uuid4().hex[:6])
        self.age(out["code_id"])
        lists = self.fake.count("GET", "/manage/invites")
        self.fake.fault("list", "delay_after", seconds=4)
        box = {}
        runner = threading.Thread(target=lambda: box.update(first=self.raw_cli("reconcile", "--organization", ORG)))
        runner.start()
        for _ in range(200):
            if self.fake.count("GET", "/manage/invites") > lists:
                break
            time.sleep(0.05)
        refused = self.run_cli("reconcile", "--organization", ORG, expect=1)
        runner.join()
        self.assertEqual(json.loads(refused.stderr), {"refused": "reconcile_in_progress"})
        self.assertEqual(box["first"].returncode, 0, box["first"].stderr)
        self.assertEqual(self.rows(out["code_id"])[0][2], "confirmed")

    def test_revocation_during_an_in_flight_create_leaves_no_capability(self):
        """Review m1/H1: the create lands after the code was revoked; its capability is never written and it is revoked."""
        self.ensure_backfill()
        email = "h1-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "delay_after", seconds=4)
        box = {}
        runner = threading.Thread(target=lambda: box.update(p=self.raw_cli(
            "issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
            "--provider-email", email, "--provider-role", "user", "--provider-issuer", ISSUER, "--provider-tenant", TENANT)))
        runner.start()
        for _ in range(200):
            if any(b["email"] == email for b in self.fake.create_bodies):
                break
            time.sleep(0.05)
        code_id, armed = self.query("""SELECT code_id::text,updated_at>clock_timestamp() FROM genui.provider_invitations
                                       WHERE organization_id=%s AND intended_email_digest=%s AND operation='create'""", (ORG, op.email_digest(email)))[0]
        self.assertTrue(armed, "the send deadline is recorded before the create is sent")
        revoked = self.run_cli("revoke", "--organization", ORG, "--code-id", code_id)
        self.assertEqual((revoked["capabilities_destroyed"], [r["detail"] for r in revoked["provider_revocations"]]),
                         (0, ["awaiting_create_reconciliation"]))
        runner.join()
        self.assertEqual(box["p"].returncode, 0, box["p"].stderr)
        issued = json.loads(box["p"].stdout)
        invitation = issued["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["capability_stored"], invitation["detail"]), ("confirmed", False, "code_no_longer_usable"))
        self.assertEqual(invitation["revocation"]["state"], "confirmed")
        self.assertFalse(self.capability(invitation["invitation_id"]).exists())
        self.assertEqual(self.fake.status_of(invitation["remote_invitation_id"]), "revoked")
        self.assertEqual([r[1:] for r in self.rows(code_id)], [("create", "confirmed", invitation["remote_invitation_id"]),
                                                               ("revoke", "confirmed", invitation["remote_invitation_id"])])
        self.assertEqual(self.code_status(code_id), "revoked")

    def test_an_unresolvable_create_closes_after_the_provider_expiry(self):
        """Review m3: past the invitation's 72 h provider lifetime nothing it made can still be pending."""
        email = "stuck-%s@example.test" % uuid.uuid4().hex[:6]
        self.fake.fault("create", "drop_after")
        out = self.invite(email)
        self.fake.create_direct(email)  # another management-key tool: indistinguishable, so ambiguous
        self.run_cli("revoke", "--organization", ORG, "--code-id", out["code_id"])
        self.age(out["code_id"])
        create = self.rows(out["code_id"])[0][0]
        self.assertEqual(self.entry(self.reconcile(), create)["detail"], "ambiguous_remote_match")
        self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "unknown"), ("revoke", "pending")])
        self.age(out["code_id"], seconds=72 * 3600)
        result = self.reconcile()
        entry = self.entry(result, create)
        self.assertEqual((entry["state_before"], entry["state"], entry["detail"]), ("unknown", "failed", "expired_unconfirmed"))
        revoke = self.rows(out["code_id"])[1][0]
        self.assertEqual((self.entry(result, revoke)["state"], self.entry(result, revoke)["detail"]), ("confirmed", "nothing_to_revoke"))
        self.assertEqual([r[1:] for r in self.rows(out["code_id"])], [("create", "failed", None), ("revoke", "confirmed", None)])
        self.assertEqual([o for o in self.reconcile()["operations"] if o["invitation_id"] in (create, revoke)], [])

    def test_the_code_is_printed_even_if_the_provider_phase_fails_after_commit(self):
        """Review m5: a database error in the revocation that follows a contract mismatch never hides the committed code."""
        self.ensure_backfill()

        def admin(body):
            body = json.loads(json.dumps(body))
            body["invite"]["role_name"] = "admin"
            return body
        self.fake.fault("create", "body_after", body=admin)
        email = "m5-%s@example.test" % uuid.uuid4().hex[:6]
        stdout, stderr = io.StringIO(), io.StringIO()
        env = dict(self.pg_env(), **self.provider_env())
        with mock.patch.dict(os.environ, env), mock.patch.object(op, "process_revoke", side_effect=RuntimeError("connection lost")), \
                contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = op.main(["issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source,
                              "--provider-email", email, "--provider-role", "user", "--provider-issuer", ISSUER, "--provider-tenant", TENANT])
        self.assertEqual((status, stderr.getvalue()), (0, ""))
        out = json.loads(stdout.getvalue())
        self.assertIsNotNone(op.normalize_code(out["code"]))
        invitation = out["provider_invitation"]
        self.assertEqual((invitation["state"], invitation["detail"], invitation["revocation"]["detail"]),
                         ("confirmed", "contract_mismatch", "provider_phase_incomplete"))
        self.assertEqual([r[1:3] for r in self.rows(out["code_id"])], [("create", "confirmed"), ("revoke", "pending")])
        self.reconcile()
        self.assertEqual(self.fake.status_of(invitation["remote_invitation_id"]), "revoked")

    def test_reconcile_records_invitations_the_provider_already_ended(self):
        """Review m7: drift upstream (another create, the sweeper, registration, the lifetime) is noticed."""
        outs = {name: self.invite("%s-%s@example.test" % (name, uuid.uuid4().hex[:6])) for name in ("revoked", "expired", "accepted", "lifetime", "live")}
        remote = {name: out["provider_invitation"]["remote_invitation_id"] for name, out in outs.items()}
        capability = {name: self.capability(out["provider_invitation"]["invitation_id"]) for name, out in outs.items()}
        self.fake.set_status(remote["revoked"], "revoked")
        self.fake.set_status(remote["expired"], "expired")
        token = json.loads(capability["accepted"].read_text())["invite_token"]
        self.assertIsNotNone(self.fake.register(token, self.fake.invites[remote["accepted"]]["email"]))
        self.age(outs["lifetime"]["code_id"], seconds=73 * 3600)
        result = self.reconcile()
        for name, detail in (("revoked", "remote_revoked"), ("expired", "remote_expired"), ("lifetime", "provider_lifetime_ended")):
            with self.subTest(name=name):
                rows = self.rows(outs[name]["code_id"])
                self.assertEqual([r[1:] for r in rows], [("create", "confirmed", remote[name]), ("revoke", "confirmed", remote[name])])
                self.assertEqual(self.entry(result, rows[1][0])["detail"], detail)
                self.assertFalse(capability[name].exists())
        # A registered invitee's token is spent: the capability ends, the ledger keeps the confirmed create.
        self.assertFalse(capability["accepted"].exists())
        self.assertEqual([r[1:3] for r in self.rows(outs["accepted"]["code_id"])], [("create", "confirmed")])
        self.assertTrue(capability["live"].exists())
        self.assertEqual([r[1:3] for r in self.rows(outs["live"]["code_id"])], [("create", "confirmed")])
        self.assertEqual(self.fake.status_of(remote["live"]), "pending")
        for name in outs:
            self.assertEqual(self.code_status(outs[name]["code_id"]), "active")
