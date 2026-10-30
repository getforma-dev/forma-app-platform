"""fAuth management client and private invitation-capability custody for the operator channel.

APP-073 acceptance 6 and 7; ADR-003 §Proposed confidential provider handoff;
ADR-007 D7.19. Used only by admission_operator.py, never by an HTTP, application
or signup path, and not packaged in the runtime image.

Consumed contract (merged, hash-bound under evidence/APP-006/upstream-auth-source/
merged-contract-2026-10-29/; source read at gatewasm 1bf5d3b2fb515c8ed82b0c5a044f7846915e28d0,
crates/server/src/handlers/management.rs blob bc12859a):
- every call carries `Authorization: Bearer mgmt_…`, the tenant-scoped management
  key (middleware/management_auth.rs:21-50); refusals are JSON `{error, message}`
  (error.rs:55-101);
- `GET /manage/config` → 200 `{tenant_id, slug, status, signup_mode, …}` (:1375-1391);
- `POST /manage/invites` `{email, role, delivery_mode: "server_only"}` → 201
  `{invite: {id, email, role_id, role_name, status, expires_at}, invite_token,
  email_delivery: "suppressed"}` with no `invite_url` (:1672-1800); 400
  `invalid_request`/`invalid_delivery_mode`/`role_required`/`invalid_email`/
  `invalid_role` before any write; not idempotent, and it revokes earlier pending
  invitations for the same address (:1722-1733);
- `GET /manage/invites?limit=&offset=` → 200 `{invites: [...], total, limit,
  offset}`, `created_at DESC`, limit 1..200 (:1550-1670); there is no lookup by
  id or address, so inspection pages through the list. Each item carries
  `invited_by_user_id` (:1603,1653): null for a management-key create (the
  management middleware inserts no user claims, middleware/management_auth.rs:59;
  create binds the absent claims at :1678,1744), a user id for a console or
  admin-UI create;
- `POST /manage/invites/{id}/revoke` → 200 `{success: true, invite_id}`, or 404
  `invite_not_found` when the tenant has no *pending* invitation with that id
  (:1802-1832). Repeating a revoke is therefore safe; repeating a create is not.

The management key is read from the environment or a private file, never argv.
Nothing here prints or logs the key, an `invite_token` or an address; callers
receive fixed reason codes. A token is written only to a 0600 capability file in
a 0700 directory owned by the operator (write_capability).
"""
import datetime
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import socket
import ssl
import stat
import tempfile
import threading
import time
from urllib.parse import urlsplit

# Bounds of the provider phase, recorded by `admission_operator.py defaults`.
PROVIDER_DEFAULTS = {
    "request_deadline_seconds": 10,
    "connect_timeout_seconds": 5,
    "response_bytes_max": 1048576,
    "list_page_size": 200,
    "list_pages_max": 50,
    "reconcile_grace_seconds": 120,
    "reconcile_clock_skew_seconds": 300,
    "reconcile_operations_max": 100,
    "provider_invitation_hours_default": 72,
    "create_send_margin_seconds": 5,
}
ENV_URL = "GENUI_PROVIDER_URL"
ENV_KEY = "GENUI_PROVIDER_MANAGEMENT_KEY"
ENV_KEY_FILE = "GENUI_PROVIDER_MANAGEMENT_KEY_FILE"
ENV_CAPABILITY_DIR = "GENUI_PROVIDER_CAPABILITY_DIR"
ENV_NAMES = (ENV_URL, ENV_KEY, ENV_KEY_FILE, ENV_CAPABILITY_DIR)
LOOPBACK = ("127.0.0.1", "localhost", "::1")
KEY = re.compile(r"mgmt_[A-Za-z0-9_-]{8,256}")
REMOTE_ID = re.compile(r"tinv_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
TOKEN = REMOTE_ID  # same shape as the id (management.rs:1718-1719); never interchangeable
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
ERROR_CODE = re.compile(r"[a-z][a-z0-9_]{0,63}")
CAPABILITY_FORMAT = "forma:fauth-invite-capability:v1"
TIMESTAMP = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})")


