"""APP-154 operator-owned, single-writer invitation broker.

The service receives only a broker token. The existing tenant management key
stays in this isolated operator process. Durable capabilities/ambiguous outcomes
live in a separate Secrets Manager secret, never on disk or in logs. This is a
long-running operator service, not an application-role or migration endpoint.
Run `python3 -I shared_invite_broker.py` behind a private HTTPS proxy; configuration
is a same-UID 0600 file named by GENUI_SHARED_BROKER_CONFIG_FILE. Deployment is
explicit; importing this module performs no network or AWS action.
"""
import datetime
import hashlib
import hmac
import http.server
import json
import os
from pathlib import Path
import re
import socket
import stat
import sys
import threading
import time

# -I prevents the adjacent operator.py from shadowing the standard library.
sys.path.append(str(Path(__file__).resolve().parents[1] / "runtime"))
import provider_management as provider

MAX_BODY = 4096
MAX_ENTRIES = 1000
# Secrets Manager SecretString is limited to 65,536 bytes. Leave headroom;
# new provider operations also reserve room for their bounded completion.
MAX_STATE = 60 * 1024
MAX_COMPLETION_BYTES = 256
MAX_RESERVED_WRITES = 40
WRITE_WINDOW_SECONDS = 24 * 60 * 60
TOKEN = re.compile(r"[A-Za-z0-9_-]{43}\Z")
KEY = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z")
SLUG = re.compile(r"[a-z0-9][a-z0-9-]{0,63}\Z")

class Refused(Exception):
    def __init__(self, status=503):
        self.status = status
        super().__init__("shared invitation unavailable")


def strict_json(raw):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise Refused(400)
            result[key] = value
        return result
    try:
        return json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: (_ for _ in ()).throw(Refused(400)))
    except (ValueError, TypeError, UnicodeError):
        raise Refused(400) from None


def private_config(path):
    if not path or not os.path.isabs(path):
        raise Refused()
    parent = os.stat(os.path.dirname(path), follow_symlinks=False)
    if not stat.S_ISDIR(parent.st_mode) or parent.st_uid != os.geteuid() or parent.st_mode & 0o077:
        raise Refused()
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid() or info.st_mode & 0o077 or info.st_nlink != 1 or info.st_size > MAX_BODY:
            raise Refused()
        return strict_json(os.read(fd, MAX_BODY + 1))
    finally:
        os.close(fd)


def validate_state(state):
    if (not isinstance(state, dict) or set(state) != {"version", "entries", "write_reservations"}
            or type(state["version"]) is not int or state["version"] != 1
            or not isinstance(state["entries"], dict) or len(state["entries"]) > MAX_ENTRIES
            or not isinstance(state["write_reservations"], list)
            or len(state["write_reservations"]) > MAX_RESERVED_WRITES
            or any(type(at) is not int or at < 0 for at in state["write_reservations"])):
        raise Refused()
    return state


def encoded_state(state, reserve_bytes=0):
    validate_state(state)
    try:
        encoded = json.dumps(state, sort_keys=True, separators=(",", ":"), allow_nan=False)
        if len(encoded.encode("utf-8")) + reserve_bytes > MAX_STATE:
            raise Refused()
        return encoded
    except (ValueError, TypeError, UnicodeError):
        raise Refused() from None


def reserve_writes(state, count, now):
    """Called under the broker lock, before provider contact. The next state
    write persists reservations for itself and every possible completion write.
    Unknown outcomes retain their reservations; restart cannot reset the budget.
    A backward clock retains future reservations conservatively.
    """
    validate_state(state)
    live = [at for at in state["write_reservations"] if at > now - WRITE_WINDOW_SECONDS]
    if len(live) + count > MAX_RESERVED_WRITES:
        raise Refused(429)
    state["write_reservations"] = live + [now] * count


class SecretState:
    """Dedicated secret, never the ordinary admission capability map.

    One configured broker instance is the sole writer. A pending record is
    persisted before the non-idempotent provider POST. Process death or an
    ambiguous POST cannot cause a repeated create (which would revoke another
    pending provider invitation). Recovery is explicit operator reconciliation.
    """
    def __init__(self, arn):
        self.secret = provider.ManagedSecret(arn)

    def read(self):
        text = self.secret.read_text()
        if len(text.encode("utf-8")) > MAX_STATE:
            raise Refused()
        return validate_state(strict_json(text))

    def write(self, state):
        encoded = encoded_state(state)
        try:
            # SDK retries carry the same version token and exact bytes.
            self.secret.client().put_secret_value(SecretId=self.secret.secret_id,
                ClientRequestToken=hashlib.sha256(encoded.encode("utf-8")).hexdigest(), SecretString=encoded)
        except Exception:
            raise Refused() from None


