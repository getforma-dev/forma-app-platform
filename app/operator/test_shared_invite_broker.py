"""Real loopback broker/provider HTTP, fake provider contract, no AWS calls."""
import copy
import http.client
import importlib.util
import json
from pathlib import Path
import sys
import threading
import time
import unittest
from unittest import mock
import uuid

HERE = Path(__file__).resolve().parent
sys.path.append(str(HERE.parent / "runtime"))
spec = importlib.util.spec_from_file_location("shared_invite_broker", HERE / "shared_invite_broker.py")
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
import fake_fauth

class MemoryState:
    def __init__(self):
        self.value = {"version": 1, "entries": {}, "write_reservations": []}
        self.writes = 0
        self.fail_write = None
    def read(self):
        return copy.deepcopy(self.value)
    def write(self, state):
        self.writes += 1
        if self.writes == self.fail_write:
            raise b.Refused()
        self.value = copy.deepcopy(state)

class BrokerTests(unittest.TestCase):
    def setUp(self):
        self.tenant = "ten_shared_test"
        self.fake = fake_fauth.FakeFauth(self.tenant, "mgmt_" + str(uuid.uuid4()), slug="northstar")
        self.fake.start()
        from urllib.parse import urlsplit
        url = urlsplit(self.fake.url)
        self.client = b.provider.Client(b.provider.Config("http", url.hostname, url.port, self.fake.key, None))
        self.state = MemoryState()
        self.token = "A" * 43
        self.broker = b.Broker("northstar", self.tenant, self.token, self.client, self.state)
        self.body = {"tenant_slug": "northstar", "email": "guest@example.test", "role": "user", "delivery_mode": "server_only", "idempotency_key": str(uuid.uuid4())}
        self.server = b.Server(("127.0.0.1", 0), self.broker)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.fake.stop()
    def request(self, body=None, token=None, extra=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.server.server_address[1], timeout=5)
        headers = {"Authorization": "Bearer " + (token or self.token), "Content-Type": "application/json"}
        headers.update(extra or {})
        connection.request("POST", "/v1/shared-invitations", json.dumps(body or self.body), headers)
        response = connection.getresponse()
        data = json.loads(response.read())
        connection.close()
        return response.status, data
    def creates(self):
        return self.fake.count("POST", "/manage/invites")
    def test_real_http_exact_contract_and_retry_across_broker_restart(self):
        status, first = self.request()
        self.assertEqual(status, 201)
        self.assertEqual(set(first), {"invite_token", "expires_at"})
        self.assertGreater(first["expires_at"], time.time())
        self.assertLessEqual(first["expires_at"], time.time() + 600)
        self.assertEqual(self.fake.create_bodies, [{"email": "guest@example.test", "role": "user", "delivery_mode": "server_only"}])
        self.server.broker = b.Broker("northstar", self.tenant, self.token, self.client, self.state)
        self.assertEqual(self.request()[1]["invite_token"], first["invite_token"])
        self.assertEqual(self.creates(), 1)
        self.assertEqual(len(self.state.value["write_reservations"]), 2)
        self.assertEqual(self.state.writes, 2, "same-key replay performs no state write")
        self.assertTrue(self.fake.register(first["invite_token"], self.body["email"]).startswith("usr_"))
    def test_different_share_same_email_reuses_and_binds_idempotency_key(self):
        first = self.request()[1]
        second = dict(self.body, idempotency_key=str(uuid.uuid4()))
        self.assertEqual(self.request(second)[1]["invite_token"], first["invite_token"])
        second["email"] = "someoneelse@example.test"
        self.assertEqual(self.request(second)[0], 409)
        self.assertEqual(self.creates(), 1)
    def test_authority_tenant_role_and_closed_body_refusals_never_create(self):
        self.assertEqual(self.request(token="B" * 43)[0], 404)
        for change in ({"role": "admin"}, {"tenant_slug": "elsewhere"}, {"delivery_mode": "email"}, {"email": "not-an-email"}, {"url": "https://evil.invalid"}):
            self.assertEqual(self.request(dict(self.body, **change))[0], 400)
        self.assertEqual(self.request(extra={"Origin": "https://other.invalid"})[0], 400)
        self.assertEqual(self.creates(), 0)
        self.server.broker.tenant_id = "ten_wrong"
        self.assertEqual(self.request()[0], 503)
        self.assertEqual(self.creates(), 0)
    def test_pending_before_send_and_unknown_outcome_never_replays(self):
        self.state.fail_write = 1
        self.assertEqual(self.request()[0], 503)
        self.assertEqual(self.creates(), 0)
        self.state.fail_write = None
        original = self.client.create
        def lost(email, role):
            original(email, role)
            return b.provider.CreateOutcome("unknown", "transport_ambiguous")
        self.client.create = lost
        self.assertEqual(self.request()[0], 503)
        self.server.broker = b.Broker("northstar", self.tenant, self.token, self.client, self.state)
        self.assertEqual(self.request()[0], 503)
        self.assertEqual(self.creates(), 1)
        self.assertNotIn("invite_token", next(iter(self.state.value["entries"].values())))
        self.assertEqual(len(self.state.value["write_reservations"]), 2)
    def test_lost_secret_write_does_not_issue_a_second_provider_invite(self):
        self.state.fail_write = 2
        self.assertEqual(self.request()[0], 503)
        self.state.fail_write = None
        self.assertEqual(self.request()[0], 503)
        self.assertEqual(self.creates(), 1)
    def test_concurrent_retries_issue_once(self):
        result = []
        threads = [threading.Thread(target=lambda: result.append(self.request())) for _ in range(4)]
        for thread in threads: thread.start()
        for thread in threads: thread.join()
        self.assertEqual([status for status, _ in result], [201] * 4)
        self.assertEqual(len({row["invite_token"] for _, row in result}), 1)
        self.assertEqual(self.creates(), 1)
    def test_duplicate_json_and_nonfinite_inputs_refused(self):
        for raw in ('{"email":"a","email":"b"}', '{"x":NaN}', '[] trailing'):
            with self.assertRaises(b.Refused): b.strict_json(raw)
    def test_expired_replay_and_capacity_fail_closed(self):
        self.assertEqual(self.request()[0], 201)
        self.state.value["entries"][self.body["idempotency_key"]]["expires_at"] = 1
        self.assertEqual(self.request()[0], 410)
        self.assertEqual(self.creates(), 1)

    def test_expired_confirmed_entries_are_reclaimed_but_unknown_outcomes_are_retained(self):
        self.state.value["entries"] = {str(uuid.uuid4()): {"recipient": "old", "state": "issued", "expires_at": 1} for _ in range(b.MAX_ENTRIES - 1)}
        unknown = str(uuid.uuid4())
        self.state.value["entries"][unknown] = {"recipient": "unknown", "state": "pending", "created_at": 1}
        self.assertEqual(self.request()[0], 201)
        self.assertIn(unknown, self.state.value["entries"])
        self.assertEqual(len(self.state.value["entries"]), 2)
    def test_duplicate_auth_and_content_type_headers_are_rejected(self):
        for header in ("Authorization", "Content-Type"):
            connection = http.client.HTTPConnection("127.0.0.1", self.server.server_address[1], timeout=5)
            data = json.dumps(self.body)
            connection.putrequest("POST", "/v1/shared-invitations")
            connection.putheader("Authorization", "Bearer " + self.token)
            connection.putheader("Content-Type", "application/json")
            connection.putheader(header, "Bearer " + self.token if header == "Authorization" else "application/json")
            connection.putheader("Content-Length", str(len(data)))
            connection.endheaders(data.encode())
            response = connection.getresponse()
            self.assertIn(response.status, (400, 404))
            response.read(); connection.close()
        self.assertEqual(self.creates(), 0)

    def test_new_create_reserves_both_writes_before_provider_contact(self):
        now = int(time.time())
        self.state.value["write_reservations"] = [now] * 39
        self.assertEqual(self.request()[0], 429)
        self.assertEqual(self.fake.count("GET", "/manage/config"), 0)
        self.assertEqual(self.creates(), 0)
        self.assertEqual(self.state.writes, 0)
        self.state.value["write_reservations"] = [now] * 38
        original = self.client.create
        def checked_create(email, role):
            self.assertEqual(len(self.state.value["write_reservations"]), 40)
            self.assertEqual(self.state.value["entries"][self.body["idempotency_key"]]["state"], "pending")
            return original(email, role)
        self.client.create = checked_create
        self.assertEqual(self.request()[0], 201)
        self.assertEqual(self.state.writes, 2)
        self.assertEqual(len(self.state.value["write_reservations"]), 40)
        self.server.broker = b.Broker("northstar", self.tenant, self.token, self.client, self.state)
        self.assertEqual(self.request(dict(self.body, email="second@example.test", idempotency_key=str(uuid.uuid4())))[0], 429)
        self.assertEqual(self.fake.count("GET", "/manage/config"), 1)
        self.assertEqual(self.creates(), 1)

    def test_alias_budget_survives_restart_and_prunes_only_after_24_hours(self):
        now = int(time.time())
        self.server.broker.clock = lambda: now
        first = self.request()[1]
        for _ in range(38):
            self.assertEqual(self.request(dict(self.body, idempotency_key=str(uuid.uuid4())))[0], 201)
        self.assertEqual(self.state.writes, 40)
        self.assertEqual(len(self.state.value["write_reservations"]), 40)
        alias = dict(self.body, idempotency_key=str(uuid.uuid4()))
        self.server.broker = b.Broker("northstar", self.tenant, self.token, self.client, self.state, clock=lambda: now - 1)
        self.assertEqual(self.request(alias)[0], 429, "clock rollback must not discard reservations")
        self.assertEqual(self.request()[1]["invite_token"], first["invite_token"])
        self.assertEqual(self.state.writes, 40, "existing replay at the cap is read-only")
        self.server.broker.clock = lambda: now + b.WRITE_WINDOW_SECONDS - 1
        self.assertEqual(self.request(alias)[0], 429)
        self.server.broker.clock = lambda: now + b.WRITE_WINDOW_SECONDS
        self.assertEqual(self.request(alias)[0], 201)
        self.assertEqual(self.state.writes, 41)
        self.assertEqual(self.state.value["write_reservations"], [now + b.WRITE_WINDOW_SECONDS])
        self.assertEqual(self.creates(), 1)


