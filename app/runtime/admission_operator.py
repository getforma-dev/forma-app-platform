"""Operator-only admission channel (ADR-007 D7.19, D7.1, D7.7, D7.8, D7.17; APP-073).

Never an HTTP, application or signup path. Closed subcommands only; no generic
SQL. Every command connects as the migration identity (session_user and
current_user must be genui_migration) over verify-full TLS using only the
libpq-style environment (PGHOST, PGPORT, PGUSER, PGPASSWORD, PGSSLROOTCERT,
PGDATABASE=genui), proves the reviewed latest catalog, then acts as
genui_owner under explicit organization scope in D7.1's lock order
(organization FOR SHARE, then the member or code row FOR UPDATE, then the
operator audit head) and appends to the `operator` audit chain.

Authority on apps and saved work (ADR-007 amendment of 2026-10-29, APP-010
review M2 and m7): `create-app` writes an app shell and its owner's
`edit_definition`; `grant-permission` and `revoke-permission` write one row of
the closed GRANTABLE set (a revocation advances the member's epoch);
`revoke-creator-authority` ends a saved work's creator authority. Each change
appends `permission.granted`, `permission.revoked` or
`creator_authority.revoked` to the operator chain.

Source revisions and their saved work (APP-012; ADR-007 D7.10, ADR-009 D9.3;
APP-077 review M2): `register-source-revision --record` inserts one NEW
`source_revisions` row from `upload-sources`' printed record, with the real
digests of the objects already written at ADR-009 B1's keys; a revision is
never rewritten (an identical record is `unchanged`, a differing one refused).
`create-bound-work` writes a new saved work at version 1 that copies an
existing work's publication, context and bindings and binds one source input
to another live revision of the same source with the same contract; the old
work and its records are untouched. Neither writes an audit event: D7.8's
operator kinds name none (a kind is an ADR amendment), so, as with the seed,
the retained JSON output is the record.

A new access code's secret (32 CSPRNG bytes, base32) is printed exactly once,
on standard output, after its transaction commits. It is never logged, stored
or written anywhere else; only its domain-separated digest is stored. In a
managed (ECS) task, whose standard output is CloudWatch Logs, the operator
instead mints the code on its own machine (infra/scripts/window_requests.py
mint-code) and passes `issue --code-digest` only: the task never sees the
secret and prints only the digest; a reused digest refuses (code_index primary
key), and a pre-minted code never carries a provider invitation. Listing
and audit show identifiers, status and counts only. Refusals print a fixed
reason code on standard error and never echo an argument value.

Provider invitations (APP-073 acceptance 6 and 7, ADR-003): `issue` with an
intended email and the explicit least-privilege role records a `pending` create
in the code's transaction, then, after COMMIT, sends exactly one server-only
create to fAuth's management API (provider_management.py) and records
`confirmed` with the remote invitation id, `failed`, or `unknown`; a create is
never repeated. The returned `invite_token` goes only to a 0600 capability file
in the operator's private directory, named by the invitation row. `revoke`
commits the local revocation first, destroys the code's capability and then
revokes remotely; `reconcile` settles pending/unknown operations by inspecting
the provider, one run per organization at a time, attributing each remote
invitation to at most one create (see judge_creates). Provider settings come only
from the environment (GENUI_PROVIDER_*; the management key from a 0600 file or the
environment, never argv).

Sign-in handles (APP-009, ADR-006 §handle): `issue-handle` gives an organization
without one its first live handle (fresh, or an exact already-issued `--handle`
so one routing value serves every environment); `rotate-handle` adds a second
live handle; `retire-handle` stops the old one after FormaStack re-registers the
client's return_entry_url. Handles are non-secret routing and are printed.
"""
import argparse
import base64
import datetime
import hashlib
import json
import os
import re
import secrets
import sys
import time
import uuid

import provider_management as provider
import runtime

# ADR-007 D7.17, as enforced by migration 0003's CHECKs and definer bodies.
DEFAULTS = {
    "trial_entitlement_days": 30,
    "code_expiry_days_default": 14,
    "code_expiry_days_max": 30,
    "max_redemptions_default": 1,
    "max_redemptions_max": 10,
    "presignup_reservation_minutes": 10,
    "presignup_reservations_live_max": 10,
    "pending_admission_hours": 24,
    "list_rows_max": 100,
    "code_secret_bytes": 32,
    "handle_bytes": 16,
}
CHAINS = ("workspace", "control", "data", "admission", "operator")
# D7.8's closed operator kinds; the last three arrive with the ADR-007 amendment
# of 2026-10-29 (APP-010 review M2) and migration 0005's widened operator policy.
OPERATOR_KINDS = ("organization.provisioned", "handle.rotated", "code.issued", "code.revoked", "code.extended",
                  "membership.revoked", "entitlement.revoked", "permission.granted", "permission.revoked",
                  "creator_authority.revoked")
# ADR-007 amendment (APP-010 review M2, m7): what grant-permission/revoke-permission
# may write, and the resource each permission names. A closed subset of what the
# 0002 permission tables allow: never admin_policy (governance), share_work or
# export (no Phase 1 path uses them).
GRANTABLE = {"discover": "app", "run": "app", "edit_definition": "app", "review_publish": "app",
             "record_read": "work", "record_write": "work", "source_read": "source"}
# The (app, work, source) arguments each resource takes, exactly.
RESOURCE_SHAPE = {"app": (True, False, False), "work": (True, True, False), "source": (False, False, True)}
CODE_DOMAIN = b"forma:access-code:v1"
HANDLE_DOMAIN = b"forma:signin-handle:v1"
EMAIL_DOMAIN = b"forma:intended-email:v1"
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
LABEL = re.compile(r"[a-z0-9][a-z0-9._@-]{0,63}")
# ADR-003:37 / APP-073 acceptance 6: the provider's least-privilege identity role only.
# FormaStack server_only built-in roles are role_admin and role_user (gatewasm 1bf5d3b,
# crates/server/src/handlers/management.rs:1694-1712); widen only by an APP-006-accepted
# contract, never by operator input. Independent review M1.
PROVIDER_ROLES = ("user",)
APP_PERMISSIONS = ("discover", "run", "edit_definition")
WORK_PERMISSIONS = ("record_read", "record_write", "share_work", "export")


# A refusal whose message is a fixed reason code, never a value (shared with provider_management).
Refused = provider.Refused


def require(condition, reason="refused"):
    if not condition:
        raise Refused(reason)


# --- secrets, digests and canonical event bytes ------------------------------

def _b32(raw):
    return base64.b32encode(raw).decode("ascii").rstrip("=")