class Broker:
    def __init__(self, tenant_slug, tenant_id, token, client, state, clock=time.time):
        if not SLUG.fullmatch(tenant_slug) or not isinstance(tenant_id, str) or not 1 <= len(tenant_id) <= 256 or not tenant_id.isascii() or any(ord(c) <= 32 for c in tenant_id) or not TOKEN.fullmatch(token):
            raise Refused()
        self.slug, self.tenant_id, self.token = tenant_slug, tenant_id, token
        self.client, self.state, self.clock = client, state, clock
        self.lock = threading.Lock()
        self.created = []

    def authorized(self, authorization):
        return hmac.compare_digest(authorization or "", "Bearer " + self.token)

    def issue(self, body):
        if not isinstance(body, dict) or set(body) != {"tenant_slug", "email", "role", "delivery_mode", "idempotency_key"}:
            raise Refused(400)
        email, key = body["email"], body["idempotency_key"]
        if body["tenant_slug"] != self.slug or body["role"] != "user" or body["delivery_mode"] != "server_only":
            raise Refused(400)
        if not isinstance(email, str) or len(email) > 320 or not email.isascii() or any(ord(c) <= 32 or ord(c) == 127 for c in email) or email != email.lower() or email.count("@") != 1 or not isinstance(key, str) or not KEY.fullmatch(key):
            raise Refused(400)
        local, domain = email.split("@")
        if not local or len(local) > 64 or "." not in domain or domain.startswith(".") or domain.endswith("."):
            raise Refused(400)
        fingerprint = hashlib.sha256((self.tenant_id + "\0" + email).encode()).hexdigest()
        with self.lock:
            now = int(self.clock())
            state = self.state.read()
            entries = state["entries"]
            existing = entries.get(key)
            if existing is not None and existing.get("recipient") != fingerprint:
                raise Refused(409)
            if existing is not None and existing.get("state") == "issued" and existing.get("expires_at", 0) <= now:
                raise Refused(410)
            # Expired, confirmed provider capabilities are no longer usable.
            # Keep ambiguous pending operations for explicit reconciliation.
            for old_key, value in list(entries.items()):
                if value.get("state") == "issued" and value.get("expires_at", 0) <= now:
                    del entries[old_key]
            # Different shares for the same verified address reuse one pending
            # invitation; provider create otherwise revokes its predecessor.
            same = existing or next((v for v in entries.values() if v.get("recipient") == fingerprint and v.get("expires_at", now + 1) > now), None)
            if same is not None:
                if same.get("state") != "issued":
                    raise Refused(503)
                if same.get("expires_at", 0) <= now:
                    raise Refused(410)
                token = same.get("invite_token")
                if not isinstance(token, str) or not provider.TOKEN.fullmatch(token):
                    raise Refused()
                if existing is None:
                    if len(entries) >= MAX_ENTRIES:
                        raise Refused(429)
                    entries[key] = dict(same)
                    reserve_writes(state, 1, now)
                    self.state.write(state)
                return {"invite_token": token, "expires_at": min(now + 600, same["expires_at"])}
            self.created = [at for at in self.created if now - at < 3600]
            if len(self.created) >= 100 or sum(now - at < 60 for at in self.created) >= 5:
                raise Refused(429)
            if len(entries) >= MAX_ENTRIES:
                raise Refused(429)
            entry = {"recipient": fingerprint, "state": "pending", "created_at": now}
            entries[key] = entry
            reserve_writes(state, 2, now)
            # A bounded successful response must also fit. Refuse before even
            # the tenant probe when capacity cannot support pending+completion.
            encoded_state(state, MAX_COMPLETION_BYTES)
            # Pin actual provider tenant before any create. A mismatch cannot
            # issue an invite in a different tenant despite a valid broker token.
            self.client.require_tenant(self.tenant_id)
            self.state.write(state)
            self.created.append(now)
            outcome = self.client.create(email, "user")
            if outcome.state != "confirmed" or not outcome.token or not outcome.expires_at:
                # Keep pending durable on refusal/unknown; no automatic replay.
                raise Refused()
            expiry = int(outcome.expires_at.timestamp())
            if expiry <= now:
                raise Refused()
            entry.update(state="issued", invite_token=outcome.token, expires_at=expiry, remote_id=outcome.remote_id)
            self.state.write(state)
            return {"invite_token": outcome.token, "expires_at": min(now + 600, expiry)}


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "FormaInviteBroker"
    sys_version = ""

    def log_message(self, *_):
        pass  # No URL, email, bearer, provider response, or token in logs.

    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def answer(self, status, value):
        raw = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)
        self.close_connection = True

    def do_POST(self):
        try:
            if self.path != "/v1/shared-invitations" or len(self.headers.get_all("Authorization", [])) != 1 or not self.server.broker.authorized(self.headers.get("Authorization")):
                raise Refused(404)
            if len(self.headers.get_all("Content-Type", [])) != 1:
                raise Refused(400)
            if self.headers.get("Origin") or self.headers.get("Transfer-Encoding") or self.headers.get_content_type() != "application/json":
                raise Refused(400)
            lengths = self.headers.get_all("Content-Length", [])
            if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit():
                raise Refused(400)
            size = int(lengths[0])
            if size > MAX_BODY:
                raise Refused(413)
            raw = self.rfile.read(size)
            if len(raw) != size:
                raise Refused(400)
            result = self.server.broker.issue(strict_json(raw))
            self.answer(201, result)
        except Refused as failure:
            self.answer(failure.status, {"error": "invitation_unavailable"})
        except (Exception, socket.timeout):
            self.answer(503, {"error": "invitation_unavailable"})

    def do_GET(self):
        if self.path == "/health/live":
            self.answer(200, {"status": "ready"})
        else:
            self.answer(404, {"error": "invitation_unavailable"})


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    def __init__(self, address, broker):
        self.broker = broker
        self.slots = threading.BoundedSemaphore(16)
        super().__init__(address, Handler)
    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            self.shutdown_request(request)
            return
        try:
            super().process_request(request, client_address)
        except Exception:
            self.slots.release()
            raise
    def process_request_thread(self, request, client_address):
        try:
            super().process_request_thread(request, client_address)
        finally:
            self.slots.release()
    def handle_error(self, *_):
        pass


