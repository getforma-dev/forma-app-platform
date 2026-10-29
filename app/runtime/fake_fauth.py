"""Loopback stand-in for fAuth's tenant management invitation API (APP-073 tests only).

SIMULATED TRANSPORT, never hosted identity evidence. Built from the merged
contract text (gatewasm handoff/forma-247/README.md at dfba787 and 1bf5d3b,
hash-bound under evidence/APP-006/upstream-auth-source/merged-contract-2026-10-29/)
and the public source at 1bf5d3b2fb515c8ed82b0c5a044f7846915e28d0:

- crates/server/src/middleware/management_auth.rs:21-50 — `Authorization: Bearer mgmt_…`,
  401 `unauthorized` / `invalid_api_key`;
- crates/server/src/error.rs:55-101 — every refusal is JSON `{error, message}`;
- crates/server/src/handlers/management.rs:1375-1391 `GET /manage/config`,
  :1550-1670 `GET /manage/invites` (limit 1..200, default 50, offset, optional
  status; expired pending rows are marked `expired` first; `created_at DESC`),
  :1672-1800 `POST /manage/invites` (`delivery_mode` email|server_only,
  server_only requires a role, built-in roles admin/user, 72 h default expiry
  clamped 1..168, earlier pending invitations for the same address are revoked,
  201 with `Cache-Control: no-store`, `invite_url` only outside server_only),
  :1802-1832 `POST /manage/invites/{id}/revoke` (200 `{success, invite_id}` or
  404 `invite_not_found` when no pending invitation has that id in the tenant);
- crates/server/src/handlers/auth.rs:36-38 — addresses are trimmed and lowercased.

Fault injection (per route, first in first out) reproduces the outcomes the
operator must classify: a refusal status with nothing processed, a drop or
status after the write committed, a response delayed past the client deadline,
and a 201 whose body breaks the contract. `register` stands in for fAuth's
hosted registration consuming the invitation (status `accepted`); it is the
simulation of a completed provider identity, not a provider flow.

Run as a script, it serves on 127.0.0.1, prints `{"port": N}` and answers one
JSON control line per stdin line until stdin closes (used by the Rust admission
campaign). The management key is read from a private file, never argv.
"""
import argparse
import datetime
import hashlib
import json
import socket
import sys
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

ROUTES = {"config", "list", "create", "revoke"}


def _now():
    return datetime.datetime.now(datetime.timezone.utc)


def _rfc3339(value):
    # chrono's to_rfc3339() emits nanoseconds and +00:00.
    return value.strftime("%Y-%m-%dT%H:%M:%S.") + "%06d000+00:00" % value.microsecond