def _unb32(text, size):
    """Canonical unpadded RFC 4648 base32 only (trailing bits zero)."""
    if not re.fullmatch(r"[A-Z2-7]*", text) or len(text) != -(-size * 8 // 5):
        return None
    raw = base64.b32decode(text + "=" * (-len(text) % 8))
    return raw if len(raw) == size and _b32(raw) == text else None


def encode_code(raw):
    text = _b32(raw)
    return "-".join(text[i:i + 4] for i in range(0, len(text), 4))


def normalize_code(text):
    if not isinstance(text, str) or len(text) > 256:
        return None
    compact = "".join(c for c in text if c != "-" and c not in " \t\r\n\f\v").upper()
    return _unb32(compact, 32) if compact.isascii() else None


def code_digest(raw):
    return "sha256:" + hashlib.sha256(CODE_DOMAIN + b"\x00" + raw).hexdigest()


def encode_handle(raw):
    return _b32(raw).lower()


def normalize_handle(text):
    if not isinstance(text, str) or not re.fullmatch(r"[a-z2-7]{26}", text):
        return None
    return _unb32(text.upper(), 16)


def handle_digest(raw):
    return "sha256:" + hashlib.sha256(HANDLE_DOMAIN + b"\x00" + raw).hexdigest()


def email_digest(email):
    return "sha256:" + hashlib.sha256(EMAIL_DOMAIN + b"\x00" + email.strip().lower().encode("utf-8")).hexdigest()


def canonical(value):
    """forma-json-v1 bytes of a flat event object: ASCII-ordered keys, explicit nulls."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def genesis(organization, writer_role):
    """D7.8 genesis head; the genesis head holds chain_sequence 1, so a chain's first event is 2."""
    return "sha256:" + hashlib.sha256(b"forma:audit-genesis:v1\x00" + organization.encode() + b"\x00" + writer_role.encode()).hexdigest()


def event_hash(previous, body):
    return "sha256:" + hashlib.sha256(b"forma:audit-event:v1\x00" + previous.encode() + canonical(body)).hexdigest()


def timestamp(value):
    return value.astimezone(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


# --- arguments ----------------------------------------------------------------

def identifier(value):
    require(isinstance(value, str) and UUID.fullmatch(value) is not None, "malformed_identifier")
    return value


def parse_grant(text):
    """Closed (permission, app_id, work_id, source_id) template; never admin_policy or review_publish."""
    parts = text.split(":") if isinstance(text, str) else []
    require(len(parts) >= 2, "malformed_grant")
    permission, ids = parts[0], [identifier(p) for p in parts[1:]]
    if permission in APP_PERMISSIONS and len(ids) == 1:
        return (permission, ids[0], None, None)
    if permission in WORK_PERMISSIONS and len(ids) == 2:
        return (permission, ids[0], ids[1], None)
    if permission == "source_read" and len(ids) == 1:
        return (permission, None, None, ids[0])
    raise Refused("grant_refused")


class Parser(argparse.ArgumentParser):
    """Usage errors raise a fixed refusal; argparse would echo the offending value."""
    def error(self, message):
        raise Refused("usage")


def parser():
    p = Parser(description="APP-073 operator admission channel (closed subcommands).")
    sub = p.add_subparsers(dest="command", required=True)
    sub.add_parser("defaults")
    for name in ("backfill", "list", "verify", "reconcile"):
        sub.add_parser(name).add_argument("--organization", required=True)
    q = sub.add_parser("provision")
    for arg in ("--organization", "--display-name", "--issuer", "--provider-tenant"):
        q.add_argument(arg, required=True)
    q = sub.add_parser("issue")
    q.add_argument("--organization", required=True)
    q.add_argument("--issued-by", required=True)
    q.add_argument("--kind", default="trial")
    q.add_argument("--max-redemptions", type=int, default=DEFAULTS["max_redemptions_default"])
    q.add_argument("--expires-days", type=int, default=DEFAULTS["code_expiry_days_default"])
    q.add_argument("--grant", action="append", default=[])
    # APP-078 managed channel: the operator mints the code on its own machine
    # (mint_access_code) and the task receives only its digest, so no secret
    # reaches a task override, CloudTrail or the awslogs group.
    q.add_argument("--code-digest")
    for arg in ("--provider-email", "--provider-role", "--provider-issuer", "--provider-tenant"):
        q.add_argument(arg)
    for name in ("revoke", "extend"):
        q = sub.add_parser(name)
        q.add_argument("--organization", required=True)
        q.add_argument("--code-id", required=True)
        if name == "extend":
            q.add_argument("--expires-days", type=int, required=True)
    for name in ("revoke-entitlement", "revoke-membership"):
        q = sub.add_parser(name)
        q.add_argument("--organization", required=True)
        q.add_argument("--principal", required=True)
    # ADR-007 amendment 2026-10-29 (APP-010 review M2, m7): app shells, their
    # grants and creator-authority revocation are operator actions in Phase 1.
    for name in ("grant-permission", "revoke-permission"):
        q = sub.add_parser(name)
        for arg in ("--organization", "--principal", "--permission"):
            q.add_argument(arg, required=True)
        for arg in ("--app", "--work", "--source"):
            q.add_argument(arg)
    q = sub.add_parser("create-app")
    for arg in ("--organization", "--app", "--space", "--owner"):
        q.add_argument(arg, required=True)
    q = sub.add_parser("revoke-creator-authority")
    for arg in ("--organization", "--app", "--work"):
        q.add_argument(arg, required=True)
    # ADR-006 sign-in handles (D7.19 operator channel; APP-009).
    for name in ("issue-handle", "rotate-handle", "retire-handle"):
        q = sub.add_parser(name)
        q.add_argument("--organization", required=True)
        q.add_argument("--handle", required=(name == "retire-handle"))
    q = sub.add_parser("register-source-revision")
    q.add_argument("--organization", required=True)
    # Exactly one: a local record file, or (the APP-078 managed channel, where
    # an override carries only the command) the same non-secret record inline.
    q.add_argument("--record")
    q.add_argument("--record-json")
    q = sub.add_parser("create-bound-work")
    for arg in ("--organization", "--app", "--from-work", "--work", "--creator", "--input", "--source", "--revision"):
        q.add_argument(arg, required=True)
    return p


def validate(args):
    """Every argument is checked before any connection is opened."""
    if hasattr(args, "organization"):
        identifier(args.organization)
    for name in ("code_id", "principal", "app", "work", "source", "space", "owner", "from_work", "creator", "revision"):
        if getattr(args, name, None) is not None:
            identifier(getattr(args, name))
    if args.command in ("grant-permission", "revoke-permission"):
        resource = GRANTABLE.get(args.permission)
        require(resource is not None, "permission_refused")
        given = tuple(getattr(args, name) is not None for name in ("app", "work", "source"))
        require(given == RESOURCE_SHAPE[resource], "resource_refused")
    if args.command == "provision":
        require(1 <= len(args.display_name) <= 200 and args.display_name.isprintable(), "malformed_display_name")
        require(1 <= len(args.issuer) <= 512 and args.issuer.isascii() and args.issuer.isprintable(), "malformed_issuer")
        require(1 <= len(args.provider_tenant) <= 256 and args.provider_tenant.isascii() and args.provider_tenant.isprintable(), "malformed_provider_tenant")
    if args.command == "issue":
        require(LABEL.fullmatch(args.issued_by) is not None, "malformed_issued_by")
        require(args.kind in ("trial", "member_invite"), "kind_refused")
        require(1 <= args.max_redemptions <= DEFAULTS["max_redemptions_max"], "max_redemptions_out_of_range")
        require(1 <= args.expires_days <= DEFAULTS["code_expiry_days_max"], "expiry_out_of_range")
        require(args.grant, "grant_required")
        args.templates = sorted({parse_grant(g) for g in args.grant}, key=str)
        invitation = (args.provider_email, args.provider_role, args.provider_issuer, args.provider_tenant)
        # ADR-003/APP-073: an invitation needs the intended email, an explicit
        # least-privilege provider role and the exact provider tenant; an
        # omitted role is refused, never defaulted.
        require(all(v is None for v in invitation) or all(v is not None for v in invitation), "invitation_incomplete")
        if args.code_digest is not None:
            # A pre-minted code is never combined with a provider invitation:
            # that path keeps its own capability handling (APP-073).
            require(DIGEST.fullmatch(args.code_digest) is not None, "malformed_code_digest")
            require(args.provider_email is None, "code_digest_with_invitation_refused")
        if args.provider_email is not None:
            email = args.provider_email.strip()
            require(3 <= len(email) <= 320 and email.count("@") == 1 and email.isascii() and email.isprintable(), "malformed_email")
            require(args.provider_role in PROVIDER_ROLES, "provider_role_refused")
            require(1 <= len(args.provider_issuer) <= 512 and 1 <= len(args.provider_tenant) <= 256, "malformed_provider_tenant")
    if args.command == "extend":
        require(1 <= args.expires_days <= DEFAULTS["code_expiry_days_max"], "expiry_out_of_range")
    if args.command in ("issue-handle", "rotate-handle", "retire-handle"):
        args.raw_handle = None
        if args.handle is not None:
            args.raw_handle = normalize_handle(args.handle)
            require(args.raw_handle is not None, "malformed_handle")
    if args.command == "register-source-revision":
        require((args.record is None) != (args.record_json is None), "record_refused")
        if args.record is not None:
            args.revision_record = source_record(args.record, args.organization)
        else:
            raw = args.record_json.encode("utf-8", errors="strict") if args.record_json.isascii() else b""
            args.revision_record = source_record_bytes(raw, args.organization)
    if args.command == "create-bound-work":
        require(LOCAL_ID.fullmatch(args.input) is not None, "malformed_input")
        require(args.work != args.from_work, "work_exists")
    return args


LOCAL_ID = re.compile(r"[a-z][a-z0-9_]{0,63}")
DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
RECORD_KEYS = {"organization_id", "source_id", "revision_id", "content_digest", "normalized_digest", "object_digest",
               "row_count", "contract_digest"}


def revision_id(organization, content_digest):
    """`upload-sources`' revision identity: uuid5(organization, content digest) re-stamped as version 4.

    The contract and every validator accept only version 4 or 7 UUIDs.
    """
    return str(uuid.UUID(bytes=uuid.uuid5(uuid.UUID(organization), content_digest).bytes, version=4))


def source_record(path, organization):
    """`upload-sources`' one printed record, closed and checked before any connection.

    `normalized_digest` is the SHA-256 of the uploaded `part.parquet` bytes
    (ADR-009 D9.3 M6), which the query cache verifies (D7.10); the record's
    `object_digest` names the same object and must agree, and `revision_id`
    must be the content-derived identity (`revision_id`).
    """
    try:
        with open(path, "rb") as stream:
            raw = stream.read(4097)
    except OSError:
        raise Refused("record_refused") from None
    return source_record_bytes(raw, organization)


def _closed_pairs(pairs):
    keys = [key for key, _ in pairs]
    require(len(keys) == len(set(keys)), "record_refused")
    return dict(pairs)


def source_record_bytes(raw, organization):
    """The same closed checks for a record given inline (``--record-json``)."""
    try:
        require(0 < len(raw) <= 4096, "record_refused")
        record = json.loads(raw, object_pairs_hook=_closed_pairs)
    except ValueError:
        raise Refused("record_refused") from None
    require(isinstance(record, dict) and set(record) == RECORD_KEYS, "record_refused")
    for key in ("organization_id", "source_id", "revision_id"):
        identifier(record[key])
    for key in ("content_digest", "normalized_digest", "object_digest", "contract_digest"):
        require(isinstance(record[key], str) and DIGEST.fullmatch(record[key]) is not None, "record_refused")
    require(record["organization_id"] == organization, "record_organization_refused")
    require(record["object_digest"] == record["normalized_digest"], "record_digest_refused")
    require(record["revision_id"] == revision_id(organization, record["content_digest"]), "record_revision_refused")
    require(type(record["row_count"]) is int and 0 <= record["row_count"] <= 9007199254740991, "record_refused")
    return record


# --- operator transaction -----------------------------------------------------

def connect():
    import psycopg
    # Always verify-full TLS: a Unix-socket host never uses TLS, and libpq would
    # prefer GSSAPI encryption over TLS when Kerberos credentials exist.
    require(not os.environ["PGHOST"].startswith("/"), "socket_host_refused")
    return psycopg.connect(host=os.environ["PGHOST"], port=int(os.environ.get("PGPORT", "5432")),
                           dbname=os.environ.get("PGDATABASE", "genui"), user=os.environ["PGUSER"],
                           password=os.environ.get("PGPASSWORD"), sslmode="verify-full", gssencmode="disable",
                           sslrootcert=os.environ["PGSSLROOTCERT"], connect_timeout=5,
                           options="-c search_path=pg_catalog,pg_temp -c statement_timeout=5000 -c lock_timeout=3000")


def begin(c, organization):
    """Identity and catalog proof, then genui_owner under explicit organization scope."""
    require(not c.autocommit, "transaction_required")
    require(c.execute("SHOW session_replication_role").fetchone() == ("origin",), "replication_role_refused")
    c.execute("SET LOCAL search_path=pg_catalog, pg_temp")
    require(c.execute("SELECT session_user::text,current_user::text,current_database()").fetchone()
            == ("genui_migration", "genui_migration", "genui"), "operator_identity_refused")
    runtime.verify_schema_catalog(c, runtime.migration_directory())
    c.execute("SET LOCAL ROLE genui_owner")
    c.execute("SELECT set_config('genui.organization_id',%s,true),set_config('genui.principal_id','',true),"
              "set_config('genui.membership_epoch','',true)", (organization,))


def lock_organization(c, organization, active=False, lock=True):
    row = c.execute("SELECT status FROM genui.organizations WHERE organization_id=%s" + (" FOR SHARE" if lock else ""),
                    (organization,)).fetchone()
    require(row is not None, "organization_unknown")
    require(not active or row[0] == "active", "organization_inactive")


def append(c, organization, kind, code_id=None, actor=None, app=None, work=None):
    """D7.8 append on the operator chain: the head row lock serializes appenders."""
    require(kind in OPERATOR_KINDS, "kind_refused")
    head = c.execute("SELECT chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s AND writer_role='operator' FOR UPDATE",
                     (organization,)).fetchone()
    require(head is not None, "audit_chain_not_provisioned")
    sequence, previous = head[0] + 1, head[1]
    occurred = c.execute("SELECT date_trunc('second',now())").fetchone()[0]
    body = {"actor_principal_id": actor, "app_id": app, "chain_sequence": sequence, "code_id": code_id,
            "event_id": str(uuid.uuid4()), "kind": kind, "occurred_at": timestamp(occurred),
            "organization_id": organization, "outcome_digest": None, "previous_hash": previous,
            "record_id": None, "work_id": work, "writer_role": "operator"}
    digest = event_hash(previous, body)
    c.execute("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,
                 work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
                 VALUES (%s,%s,'operator',%s,%s,%s,%s,%s,NULL,%s,NULL,%s,%s,%s)""",
              (organization, body["event_id"], sequence, kind, actor, app, work, code_id, occurred, previous, digest))
    c.execute("UPDATE genui.audit_heads SET chain_sequence=%s,head_hash=%s WHERE organization_id=%s AND writer_role='operator'",
              (sequence, digest, organization))


def organization_sequence_hook(c, organization):
    """D7.7 TODO(APP-013, migration 0007): organization_sequences arrives with 0007.

    Until then there is nothing to create. When the relation exists this hook
    refuses, so provisioning and backfill cannot silently skip the row; 0007's
    slice must replace it with the D7.7 insert (next_commit_sequence = 1, or
    1 + max(internal_record_revisions.commit_sequence) for a backfill).
    """
    require(c.execute("SELECT to_regclass('genui.organization_sequences') IS NULL").fetchone() == (True,),
            "organization_sequences_hook_not_implemented")
    return "organization_sequences: not created; the relation arrives with migration 0007 (D7.7)"


# --- commands -----------------------------------------------------------------

def provision(c, args):
    org = args.organization
    begin(c, org)
    require(c.execute("SELECT count(*) FROM genui.organizations WHERE organization_id=%s", (org,)).fetchone() == (0,), "organization_exists")
    c.execute("INSERT INTO genui.organizations(organization_id,display_name,status) VALUES (%s,%s,'active')", (org, args.display_name))
    c.execute("INSERT INTO genui.provider_tenant_bindings(organization_id,issuer,provider_tenant_id,active) VALUES (%s,%s,%s,true)",
              (org, args.issuer, args.provider_tenant))
    for chain in CHAINS:
        c.execute("INSERT INTO genui.audit_heads(organization_id,writer_role,chain_sequence,head_hash) VALUES (%s,%s,1,%s)",
                  (org, chain, genesis(org, chain)))
    deferred = organization_sequence_hook(c, org)
    raw = secrets.token_bytes(DEFAULTS["handle_bytes"])
    c.execute("INSERT INTO genui_private.handle_index(handle_digest,organization_id,retired_at) VALUES (%s,%s,NULL)", (handle_digest(raw), org))
    lock_organization(c, org, active=True)
    append(c, org, "organization.provisioned")
    c.commit()
    # The handle is non-secret routing (ADR-006); it is still shown only here.
    return {"organization_id": org, "handle": encode_handle(raw), "audit_heads": list(CHAINS), "deferred": [deferred]}


def backfill(c, args):
    org = args.organization
    begin(c, org)
    lock_organization(c, org)
    created, existing = [], []
    for chain in CHAINS:
        if c.execute("SELECT 1 FROM genui.audit_heads WHERE organization_id=%s AND writer_role=%s", (org, chain)).fetchone():
            existing.append(chain)
        else:
            c.execute("INSERT INTO genui.audit_heads(organization_id,writer_role,chain_sequence,head_hash) VALUES (%s,%s,1,%s)",
                      (org, chain, genesis(org, chain)))
            created.append(chain)
    deferred = organization_sequence_hook(c, org)
    # D7.8's operator kinds are closed and name no backfill event; the genesis
    # heads themselves are the record (verify recomputes them).
    c.commit()
    return {"organization_id": org, "created": created, "existing": existing, "deferred": [deferred]}


def issue(c, args):
    org = args.organization
    invited = args.provider_email is not None
    client = None
    begin(c, org)
    if invited:
        # After the operator identity proof and before any lock or write: the
        # management key's tenant must be the code's bound, invite-only provider
        # tenant (GET /manage/config, read-only), so a wrong key never creates an
        # invitation in another tenant.
        client = provider.Client(args.provider_config)
        client.require_tenant(args.provider_tenant)
    lock_organization(c, org, active=True)
    if invited:
        # The binding row lock serializes invitation-bearing issuance per provider tenant,
        # and against a running reconcile (whose guard holds it, review M1(b)).
        require(lock_binding(c, org, args.provider_issuer, args.provider_tenant) == (True,), "provider_binding_refused")
        # fAuth's create revokes every earlier pending invitation for the same
        # address (management.rs:1722-1733): a second one would silently disable a
        # live code's invitation, so it waits until that code no longer admits.
        require(c.execute("""SELECT count(*) FROM genui.provider_invitations p JOIN genui.access_codes a
              ON a.organization_id=p.organization_id AND a.code_id=p.code_id
              WHERE p.organization_id=%s AND p.issuer=%s AND p.provider_tenant_id=%s AND p.intended_email_digest=%s
              AND p.operation='create' AND p.state<>'failed' AND a.status='active' AND a.expires_at>now()
              AND NOT EXISTS (SELECT 1 FROM genui.provider_invitations r WHERE r.organization_id=p.organization_id
                              AND r.code_id=p.code_id AND r.operation='revoke')""",
                          (org, args.provider_issuer, args.provider_tenant, email_digest(args.provider_email))).fetchone() == (0,),
                "provider_invitation_exists")
    # Pre-minted (APP-078 managed channel): only the digest is known here and
    # the secret is never printed; otherwise draw it now and print it once.
    raw = None if args.code_digest is not None else secrets.token_bytes(DEFAULTS["code_secret_bytes"])
    digest = args.code_digest if raw is None else code_digest(raw)
    code_id = str(uuid.uuid4())
    c.execute("INSERT INTO genui_private.code_index(code_digest,organization_id) VALUES (%s,%s)", (digest, org))
    c.execute("""INSERT INTO genui.access_codes(organization_id,code_id,code_digest,kind,status,destination_organization_id,max_redemptions,
                 redeemed_count,issued_at,expires_at,trial_days,issued_by,revoked_at)
                 VALUES (%s,%s,%s,%s,'active',%s,%s,0,now(),now()+make_interval(days=>%s),%s,%s,NULL)""",
              (org, code_id, digest, args.kind, org, args.max_redemptions, args.expires_days,
               DEFAULTS["trial_entitlement_days"], "genui_migration:" + args.issued_by))
    for permission, app, work, source in args.templates:
        c.execute("""INSERT INTO genui.code_grant_templates(organization_id,code_id,permission,app_id,work_id,source_id)
                     VALUES (%s,%s,%s,%s,%s,%s)""", (org, code_id, permission, app, work, source))
    invitation_id = str(uuid.uuid4())
    if invited:
        # The durable record of the intended create precedes any remote call.
        c.execute("""INSERT INTO genui.provider_invitations(organization_id,invitation_id,issuer,provider_tenant_id,remote_invitation_id,
                     intended_email_digest,provider_role,delivery_mode,operation,state,code_id,created_at,updated_at)
                     VALUES (%s,%s,%s,%s,NULL,%s,%s,'server_only','create','pending',%s,now(),now())""",
                  (org, invitation_id, args.provider_issuer, args.provider_tenant, email_digest(args.provider_email),
                   args.provider_role, code_id))
    append(c, org, "code.issued", code_id=code_id)
    expires = c.execute("SELECT expires_at FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (org, code_id)).fetchone()[0]
    c.commit()
    # Only after the commit succeeded: the one and only appearance of the secret
    # (printed by main once the provider phase below has finished).
    result = {"code_id": code_id, "kind": args.kind, "max_redemptions": args.max_redemptions, "expires_at": timestamp(expires)}
    if raw is None:
        result["code_digest"] = digest
    else:
        result["code"] = encode_code(raw)
    if invited:
        # Review m5: the code committed above is printed whatever happens in the provider phase,
        # including an interrupt; an unrecorded outcome is left to reconcile.
        try:
            result["provider_invitation"] = create_remote(c, org, invitation_id, code_id, args.provider_email, args.provider_role, client)
        except (Exception, KeyboardInterrupt):
            _rollback(c)
            result["provider_invitation"] = operation_result(invitation_id, "create", None, None, "provider_phase_incomplete",
                                                             capability_stored=None)
    return result


def lock_binding(c, org, issuer, tenant):
    import psycopg
    try:
        return c.execute("""SELECT active FROM genui.provider_tenant_bindings WHERE organization_id=%s AND issuer=%s
              AND provider_tenant_id=%s FOR UPDATE""", (org, issuer, tenant)).fetchone()
    except psycopg.errors.LockNotAvailable:
        raise Refused("provider_tenant_busy") from None


def list_codes(c, args):
    org = args.organization
    begin(c, org)
    rows = c.execute("""SELECT a.code_id::text,a.kind,CASE WHEN a.status='active' AND a.expires_at<=now() THEN 'expired' ELSE a.status END,
          a.max_redemptions,a.redeemed_count,
          (SELECT count(*) FROM genui.code_reservations r WHERE r.organization_id=a.organization_id AND r.code_id=a.code_id
            AND r.kind='pending' AND r.state='live' AND r.expires_at>now()),
          (SELECT count(*) FROM genui.code_reservations r WHERE r.organization_id=a.organization_id AND r.code_id=a.code_id
            AND r.kind='presignup' AND r.state='live' AND r.expires_at>now()),
          a.issued_at,a.expires_at,a.revoked_at,
          -- Review m3(b), ADR-006:43: this listing is the reconciliation record for
          -- pending admissions that lapsed with no later sign-in (still 'live'
          -- in storage, never counted) and for those already denied.
          (SELECT count(*) FROM genui.code_reservations r WHERE r.organization_id=a.organization_id AND r.code_id=a.code_id
            AND r.kind='pending' AND r.state='live' AND r.expires_at<=now()),
          (SELECT count(*) FROM genui.code_reservations r WHERE r.organization_id=a.organization_id AND r.code_id=a.code_id
            AND r.kind='pending' AND r.state='denied_for_reconciliation')
        FROM genui.access_codes a WHERE a.organization_id=%s ORDER BY a.issued_at DESC,a.code_id LIMIT %s""",
                     (org, DEFAULTS["list_rows_max"] + 1)).fetchall()
    invitations = {}
    for invitation, code, operation, state, remote in c.execute("""SELECT invitation_id::text,code_id::text,operation,state,remote_invitation_id
          FROM genui.provider_invitations WHERE organization_id=%s ORDER BY created_at,operation,invitation_id""", (org,)).fetchall():
        invitations.setdefault(code, []).append({"invitation_id": invitation, "operation": operation, "state": state,
                                                 "remote_invitation_id": remote})
    c.commit()
    codes = [{"code_id": r[0], "kind": r[1], "status": r[2], "max_redemptions": r[3], "redeemed_count": r[4],
              "live_pending_admissions": r[5], "lapsed_pending_admissions": r[10], "denied_for_reconciliation": r[11],
              "live_reservations": r[6], "issued_at": timestamp(r[7]),
              "expires_at": timestamp(r[8]), "revoked_at": timestamp(r[9]) if r[9] else None,
              "provider_invitations": invitations.get(r[0], [])} for r in rows[:DEFAULTS["list_rows_max"]]]
    return {"organization_id": org, "codes": codes, "truncated": len(rows) > DEFAULTS["list_rows_max"]}


def lock_code(c, org, code_id):
    lock_organization(c, org)
    row = c.execute("""SELECT status,expires_at>now(),issued_at FROM genui.access_codes WHERE organization_id=%s AND code_id=%s FOR UPDATE""",
                    (org, code_id)).fetchone()
    require(row is not None, "code_unknown")
    return row


def revoke(c, args):
    org, code_id = args.organization, args.code_id
    begin(c, org)
    status, _, _ = lock_code(c, org, code_id)
    if status == "revoked":
        c.commit()
        return {"code_id": code_id, "status": "revoked", "changed": False, "provider_revocations_pending": 0}
    c.execute("UPDATE genui.access_codes SET status='revoked',revoked_at=now() WHERE organization_id=%s AND code_id=%s", (org, code_id))
    # ADR-003: code revocation stops new handoff now and requires the remote
    # revocation of its provider invitation; record that operation as pending.
    revocations = record_revocations(c, org, code_id, lock=False)  # the code row is already held
    append(c, org, "code.revoked", code_id=code_id)
    c.commit()
    result = {"code_id": code_id, "status": "revoked", "changed": True, "provider_revocations_pending": len(revocations)}
    # The local revocation above is final whatever happens remotely; a failed or
    # unknown remote revocation never restores it.
    config = args.provider_config
    result.update(provider_phase="not_configured", capabilities_destroyed=None, provider_revocations=[])
    if config is None:
        return result
    try:
        result["capabilities_destroyed"] = destroy_code_capabilities(c, org, code_id, config.capability_dir)
        result["provider_revocations"], result["provider_phase"] = [], "completed"
        if revocations:
            client = provider.Client(config)
            try:
                client.require_tenant(revocations[0][1])
            except Refused as refusal:
                result["provider_phase"] = str(refusal)
                return result
            result["provider_revocations"] = [process_revoke(c, org, invitation, client) for invitation, _ in revocations]
    except Exception:
        _rollback(c)
        result["provider_phase"] = "incomplete"
    return result


def extend(c, args):
    org, code_id = args.organization, args.code_id
    begin(c, org)
    status, live, _ = lock_code(c, org, code_id)
    # Only a live, active code is extended; an expired, revoked or exhausted
    # code is reissued instead, so expiry never resurrects a code.
    require(status == "active" and live, "code_not_extendable")
    later, within = c.execute("""SELECT now()+make_interval(days=>%s)>expires_at,now()+make_interval(days=>%s)<=issued_at+interval '30 days'
          FROM genui.access_codes WHERE organization_id=%s AND code_id=%s""", (args.expires_days, args.expires_days, org, code_id)).fetchone()
    # Review m5: extension only ever moves the expiry later; shortening is revocation's job.
    require(later, "expiry_not_extended")
    require(within, "expiry_beyond_issuance_maximum")
    c.execute("UPDATE genui.access_codes SET expires_at=now()+make_interval(days=>%s) WHERE organization_id=%s AND code_id=%s",
              (args.expires_days, org, code_id))
    append(c, org, "code.extended", code_id=code_id)
    expires = c.execute("SELECT expires_at FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (org, code_id)).fetchone()[0]
    c.commit()
    return {"code_id": code_id, "changed": True, "expires_at": timestamp(expires)}


def lock_member(c, org, principal):
    lock_organization(c, org)
    row = c.execute("SELECT status,membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR UPDATE",
                    (org, principal)).fetchone()
    require(row is not None, "member_unknown")
    return row


def revoke_entitlement(c, args):
    """Entitlement revocation: org FOR SHARE, member FOR UPDATE, status, epoch + 1, one event."""
    org, principal = args.organization, args.principal
    begin(c, org)
    _, epoch = lock_member(c, org, principal)
    count = c.execute("""UPDATE genui.entitlements SET status='revoked',revoked_at=now()
          WHERE organization_id=%s AND principal_id=%s AND status='active'""", (org, principal)).rowcount
    if count == 0:
        c.commit()
        return {"principal_id": principal, "changed": False, "entitlements_revoked": 0, "membership_epoch": epoch}
    epoch = c.execute("""UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s
          RETURNING membership_epoch""", (org, principal)).fetchone()[0]
    append(c, org, "entitlement.revoked", actor=principal)
    c.commit()
    return {"principal_id": principal, "changed": True, "entitlements_revoked": count, "membership_epoch": epoch}


def revoke_membership(c, args):
    org, principal = args.organization, args.principal
    begin(c, org)
    status, epoch = lock_member(c, org, principal)
    if status == "revoked":
        c.commit()
        return {"principal_id": principal, "changed": False, "membership_epoch": epoch}
    epoch = c.execute("""UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1
          WHERE organization_id=%s AND principal_id=%s RETURNING membership_epoch""", (org, principal)).fetchone()[0]
    append(c, org, "membership.revoked", actor=principal)
    c.commit()
    return {"principal_id": principal, "changed": True, "membership_epoch": epoch}


def lock_active_member(c, org, principal):
    """Organization (active) FOR SHARE, then the member FOR UPDATE; the member must be active."""
    lock_organization(c, org, active=True)
    status, epoch = lock_member(c, org, principal)
    require(status == "active", "member_inactive")
    return epoch


def permission_statements(args):
    """The fixed (exists, insert, delete) statements of one closed permission; never caller SQL."""
    org, principal, permission = args.organization, args.principal, args.permission
    resource = GRANTABLE[permission]
    if resource == "app":
        return (("SELECT 1 FROM genui.apps WHERE organization_id=%s AND app_id=%s", (org, args.app)),
                ("""INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission) VALUES (%s,%s,%s,%s)
                    ON CONFLICT DO NOTHING""", (org, principal, args.app, permission)),
                ("""DELETE FROM genui.app_permissions WHERE organization_id=%s AND principal_id=%s AND app_id=%s AND permission=%s""",
                 (org, principal, args.app, permission)))
    if resource == "work":
        return (("SELECT 1 FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s", (org, args.app, args.work)),
                ("""INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission) VALUES (%s,%s,%s,%s,%s)
                    ON CONFLICT DO NOTHING""", (org, principal, args.app, args.work, permission)),
                ("""DELETE FROM genui.work_permissions WHERE organization_id=%s AND principal_id=%s AND app_id=%s AND work_id=%s
                    AND permission=%s""", (org, principal, args.app, args.work, permission)))
    return (("SELECT 1 FROM genui.sources WHERE organization_id=%s AND source_id=%s", (org, args.source)),
            ("""INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission) VALUES (%s,%s,%s,%s)
                ON CONFLICT DO NOTHING""", (org, principal, args.source, permission)),
            ("""DELETE FROM genui.source_permissions WHERE organization_id=%s AND principal_id=%s AND source_id=%s AND permission=%s""",
             (org, principal, args.source, permission)))


def grant_permission(c, args):
    """ADR-007 amendment (APP-010 review M2): org FOR SHARE, member FOR UPDATE, the row, one event.

    Adding authority advances no epoch; an existing grant changes nothing and is not audited again.
    """
    org, principal = args.organization, args.principal
    begin(c, org)
    epoch = lock_active_member(c, org, principal)
    exists, insert, _ = permission_statements(args)
    require(c.execute(*exists).fetchone() is not None, "resource_unknown")
    changed = c.execute(*insert).rowcount == 1
    if changed:
        append(c, org, "permission.granted", actor=principal, app=args.app, work=args.work)
    c.commit()
    return {"principal_id": principal, "permission": args.permission, "changed": changed, "membership_epoch": epoch}


def revoke_permission(c, args):
    """Revocation (D7.1 order): org FOR SHARE, member FOR UPDATE, delete, membership_epoch + 1, one event."""
    org, principal = args.organization, args.principal
    begin(c, org)
    _, epoch = lock_member(c, org, principal)
    _, _, delete = permission_statements(args)
    if c.execute(*delete).rowcount == 0:
        c.commit()
        return {"principal_id": principal, "permission": args.permission, "changed": False, "membership_epoch": epoch}
    epoch = c.execute("""UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s
          RETURNING membership_epoch""", (org, principal)).fetchone()[0]
    append(c, org, "permission.revoked", actor=principal, app=args.app, work=args.work)
    c.commit()
    return {"principal_id": principal, "permission": args.permission, "changed": True, "membership_epoch": epoch}


def create_app(c, args):
    """An app shell and its owner's edit_definition grant in one transaction.

    D7.8 names no operator app-creation kind, so the owner's `permission.granted`
    event (with the app id) is the audited record of the creation.
    """
    org, app, owner = args.organization, args.app, args.owner
    begin(c, org)
    lock_active_member(c, org, owner)
    require(c.execute("SELECT count(*) FROM genui.apps WHERE organization_id=%s AND app_id=%s", (org, app)).fetchone() == (0,), "app_exists")
    require(c.execute("SELECT 1 FROM genui.spaces WHERE organization_id=%s AND space_id=%s", (org, args.space)).fetchone() is not None,
            "space_unknown")
    c.execute("INSERT INTO genui.apps(organization_id,app_id,space_id,owner_principal_id) VALUES (%s,%s,%s,%s)", (org, app, args.space, owner))
    c.execute("INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission) VALUES (%s,%s,%s,'edit_definition')",
              (org, owner, app))
    append(c, org, "permission.granted", actor=owner, app=app)
    c.commit()
    return {"app_id": app, "owner_principal_id": owner, "granted": ["edit_definition"]}


def revoke_creator_authority(c, args):
    """ADR-007 amendment (APP-010 review M2): ends a saved work's creator authority.

    ADR-004 order: org FOR SHARE, the creator's membership FOR UPDATE, then the
    work's creator_revoked_at, membership_epoch + 1 and one event. The creator
    column is immutable (no application UPDATE), so reading it first is safe.
    """
    org, app, work = args.organization, args.app, args.work
    begin(c, org)
    lock_organization(c, org)
    row = c.execute("SELECT created_by::text FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s",
                    (org, app, work)).fetchone()
    require(row is not None, "work_unknown")
    require(row[0] is not None, "creator_unknown")
    creator = row[0]
    _, epoch = lock_member(c, org, creator)
    result = {"work_id": work, "creator_principal_id": creator, "changed": False, "membership_epoch": epoch}
    if c.execute("""UPDATE genui.saved_work SET creator_revoked_at=now() WHERE organization_id=%s AND app_id=%s AND work_id=%s
          AND creator_revoked_at IS NULL""", (org, app, work)).rowcount == 0:
        c.commit()
        return result
    epoch = c.execute("""UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s
          RETURNING membership_epoch""", (org, creator)).fetchone()[0]
    append(c, org, "creator_authority.revoked", actor=creator, app=app, work=work)
    c.commit()
    return dict(result, changed=True, membership_epoch=epoch)


def register_source_revision(c, args):
    """One new immutable revision row with real digests; never a rewrite (ADR-009 D9.3).

    The envelope is the source's existing revision envelope with the same
    contract (its input contract, parser, mapping, format and time semantics),
    carrying the new identity, digests, row count and creation time; it must
    validate as a `source_revision` and reproduce the record's contract digest.
    """
    contracts = str(runtime.Path(runtime.__file__).resolve().parents[2] / "contracts")
    if contracts not in sys.path:
        sys.path.insert(0, contracts)
    from validate import canonical as contract_canonical, digest as contract_digest, validate as contract_validate
    record, org = args.revision_record, args.organization
    begin(c, org)
    lock_organization(c, org, active=True)
    require(c.execute("SELECT 1 FROM genui.sources WHERE organization_id=%s AND source_id=%s",
                      (org, record["source_id"])).fetchone() is not None, "source_unknown")
    existing = c.execute("""SELECT content_digest::text,normalized_digest::text,contract_digest::text,deleted_at IS NULL
          FROM genui.source_revisions WHERE organization_id=%s AND source_id=%s AND revision_id=%s""",
                         (org, record["source_id"], record["revision_id"])).fetchone()
    result = {"organization_id": org, "source_id": record["source_id"], "revision_id": record["revision_id"],
              "normalized_digest": record["normalized_digest"], "contract_digest": record["contract_digest"]}
    if existing is not None:
        require(tuple(existing) == (record["content_digest"], record["normalized_digest"], record["contract_digest"], True),
                "revision_exists_differs")
        c.commit()
        return dict(result, changed=False)
    template = c.execute("""SELECT canonical_envelope FROM genui.source_revisions WHERE organization_id=%s AND source_id=%s
          AND contract_digest=%s AND deleted_at IS NULL ORDER BY revision_id LIMIT 1""",
                         (org, record["source_id"], record["contract_digest"])).fetchone()
    require(template is not None, "contract_unknown")
    envelope = json.loads(bytes(template[0]))
    created = c.execute("SELECT now()").fetchone()[0]
    envelope.update({"revision_id": record["revision_id"], "content_digest": record["content_digest"],
                     "normalized_digest": record["normalized_digest"], "row_count": record["row_count"],
                     "created_at": timestamp(created)})
    try:
        contract_validate(envelope)
    except Exception:
        raise Refused("envelope_refused") from None
    require(envelope["organization_id"] == org and envelope["source_id"] == record["source_id"], "envelope_refused")
    require(contract_digest("input-contract", envelope["input_contract"]) == record["contract_digest"], "contract_refused")
    c.execute("""INSERT INTO genui.source_revisions(organization_id,source_id,revision_id,content_digest,normalized_digest,
          contract_digest,deleted_at,canonical_envelope) VALUES (%s,%s,%s,%s,%s,%s,NULL,%s)""",
              (org, record["source_id"], record["revision_id"], record["content_digest"], record["normalized_digest"],
               record["contract_digest"], contract_canonical(envelope)))
    c.commit()
    return dict(result, changed=True, row_count=record["row_count"], created_at=envelope["created_at"])


def create_bound_work(c, args):
    """A new saved work (version 1) rebinding one source input; nothing is rewritten.

    It copies `--from-work`'s publication, context and every binding, except
    that `--input` binds `--revision` of the same source, which must be live
    and carry the same contract digest. Each published record schema gets a
    new, empty collection. The creator must be an active member holding `run`
    on the app and `source_read` on every bound source, as the workspace's own
    `create_work` requires; the work records them as its creator (ADR-007
    amendment, review M2), so no work grant is written. A rerun with the same
    bindings is `unchanged`; any other existing work id is refused.
    """
    org, app, work, origin, creator = args.organization, args.app, args.work, args.from_work, args.creator
    begin(c, org)
    lock_active_member(c, org, creator)
    row = c.execute("SELECT publication_id::text,context FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s",
                    (org, app, origin)).fetchone()
    require(row is not None, "work_unknown")
    publication, context = row
    bindings = c.execute("""SELECT input_id::text,kind,contract_digest::text,source_id::text,source_revision_id::text,source_digest::text,
          schema_id::text,schema_version FROM genui.work_bindings WHERE organization_id=%s AND app_id=%s AND work_id=%s ORDER BY input_id""",
                         (org, app, origin)).fetchall()
    target = [b for b in bindings if b[0] == args.input]
    require(len(target) == 1 and target[0][1] == "source" and target[0][3] == args.source, "binding_refused")
    revision = c.execute("""SELECT normalized_digest::text FROM genui.source_revisions WHERE organization_id=%s AND source_id=%s
          AND revision_id=%s AND contract_digest=%s AND deleted_at IS NULL""", (org, args.source, args.revision, target[0][2])).fetchone()
    require(revision is not None, "revision_refused")
    planned = []
    for input_id, kind, contract, source, source_revision, source_digest, schema, version in bindings:
        if input_id == args.input:
            source_revision, source_digest = args.revision, revision[0]
        planned.append((input_id, kind, contract, source, source_revision, source_digest, schema, version))
    sources = sorted({b[3] for b in planned if b[1] == "source"})
    require(c.execute("SELECT 1 FROM genui.app_permissions WHERE organization_id=%s AND principal_id=%s AND app_id=%s AND permission='run'",
                      (org, creator, app)).fetchone() is not None, "creator_lacks_authority")
    for source in sources:
        require(c.execute("""SELECT 1 FROM genui.source_permissions WHERE organization_id=%s AND principal_id=%s AND source_id=%s
              AND permission='source_read'""", (org, creator, source)).fetchone() is not None, "creator_lacks_authority")
    result = {"work_id": work, "publication_id": publication, "version": 1, "creator_principal_id": creator,
              "bindings": [{"input_id": b[0], "kind": b[1], "source_id": b[3], "source_revision_id": b[4], "source_digest": b[5]}
                           for b in planned]}
    existing = c.execute("SELECT publication_id::text,created_by::text FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s",
                         (org, app, work)).fetchone()
    if existing is not None:
        current = c.execute("""SELECT input_id::text,source_id::text,source_revision_id::text,source_digest::text FROM genui.work_bindings
              WHERE organization_id=%s AND app_id=%s AND work_id=%s AND kind='source' ORDER BY input_id""", (org, app, work)).fetchall()
        require(tuple(existing) == (publication, creator)
                and [tuple(r) for r in current] == [(b[0], b[3], b[4], b[5]) for b in planned if b[1] == "source"], "work_exists")
        c.commit()
        return dict(result, changed=False)
    c.execute("""INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by)
          VALUES (%s,%s,%s,%s,1,%s,%s)""", (org, app, work, publication, context, creator))
    collections = {}
    for schema, version in c.execute("""SELECT schema_id::text,schema_version FROM genui.publication_record_schemas
          WHERE organization_id=%s AND app_id=%s AND publication_id=%s ORDER BY schema_id,schema_version""", (org, app, publication)).fetchall():
        collections[(schema, version)] = str(uuid.uuid4())
        c.execute("""INSERT INTO genui.record_collections(organization_id,app_id,work_id,publication_id,collection_id,schema_id,schema_version)
              VALUES (%s,%s,%s,%s,%s,%s,%s)""", (org, app, work, publication, collections[(schema, version)], schema, version))
    for input_id, kind, contract, source, source_revision, source_digest, schema, version in planned:
        if kind == "source":
            c.execute("""INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,
                  source_id,source_revision_id,source_digest) VALUES (%s,%s,%s,%s,%s,'source',%s,%s,%s,%s)""",
                      (org, app, work, publication, input_id, contract, source, source_revision, source_digest))
            c.execute("""INSERT INTO genui.work_source_dependencies(organization_id,app_id,work_id,input_id,source_id,source_revision_id,
                  source_digest) VALUES (%s,%s,%s,%s,%s,%s,%s)""", (org, app, work, input_id, source, source_revision, source_digest))
        else:
            require((schema, version) in collections, "binding_refused")
            c.execute("""INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,
                  collection_id,schema_id,schema_version,revision_policy) VALUES (%s,%s,%s,%s,%s,'records',%s,%s,%s,%s,'committed_snapshot')""",
                      (org, app, work, publication, input_id, contract, collections[(schema, version)], schema, version))
    c.commit()
    return dict(result, changed=True)


def verify(c, args):
    """SR10: recompute every chain of the organization in chain_sequence order; the tail equals the head."""
    import psycopg
    org = args.organization
    # Review m1: every head and event read shares one snapshot, so an admission
    # appended concurrently is either wholly visible or wholly absent; verify
    # writes and locks nothing.
    c.isolation_level = psycopg.IsolationLevel.REPEATABLE_READ
    c.read_only = True
    begin(c, org)
    lock_organization(c, org, lock=False)
    chains = {}
    for chain in CHAINS:
        head = c.execute("SELECT chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s AND writer_role=%s", (org, chain)).fetchone()
        require(head is not None, "audit_chain_not_provisioned")
        previous, sequence = genesis(org, chain), 1
        events = c.execute("""SELECT organization_id::text,event_id::text,writer_role,chain_sequence,kind,actor_principal_id::text,app_id::text,
              work_id::text,record_id::text,code_id::text,outcome_digest::text,occurred_at,previous_hash,hash FROM genui.audit_events
              WHERE organization_id=%s AND writer_role=%s ORDER BY chain_sequence""", (org, chain)).fetchall()
        names = ("organization_id", "event_id", "writer_role", "chain_sequence", "kind", "actor_principal_id", "app_id", "work_id",
                 "record_id", "code_id", "outcome_digest", "occurred_at", "previous_hash")
        for row in events:
            body = dict(zip(names, row[:13]))
            require(row[3] == sequence + 1 and row[12] == previous and body["occurred_at"].microsecond == 0, "audit_chain_mismatch")
            body["occurred_at"] = timestamp(body["occurred_at"])
            computed = event_hash(previous, body)
            require(computed == row[13], "audit_chain_mismatch")
            previous, sequence = computed, row[3]
        require(head == (sequence, previous), "audit_chain_mismatch")
        chains[chain] = {"events": len(events), "head_sequence": sequence}
    # Review m5: the dropped code_redemptions.audit_event_id foreign key is
    # covered here: every redemption names its own admission.redeemed event.
    orphans = c.execute("""SELECT count(*) FROM genui.code_redemptions d WHERE d.organization_id=%s AND NOT EXISTS (
          SELECT 1 FROM genui.audit_events e WHERE e.organization_id=d.organization_id AND e.event_id=d.audit_event_id
          AND e.writer_role='admission' AND e.kind='admission.redeemed' AND e.code_id=d.code_id
          AND e.actor_principal_id=d.principal_id AND e.outcome_digest=d.payload_digest)""", (org,)).fetchone()[0]
    require(orphans == 0, "redemption_audit_mismatch")
    c.commit()
    return {"organization_id": org, "verified": True, "chains": chains}


# --- provider invitation operations (APP-073 acceptance 6 and 7) -------------------
#
# State machine of genui.provider_invitations (0003): a `create` row is written
# `pending` in the code's own transaction; a `revoke` row is written `pending`
# when the code is revoked, or when reconcile finds the code expired or
# exhausted, or when a create proves unusable. Each remote call runs while its
# row is held FOR UPDATE (SKIP LOCKED everywhere else), and its outcome is
# recorded in the same short transaction. A create moves once to `confirmed`
# (with the remote id), `failed` (definitively not created) or `unknown`; it is
# never sent again. Reconcile judges pending/unknown creates only by inspecting
# the provider's invitation list, and only once they are older than the grace
# period. Revocation is idempotent upstream, so pending/unknown/failed revokes
# are simply sent again. Provider operation state is not an audit event: D7.8's
# operator kinds are closed and name none (review m8: needs an ADR-007 amendment
# and a migration).
#
# `updated_at` of a pending create is its send deadline once create_remote has
# recorded it (committed before the request leaves); after an outcome it is the
# outcome's time. Either way no request for the row reaches the provider after
# it, which bounds reconcile's attribution window above (review M1).

def _rollback(c):
    try:
        c.rollback()
    except Exception:
        pass


def _close(c):
    """Every command commits its own work; whatever is left open is discarded."""
    _rollback(c)
    try:
        c.close()
    except Exception:
        pass


def record_revocations(c, org, code_id, lock=True):
    """One pending `revoke` per non-failed create of the code, unless one exists; (invitation, tenant) pairs.

    Every writer of revoke rows holds the code row FOR UPDATE first (D7.1 order:
    organization FOR SHARE, then the code), so NOT EXISTS cannot race."""
    if lock:
        lock_code(c, org, code_id)
    return [(r[0], r[1]) for r in c.execute("""INSERT INTO genui.provider_invitations(organization_id,invitation_id,issuer,provider_tenant_id,
          remote_invitation_id,intended_email_digest,provider_role,delivery_mode,operation,state,code_id,created_at,updated_at)
          SELECT organization_id,gen_random_uuid(),issuer,provider_tenant_id,remote_invitation_id,intended_email_digest,provider_role,
          delivery_mode,'revoke','pending',code_id,now(),now() FROM genui.provider_invitations p
          WHERE p.organization_id=%s AND p.code_id=%s AND p.operation='create' AND p.state<>'failed'
          AND NOT EXISTS (SELECT 1 FROM genui.provider_invitations r WHERE r.organization_id=p.organization_id AND r.code_id=p.code_id
                          AND r.operation='revoke')
          RETURNING invitation_id::text,provider_tenant_id""", (org, code_id)).fetchall()]


def destroy_code_capabilities(c, org, code_id, directory):
    """Revocation ends the private capability locally first, whatever the provider answers."""
    begin(c, org)
    invitations = [r[0] for r in c.execute("""SELECT invitation_id::text FROM genui.provider_invitations
          WHERE organization_id=%s AND code_id=%s AND operation='create'""", (org, code_id)).fetchall()]
    c.commit()
    return sum(1 for invitation in invitations if provider.destroy_capability(directory, org, invitation))


def operation_result(invitation_id, operation, state, remote, detail, **extra):
    return dict({"invitation_id": invitation_id, "operation": operation, "state": state, "remote_invitation_id": remote,
                 "detail": detail}, **extra)


def open_revocations(c, org, code_id):
    """The code's revoke rows that still need a remote call."""
    begin(c, org)
    rows = [r[0] for r in c.execute("""SELECT invitation_id::text FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s
          AND operation='revoke' AND state IN ('pending','unknown','failed') ORDER BY created_at,invitation_id""", (org, code_id)).fetchall()]
    c.commit()
    return rows