class Refused(ValueError):
    """A refusal whose message is a fixed reason code, never a value."""


def require(condition, reason="refused"):
    if not condition:
        raise Refused(reason)


def parse_time(text):
    """RFC 3339 as chrono's to_rfc3339 writes it (nanoseconds allowed), to an aware datetime."""
    match = TIMESTAMP.fullmatch(text) if isinstance(text, str) else None
    require(match is not None, "provider_contract_mismatch")
    zone = "+00:00" if match.group(3) == "Z" else match.group(3)
    fraction = (match.group(2) or "0")[:6].ljust(6, "0")
    return datetime.datetime.fromisoformat("%s.%s%s" % (match.group(1), fraction, zone))


def normalize_email(email):
    return email.strip().lower()


# --- configuration ----------------------------------------------------------------

class Config:
    """Validated provider configuration. The key never appears in repr or str."""

    def __init__(self, scheme, host, port, key, capability_dir):
        self.scheme, self.host, self.port, self.capability_dir = scheme, host, port, capability_dir
        self._key = key

    def authorization(self):
        return "Bearer " + self._key

    def __repr__(self):
        return "Config(%s://%s:%d, capability_dir=%s)" % (self.scheme, self.host, self.port, self.capability_dir)

    __str__ = __repr__


def _private_file(path):
    """An absolute regular file, not a link, owned by this user and unreadable by anyone else."""
    require(isinstance(path, str) and os.path.isabs(path), "provider_key_refused")
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0))
    except OSError:
        raise Refused("provider_key_refused") from None
    try:
        info = os.fstat(fd)
        require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0
                and info.st_size <= 1024, "provider_key_refused")
        return os.read(fd, 1025).decode("ascii", "replace").strip()
    finally:
        os.close(fd)


# Folders whose contents a sync client copies off the operator host (review M2, interim
# until ADR-003 names the approved store). Matched on any path component, case-folded,
# of both the configured path and its real path.
def _sync_class(parts):
    folded = [part.casefold() for part in parts]
    for i, part in enumerate(folded):
        parent = folded[i - 1] if i else ""
        if parent == "library" and part == "mobile documents" or part == "icloud drive":
            return "sync:icloud_drive"
        if parent == "library" and part == "cloudstorage":
            return "sync:cloud_storage"
        if part == "dropbox" or part.startswith(("dropbox (", "dropbox-")):
            return "sync:dropbox"
        if part in ("google drive", "my drive") or part.startswith("googledrive"):
            return "sync:google_drive"
        if part == "onedrive" or part.startswith(("onedrive -", "onedrive-")):
            return "sync:onedrive"
    return None


def _under(path, root):
    try:
        Path(path).relative_to(root)
        return True
    except ValueError:
        return False


def directory_class(path):
    """The capability directory's path class, recorded as evidence: `sync:<client>`
    (refused), `temporary`, `home` or `other`. Never the path itself."""
    real = os.path.realpath(path)
    for candidate in (path, real):
        found = _sync_class(Path(candidate).parts)
        if found:
            return found
    temporary = {os.path.realpath(tempfile.gettempdir()), "/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp",
                 "/var/folders", "/private/var/folders"}
    if any(_under(real, root) or _under(path, root) for root in temporary):
        return "temporary"
    home = os.path.expanduser("~")
    if home != "~" and (_under(real, os.path.realpath(home)) or _under(path, home)):
        return "home"
    return "other"


def private_directory(path):
    """An absolute directory, not a link, owned by this user, with no group or other access,
    and not inside a folder that a sync client copies off the host."""
    require(isinstance(path, str) and os.path.isabs(path), "provider_capability_directory_refused")
    require(not directory_class(path).startswith("sync:"), "provider_capability_directory_synced")
    try:
        info = os.lstat(path)
    except OSError:
        raise Refused("provider_capability_directory_refused") from None
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0,
            "provider_capability_directory_refused")
    return Path(path)