class FakeFauth:
    def __init__(self, tenant_id, key, slug="genui-beta", signup_mode="invite_only"):
        self.tenant_id, self.key, self.slug, self.signup_mode = tenant_id, key, slug, signup_mode
        self.invites = {}
        self.order = []
        self.users = {}
        self.requests = []
        self.create_bodies = []
        self.faults = {route: [] for route in ROUTES}
        self.lock = threading.Lock()
        self.server = None

    # --- control ------------------------------------------------------------
    def start(self):
        fake = self

        class Handler(_Handler):
            owner = fake
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self.server.server_address[1]

    @property
    def url(self):
        return "http://127.0.0.1:%d" % self.server.server_address[1]

    def stop(self):
        if self.server is not None:
            self.server.shutdown()
            self.server.server_close()
            self.server = None

    def fault(self, route, mode, status=None, seconds=0.0, body=None, error=None):
        assert route in ROUTES and mode in ("status", "status_after", "drop_before", "drop_after", "delay_after", "body_after")
        error = error or ("server_error" if (status or 0) >= 500 else "invalid_request")
        with self.lock:
            self.faults[route].append({"mode": mode, "status": status, "seconds": seconds, "body": body, "error": error})

    def count(self, method, route_path):
        with self.lock:
            return sum(1 for m, p in self.requests if m == method and p == route_path)

    def create_direct(self, email, role="user", created_at=None):
        """An invitation created outside GenUI (another console or tool)."""
        with self.lock:
            return self._create(email, role, created_at)[0]

    def register(self, invite_token, email):
        """Hosted registration stand-in: a valid pending invitation for this address becomes accepted."""
        digest = hashlib.sha256(invite_token.encode()).hexdigest()
        with self.lock:
            self._expire()
            for invite in self.invites.values():
                if invite["token_hash"] == digest and invite["status"] == "pending" and invite["email"] == email.strip().lower():
                    user = "usr_" + uuid.uuid4().hex
                    self.users[user] = invite["email"]
                    invite.update(status="accepted", accepted_by_user_id=user, accepted_at=_now(), updated_at=_now())
                    return user
        return None

    def status_of(self, invite_id):
        with self.lock:
            invite = self.invites.get(invite_id)
            return invite["status"] if invite else None

    # --- contract behaviour (caller holds the lock) ----------------------------
    def _expire(self):
        now = _now()
        for invite in self.invites.values():
            if invite["status"] == "pending" and invite["expires_at"] < now:
                invite.update(status="expired", updated_at=now)

    def _create(self, email, role, created_at=None, hours=72):
        now = created_at or _now()
        for invite in self.invites.values():
            if invite["email"] == email and invite["status"] == "pending":
                invite.update(status="revoked", updated_at=now)
        invite_id, token = "tinv_" + str(uuid.uuid4()), "tinv_" + str(uuid.uuid4())
        self.invites[invite_id] = {"id": invite_id, "email": email, "role_id": "role_" + role, "role_name": role, "status": "pending",
                                   "token_hash": hashlib.sha256(token.encode()).hexdigest(), "invited_by_user_id": None,
                                   "accepted_by_user_id": None, "expires_at": now + datetime.timedelta(hours=hours), "accepted_at": None,
                                   "created_at": now, "updated_at": now}
        self.order.append(invite_id)
        return invite_id, token

    def public(self, invite, keys=("id", "email", "role_id", "role_name", "status", "invited_by_user_id", "accepted_by_user_id",
                                   "expires_at", "accepted_at", "created_at", "updated_at")):
        return {k: (_rfc3339(invite[k]) if isinstance(invite[k], datetime.datetime) else invite[k]) for k in keys}