def create_remote(c, org, invitation_id, code_id, email, role, client):
    """Record the send deadline, send the one create while holding the row, record the outcome.

    Review M1: the deadline (`updated_at` of a pending create, committed before anything is
    sent) closes reconcile's attribution window above: no request for this row can reach the
    provider after it. Review m1: the outcome transaction locks the code (D7.1: organization,
    then the code) and writes the capability only if the code can still admit and no
    revocation is recorded; otherwise the invitation is revoked at once. It takes those locks
    while holding its own invitation row, outside D7.1's stated order: no writer holds a code
    row and then waits on a create row, so there is no wait cycle (review n7). Review m5:
    nothing after the outcome's COMMIT can stop the committed code from being printed."""
    limits = provider.PROVIDER_DEFAULTS
    directory = client.config.capability_dir
    margin = limits["create_send_margin_seconds"]
    try:
        begin(c, org)
        held = c.execute("""SELECT 1 FROM genui.provider_invitations WHERE organization_id=%s AND invitation_id=%s
              AND operation='create' AND state='pending' AND updated_at=created_at FOR UPDATE SKIP LOCKED""", (org, invitation_id)).fetchone()
        if held is None:
            _rollback(c)
            return operation_result(invitation_id, "create", "pending", None, "busy", capability_stored=False)
        started = time.monotonic()
        # The request's own deadline starts within `margin` of `started`, so it ends before this one.
        c.execute("""UPDATE genui.provider_invitations SET updated_at=clock_timestamp()+make_interval(secs=>%s)
              WHERE organization_id=%s AND invitation_id=%s""", (limits["request_deadline_seconds"] + margin, org, invitation_id))
        c.commit()
        begin(c, org)
        held = c.execute("""SELECT 1 FROM genui.provider_invitations WHERE organization_id=%s AND invitation_id=%s
              AND operation='create' AND state='pending' FOR UPDATE SKIP LOCKED""", (org, invitation_id)).fetchone()
        if held is None:
            _rollback(c)
            return operation_result(invitation_id, "create", "pending", None, "busy", capability_stored=False)
        if time.monotonic() - started > margin:
            # Too late to fit inside the recorded deadline: never sent, so definitively not created.
            c.execute("""UPDATE genui.provider_invitations SET state='failed',updated_at=clock_timestamp()
                  WHERE organization_id=%s AND invitation_id=%s""", (org, invitation_id))
            c.commit()
            return operation_result(invitation_id, "create", "failed", None, "send_window_passed", capability_stored=False)
        outcome = client.create(email, role)
        status, live, _ = lock_code(c, org, code_id)
        revoking = c.execute("""SELECT count(*) FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s
              AND operation='revoke'""", (org, code_id)).fetchone()[0]
        stored, detail = False, outcome.detail
        if outcome.usable and not (status == "active" and live and revoking == 0):
            detail = "code_no_longer_usable"  # revoked, expired or exhausted while the create was in flight
        elif outcome.usable:
            # The file precedes the database update: a crash in between leaves a
            # pending row that reconcile confirms from the file. It records the
            # code's stored digest so the service's invitation entry (APP-074)
            # can find this capability for a validated code.
            digest = c.execute("SELECT code_digest FROM genui.access_codes WHERE organization_id=%s AND code_id=%s",
                               (org, code_id)).fetchone()[0]
            try:
                provider.write_capability(directory, org, invitation_id, outcome.remote_id, outcome.token, timestamp(outcome.expires_at),
                                          code_digest=digest)
                stored = True
            except Refused:
                detail = "capability_unwritable"
        c.execute("""UPDATE genui.provider_invitations SET state=%s,remote_invitation_id=%s,updated_at=clock_timestamp()
              WHERE organization_id=%s AND invitation_id=%s""", (outcome.state, outcome.remote_id, org, invitation_id))
        if outcome.state == "confirmed" and not stored:
            # It exists remotely but can never be handed off: record its revocation
            # (unless the code's revocation already is); the operator revokes or keeps the code.
            record_revocations(c, org, code_id, lock=False)  # the code row is already held
        c.commit()
    except Exception:
        _rollback(c)
        return operation_result(invitation_id, "create", "pending", None, "outcome_unrecorded",
                                capability_stored=provider.capability_present(directory, org, invitation_id))
    result = operation_result(invitation_id, "create", outcome.state, outcome.remote_id, detail, capability_stored=stored)
    if outcome.provider_error:
        result["provider_error"] = outcome.provider_error
    if outcome.state == "confirmed" and not stored:
        try:
            sent = [process_revoke(c, org, revocation, client) for revocation in open_revocations(c, org, code_id)]
            result["revocation"] = sent[0] if sent else operation_result(None, "revoke", None, outcome.remote_id, "settled")
        except Exception:
            # The revocation row is committed; reconcile sends it.
            _rollback(c)
            result["revocation"] = operation_result(None, "revoke", "pending", outcome.remote_id, "provider_phase_incomplete")
    return result