def load_config(environ, required):
    """None when no provider variable is set and none is required; otherwise all of them, validated."""
    present = {name: environ.get(name) for name in ENV_NAMES if environ.get(name) is not None}
    if not present:
        require(not required, "provider_configuration_missing")
        return None
    require(ENV_URL in present and ENV_CAPABILITY_DIR in present and (ENV_KEY in present or ENV_KEY_FILE in present),
            "provider_configuration_incomplete")
    try:
        url = urlsplit(present[ENV_URL])
        port = url.port
    except ValueError:
        raise Refused("provider_url_refused") from None
    scheme, host = url.scheme, url.hostname
    require(scheme in ("https", "http") and host and url.username is None and url.password is None and not url.query
            and not url.fragment and url.path in ("", "/") and (port is None or 1 <= port <= 65535)
            and (scheme == "https" or host in LOOPBACK), "provider_url_refused")
    require(ENV_KEY not in present or ENV_KEY_FILE not in present, "provider_key_refused")
    key = _private_file(present[ENV_KEY_FILE]) if ENV_KEY_FILE in present else present[ENV_KEY]
    require(isinstance(key, str) and KEY.fullmatch(key) is not None, "provider_key_refused")
    directory = private_directory(present[ENV_CAPABILITY_DIR])
    return Config(scheme, host, port or (443 if scheme == "https" else 80), key, directory)


# --- transport -------------------------------------------------------------------

class Response:
    """`sent` False: the request never left (connect or TLS failed). `status` None with
    `sent` True: the request may have reached the provider and no complete answer came back."""

    def __init__(self, sent, status=None, body=None):
        self.sent, self.status, self.body = sent, status, body


def _shutdown(sock):
    try:
        sock.shutdown(socket.SHUT_RDWR)
    except OSError:
        pass


def request(config, method, path, payload=None):
    """One bounded HTTP exchange; never retried here. The whole exchange ends by the deadline."""
    deadline = time.monotonic() + PROVIDER_DEFAULTS["request_deadline_seconds"]
    connect_timeout = min(PROVIDER_DEFAULTS["connect_timeout_seconds"], PROVIDER_DEFAULTS["request_deadline_seconds"])
    if config.scheme == "https":
        connection = http.client.HTTPSConnection(config.host, config.port, timeout=connect_timeout, context=ssl.create_default_context())
    else:
        connection = http.client.HTTPConnection(config.host, config.port, timeout=connect_timeout)
    try:
        connection.connect()
    except (OSError, ssl.SSLError, http.client.HTTPException):
        connection.close()
        return Response(sent=False)
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        connection.close()
        return Response(sent=False)
    sock = connection.sock
    sock.settimeout(remaining)
    # A slow trickle could otherwise keep each read under the socket timeout.
    watchdog = threading.Timer(remaining, _shutdown, (sock,))
    watchdog.daemon = True
    watchdog.start()
    try:
        headers = {"Authorization": config.authorization(), "Accept": "application/json", "Connection": "close"}
        data = None
        if payload is not None:
            data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
            headers["Content-Type"] = "application/json"
        connection.request(method, path, body=data, headers=headers)
        response = connection.getresponse()
        limit, chunks, size = PROVIDER_DEFAULTS["response_bytes_max"], [], 0
        while True:
            chunk = response.read(65536)
            if not chunk:
                break
            size += len(chunk)
            if size > limit or time.monotonic() > deadline:
                return Response(sent=True)
            chunks.append(chunk)
        status, raw = response.status, b"".join(chunks)
    except Exception:
        return Response(sent=True)
    finally:
        watchdog.cancel()
        connection.close()
    try:
        body = json.loads(raw.decode("utf-8")) if raw else None
    except ValueError:
        body = None
    return Response(sent=True, status=status, body=body)