def broker_configuration(config):
    if not isinstance(config, dict) or set(config) != {"tenant_slug", "provider_tenant_id", "client_token", "state_secret_arn", "port"} or type(config["port"]) is not int or not 1024 <= config["port"] <= 65535:
        raise Refused()
    if not isinstance(config["tenant_slug"], str) or not SLUG.fullmatch(config["tenant_slug"]) or not isinstance(config["client_token"], str) or not TOKEN.fullmatch(config["client_token"]):
        raise Refused()
    if not isinstance(config["provider_tenant_id"], str) or not re.fullmatch(r"ten_[A-Za-z0-9_-]{1,250}", config["provider_tenant_id"]):
        raise Refused()
    if not isinstance(config["state_secret_arn"], str) or not provider.SECRET_ARN.fullmatch(config["state_secret_arn"]):
        raise Refused()
    return config


def main():
    # ECS injects only into this isolated task. Consume before provider/AWS work
    # and materialize privately; the ordinary app never receives these fields.
    injected = os.environ.pop("GENUI_SHARED_BROKER_CONFIG_JSON", None)
    directory = None
    try:
        if injected is not None:
            if "GENUI_SHARED_BROKER_CONFIG_FILE" in os.environ or len(injected.encode("utf-8")) > MAX_BODY:
                raise Refused()
            config = broker_configuration(strict_json(injected))
            directory = provider.runtime.SharedBrokerDirectory("/run/operator/credentials.json", json.dumps(config).encode())
            path = str(directory.config_path)
        else:
            path = os.environ.get("GENUI_SHARED_BROKER_CONFIG_FILE")
        config = broker_configuration(private_config(path))
        injected = None
        bind = os.environ.get("GENUI_SHARED_BROKER_BIND_ADDRESS", "127.0.0.1")
        if bind not in ("127.0.0.1", "0.0.0.0"):
            raise Refused()
        # Existing managed-key custody and its injected tenant binding remain
        # authoritative; no new management credential path is introduced.
        if provider.ENV_CAPABILITY_SECRET in os.environ:
            raise Refused()
        provider_config = provider.load_config(os.environ, required=True, capability_store_required=False)
        client = provider.Client(provider_config)
        client.require_tenant(config["provider_tenant_id"])
        broker = Broker(config["tenant_slug"], config["provider_tenant_id"], config["client_token"], client, SecretState(config["state_secret_arn"]))
        broker.state.read()  # Fail startup on wrong/missing durable state.
        with Server((bind, config["port"]), broker) as server:
            server.serve_forever(poll_interval=0.5)
    except Exception:
        print("shared invitation broker unavailable", file=sys.stderr)
        return 1
    finally:
        if directory is not None:
            directory.close()
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