def process_revoke(c, org, invitation_id, client):
    """Send (again) the revocation of one revoke row's remote invitation while holding the row."""
    begin(c, org)
    row = c.execute("""SELECT state,code_id::text FROM genui.provider_invitations WHERE organization_id=%s AND invitation_id=%s
          AND operation='revoke' AND state IN ('pending','unknown','failed') FOR UPDATE SKIP LOCKED""", (org, invitation_id)).fetchone()
    if row is None:
        state = c.execute("""SELECT state FROM genui.provider_invitations WHERE organization_id=%s AND invitation_id=%s""",
                          (org, invitation_id)).fetchone()
        c.commit()
        return operation_result(invitation_id, "revoke", state[0] if state else None, None, "busy" if state and state[0] != "confirmed" else "settled")
    state_before, code_id = row
    create = c.execute("""SELECT state,remote_invitation_id FROM genui.provider_invitations WHERE organization_id=%s AND code_id=%s
          AND operation='create'""", (org, code_id)).fetchone()
    if create is None or create[0] == "failed":
        c.execute("""UPDATE genui.provider_invitations SET state='confirmed',updated_at=now() WHERE organization_id=%s AND invitation_id=%s""",
                  (org, invitation_id))
        c.commit()
        return operation_result(invitation_id, "revoke", "confirmed", None, "nothing_to_revoke")
    if create[0] != "confirmed":
        c.commit()
        return operation_result(invitation_id, "revoke", state_before, None, "awaiting_create_reconciliation")
    try:
        outcome = client.revoke(create[1])
    except Refused:
        _rollback(c)
        return operation_result(invitation_id, "revoke", state_before, create[1], "provider_contract_mismatch")
    c.execute("""UPDATE genui.provider_invitations SET state=%s,remote_invitation_id=%s,updated_at=now()
          WHERE organization_id=%s AND invitation_id=%s""", (outcome.state, create[1], org, invitation_id))
    c.commit()
    result = operation_result(invitation_id, "revoke", outcome.state, create[1], outcome.detail)
    if outcome.provider_error:
        result["provider_error"] = outcome.provider_error
    return result