def _error_code(body):
    code = body.get("error") if isinstance(body, dict) else None
    return code if isinstance(code, str) and ERROR_CODE.fullmatch(code) else None


# --- operations ------------------------------------------------------------------

class CreateOutcome:
    """state: confirmed (the provider created it), failed (definitively not created) or
    unknown (it may exist). usable: confirmed and the response met the contract, so
    `token` is the capability. The token never appears in repr."""

    def __init__(self, state, detail, status=None, remote_id=None, token=None, expires_at=None, provider_error=None):
        self.state, self.detail, self.status, self.remote_id = state, detail, status, remote_id
        self.token, self.expires_at, self.provider_error = token, expires_at, provider_error
        self.usable = token is not None

    def __repr__(self):
        return "CreateOutcome(%s, %s, %s, usable=%s)" % (self.state, self.detail, self.remote_id, self.usable)


class RevokeOutcome:
    """state: confirmed (no pending invitation with that id remains), failed (refused),
    unknown (may or may not have applied) or pending (never sent)."""

    def __init__(self, state, detail, status=None, provider_error=None):
        self.state, self.detail, self.status, self.provider_error = state, detail, status, provider_error


class Invite:
    def __init__(self, remote_id, email, role_name, status, created_at, invited_by=None):
        self.remote_id, self.email, self.role_name, self.status, self.created_at = remote_id, email, role_name, status, created_at
        # None only for a management-key (API) create; a console or admin-UI create names its user.
        self.invited_by = invited_by


# Statuses that the contract returns before any write (management.rs:1680-1712,
# middleware/management_auth.rs, the rate limiter): definitively not created.
DEFINITIVE_REFUSALS = (400, 401, 403, 404, 429)