class SecretApiStub:
    """The actual SecretState serializer/SDK boundary, without any AWS client."""
    secret_id = "synthetic-dedicated-state"
    def __init__(self):
        self.text = json.dumps({"version": 1, "entries": {}, "write_reservations": []})
        self.calls = []
    def read_text(self):
        return self.text
    def client(self):
        return self
    def put_secret_value(self, **kwargs):
        self.calls.append(kwargs)
        self.text = kwargs["SecretString"]


class SecretStateTests(unittest.TestCase):
    def setUp(self):
        self.api = SecretApiStub()
        with mock.patch.object(b.provider, "ManagedSecret", return_value=self.api):
            self.state = b.SecretState("synthetic-dedicated-state")

    def test_read_enforces_actual_utf8_byte_bound_and_closed_budget_shape(self):
        base = self.api.text
        self.api.text = base + " " * (b.MAX_STATE - len(base.encode("utf-8")))
        self.assertEqual(self.state.read()["write_reservations"], [])
        self.api.text += " "
        with self.assertRaises(b.Refused): self.state.read()
        # Non-ASCII is measured in bytes, not Python character count.
        self.api.text = json.dumps({"version": 1, "entries": {"padding": "é" * (b.MAX_STATE // 2)}, "write_reservations": []}, ensure_ascii=False)
        self.assertLess(len(self.api.text), b.MAX_STATE)
        with self.assertRaises(b.Refused): self.state.read()
        for reservations in ([True], [-1], [1.5], [0] * 41, "none"):
            self.api.text = json.dumps({"version": 1, "entries": {}, "write_reservations": reservations})
            with self.assertRaises(b.Refused): self.state.read()
        self.api.text = json.dumps({"version": 1, "entries": {}})
        with self.assertRaises(b.Refused): self.state.read()
        self.assertEqual(self.api.calls, [])

    def test_write_checks_bytes_before_sdk_and_retries_have_same_version_token(self):
        state = {"version": 1, "entries": {"padding": ""}, "write_reservations": []}
        base = len(b.encoded_state(state).encode("utf-8"))
        state["entries"]["padding"] = "x" * (b.MAX_STATE - base)
        self.state.write(state)
        self.assertEqual(len(self.api.calls[-1]["SecretString"].encode("utf-8")), b.MAX_STATE)
        self.state.write(dict(reversed(list(state.items()))))
        self.assertEqual(self.api.calls[-1], self.api.calls[-2])
        self.assertEqual(self.api.calls[-1]["ClientRequestToken"], b.hashlib.sha256(self.api.calls[-1]["SecretString"].encode("utf-8")).hexdigest())
        self.assertEqual(len(self.api.calls[-1]["ClientRequestToken"]), 64)
        state["entries"]["padding"] += "x"
        with self.assertRaises(b.Refused): self.state.write(state)
        self.assertEqual(len(self.api.calls), 2)

    def test_sdk_pending_state_contains_final_write_reservation_before_provider_post(self):
        key = str(uuid.uuid4())
        body = {"tenant_slug": "northstar", "email": "guest@example.test", "role": "user", "delivery_mode": "server_only", "idempotency_key": key}
        outer = self
        class Provider:
            calls = 0
            def require_tenant(self, tenant):
                outer.assertEqual(tenant, "ten_test")
            def create(self, email, role):
                self.calls += 1
                persisted = json.loads(outer.api.text)
                outer.assertEqual(persisted["write_reservations"], [1000, 1000])
                outer.assertEqual(persisted["entries"][key]["state"], "pending")
                outer.assertEqual(len(outer.api.calls), 1)
                return b.provider.CreateOutcome("confirmed", "created", 201,
                    "tinv_" + str(uuid.uuid4()), "tinv_" + str(uuid.uuid4()),
                    b.datetime.datetime.fromtimestamp(2000, b.datetime.timezone.utc))
        client = Provider()
        broker = b.Broker("northstar", "ten_test", "A" * 43, client, self.state, clock=lambda: 1000)
        first = broker.issue(body)
        self.assertEqual(len(self.api.calls), 2)
        self.assertEqual(self.state.read()["write_reservations"], [1000, 1000])
        broker = b.Broker("northstar", "ten_test", "A" * 43, client, self.state, clock=lambda: 1001)
        self.assertEqual(broker.issue(body)["invite_token"], first["invite_token"])
        self.assertEqual(client.calls, 1)
        self.assertEqual(len(self.api.calls), 2)
        broker.issue(dict(body, idempotency_key=str(uuid.uuid4())))
        self.assertEqual(self.state.read()["write_reservations"], [1000, 1000, 1001])
        self.assertEqual(len(self.api.calls), 3)
        self.assertEqual(client.calls, 1)

    def test_completion_capacity_refuses_before_any_provider_call(self):
        state = {"version": 1, "entries": {"padding": {"state": "pending", "recipient": "x"}}, "write_reservations": []}
        state["entries"]["padding"]["recipient"] = "x" * (b.MAX_STATE - len(b.encoded_state(state).encode("utf-8")) - 300)
        self.api.text = b.encoded_state(state)
        client = mock.Mock()
        broker = b.Broker("northstar", "ten_test", "A" * 43, client, self.state, clock=lambda: 1000)
        body = {"tenant_slug": "northstar", "email": "guest@example.test", "role": "user", "delivery_mode": "server_only", "idempotency_key": str(uuid.uuid4())}
        pending = copy.deepcopy(state)
        pending["entries"][body["idempotency_key"]] = {"recipient": b.hashlib.sha256(b"ten_test\0guest@example.test").hexdigest(), "state": "pending", "created_at": 1000}
        pending["write_reservations"] = [1000, 1000]
        self.assertLessEqual(len(b.encoded_state(pending).encode("utf-8")), b.MAX_STATE)
        with self.assertRaises(b.Refused): b.encoded_state(pending, b.MAX_COMPLETION_BYTES)
        with self.assertRaises(b.Refused): broker.issue(body)
        client.require_tenant.assert_not_called()
        client.create.assert_not_called()
        self.assertEqual(self.api.calls, [])

if __name__ == "__main__":
    unittest.main()