def inspect(scan, client, not_before):
    """The provider's invitations created at or after `not_before`: one list scan per reconcile,
    repeated only when an older bound is needed. None once the scan has failed."""
    if "error" in scan:
        return None
    if "invites" in scan and scan["not_before"] <= not_before:
        return scan["invites"]
    not_before = min(not_before, scan.get("want", not_before))
    try:
        scan["invites"], scan["not_before"] = client.scan(not_before), not_before
    except Refused as refusal:
        scan["error"] = str(refusal)
        return None
    return scan["invites"]


def judge_creates(c, org, client, tenant, scan):
    """Settle pending/unknown creates past the grace; never by creating again (review M1, m2, m3).

    Every held row is judged together, while held (FOR UPDATE SKIP LOCKED), under reconcile's
    tenant guard:
    1. a row whose capability file exists takes its remote id from the file, which was
       written from a 201 that met the contract (M1(d));
    2. a row whose recorded deadline is more than the provider's invitation lifetime (72 h,
       GenUI never sends `expires_in_hours`, gatewasm management.rs:1715) plus the skew in the
       past is closed `failed` / `expired_unconfirmed`: nothing it made can still be pending (m3);
    3. otherwise only the provider's list decides. A remote invitation is eligible when no
       operation already holds its id and it was created through the management API
       (`invited_by_user_id` null, M1(c)). Each unresolved create of the tenant has a window
       [created_at - skew, deadline + skew], where the deadline is `updated_at` (the send
       deadline or the recorded outcome) and is open above only for a create not held here that
       has not recorded one yet. A row is `confirmed` with the one eligible invitation in its
       window only if no other unresolved create's window holds that invitation (one-to-one,
       M1(a)); `failed` (`not_found`) if its window holds none; otherwise it stays as it was
       (`ambiguous_remote_match`).
    A create confirmed without its capability can never be handed off, so its revocation is
    recorded at once (m2), and sent with the other revocations."""
    limits = provider.PROVIDER_DEFAULTS
    skew = datetime.timedelta(seconds=limits["reconcile_clock_skew_seconds"])
    lifetime = limits["provider_invitation_hours_default"] * 3600 + limits["reconcile_clock_skew_seconds"]
    directory = client.config.capability_dir
    begin(c, org)
    held = c.execute("""SELECT invitation_id::text,state,intended_email_digest,provider_role,created_at,updated_at,code_id::text,
          updated_at<=now()-make_interval(secs=>%s) FROM genui.provider_invitations
          WHERE organization_id=%s AND provider_tenant_id=%s AND operation='create' AND state IN ('pending','unknown')
          AND updated_at<=now()-make_interval(secs=>%s) ORDER BY created_at,invitation_id LIMIT %s FOR UPDATE SKIP LOCKED""",
                     (lifetime, org, tenant, limits["reconcile_grace_seconds"], limits["reconcile_operations_max"])).fetchall()
    if not held:
        _rollback(c)
        return []
    known = {r[0] for r in c.execute("""SELECT remote_invitation_id FROM genui.provider_invitations
          WHERE organization_id=%s AND remote_invitation_id IS NOT NULL""", (org,)).fetchall()}
    decided, waiting = {}, {}
    for invitation, _, _, _, _, _, _, lapsed in held:
        record = provider.capability_record(directory, org, invitation)
        if record is not None:
            decided[invitation] = ("confirmed", record["remote_invitation_id"], "capability_record")
            known.add(record["remote_invitation_id"])
        elif lapsed:
            decided[invitation] = ("failed", None, "expired_unconfirmed")
    rest = [r for r in held if r[0] not in decided]
    if rest:
        invites = inspect(scan, client, min(r[4] for r in rest) - skew)
        if invites is None:
            waiting = {r[0]: scan["error"] for r in rest}
        else:
            held_ids = {r[0] for r in held}
            unresolved = [u for u in c.execute("""SELECT invitation_id::text,intended_email_digest,provider_role,created_at,updated_at
                  FROM genui.provider_invitations WHERE organization_id=%s AND provider_tenant_id=%s AND operation='create'
                  AND state IN ('pending','unknown')""", (org, tenant)).fetchall() if u[0] not in decided]

            def holds(row, invite):
                upper = row[4] + skew if row[0] in held_ids or row[4] != row[3] else None
                return (email_digest(invite.email) == row[1] and invite.role_name == row[2] and invite.created_at >= row[3] - skew
                        and (upper is None or invite.created_at <= upper))
            eligible = [i for i in invites if i.remote_id not in known and i.invited_by is None]
            for row in rest:
                matches = [i for i in eligible if holds((row[0],) + row[2:6], i)]
                if not matches:
                    decided[row[0]] = ("failed", None, "not_found")
                elif len(matches) == 1 and [u[0] for u in unresolved if holds(u, matches[0])] == [row[0]]:
                    decided[row[0]] = ("confirmed", matches[0].remote_id, "found")
                else:
                    waiting[row[0]] = "ambiguous_remote_match"
    for invitation, (state, remote, _) in decided.items():
        c.execute("""UPDATE genui.provider_invitations SET state=%s,remote_invitation_id=%s,updated_at=now()
              WHERE organization_id=%s AND invitation_id=%s""", (state, remote, org, invitation))
    c.commit()
    entries, unusable = [], []
    for invitation, state_before, _, _, _, _, code_id, _ in held:
        if invitation in waiting:
            entries.append(operation_result(invitation, "create", state_before, None, waiting[invitation], state_before=state_before,
                                            capability_stored=False))
            continue
        state, remote, detail = decided[invitation]
        stored = state == "confirmed" and provider.capability_present(directory, org, invitation)
        if state == "confirmed" and not stored:
            unusable.append(code_id)
        entries.append(operation_result(invitation, "create", state, remote, detail, state_before=state_before, capability_stored=stored))
    for code_id in unusable:
        begin(c, org)
        record_revocations(c, org, code_id)
        c.commit()
    return entries