class Client:
    def __init__(self, config):
        self.config = config

    def tenant(self):
        """The management key's own tenant (read-only, safe to repeat); it must be invite-only."""
        response = request(self.config, "GET", "/manage/config")
        require(response.status not in (401, 403), "provider_unauthorized")
        body = response.body
        require(response.status == 200 and isinstance(body, dict) and isinstance(body.get("tenant_id"), str), "provider_unavailable")
        require(body.get("signup_mode") == "invite_only", "provider_tenant_not_invite_only")
        return body["tenant_id"]

    def require_tenant(self, provider_tenant):
        """The key's tenant must be the code's bound provider tenant."""
        require(self.tenant() == provider_tenant, "provider_tenant_mismatch")
        return provider_tenant

    def create(self, email, role):
        """Exactly one POST, never repeated, whatever the outcome."""
        email = normalize_email(email)
        response = request(self.config, "POST", "/manage/invites", {"email": email, "role": role, "delivery_mode": "server_only"})
        if not response.sent:
            return CreateOutcome("failed", "not_sent")
        if response.status is None:
            return CreateOutcome("unknown", "transport_ambiguous")
        if response.status in DEFINITIVE_REFUSALS:
            return CreateOutcome("failed", "refused", response.status, provider_error=_error_code(response.body))
        if response.status != 201:
            return CreateOutcome("unknown", "status_%d" % response.status, response.status)
        body = response.body
        invite = body.get("invite") if isinstance(body, dict) else None
        remote_id = invite.get("id") if isinstance(invite, dict) else None
        if not isinstance(remote_id, str) or REMOTE_ID.fullmatch(remote_id) is None:
            return CreateOutcome("unknown", "contract_mismatch", 201)
        token = body.get("invite_token")
        try:
            expires_at = parse_time(invite.get("expires_at"))
        except Refused:
            expires_at = None
        if (isinstance(token, str) and TOKEN.fullmatch(token) and token != remote_id and expires_at is not None
                and invite.get("email") == email and invite.get("role_name") == role and invite.get("status") == "pending"
                and body.get("email_delivery") == "suppressed" and "invite_url" not in body):
            return CreateOutcome("confirmed", "created", 201, remote_id, token, expires_at)
        # It exists, but the response broke the server-only contract: never store or use it.
        return CreateOutcome("confirmed", "contract_mismatch", 201, remote_id)

    def revoke(self, remote_id):
        """Safe to repeat: the provider only changes a pending invitation."""
        require(isinstance(remote_id, str) and REMOTE_ID.fullmatch(remote_id) is not None, "provider_contract_mismatch")
        response = request(self.config, "POST", "/manage/invites/%s/revoke" % remote_id)
        if not response.sent:
            return RevokeOutcome("pending", "not_sent")
        if response.status is None:
            return RevokeOutcome("unknown", "transport_ambiguous")
        body = response.body if isinstance(response.body, dict) else {}
        if response.status == 200 and body.get("success") is True and body.get("invite_id") == remote_id:
            return RevokeOutcome("confirmed", "revoked", 200)
        if response.status == 404 and body.get("error") == "invite_not_found":
            return RevokeOutcome("confirmed", "not_pending", 404)
        if response.status in (400, 401, 403, 429):
            return RevokeOutcome("failed", "refused", response.status, _error_code(body))
        return RevokeOutcome("unknown", "status_%d" % response.status, response.status)

    def scan(self, not_before):
        """Every invitation created at or after `not_before`, newest first, across pages.

        Offset paging over `created_at DESC` with no status filter can repeat a
        row (new invitations shift older ones down) but never skip one, because
        invitations are never deleted (management.rs:1550-1670)."""
        size, seen, invites = PROVIDER_DEFAULTS["list_page_size"], set(), []
        for page in range(PROVIDER_DEFAULTS["list_pages_max"]):
            response = request(self.config, "GET", "/manage/invites?limit=%d&offset=%d" % (size, page * size))
            require(response.status not in (401, 403), "provider_unauthorized")
            body = response.body
            require(response.status == 200 and isinstance(body, dict) and isinstance(body.get("invites"), list)
                    and isinstance(body.get("total"), int), "provider_unavailable")
            older = False
            for item in body["invites"]:
                require(isinstance(item, dict) and isinstance(item.get("id"), str) and isinstance(item.get("email"), str)
                        and isinstance(item.get("status"), str) and "invited_by_user_id" in item
                        and (item["invited_by_user_id"] is None or isinstance(item["invited_by_user_id"], str)),
                        "provider_contract_mismatch")
                created = parse_time(item.get("created_at"))
                if created < not_before:
                    older = True
                    continue
                if item["id"] not in seen:
                    seen.add(item["id"])
                    invites.append(Invite(item["id"], item["email"], item.get("role_name"), item["status"], created,
                                          item["invited_by_user_id"]))
            if older or not body["invites"] or (page + 1) * size >= body["total"]:
                return invites
        raise Refused("provider_inspection_incomplete")


# --- capability custody ------------------------------------------------------------

def capability_path(directory, organization, invitation_id):
    require(isinstance(organization, str) and UUID.fullmatch(organization) is not None
            and isinstance(invitation_id, str) and UUID.fullmatch(invitation_id) is not None, "malformed_identifier")
    return Path(directory, "%s.%s.capability" % (organization, invitation_id))


