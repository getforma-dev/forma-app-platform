"""Operator-only admission channel (ADR-007 D7.19, D7.1, D7.7, D7.8, D7.17; APP-073).

Never an HTTP, application or signup path. Closed subcommands only; no generic
SQL. Every command connects as the migration identity (session_user and
current_user must be genui_migration) over verify-full TLS using only the
libpq-style environment (PGHOST, PGPORT, PGUSER, PGPASSWORD, PGSSLROOTCERT,
PGDATABASE=genui), proves the reviewed latest catalog, then acts as
genui_owner under explicit organization scope in D7.1's lock order
(organization FOR SHARE, then the member or code row FOR UPDATE, then the
operator audit head) and appends to the `operator` audit chain.

A new access code's secret (32 CSPRNG bytes, base32) is printed exactly once,
on standard output, after its transaction commits. It is never logged, stored
or written anywhere else; only its domain-separated digest is stored. Listing
and audit show identifiers, status and counts only. Refusals print a fixed
reason code on standard error and never echo an argument value.
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
import uuid

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
OPERATOR_KINDS = ("organization.provisioned", "handle.rotated", "code.issued", "code.revoked", "code.extended",
                  "membership.revoked", "entitlement.revoked")
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


class Refused(ValueError):
    """A refusal whose message is a fixed reason code, never a value."""


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
    for name in ("backfill", "list", "verify"):
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
    return p


def validate(args):
    """Every argument is checked before any connection is opened."""
    if hasattr(args, "organization"):
        identifier(args.organization)
    for name in ("code_id", "principal"):
        if hasattr(args, name):
            identifier(getattr(args, name))
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
        if args.provider_email is not None:
            email = args.provider_email.strip()
            require(3 <= len(email) <= 320 and email.count("@") == 1 and email.isascii() and email.isprintable(), "malformed_email")
            require(args.provider_role in PROVIDER_ROLES, "provider_role_refused")
            require(1 <= len(args.provider_issuer) <= 512 and 1 <= len(args.provider_tenant) <= 256, "malformed_provider_tenant")
    if args.command == "extend":
        require(1 <= args.expires_days <= DEFAULTS["code_expiry_days_max"], "expiry_out_of_range")
    return args


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


def append(c, organization, kind, code_id=None, actor=None):
    """D7.8 append on the operator chain: the head row lock serializes appenders."""
    require(kind in OPERATOR_KINDS, "kind_refused")
    head = c.execute("SELECT chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s AND writer_role='operator' FOR UPDATE",
                     (organization,)).fetchone()
    require(head is not None, "audit_chain_not_provisioned")
    sequence, previous = head[0] + 1, head[1]
    occurred = c.execute("SELECT date_trunc('second',now())").fetchone()[0]
    body = {"actor_principal_id": actor, "app_id": None, "chain_sequence": sequence, "code_id": code_id,
            "event_id": str(uuid.uuid4()), "kind": kind, "occurred_at": timestamp(occurred),
            "organization_id": organization, "outcome_digest": None, "previous_hash": previous,
            "record_id": None, "work_id": None, "writer_role": "operator"}
    digest = event_hash(previous, body)
    c.execute("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,
                 work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
                 VALUES (%s,%s,'operator',%s,%s,%s,NULL,NULL,NULL,%s,NULL,%s,%s,%s)""",
              (organization, body["event_id"], sequence, kind, actor, code_id, occurred, previous, digest))
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
    begin(c, org)
    lock_organization(c, org, active=True)
    raw = secrets.token_bytes(DEFAULTS["code_secret_bytes"])
    code_id = str(uuid.uuid4())
    c.execute("INSERT INTO genui_private.code_index(code_digest,organization_id) VALUES (%s,%s)", (code_digest(raw), org))
    c.execute("""INSERT INTO genui.access_codes(organization_id,code_id,code_digest,kind,status,destination_organization_id,max_redemptions,
                 redeemed_count,issued_at,expires_at,trial_days,issued_by,revoked_at)
                 VALUES (%s,%s,%s,%s,'active',%s,%s,0,now(),now()+make_interval(days=>%s),%s,%s,NULL)""",
              (org, code_id, code_digest(raw), args.kind, org, args.max_redemptions, args.expires_days,
               DEFAULTS["trial_entitlement_days"], "genui_migration:" + args.issued_by))
    for permission, app, work, source in args.templates:
        c.execute("""INSERT INTO genui.code_grant_templates(organization_id,code_id,permission,app_id,work_id,source_id)
                     VALUES (%s,%s,%s,%s,%s,%s)""", (org, code_id, permission, app, work, source))
    if args.provider_email is not None:
        c.execute("""INSERT INTO genui.provider_invitations(organization_id,invitation_id,issuer,provider_tenant_id,remote_invitation_id,
                     intended_email_digest,provider_role,delivery_mode,operation,state,code_id,created_at,updated_at)
                     VALUES (%s,%s,%s,%s,NULL,%s,%s,'server_only','create','pending',%s,now(),now())""",
                  (org, str(uuid.uuid4()), args.provider_issuer, args.provider_tenant, email_digest(args.provider_email),
                   args.provider_role, code_id))
    append(c, org, "code.issued", code_id=code_id)
    expires = c.execute("SELECT expires_at FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (org, code_id)).fetchone()[0]
    c.commit()
    # Only after the commit succeeded: the one and only appearance of the secret.
    return {"code_id": code_id, "code": encode_code(raw), "kind": args.kind, "max_redemptions": args.max_redemptions,
            "expires_at": timestamp(expires)}


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
    for invitation, code, operation, state in c.execute("""SELECT invitation_id::text,code_id::text,operation,state FROM genui.provider_invitations
          WHERE organization_id=%s ORDER BY created_at,invitation_id""", (org,)).fetchall():
        invitations.setdefault(code, []).append({"invitation_id": invitation, "operation": operation, "state": state})
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
    pending = c.execute("""INSERT INTO genui.provider_invitations(organization_id,invitation_id,issuer,provider_tenant_id,remote_invitation_id,
          intended_email_digest,provider_role,delivery_mode,operation,state,code_id,created_at,updated_at)
          SELECT organization_id,gen_random_uuid(),issuer,provider_tenant_id,remote_invitation_id,intended_email_digest,provider_role,
          delivery_mode,'revoke','pending',code_id,now(),now() FROM genui.provider_invitations
          WHERE organization_id=%s AND code_id=%s AND operation='create' AND state<>'failed'""", (org, code_id)).rowcount
    append(c, org, "code.revoked", code_id=code_id)
    c.commit()
    return {"code_id": code_id, "status": "revoked", "changed": True, "provider_revocations_pending": pending}


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


COMMANDS = {"provision": provision, "backfill": backfill, "issue": issue, "list": list_codes, "revoke": revoke,
            "extend": extend, "revoke-entitlement": revoke_entitlement, "revoke-membership": revoke_membership, "verify": verify}


def main(argv=None):
    try:
        args = validate(parser().parse_args(argv))
        if args.command == "defaults":
            result = dict(DEFAULTS)
        else:
            with connect() as connection:
                result = COMMANDS[args.command](connection, args)
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