def ended_remotely(c, org, client, tenant, scan):
    """Review m7: confirmed invitations of codes that can still admit, which fAuth already ended.

    Revoked (another create for the address, the console) or expired (its sweeper) upstream, or
    older than the provider lifetime: the revocation is recorded `confirmed` (no pending
    invitation with that id remains, the same postcondition as a 404 on revoke) and the
    capability ends. Accepted (the invitee registered): the token is spent, so the capability
    ends; the ledger keeps the confirmed create."""
    limits = provider.PROVIDER_DEFAULTS
    skew = datetime.timedelta(seconds=limits["reconcile_clock_skew_seconds"])
    lifetime = limits["provider_invitation_hours_default"] * 3600 + limits["reconcile_clock_skew_seconds"]
    directory = client.config.capability_dir
    begin(c, org)
    rows = c.execute("""SELECT p.invitation_id::text,p.code_id::text,p.remote_invitation_id,p.created_at,
          p.updated_at<=now()-make_interval(secs=>%s) FROM genui.provider_invitations p JOIN genui.access_codes a
          ON a.organization_id=p.organization_id AND a.code_id=p.code_id
          WHERE p.organization_id=%s AND p.provider_tenant_id=%s AND p.operation='create' AND p.state='confirmed'
          AND p.remote_invitation_id IS NOT NULL AND a.status='active' AND a.expires_at>now()
          AND NOT EXISTS (SELECT 1 FROM genui.provider_invitations r WHERE r.organization_id=p.organization_id
                          AND r.code_id=p.code_id AND r.operation='revoke')
          ORDER BY p.created_at,p.invitation_id LIMIT %s""", (lifetime, org, tenant, limits["reconcile_operations_max"])).fetchall()
    c.commit()
    ended, spent = {}, []
    recent = [r for r in rows if not r[4]]
    invites = inspect(scan, client, min(r[3] for r in recent) - skew) if recent else []
    statuses = {i.remote_id: i.status for i in invites or []}
    for invitation, code_id, remote, _, lapsed in rows:
        if lapsed:
            ended[invitation] = (code_id, remote, "provider_lifetime_ended")
        elif statuses.get(remote) in ("revoked", "expired"):
            ended[invitation] = (code_id, remote, "remote_" + statuses[remote])
        elif statuses.get(remote) == "accepted":
            spent.append((invitation, remote))
    entries, destroyed = [], 0
    for invitation, (code_id, remote, detail) in ended.items():
        begin(c, org)
        recorded = record_revocations(c, org, code_id)
        for revocation, _ in recorded:
            c.execute("""UPDATE genui.provider_invitations SET state='confirmed',updated_at=now() WHERE organization_id=%s AND invitation_id=%s""",
                      (org, revocation))
        c.commit()
        destroyed += provider.destroy_capability(directory, org, invitation)
        entries += [operation_result(revocation, "revoke", "confirmed", remote, detail, state_before=None) for revocation, _ in recorded]
    for invitation, remote in spent:
        if provider.destroy_capability(directory, org, invitation):
            destroyed += 1
            entries.append(operation_result(invitation, "create", "confirmed", remote, "remote_accepted", state_before="confirmed",
                                            capability_stored=False))
    return entries, destroyed