class _Handler(BaseHTTPRequestHandler):
    owner = None
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def _send(self, status, body, extra=()):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        for name, value in extra:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def _error(self, status, error, message):
        self._send(status, {"error": error, "message": message})

    def _drop(self):
        try:
            self.connection.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.close_connection = True

    def _route(self, method, path):
        if method == "GET" and path == "/manage/config":
            return "config", None
        if path == "/manage/invites":
            return ("list" if method == "GET" else "create" if method == "POST" else None), None
        parts = path.split("/")
        if method == "POST" and len(parts) == 5 and parts[:3] == ["", "manage", "invites"] and parts[4] == "revoke":
            return "revoke", parts[3]
        return None, None

    def _handle(self, method):
        fake = self.owner
        target = urlsplit(self.path)
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        route, invite_id = self._route(method, target.path)
        with fake.lock:
            fake.requests.append((method, target.path))
            fault = fake.faults[route].pop(0) if route and fake.faults[route] else None
        auth = self.headers.get("Authorization") or ""
        if not auth.startswith("Bearer "):
            return self._error(401, "unauthorized", "Missing or invalid Authorization header")
        if not auth[7:].startswith("mgmt_") or auth[7:] != fake.key:
            return self._error(401, "invalid_api_key", "Invalid API key")
        if route is None:
            return self._error(404, "not_found", "Not found")
        if fault and fault["mode"] == "drop_before":
            return self._drop()
        if fault and fault["mode"] == "status":
            return self._error(fault["status"], fault["error"], "injected")
        status, body, extra = getattr(self, "_" + route)(fake, target, raw, invite_id)
        if fault and fault["mode"] == "drop_after":
            return self._drop()
        if fault and fault["mode"] == "delay_after":
            time.sleep(fault["seconds"])
        if fault and fault["mode"] == "status_after":
            return self._error(fault["status"], "bad_gateway", "injected after the write")
        if fault and fault["mode"] == "body_after" and status == 201:
            body = fault["body"](body)
        try:
            self._send(status, body, extra)
        except OSError:
            pass

    def do_GET(self):
        self._handle("GET")

    def do_POST(self):
        self._handle("POST")

    # --- routes (each returns status, body, extra headers) ----------------------
    def _config(self, fake, target, raw, invite_id):
        return 200, {"tenant_id": fake.tenant_id, "slug": fake.slug, "name": "Fake beta", "allowed_providers": ["email"],
                     "google_configured": False, "github_configured": False, "allowed_origins": [], "status": "active",
                     "signup_mode": fake.signup_mode, "security_config": {}}, ()

    def _list(self, fake, target, raw, invite_id):
        query = parse_qs(target.query)

        def number(name, default):
            try:
                return int(query[name][0])
            except (KeyError, ValueError):
                return default
        limit, offset = min(max(number("limit", 50), 1), 200), max(number("offset", 0), 0)
        status = (query.get("status") or [""])[0].strip().lower() or None
        with fake.lock:
            fake._expire()
            rows = sorted((i for i in fake.invites.values() if status is None or i["status"] == status),
                          key=lambda i: i["created_at"], reverse=True)
            page = [fake.public(i) for i in rows[offset:offset + limit]]
            # role_name falls back to "unknown" in the real join (management.rs:1647).
        return 200, {"invites": page, "total": len(rows), "limit": limit, "offset": offset}, ()

    def _create(self, fake, target, raw, invite_id):
        try:
            body = json.loads(raw)
            assert isinstance(body, dict) and isinstance(body.get("email"), str)
        except (ValueError, AssertionError):
            return 400, {"error": "invalid_request", "message": "Invalid request body. Required: email"}, ()
        with fake.lock:
            fake.create_bodies.append(body)
        mode = body.get("delivery_mode") or "email"
        if mode not in ("email", "server_only"):
            return 400, {"error": "invalid_delivery_mode", "message": "delivery_mode must be email or server_only"}, ()
        role = body.get("role")
        if mode == "server_only" and (not isinstance(role, str) or not role.strip()):
            return 400, {"error": "role_required", "message": "server_only invitations require an explicit role"}, ()
        email = body["email"].strip().lower()
        local, _, domain = email.partition("@")
        if not local or not domain or "." not in domain or domain.startswith(".") or domain.endswith("."):
            return 400, {"error": "invalid_email", "message": "A valid email is required"}, ()
        role = (role or "admin").strip().lower()
        if role not in ("admin", "user"):
            return 400, {"error": "invalid_role", "message": "Role not found"}, ()
        hours = body.get("expires_in_hours") if isinstance(body.get("expires_in_hours"), int) else 72
        with fake.lock:
            new_id, token = fake._create(email, role, hours=min(max(hours, 1), 168))
            invite = fake.public(fake.invites[new_id], ("id", "email", "role_id", "role_name", "status", "expires_at"))
        response = {"invite": invite, "invite_token": token, "email_delivery": "suppressed" if mode == "server_only" else "not_configured"}
        if mode != "server_only":
            response["invite_url"] = "http://127.0.0.1/t/%s/login?invite_token=%s" % (fake.slug, token)
        return 201, response, (("Cache-Control", "no-store"),)

    def _revoke(self, fake, target, raw, invite_id):
        with fake.lock:
            invite = fake.invites.get(invite_id)
            if invite is None or invite["status"] != "pending":
                return 404, {"error": "invite_not_found", "message": "Pending invite not found"}, ()
            invite.update(status="revoked", updated_at=_now())
        return 200, {"success": True, "invite_id": invite_id}, ()


def _serve(argv=None):
    p = argparse.ArgumentParser(description="Loopback fAuth management stand-in (tests only).")
    p.add_argument("--tenant-id", required=True)
    p.add_argument("--key-file", required=True)
    args = p.parse_args(argv)
    with open(args.key_file, encoding="ascii") as f:
        key = f.read().strip()
    fake = FakeFauth(args.tenant_id, key)
    port = fake.start()
    print(json.dumps({"port": port}), flush=True)
    for line in sys.stdin:
        command = json.loads(line)
        op = command.get("op")
        if op == "register":
            user = fake.register(command["invite_token"], command["email"])
            reply = {"user_id": user} if user else {"error": "invitation_unusable"}
        elif op == "fault":
            fake.fault(command["route"], command["mode"], status=command.get("status"), seconds=command.get("seconds", 0.0),
                       error=command.get("error"))
            reply = {"ok": True}
        elif op == "status":
            reply = {"status": fake.status_of(command["id"])}
        elif op == "count":
            reply = {"count": fake.count(command["method"], command["path"])}
        else:
            reply = {"error": "unknown_op"}
        print(json.dumps(reply), flush=True)
    fake.stop()


if __name__ == "__main__":
    _serve()
