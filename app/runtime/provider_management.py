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
In the managed operator task (ADR-003 amendment 2026-10-30, managed invitations,
as revised the same day after FormaStack's answers) it arrives instead from
FormaStack's own tenant-scoped management secret (for Northstar
`fauth-beta/management/northstar`, JSON `{tenant_id, management_key}`), which
the stack attaches by ARN (parameter ProviderManagementSecretArn, approval A12)
and never copies: ECS injects its two JSON keys into the operator container
alone, as GENUI_PROVIDER_MANAGEMENT_SECRET_KEY and
GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID, beside the non-secret
GENUI_PROVIDER_MANAGEMENT_URL (parameter ProviderManagementUrl). All three or
none; the plain variables never appear beside them; the injected tenant must be
the code's bound tenant before any provider call. Nothing here prints or logs
the key, an `invite_token` or an address; callers receive fixed reason codes.

Capability custody (ADR-003 amendment 2026-10-30, APP-074 review M1; it
supersedes the operator-host 0600 directory): a token is written only into the
environment's one AWS Secrets Manager secret (CapabilitySecret), a JSON map keyed
by `{organization}.{invitation}` whose entries are exactly the records the
service reads, with PutSecretValue under the operator's own AWS credentials. ECS
injects that secret into the service task, whose runtime.py serve() writes each
entry as a 0600 file for the service child alone. The operator host keeps no
copy: nothing here writes a capability to disk. The secret's id comes from the
environment (GENUI_PROVIDER_CAPABILITY_SECRET_ID) or the operator's
`--capability-secret-id` (an ARN, not a secret); a value never travels in argv.
"""
import datetime
import http.client
import json
import os
import re
import socket
import ssl
import stat
import threading
import time
import uuid
from urllib.parse import urlsplit

import runtime

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
ENV_CAPABILITY_SECRET = "GENUI_PROVIDER_CAPABILITY_SECRET_ID"
# The retired operator-host directory (ADR-003 amendment 2026-10-29); refused if still set,
# so a stale setup never believes it keeps capabilities on the host.
ENV_RETIRED_CAPABILITY_DIR = "GENUI_PROVIDER_CAPABILITY_DIR"
ENV_NAMES = (ENV_URL, ENV_KEY, ENV_KEY_FILE)
# ADR-003 amendment 2026-10-30 (managed invitations, revised after FormaStack's answers): the
# managed operator task's only provider source. ECS injects `management_key` and `tenant_id` from
# FormaStack's existing management secret (attached by ARN, never copied) and the stack sets the
# non-secret URL; all three are present (attached) or none is (not configured).
ENV_MANAGED_URL = "GENUI_PROVIDER_MANAGEMENT_URL"
ENV_MANAGED_KEY = "GENUI_PROVIDER_MANAGEMENT_SECRET_KEY"
ENV_MANAGED_TENANT = "GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID"
MANAGED_NAMES = (ENV_MANAGED_URL, ENV_MANAGED_KEY, ENV_MANAGED_TENANT)
# The retired whole-document injection of the CDK-created ProviderManagementKey secret; refused if
# still set, so a stale task definition never runs beside the attached secret.
ENV_RETIRED_MANAGEMENT_JSON = "GENUI_PROVIDER_MANAGEMENT_JSON"
# Every variable that carries provider settings (operator.py scrubs them from commands that need none).
PROVIDER_VARIABLES = ENV_NAMES + MANAGED_NAMES + (ENV_RETIRED_MANAGEMENT_JSON,)
# The capability secret: a full Secrets Manager ARN (its region is the client's region).
SECRET_ARN = re.compile(r"arn:aws:secretsmanager:([a-z]{2}(?:-[a-z]+)+-[0-9]):[0-9]{12}:secret:[A-Za-z0-9/_+=.@-]{1,512}")
LOOPBACK = ("127.0.0.1", "localhost", "::1")
KEY = re.compile(r"mgmt_[A-Za-z0-9_-]{8,256}")
REMOTE_ID = re.compile(r"tinv_[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
TOKEN = REMOTE_ID  # same shape as the id (management.rs:1718-1719); never interchangeable
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
ERROR_CODE = re.compile(r"[a-z][a-z0-9_]{0,63}")
CAPABILITY_FORMAT = runtime.CAPABILITY_FORMAT
CODE_DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
CAPABILITY_EXPIRY = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
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

    def __init__(self, scheme, host, port, key, capability_store, tenant_id=None):
        self.scheme, self.host, self.port, self.capability_store = scheme, host, port, capability_store
        # The managed source's injected tenant (None for the plain variables): the key's own tenant
        # as FormaStack recorded it beside the key, checked before any provider call.
        self.tenant_id = tenant_id
        self._key = key

    def authorization(self):
        return "Bearer " + self._key

    def __repr__(self):
        return "Config(%s://%s:%d, capability_store=%r)" % (self.scheme, self.host, self.port, self.capability_store)

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


def load_store(environ, secret_id=None, required=False):
    """The capability secret named by `--capability-secret-id` or GENUI_PROVIDER_CAPABILITY_SECRET_ID,
    or None when neither is set and none is required. A full ARN only."""
    require(ENV_RETIRED_CAPABILITY_DIR not in environ, "provider_capability_directory_retired")
    value = secret_id if secret_id is not None else environ.get(ENV_CAPABILITY_SECRET)
    if value is None:
        require(not required, "provider_configuration_incomplete")
        return None
    require(isinstance(value, str) and SECRET_ARN.fullmatch(value) is not None, "capability_secret_refused")
    return CapabilitySecret(value)


def _graphic(value, bound):
    return isinstance(value, str) and 1 <= len(value) <= bound and all(0x21 <= ord(ch) <= 0x7E for ch in value)


def management_configuration(environ):
    """The managed source: None when none of its three variables is set; otherwise all three,
    non-empty (a missing piece is `provider_configuration_missing`), the key of the shape fAuth
    issues and the tenant a registry-shaped id. No value ever appears in a refusal."""
    present = {name: environ.get(name) for name in MANAGED_NAMES if environ.get(name) is not None}
    if not present:
        return None
    require(len(present) == len(MANAGED_NAMES) and all(present.values()), "provider_configuration_missing")
    require(len(present[ENV_MANAGED_URL]) <= 512, "provider_url_refused")
    require(KEY.fullmatch(present[ENV_MANAGED_KEY]) is not None, "provider_key_refused")
    # runtime.identity_registry's provider_tenant_id rule (1..128 printable ASCII, no space).
    require(_graphic(present[ENV_MANAGED_TENANT], 128), "provider_tenant_refused")
    return {"url": present[ENV_MANAGED_URL], "management_key": present[ENV_MANAGED_KEY], "tenant_id": present[ENV_MANAGED_TENANT]}


def load_config(environ, required, secret_id=None):
    """None when no provider variable is set and none is required; otherwise all of them, validated,
    with the capability secret (a provider configuration without one is incomplete)."""
    require(ENV_RETIRED_CAPABILITY_DIR not in environ, "provider_capability_directory_retired")
    require(ENV_RETIRED_MANAGEMENT_JSON not in environ, "provider_management_json_retired")
    present = {name: environ.get(name) for name in ENV_NAMES if environ.get(name) is not None}
    tenant_id = None
    if any(environ.get(name) is not None for name in MANAGED_NAMES):
        # The managed operator task: the attached secret is the only source of the key and its
        # tenant, and the plain variables are refused beside it (never merged, never preferred).
        require(not present, "provider_configuration_conflict")
        managed = management_configuration(environ)
        present, tenant_id = {ENV_URL: managed["url"], ENV_KEY: managed["management_key"]}, managed["tenant_id"]
    if not present:
        require(not required, "provider_configuration_missing")
        return None
    require(ENV_URL in present and (ENV_KEY in present or ENV_KEY_FILE in present), "provider_configuration_incomplete")
    store = load_store(environ, secret_id, required=True)
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
    return Config(scheme, host, port or (443 if scheme == "https" else 80), key, store, tenant_id)


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
        """The key's tenant must be the code's bound provider tenant: first the tenant injected beside a
        managed key (no request is made for another tenant's code), then the key's own, asked of fAuth."""
        require(self.config.tenant_id is None or self.config.tenant_id == provider_tenant, "provider_tenant_mismatch")
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


# --- capability custody (ADR-003 amendment 2026-10-30; APP-074 review M1) -----------
#
# One Secrets Manager secret per environment holds every live capability as a JSON
# map `{organization}.{invitation}` -> record, the record exactly what the service's
# invitation entry reads (app/src/auth/invite.rs Record) and runtime.py validates.
# Every change is one read and one PutSecretValue of the whole map; the operator
# serializes them with a database advisory lock (admission_operator.capability_lock),
# so two operator commands never lose each other's entry. Reads and writes answer
# fixed reason codes only: no botocore error text (which can carry request data)
# ever leaves this module.

CAPABILITY_SECRET_BYTES_MAX = runtime.CAPABILITY_SECRET_BYTES_MAX
CAPABILITIES_MAX = runtime.CAPABILITIES_MAX
SECRET_CLIENT = {"connect_timeout": 5, "read_timeout": 10, "total_max_attempts": 3}


def capability_id(organization, invitation_id):
    require(isinstance(organization, str) and UUID.fullmatch(organization) is not None
            and isinstance(invitation_id, str) and UUID.fullmatch(invitation_id) is not None, "malformed_identifier")
    return "%s.%s" % (organization, invitation_id)


def capability_entry(organization, invitation_id, remote_id, token, expires_at, code_digest=None):
    """(id, record) of one capability, validated as the service's supervisor validates it.

    APP-074: `code_digest` (the code's stored `sha256:` digest, never the code) lets the service's
    invitation entry find the one capability of a validated code; the service never reads the
    operator's tables, and a record without it is never handed off."""
    key = capability_id(organization, invitation_id)
    require(isinstance(token, str) and TOKEN.fullmatch(token) is not None and isinstance(remote_id, str)
            and REMOTE_ID.fullmatch(remote_id) is not None and token != remote_id
            and isinstance(expires_at, str) and CAPABILITY_EXPIRY.fullmatch(expires_at) is not None, "provider_contract_mismatch")
    require(code_digest is None or (isinstance(code_digest, str) and CODE_DIGEST.fullmatch(code_digest) is not None),
            "malformed_identifier")
    record = {"format": CAPABILITY_FORMAT, "organization_id": organization, "invitation_id": invitation_id,
              "remote_invitation_id": remote_id, "provider_expires_at": expires_at, "invite_token": token}
    if code_digest is not None:
        record["code_digest"] = code_digest
    try:
        runtime.check_capability(key, record)
    except runtime.Refused:
        raise Refused("provider_contract_mismatch") from None
    return key, record


def parse_capabilities(text):
    """The secret's map, validated whole exactly as the service's supervisor will (runtime.capability_map)."""
    try:
        return runtime.capability_map(text)
    except (runtime.Refused, ValueError, TypeError):
        raise Refused("capability_secret_malformed") from None


def serialize_capabilities(capabilities):
    """The map's one canonical text; never a value the service would refuse to start with."""
    require(len(capabilities) <= CAPABILITIES_MAX, "capability_secret_full")
    text = json.dumps(capabilities, sort_keys=True, separators=(",", ":"))
    require(len(text.encode("utf-8")) <= CAPABILITY_SECRET_BYTES_MAX, "capability_secret_full")
    parse_capabilities(text)
    return text


def secrets_client(region):
    import boto3
    from botocore.config import Config as SdkConfig
    return boto3.session.Session(region_name=region).client("secretsmanager", config=SdkConfig(
        connect_timeout=SECRET_CLIENT["connect_timeout"], read_timeout=SECRET_CLIENT["read_timeout"],
        retries={"total_max_attempts": SECRET_CLIENT["total_max_attempts"], "mode": "standard"}))


# B1 review m5: the operator reads the environment's AuthConfiguration secret (its ARN, an identifier,
# from this variable or `--auth-configuration-secret-id`) so `bind-provider-tenant` binds exactly the
# (issuer, tenant) the service's registry names, with no identifier on argv.
ENV_AUTH_CONFIGURATION_SECRET = "GENUI_AUTH_CONFIGURATION_SECRET_ID"


class ManagedSecret:
    """One Secrets Manager secret read whole (AWSCURRENT) with the caller's own AWS credentials; the
    value is returned to the caller and never appears in a refusal."""

    def __init__(self, secret_id, client=None):
        match = SECRET_ARN.fullmatch(secret_id) if isinstance(secret_id, str) else None
        require(match is not None, "secret_id_refused")
        self.secret_id, self.region, self._client = secret_id, match.group(1), client

    def __repr__(self):
        return "%s(%s)" % (type(self).__name__, self.secret_id)

    __str__ = __repr__

    def client(self):
        if self._client is None:
            self._client = secrets_client(self.region)
        return self._client

    def read_text(self):
        try:
            text = self.client().get_secret_value(SecretId=self.secret_id).get("SecretString")
        except Exception:
            raise Refused("secret_unavailable") from None
        require(isinstance(text, str), "secret_unavailable")
        return text


class CapabilitySecret(ManagedSecret):
    """The environment's capability secret, read and written with the caller's own AWS credentials
    (the operator's; the service only receives it through ECS injection)."""

    def __init__(self, secret_id, client=None):
        match = SECRET_ARN.fullmatch(secret_id) if isinstance(secret_id, str) else None
        require(match is not None, "capability_secret_refused")
        super().__init__(secret_id, client)

    def read(self):
        """The current map (AWSCURRENT)."""
        try:
            text = self.read_text()
        except Refused:
            raise Refused("capability_secret_unavailable") from None
        return parse_capabilities(text)

    def write(self, capabilities):
        """Replace the map with one PutSecretValue. Its token is fixed for this call, so the SDK's
        retries of an ambiguous answer are idempotent; a refusal leaves the outcome to the next read."""
        text = serialize_capabilities(capabilities)
        try:
            self.client().put_secret_value(SecretId=self.secret_id, SecretString=text, ClientRequestToken=str(uuid.uuid4()))
        except Exception:
            raise Refused("capability_secret_unwritten") from None

    def add(self, organization, invitation_id, remote_id, token, expires_at, code_digest=None):
        """Add one capability; an existing entry is never replaced (refused, nothing written)."""
        key, record = capability_entry(organization, invitation_id, remote_id, token, expires_at, code_digest)
        current = self.read()
        require(key not in current, "capability_unwritable")
        self.write(dict(current, **{key: record}))
        return key

    def remove(self, pairs):
        """Remove these (organization, invitation) entries with at most one write; the number removed."""
        keys = {capability_id(organization, invitation) for organization, invitation in pairs}
        if not keys:
            return 0
        current = self.read()
        gone = keys & set(current)
        if gone:
            self.write({key: record for key, record in current.items() if key not in gone})
        return len(gone)


def capability_present(capabilities, organization, invitation_id):
    return capability_id(organization, invitation_id) in capabilities


def capability_record(capabilities, organization, invitation_id):
    """The non-secret part of a capability in this map (remote id and provider expiry), or None.

    Its remote id came from a 201 that met the server-only contract, so reconcile takes it
    from here instead of inspecting the provider (review M1(d))."""
    record = capabilities.get(capability_id(organization, invitation_id))
    if record is None:
        return None
    try:
        return {"remote_invitation_id": record["remote_invitation_id"], "provider_expires_at": parse_time(record["provider_expires_at"])}
    except Refused:
        return None


def capability_expiry(capabilities, organization, invitation_id):
    """The provider expiry recorded beside the token, or None when absent."""
    record = capability_record(capabilities, organization, invitation_id)
    return record["provider_expires_at"] if record else None


def capability_invitations(capabilities, organization):
    """Invitation ids of this organization's entries (ids only, no token read out)."""
    require(isinstance(organization, str) and UUID.fullmatch(organization) is not None, "malformed_identifier")
    prefix = organization + "."
    return sorted(key[len(prefix):] for key in capabilities if key.startswith(prefix))