def sweep_capabilities(c, org, directory):
    """End every capability that can no longer be handed off: its code is revoked, expired or
    exhausted, a revocation is recorded, its create failed or is unknown to the ledger, or the
    provider's own invitation expiry has passed. A partial write older than the grace is a
    writer that stopped before linking it (review m6); younger ones are left to their writer."""
    begin(c, org)
    now, destroyed = datetime.datetime.now(datetime.timezone.utc), 0
    destroyed += provider.remove_stale_partials(directory, org, provider.PROVIDER_DEFAULTS["reconcile_grace_seconds"])
    for invitation in provider.capability_invitations(directory, org):
        row = c.execute("""SELECT p.state<>'failed' AND a.status='active' AND a.expires_at>now() AND NOT EXISTS (
              SELECT 1 FROM genui.provider_invitations r WHERE r.organization_id=p.organization_id AND r.code_id=p.code_id
              AND r.operation='revoke') FROM genui.provider_invitations p JOIN genui.access_codes a
              ON a.organization_id=p.organization_id AND a.code_id=p.code_id
              WHERE p.organization_id=%s AND p.invitation_id=%s AND p.operation='create'""", (org, invitation)).fetchone()
        expiry = provider.capability_expiry(directory, org, invitation)
        if not (row == (True,) and expiry is not None and expiry > now) and provider.destroy_capability(directory, org, invitation):
            destroyed += 1
    c.commit()
    return destroyed


def reconcile_guard(org):
    """Review M1(b): one reconcile at a time per organization and its provider tenant.

    A second connection, after the same identity and catalog proof, holds the organization's
    provider_tenant_bindings rows FOR NO KEY UPDATE NOWAIT for the whole run. That conflicts
    with another reconcile's guard and with `issue`'s FOR UPDATE of the binding (which then
    refuses `provider_tenant_busy`), but not with the FOR KEY SHARE that foreign-key checks
    take (pending admissions and invitation rows reference the binding), so admission and this
    run's own writes proceed. NOWAIT: the guard never waits, so it is in no wait cycle."""
    import psycopg
    guard = connect()
    try:
        begin(guard, org)
        guard.execute("SELECT 1 FROM genui.provider_tenant_bindings WHERE organization_id=%s FOR NO KEY UPDATE NOWAIT", (org,)).fetchall()
    except psycopg.errors.LockNotAvailable:
        _close(guard)
        raise Refused("reconcile_in_progress") from None
    except BaseException:
        _close(guard)
        raise
    return guard