def _sync_directory(directory):
    fd = os.open(directory, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _partial_path(directory, organization, invitation_id):
    """A hidden, unique name the sweep never reads as a capability (review m6)."""
    capability_path(directory, organization, invitation_id)
    return Path(directory, ".%s.%s.%s.partial" % (organization, invitation_id, secrets.token_hex(8)))


def write_capability(directory, organization, invitation_id, remote_id, token, expires_at):
    """Write the 0600 capability under a private partial name, fsync it, then link it to its final
    name, which never existed (link fails rather than overwrite). The final name therefore only
    ever holds a complete record, so a concurrent sweep cannot read or delete a half-written one."""
    path = capability_path(directory, organization, invitation_id)
    require(isinstance(token, str) and TOKEN.fullmatch(token) is not None and isinstance(remote_id, str)
            and REMOTE_ID.fullmatch(remote_id) is not None, "provider_contract_mismatch")
    private_directory(str(directory))
    record = {"format": CAPABILITY_FORMAT, "organization_id": organization, "invitation_id": invitation_id,
              "remote_invitation_id": remote_id, "provider_expires_at": expires_at, "invite_token": token}
    partial = _partial_path(directory, organization, invitation_id)
    try:
        fd = os.open(partial, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0), 0o600)
    except OSError:
        raise Refused("capability_unwritable") from None
    try:
        try:
            os.fchmod(fd, 0o600)
            os.write(fd, json.dumps(record, sort_keys=True).encode("utf-8"))
            os.fsync(fd)
        finally:
            os.close(fd)
        try:
            os.link(partial, path)
        except OSError:
            raise Refused("capability_unwritable") from None
    finally:
        try:
            os.unlink(partial)
        except FileNotFoundError:
            pass
    _sync_directory(directory)
    return path


def capability_present(directory, organization, invitation_id):
    return os.path.lexists(capability_path(directory, organization, invitation_id))


def capability_record(directory, organization, invitation_id):
    """The non-secret part of a well-formed capability (remote id and provider expiry), or None.

    Its remote id came from a 201 that met the server-only contract, so reconcile takes it
    from here instead of inspecting the provider (review M1(d))."""
    path = capability_path(directory, organization, invitation_id)
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    except OSError:
        return None
    try:
        record = json.loads(os.read(fd, 65536).decode("utf-8"))
        remote = record["remote_invitation_id"]
        if (record["format"] != CAPABILITY_FORMAT or record["organization_id"] != organization
                or record["invitation_id"] != invitation_id or not isinstance(remote, str) or REMOTE_ID.fullmatch(remote) is None
                or not isinstance(record["invite_token"], str) or TOKEN.fullmatch(record["invite_token"]) is None):
            return None
        return {"remote_invitation_id": remote, "provider_expires_at": parse_time(record["provider_expires_at"])}
    except (ValueError, KeyError, TypeError, Refused):
        return None
    finally:
        os.close(fd)


def capability_expiry(directory, organization, invitation_id):
    """The provider expiry recorded beside the token, or None when absent or unreadable."""
    record = capability_record(directory, organization, invitation_id)
    return record["provider_expires_at"] if record else None


def remove_stale_partials(directory, organization, older_than_seconds):
    """Unlink this organization's partial writes older than the bound: a writer that stopped between
    its write and its link (crash). A younger one may still be linked, so it is left alone."""
    prefix, removed, now = "." + organization + ".", 0, time.time()
    for name in os.listdir(directory):
        if not (name.startswith(prefix) and name.endswith(".partial")):
            continue
        path = Path(directory, name)
        try:
            info = os.lstat(path)
            if now - info.st_mtime > older_than_seconds:
                os.unlink(path)
                removed += 1
        except FileNotFoundError:
            continue
    if removed:
        _sync_directory(directory)
    return removed


def destroy_capability(directory, organization, invitation_id):
    """Unlink the capability; True when one existed. Its end of life is the file's removal."""
    path = capability_path(directory, organization, invitation_id)
    try:
        os.unlink(path)
    except FileNotFoundError:
        return False
    _sync_directory(directory)
    return True


def capability_invitations(directory, organization):
    """Invitation ids of this organization's capability files (names only, nothing read)."""
    prefix = organization + "."
    found = []
    for name in os.listdir(directory):
        if name.startswith(prefix) and name.endswith(".capability"):
            invitation = name[len(prefix):-len(".capability")]
            if UUID.fullmatch(invitation):
                found.append(invitation)
    return sorted(found)