def reconcile(c, args):
    guard = reconcile_guard(args.organization)
    try:
        return _reconcile(c, args.organization, args.provider_config)
    finally:
        _close(guard)


def _reconcile(c, org, config):
    limits = provider.PROVIDER_DEFAULTS
    client = provider.Client(config)
    # 1. Propagate code expiry, exhaustion and any revocation without a recorded
    #    remote revocation (D7.1 order: organization FOR SHARE, then the codes).
    begin(c, org)
    lock_organization(c, org)
    codes = c.execute("""SELECT a.code_id::text FROM genui.access_codes a WHERE a.organization_id=%s
          AND (a.status<>'active' OR a.expires_at<=now())
          AND EXISTS (SELECT 1 FROM genui.provider_invitations p WHERE p.organization_id=a.organization_id AND p.code_id=a.code_id
                      AND p.operation='create' AND p.state<>'failed')
          AND NOT EXISTS (SELECT 1 FROM genui.provider_invitations p WHERE p.organization_id=a.organization_id AND p.code_id=a.code_id
                          AND p.operation='revoke')
          ORDER BY a.code_id LIMIT %s FOR UPDATE OF a""", (org, limits["reconcile_operations_max"])).fetchall()
    recorded = sum(len(record_revocations(c, org, code, lock=False)) for (code,) in codes)
    c.commit()
    # 2. End capabilities locally before any provider call.
    destroyed = sweep_capabilities(c, org, config.capability_dir)
    # 3. Inventory: what needs the provider.
    begin(c, org)
    # Operations past the provider lifetime are settled without inspection, so they never widen the scan.
    lifetime = limits["provider_invitation_hours_default"] * 3600 + limits["reconcile_clock_skew_seconds"]
    creates = c.execute("""SELECT provider_tenant_id,updated_at<=now()-make_interval(secs=>%s),created_at,
          updated_at<=now()-make_interval(secs=>%s) FROM genui.provider_invitations
          WHERE organization_id=%s AND operation='create' AND state IN ('pending','unknown') ORDER BY created_at,invitation_id LIMIT %s""",
                        (limits["reconcile_grace_seconds"], lifetime, org, limits["reconcile_operations_max"] + 1)).fetchall()
    revokes = c.execute("""SELECT count(*) FROM genui.provider_invitations WHERE organization_id=%s AND operation='revoke'
          AND state IN ('pending','unknown','failed')""", (org,)).fetchone()[0]
    live, recent = c.execute("""SELECT count(*),min(p.created_at) FILTER (WHERE p.updated_at>now()-make_interval(secs=>%s))
          FROM genui.provider_invitations p JOIN genui.access_codes a
          ON a.organization_id=p.organization_id AND a.code_id=p.code_id WHERE p.organization_id=%s AND p.operation='create'
          AND p.state='confirmed' AND a.status='active' AND a.expires_at>now() AND NOT EXISTS (SELECT 1 FROM genui.provider_invitations r
          WHERE r.organization_id=p.organization_id AND r.code_id=p.code_id AND r.operation='revoke')""", (lifetime, org)).fetchone()
    c.commit()
    truncated = len(creates) > limits["reconcile_operations_max"] or revokes > limits["reconcile_operations_max"]
    creates = creates[:limits["reconcile_operations_max"]]
    result = {"organization_id": org, "revocations_recorded": recorded, "capabilities_destroyed": destroyed, "operations": [],
              "deferred": sum(1 for r in creates if not r[1]), "truncated": truncated, "provider_phase": "completed",
              "capability_directory_class": provider.directory_class(str(config.capability_dir))}
    ready = [r for r in creates if r[1]]
    if not ready and not revokes and not live:
        return result
    try:
        tenant = client.tenant()
    except Refused as refusal:
        result["provider_phase"] = str(refusal)
        result["deferred"] += len(ready)
        return result
    skew = datetime.timedelta(seconds=limits["reconcile_clock_skew_seconds"])
    bounds = [r[2] for r in ready if r[0] == tenant and not r[3]] + ([recent] if recent is not None else [])
    scan = {"want": min(bounds) - skew} if bounds else {}
    # 4. Creates first, so a revocation can follow a confirmed create.
    judged = judge_creates(c, org, client, tenant, scan)
    result["operations"] += judged
    result["deferred"] += max(0, len(ready) - len(judged))
    # 5. Confirmed invitations the provider already ended (review m7).
    ended, swept = ended_remotely(c, org, client, tenant, scan)
    result["operations"] += ended
    result["capabilities_destroyed"] += swept
    # 6. Revocations (idempotent upstream), including those recorded above.
    begin(c, org)
    rows = c.execute("""SELECT invitation_id::text,provider_tenant_id,state FROM genui.provider_invitations
          WHERE organization_id=%s AND operation='revoke' AND state IN ('pending','unknown','failed')
          ORDER BY created_at,invitation_id LIMIT %s""", (org, limits["reconcile_operations_max"])).fetchall()
    c.commit()
    for invitation, provider_tenant, state_before in rows:
        if provider_tenant != tenant:
            result["deferred"] += 1
            continue
        entry = process_revoke(c, org, invitation, client)
        if entry["detail"] in ("busy", "settled"):
            result["deferred"] += 1
        else:
            result["operations"].append(dict(entry, state_before=state_before))
    if "error" in scan:
        result["provider_phase"] = scan["error"]
    return result


# --- sign-in handles (ADR-006 §handle; D7.2, D7.19; APP-009) ----------------------
#
# A handle is 16 CSPRNG bytes shown as 26 lowercase base32 characters; only its
# domain-separated digest is stored, in genui_private.handle_index. It is
# non-secret routing (it appears in https://beta.getforma.dev/signin/{handle}
# and in fAuth's client return_entry_url) and grants nothing. Every handle
# operation locks the organization FOR SHARE and then the organization's
# operator audit head FOR UPDATE, which serializes handle changes per
# organization, and appends one `handle.rotated` event (D7.8's only handle kind).

def live_handles(c, org):
    return c.execute("""SELECT count(*) FROM genui_private.handle_index WHERE organization_id=%s
          AND (retired_at IS NULL OR retired_at>now())""", (org,)).fetchone()[0]


def lock_handles(c, org):
    lock_organization(c, org, active=True)
    require(c.execute("SELECT 1 FROM genui.audit_heads WHERE organization_id=%s AND writer_role='operator' FOR UPDATE",
                      (org,)).fetchone() is not None, "audit_chain_not_provisioned")


def add_handle(c, org, raw):
    raw = raw if raw is not None else secrets.token_bytes(DEFAULTS["handle_bytes"])
    require(c.execute("SELECT count(*) FROM genui_private.handle_index WHERE handle_digest=%s",
                      (handle_digest(raw),)).fetchone() == (0,), "handle_taken")
    c.execute("INSERT INTO genui_private.handle_index(handle_digest,organization_id,retired_at) VALUES (%s,%s,NULL)",
              (handle_digest(raw), org))
    append(c, org, "handle.rotated")
    return raw


def issue_handle(c, args):
    """The first live handle of an organization that has none (one created before
    handles existed, such as the two fixture organizations). `--handle` installs an
    exact, already-issued handle, so one routing value can be registered with fAuth
    once and installed in each environment; otherwise a fresh handle is drawn."""
    org = args.organization
    begin(c, org)
    lock_handles(c, org)
    require(live_handles(c, org) == 0, "handle_exists")
    raw = add_handle(c, org, args.raw_handle)
    c.commit()
    return {"organization_id": org, "handle": encode_handle(raw), "live_handles": 1}


def rotate_handle(c, args):
    """Rotation step 1: a new live handle beside the current one; both route
    until FormaStack re-registers return_entry_url and `retire-handle` runs."""
    org = args.organization
    begin(c, org)
    lock_handles(c, org)
    live = live_handles(c, org)
    require(live == 1, "handle_missing" if live == 0 else "rotation_in_progress")
    raw = add_handle(c, org, args.raw_handle)
    c.commit()
    return {"organization_id": org, "handle": encode_handle(raw), "live_handles": 2}


def retire_handle(c, args):
    """Rotation step 2: the named handle stops routing; never the last live one."""
    org = args.organization
    begin(c, org)
    lock_handles(c, org)
    row = c.execute("""SELECT organization_id::text,retired_at IS NULL OR retired_at>now() FROM genui_private.handle_index
          WHERE handle_digest=%s FOR UPDATE""", (handle_digest(args.raw_handle),)).fetchone()
    require(row is not None and row[0] == org, "handle_unknown")
    require(row[1], "handle_retired")
    live = live_handles(c, org)
    require(live >= 2, "last_live_handle")
    c.execute("UPDATE genui_private.handle_index SET retired_at=now() WHERE handle_digest=%s", (handle_digest(args.raw_handle),))
    append(c, org, "handle.rotated")
    c.commit()
    return {"organization_id": org, "retired": True, "live_handles": live - 1}


COMMANDS = {"provision": provision, "backfill": backfill, "issue": issue, "list": list_codes, "revoke": revoke,
            "extend": extend, "revoke-entitlement": revoke_entitlement, "revoke-membership": revoke_membership, "verify": verify,
            "reconcile": reconcile, "grant-permission": grant_permission, "revoke-permission": revoke_permission,
            "create-app": create_app, "revoke-creator-authority": revoke_creator_authority,
            "issue-handle": issue_handle, "rotate-handle": rotate_handle, "retire-handle": retire_handle,
            "register-source-revision": register_source_revision, "create-bound-work": create_bound_work}


def provider_configuration(args):
    """Validated before any connection: required to create or reconcile, optional to revoke."""
    if args.command == "issue" and args.provider_email is not None or args.command == "reconcile":
        return provider.load_config(os.environ, required=True)
    if args.command == "revoke":
        return provider.load_config(os.environ, required=False)
    return None


def main(argv=None):
    try:
        args = validate(parser().parse_args(argv))
        args.provider_config = provider_configuration(args)
        if args.command == "defaults":
            result = dict(DEFAULTS, provider=dict(provider.PROVIDER_DEFAULTS))
        else:
            # Not `with connect()`: its exit COMMITs and would raise on a broken
            # connection after `issue` committed, hiding the code (review m5).
            connection = connect()
            try:
                result = COMMANDS[args.command](connection, args)
            finally:
                _close(connection)
    except Refused as refusal:
        print(json.dumps({"refused": str(refusal)}), file=sys.stderr)
        return 1
    except Exception:
        print(json.dumps({"refused": "admission_operator_refused"}), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
