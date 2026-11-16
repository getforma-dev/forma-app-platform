"""APP-073 operator command tests (ADR-007 D7.17, D7.19). No AWS/RDS use.

PureTests need nothing. DatabaseTests start one disposable PostgreSQL 16.15
container on a random loopback port with a throwaway TLS certificate, apply the
reviewed migrations and drive the real command line as genui_migration over
verify-full TLS, exactly as an operator would; they are skipped when Docker or
OpenSSL is absent. Every container they start is removed in tearDownClass.
"""
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import importlib.util
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
import uuid

import admission_operator as op
import runtime

HERE = Path(__file__).resolve().parent
IMAGE = "postgres@sha256:5db6dac2ef5dcff512f4f00a009c3c332b9f2f28f24277175bf3e4a3d02ef83a"
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

    def test_authoring_only_issue_is_explicit_and_never_combines_resource_grants(self):
        base = ["issue", "--organization", ORG, "--issued-by", "operator-test"]
        with self.assertRaisesRegex(op.Refused, "grant_required"):
            op.validate(op.parser().parse_args(base))
        parsed = op.validate(op.parser().parse_args(base + ["--authoring-only"]))
        self.assertEqual(parsed.templates, [])
        with self.assertRaisesRegex(op.Refused, "authoring_only_with_grants_refused"):
            op.validate(op.parser().parse_args(base + ["--authoring-only", "--grant", "discover:" + ORG]))

    def test_permission_commands_are_a_closed_set(self):
        """ADR-007 amendment (APP-010 review M2): grant/revoke take only the permissions the tables allow,
        never share_work or export, and each permission names exactly its resource. admin_policy is no
        permission since migration 0025 (ADR-021: the membership's role, set by set-role); record_review (W4, migration
        0024) names a work like record_read, and its grant is gated on schema 24."""
        app, work, source, principal = (str(uuid.uuid4()) for _ in range(4))
        parser = op.parser()
        base = ["--organization", ORG, "--principal", principal]
        self.assertEqual(op.GRANTABLE, {"discover": "app", "run": "app", "edit_definition": "app", "review_publish": "app",
                                        "record_read": "work", "record_write": "work", "record_review": "work", "source_read": "source"})
        self.assertEqual(op.GATED_PERMISSIONS, {"record_review": 24})
        self.assertEqual(op.OPERATOR_KINDS[:10], ("organization.provisioned", "handle.rotated", "code.issued", "code.revoked",
                                                  "code.extended", "membership.revoked", "entitlement.revoked",
                                                  "permission.granted", "permission.revoked", "creator_authority.revoked"))
        self.assertEqual(op.OPERATOR_KINDS[10:], ("provider.invitation.created", "provider.invitation.reconciled",
                                                  "provider.invitation.revoked", "source_revision.registered", "work.seeded",
                                                  "provider_tenant.bound", "app.seeded", "membership.role_changed",
                                                  "entitlement.extended", "member.erased"))
        for command in ("grant-permission", "revoke-permission"):
            for good in (["--permission", "run", "--app", app], ["--permission", "review_publish", "--app", app],
                         ["--permission", "record_write", "--app", app, "--work", work], ["--permission", "source_read", "--source", source],
                         ["--permission", "record_review", "--app", app, "--work", work]):
                with self.subTest(command=command, good=good[1]):
                    op.validate(parser.parse_args([command, *base, *good]))
            for bad in (["--permission", "admin_policy"], ["--permission", "admin_policy", "--app", app], ["--permission", "admin_policy", "--source", source],
                        ["--permission", "share_work", "--app", app, "--work", work],
                        ["--permission", "export", "--app", app, "--work", work], ["--permission", "owner", "--app", app],
                        ["--permission", "run"], ["--permission", "run", "--app", app, "--work", work],
                        ["--permission", "record_read", "--app", app], ["--permission", "record_review", "--app", app],
                        ["--permission", "record_review", "--source", source], ["--permission", "source_read", "--app", app],
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

    def test_issue_note_is_bounded_and_chooses_the_0023_insert(self):
        """The real-user trial (W3): --note is 1..200 printable bytes with no surrounding space; the INSERT
        carries the column only once 0023 exists, and the statement text itself is fixed."""
        parser = op.parser()
        base = ["issue", "--organization", ORG, "--issued-by", "unit", "--authoring-only"]
        self.assertIsNone(op.validate(parser.parse_args(base)).note)
        self.assertEqual(op.validate(parser.parse_args(base + ["--note", "Ada L. (wave 1)"])).note, "Ada L. (wave 1)")
        for bad in ("", " ", " Ada", "Ada ", "a\tb", "a\nb", "x" * 201, "é" * 101):
            with self.subTest(bad=bad[:8]):
                with self.assertRaises(op.Refused) as refused:
                    op.validate(parser.parse_args(base + ["--note", bad]))
                self.assertEqual(str(refused.exception), "malformed_note")
        self.assertEqual(op.NOTE_BYTES_MAX, 200)
        without, with_note = op.access_code_insert(False), op.access_code_insert(True)
        self.assertNotIn("note", without)
        self.assertEqual(without.count("%s"), 9)
        self.assertTrue(with_note.endswith(",note) VALUES (%s,%s,%s,%s,'active',%s,%s,0,now(),now()+make_interval(days=>%s),%s,%s,NULL,%s)"))
        self.assertEqual(with_note.count("%s"), 10)
        # The column probe reads pg_catalog for exactly genui.access_codes.note, never information_schema.
        class Probe:
            def __init__(self, row): self.row, self.statements = row, []
            def execute(self, statement, params=None): self.statements.append(statement); return self
            def fetchone(self): return self.row
        for row, expected in ((None, False), ((1,), True)):
            probe = Probe(row)
            self.assertIs(op.note_column_present(probe), expected)
            self.assertEqual(len(probe.statements), 1)
            for fragment in ("pg_catalog.pg_attribute", "nspname='genui'", "relname='access_codes'", "attname='note'", "attisdropped"):
                self.assertIn(fragment, probe.statements[0])
            self.assertNotIn("information_schema", probe.statements[0])

    def test_issue_with_a_note_binds_it_to_the_insert_and_the_result(self):
        """With the column present (mocked: the proof of the real schema is begin()'s), the note is the
        INSERT's last parameter and the printed result names it; without the column nothing is written."""
        import datetime as dt
        parser = op.parser()
        args = op.validate(parser.parse_args(["issue", "--organization", ORG, "--issued-by", "unit", "--authoring-only",
                                              "--code-digest", VECTOR_DIGEST, "--note", "Ada L."]))
        class Connection:
            def __init__(self): self.calls, self.committed = [], 0
            def execute(self, statement, params=None): self.calls.append((statement, params)); return self
            def fetchone(self): return (dt.datetime(2026, 12, 1, tzinfo=dt.timezone.utc),)
            def commit(self): self.committed += 1
        for present in (True, False):
            with self.subTest(present=present):
                c = Connection()
                with mock.patch.object(op, "begin"), mock.patch.object(op, "lock_organization"), \
                        mock.patch.object(op, "claim_provider_tenants"), mock.patch.object(op, "append"), \
                        mock.patch.object(op, "note_column_present", return_value=present), \
                        mock.patch.object(op, "role_column_present", return_value=False):
                    if not present:
                        with self.assertRaises(op.Refused) as refused:
                            op.issue(c, args)
                        self.assertEqual(str(refused.exception), "note_unsupported")
                        self.assertEqual((c.calls, c.committed), ([], 0))
                        continue
                    result = op.issue(c, args)
                inserts = [(s, p) for s, p in c.calls if s.startswith("INSERT INTO genui.access_codes")]
                self.assertEqual(len(inserts), 1)
                self.assertEqual(inserts[0][0], op.access_code_insert(True))
                self.assertEqual(inserts[0][1][-1], "Ada L.")
                self.assertEqual(inserts[0][1][2], VECTOR_DIGEST)
                self.assertEqual((result["note"], result["code_digest"], c.committed), ("Ada L.", VECTOR_DIGEST, 1))
                self.assertNotIn("code", result)

    def test_record_review_grant_is_gated_on_schema_24_and_revoke_is_not(self):
        """W4 review M1: the operator grants record_review only once migration 0024 widened the work_permissions
        CHECK (refusing permission_unsupported before any lock or write, like note_unsupported), and revokes
        it at any schema: the row goes, the epoch advances, permission.revoked is appended."""
        app, work, principal = (str(uuid.uuid4()) for _ in range(3))
        parser = op.parser()
        base = ["--organization", ORG, "--principal", principal, "--permission", "record_review", "--app", app, "--work", work]
        grant = op.validate(parser.parse_args(["grant-permission", *base]))
        revoke = op.validate(parser.parse_args(["revoke-permission", *base]))
        class Connection:
            def __init__(self, version, rowcount=1):
                self.version, self.calls, self.committed, self.rowcount = version, [], 0, rowcount
            def execute(self, statement, params=None):
                self.calls.append((statement, params)); self.last = statement; return self
            def fetchone(self):
                if "genui_meta.schema_version" in self.last: return (self.version,)
                if "RETURNING membership_epoch" in self.last: return (2,)
                return (1,)
            def commit(self): self.committed += 1
        appended = []
        with mock.patch.object(op, "begin"), mock.patch.object(op, "lock_active_member", return_value=1), \
                mock.patch.object(op, "lock_member", return_value=("active", 1)), \
                mock.patch.object(op, "append", side_effect=lambda c, org, kind, **k: appended.append((kind, k))):
            c = Connection(23)
            with self.assertRaises(op.Refused) as refused:
                op.grant_permission(c, grant)
            self.assertEqual(str(refused.exception), "permission_unsupported")
            self.assertEqual(c.committed, 0)
            self.assertEqual([s for s, _ in c.calls if "INSERT" in s or "FOR UPDATE" in s], [])
            self.assertEqual(appended, [])
            c = Connection(24)
            result = op.grant_permission(c, grant)
            inserts = [(s, p) for s, p in c.calls if s.strip().startswith("INSERT INTO genui.work_permissions")]
            self.assertEqual(len(inserts), 1)
            self.assertEqual(inserts[0][1], (ORG, principal, app, work, "record_review"))
            self.assertEqual((result["changed"], result["permission"], c.committed), (True, "record_review", 1))
            self.assertEqual(appended, [("permission.granted", {"actor": principal, "app": app, "work": work})])
            appended.clear()
            c = Connection(23)  # a revoke needs no gate
            result = op.revoke_permission(c, revoke)
            deletes = [(s, p) for s, p in c.calls if s.strip().startswith("DELETE FROM genui.work_permissions")]
            self.assertEqual(len(deletes), 1)
            self.assertEqual(deletes[0][1], (ORG, principal, app, work, "record_review"))
            self.assertTrue(any("membership_epoch=membership_epoch+1" in s for s, _ in c.calls))
            self.assertEqual((result["changed"], result["membership_epoch"], c.committed), (True, 2, 1))
            self.assertEqual(appended, [("permission.revoked", {"actor": principal, "app": app, "work": work})])
            self.assertFalse(any("genui_meta.schema_version" in s for s, _ in c.calls))

    def test_list_box_files_takes_two_identifiers(self):
        parser = op.parser()
        principal = str(uuid.uuid4())
        parsed = op.validate(parser.parse_args(["list-box-files", "--organization", ORG, "--principal", principal]))
        self.assertEqual((parsed.command, parsed.principal), ("list-box-files", principal))
        for bad in (["list-box-files", "--organization", ORG], ["list-box-files", "--organization", ORG, "--principal", "x"],
                    ["list-box-files", "--organization", ORG.upper(), "--principal", principal]):
            with self.subTest(bad=bad[-1]), self.assertRaises(op.Refused):
                op.validate(parser.parse_args(bad))
        self.assertIs(op.COMMANDS["list-box-files"], op.list_box_files)
        self.assertEqual(op.validate(parser.parse_args(["stats", "--organization", ORG])).command, "stats")
        with self.assertRaises(op.Refused):
            op.validate(parser.parse_args(["stats", "--organization", "x"]))
        self.assertIs(op.COMMANDS["stats"], op.stats)
        self.assertEqual(len(op.STATS_TABLES), len(set(op.STATS_TABLES)))

    def test_purge_order_is_children_first_and_the_kept_set_is_closed(self):
        self.assertEqual(op.deletion_order(["apps", "boxes", "box_files", "spaces"], [("boxes", "apps"), ("box_files", "boxes"), ("apps", "spaces")]),
                         ["box_files", "boxes", "apps", "spaces"])
        self.assertEqual(op.deletion_order(["a", "b"], []), ["a", "b"])  # independent tables, a stable order
        self.assertEqual(op.deletion_order(["a", "b"], [("a", "a"), ("b", "zz")]), ["a", "b"])  # self-references and foreign parents do not block
        with self.assertRaisesRegex(op.Refused, "purge_order_cycle"):
            op.deletion_order(["a", "b"], [("a", "b"), ("b", "a")])
        self.assertEqual(op.PURGE_KEEPS, {"organizations", "provider_tenant_bindings", "organization_sequences", "audit_heads", "audit_events"})
        self.assertEqual(op.PURGE_PRIVATE, ("code_index",))
        self.assertEqual(op.IDENTITY_TABLES, ("genui_identity.shared_auth_flows", "genui_identity.linked_apps", "genui_identity.share_events"))
        # APP-212: one declared gate (the GATED_PERMISSIONS pattern) names the guest table the purge deletes and
        # from which schema version; share_events has none and is never deleted.
        self.assertEqual(op.IDENTITY_GUEST_ROWS, ("genui_identity.linked_apps", "genui_identity.share_events"))
        self.assertEqual(op.GATED_PURGE_DELETIONS, {"genui_identity.linked_apps": 24})
        self.assertNotIn("genui_identity.share_events", op.GATED_PURGE_DELETIONS)
        # Children outside the purged set are named, never silently dropped from the order.
        self.assertEqual(op.foreign_children(["live_work_shares", "apps"], [("genui_identity.share_events", "live_work_shares"), ("apps", "spaces"),
                                                                          ("live_work_shares", "apps"), ("genui_identity.linked_apps", "live_work_shares")]),
                         ["genui_identity.linked_apps", "genui_identity.share_events"])
        self.assertFalse(set(op.STATS_TABLES) & op.PURGE_KEEPS)
        parser = op.parser()
        for name in ("suspend-organization", "purge-organization"):
            self.assertEqual(op.validate(parser.parse_args([name, "--organization", ORG])).command, name)
            with self.assertRaises(op.Refused):
                op.validate(parser.parse_args([name, "--organization", "x"]))
        self.assertIs(op.COMMANDS["purge-organization"], op.purge_organization)
        self.assertIs(op.COMMANDS["suspend-organization"], op.suspend_organization)

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

    def test_invitation_input_is_closed_and_bound_by_digest(self):
        """ADR-003 amendment 2026-10-30 (managed invitations): the injected input document and the managed form."""
        digest, other = op.code_digest(bytes(32)), op.code_digest(bytes([1] * 32))
        entry = {"organization_id": ORG, "code_digest": digest, "email": "Invitee@Example.test", "role": "user", "issuer": ISSUER, "tenant": TENANT}
        good = json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [entry]})
        self.assertEqual(op.invitation_input(good), [entry])
        self.assertEqual(op.invitation_input("{}"), [])
        for bad, reason in (("[]", "invitation_input_malformed"), ("{", "invitation_input_malformed"), ("x" * 70000, "invitation_input_malformed"),
                            (json.dumps({"format": "other", "invitations": [entry]}), "invitation_input_malformed"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT}), "invitation_input_malformed"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [entry], "extra": 1}), "invitation_input_malformed"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, label="ns")]}), "invitation_input_malformed"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, code_digest="sha256:" + "A" * 64)]}), "malformed_code_digest"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, role="admin")]}), "provider_role_refused"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, email="not-an-address")]}), "malformed_email"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, organization_id="x")]}), "malformed_identifier"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [entry, dict(entry, email="b@example.test")]}), "invitation_input_malformed"),
                            ('{"format":"%s","format":"%s","invitations":[]}' % (op.INVITATION_INPUT_FORMAT, op.INVITATION_INPUT_FORMAT), "invitation_input_malformed"),
                            (json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, code_digest=op.code_digest(bytes([i] * 32)))
                                                                                                for i in range(65)]}), "invitation_input_malformed")):
            with self.subTest(bad=bad[:40]), self.assertRaises(op.Refused) as refused:
                op.invitation_input(bad)
            self.assertEqual(str(refused.exception), reason)
            self.assertNotIn("Invitee", str(refused.exception))
        parser = op.parser()
        base = ["issue", "--organization", ORG, "--issued-by", "unit", "--grant", "source_read:" + str(uuid.uuid4())]
        managed = op.validate(parser.parse_args(base + ["--code-digest", digest, "--provider-input-from-secret"]))
        self.assertTrue(managed.provider_input_from_secret)
        for extra, reason in ((["--provider-input-from-secret"], "invitation_input_requires_code_digest"),
                              (["--code-digest", digest, "--provider-input-from-secret", "--provider-email", "a@example.test"], "invitation_input_with_flags_refused"),
                              (["--code-digest", digest, "--provider-input-from-secret", "--provider-tenant", TENANT], "invitation_input_with_flags_refused"),
                              (["--code-digest", digest, "--provider-email", "a@example.test", "--provider-role", "user", "--provider-issuer", ISSUER,
                                "--provider-tenant", TENANT], "code_digest_with_invitation_refused")):
            with self.subTest(extra=extra[:3]), self.assertRaises(op.Refused) as refused:
                op.validate(parser.parse_args(base + extra))
            self.assertEqual(str(refused.exception), reason)
        # Binding takes exactly this organization's entry for this digest, or refuses before any connection.
        bound = op.bind_invitation_input(op.validate(parser.parse_args(base + ["--code-digest", digest, "--provider-input-from-secret"])),
                                         {op.INVITATION_INPUT_ENV: good})
        self.assertEqual((bound.provider_email, bound.provider_role, bound.provider_issuer, bound.provider_tenant),
                         ("Invitee@Example.test", "user", ISSUER, TENANT))
        for environ in ({}, {op.INVITATION_INPUT_ENV: "{}"},
                        {op.INVITATION_INPUT_ENV: json.dumps({"format": op.INVITATION_INPUT_FORMAT, "invitations": [dict(entry, code_digest=other)]})},
                        {op.INVITATION_INPUT_ENV: json.dumps({"format": op.INVITATION_INPUT_FORMAT,
                                                              "invitations": [dict(entry, organization_id="00000000-0000-4000-8000-0000000000a2")]})}):
            with self.subTest(environ=sorted(environ)), self.assertRaises(op.Refused) as refused:
                op.bind_invitation_input(op.validate(parser.parse_args(base + ["--code-digest", digest, "--provider-input-from-secret"])), environ)
            self.assertEqual(str(refused.exception), "invitation_input_missing")

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


def apply_through(c, through):
    """The runner's own steps up to migration `through`, for a test that then applies the next one
    (as app/runtime/generate_schema_catalog.py builds each catalog); the ledger and version row end at
    `through`, so runtime.apply_migrations resumes from there."""
    sources = runtime.migration_sources(runtime.migration_directory())
    c.execute("SET LOCAL search_path=pg_catalog, pg_temp")
    c.execute(sources[0][2])
    for version, digest, source in sources[1:through]:
        if version == runtime.LEDGER_VERSION:
            runtime.initialize_migration_history(c, sources[0][1])
        c.execute(source)
        c.execute("INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (%s,%s)", (version, digest))
        c.execute("UPDATE genui_meta.schema_version SET version=%s WHERE singleton", (version,))
    c.commit()


class OperatorDatabase:
    """One disposable PostgreSQL 16.15 container with a throwaway TLS CA, shared by a TestCase class.

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
                assert c.execute("SHOW server_version").fetchone()[0].split()[0] == "16.15"
                c.execute("ALTER SYSTEM SET ssl='on'")
                c.execute("ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'")
                c.execute("ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'")
                c.execute("SELECT pg_reload_conf()")
                c.execute("CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS")
                c.execute("CREATE ROLE rds_iam NOLOGIN")
                c.execute("GRANT rds_iam TO genui_migration WITH ADMIN TRUE")
                c.execute("CREATE DATABASE genui OWNER genui_migration")
            with cls.connect() as c:
                through = getattr(cls, "migrate_through", None)
                if through is None:
                    runtime.apply_migrations(c, runtime.migration_directory())
                else:
                    apply_through(c, through)
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
            reservation, _, outcome = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s,1)",
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
        spaces = self.query("SELECT space_id::text,display_name FROM genui.spaces WHERE organization_id=%s", (org,))
        policies = self.query("SELECT policy_id::text,policy_version,kind FROM genui.publication_policies WHERE organization_id=%s", (org,))
        self.assertEqual(len(spaces), 1)
        self.assertEqual(spaces[0][1], "Workspace")
        self.assertEqual(uuid.UUID(spaces[0][0]).version, 4)
        self.assertEqual(len(policies), 1)
        self.assertEqual(policies[0][1:], (1, "owner_publish"))
        self.assertEqual(uuid.UUID(policies[0][0]).version, 4)
        for table in ("principals", "memberships", "entitlements", "apps", "saved_work", "sources",
                      "definition_revisions", "publications", "internal_records", "app_permissions",
                      "work_permissions", "source_permissions"):
            self.assertEqual(self.query(f"SELECT count(*) FROM genui.{table} WHERE organization_id=%s", (org,)), [(0,)], table)
        self.assertEqual(self.operator_events(org), [("organization.provisioned",)])
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)
        before = self.dump()
        self.cli("provision", "--organization", org, "--display-name", "again", "--issuer", ISSUER, "--provider-tenant", "ten_x", expect=1)
        self.assertEqual(self.dump(), before)

    def test_fresh_authoring_only_member_redeems_with_entitlement_and_no_resource_grants(self):
        org = str(uuid.uuid4()); tenant = "ten_" + org[-12:]; subject = "usr_" + uuid.uuid4().hex
        self.cli("provision", "--organization", org, "--display-name", "Clean first member",
                 "--issuer", ISSUER, "--provider-tenant", tenant)
        issued = self.cli("issue", "--organization", org, "--issued-by", "operator-test",
                          "--kind", "member_invite", "--authoring-only")
        digest = op.code_digest(op.normalize_code(issued["code"]))
        self.assertEqual(self.query("SELECT count(*) FROM genui.code_grant_templates WHERE organization_id=%s", (org,)), [(0,)])
        with self.connect("genui_workspace") as c:
            c.execute("SELECT set_config('genui.organization_id',%s,true)", (org,))
            reservation, _, outcome = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s,1)", (org, digest, "sha256:" + "a" * 64)).fetchone()
            self.assertEqual(outcome, "reserved")
            pending, _, outcome = c.execute("SELECT * FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)", (org, reservation, ISSUER, tenant, subject)).fetchone()
            self.assertEqual(outcome, "pending")
            parameters = (org, pending, ISSUER, tenant, subject, "sha256:" + "b" * 64)
            admitted = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)", parameters).fetchone()
            self.assertEqual(admitted[-1], "admitted")
            replayed = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)", parameters).fetchone()
            self.assertEqual(replayed[:-1], admitted[:-1])
            self.assertEqual(replayed[-1], "replayed")
        principal, epoch, entitlement, _, _ = admitted
        self.assertEqual(epoch, 1)
        self.assertEqual(self.query("SELECT status FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (org, principal)), [("active",)])
        self.assertEqual(self.query("SELECT status,kind,expires_at>now() FROM genui.entitlements WHERE organization_id=%s AND entitlement_id=%s", (org, entitlement)), [("active", "member_invite", True)])
        for table in ("apps", "saved_work", "internal_records", "publications", "app_permissions", "work_permissions", "source_permissions"):
            self.assertEqual(self.query(f"SELECT count(*) FROM genui.{table} WHERE organization_id=%s", (org,)), [(0,)], table)
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)

    def test_bind_provider_tenant_adds_the_real_binding_once_and_one_audit_event(self):
        """APP-078 B1 / APP-084: an existing organization gains its provider tenant once, with one event."""
        self.ensure_backfill()
        before = self.operator_events()
        rows = lambda: self.query("SELECT issuer,provider_tenant_id,active FROM genui.provider_tenant_bindings WHERE organization_id=%s ORDER BY issuer", (ORG,))
        self.assertEqual(rows(), [(ISSUER, TENANT, True)], "the fixture's existing binding, like the seed's `app008-fixture-only` on RDS")
        args = ("bind-provider-tenant", "--organization", ORG, "--issuer", "synthetic-iss", "--provider-tenant", "ten_00000000-0000-4000-8000-00000000a001")
        created = self.cli(*args)
        self.assertEqual({k: created[k] for k in ("organization_id", "binding", "active_bindings", "source")},
                         {"organization_id": ORG, "binding": "created", "active_bindings": 2, "source": "arguments"})
        self.assertRegex(created["audit_event"], r"^[0-9a-f-]{36}$")
        self.assertEqual(self.operator_events(), before + [("provider_tenant.bound",)])
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)  # APP-084 review: the new kind verifies
        self.assertEqual(rows(), [(ISSUER, TENANT, True), ("synthetic-iss", "ten_00000000-0000-4000-8000-00000000a001", True)])
        self.assertEqual(self.cli(*args), {"organization_id": ORG, "binding": "existing", "active_bindings": 2, "audit_event": None, "source": "arguments"})
        # B1 review m6: a pair already active for another organization that can sign in is refused.
        other = "00000000-0000-4000-8000-0000000000a2"
        with self.connect("postgres", autocommit=True) as c:
            c.execute("INSERT INTO genui.organizations VALUES (%s,'Operator test other','active') ON CONFLICT DO NOTHING", (other,))
            c.execute("INSERT INTO genui_private.handle_index VALUES (%s,%s,NULL) ON CONFLICT DO NOTHING", (op.handle_digest(os.urandom(16)), other))
            c.execute("INSERT INTO genui.provider_tenant_bindings VALUES (%s,'elsewhere-iss','ten_elsewhere',true) ON CONFLICT DO NOTHING", (other,))
            c.execute("INSERT INTO genui_private.provider_tenant_index VALUES ('elsewhere-iss','ten_elsewhere',%s) ON CONFLICT DO NOTHING", (other,))
        before_rows = self.dump()
        elsewhere = self.cli("bind-provider-tenant", "--organization", ORG, "--issuer", "elsewhere-iss", "--provider-tenant", "ten_elsewhere", expect=1)
        self.assertEqual(json.loads(elsewhere.stderr), {"refused": "provider_tenant_bound_elsewhere"})
        self.assertEqual(self.dump(), before_rows)
        # Both halves of the pair must match: the same tenant under another issuer is a different pair.
        self.cli("bind-provider-tenant", "--organization", ORG, "--issuer", "other-iss", "--provider-tenant", "ten_elsewhere")
        with self.connect("postgres", autocommit=True) as c:
            c.execute("DELETE FROM genui.provider_tenant_bindings WHERE organization_id=%s AND issuer='other-iss'", (ORG,))
            c.execute("DELETE FROM genui_private.provider_tenant_index WHERE issuer='other-iss' AND organization_id=%s", (ORG,))
        # The source form: either both identifiers or the auth configuration flag, never a mixture.
        for bad in (("--issuer", "x"), ("--provider-tenant", "t"), ("--issuer", "x", "--provider-tenant", "t", "--from-auth-configuration"),
                    ("--from-auth-configuration", "--issuer", "x"), ("--issuer", "x", "--provider-tenant", "t", "--auth-configuration-secret-id", "arn:aws:secretsmanager:us-east-1:000000000000:secret:x")):
            with self.subTest(bad=bad), self.assertRaises(op.Refused) as refused:
                op.validate(op.parser().parse_args(["bind-provider-tenant", "--organization", ORG, *bad]))
            self.assertEqual(str(refused.exception), "binding_source_refused")
        self.assertEqual(rows()[1:], [("synthetic-iss", "ten_00000000-0000-4000-8000-00000000a001", True)])
        self.assertEqual(self.operator_events(), before + [("provider_tenant.bound",), ("provider_tenant.bound",)])
        # The registry's own bounds, checked before any connection; an unknown organization refuses after it.
        for bad in (("--issuer", "with space"), ("--issuer", "x" * 257), ("--provider-tenant", ""), ("--provider-tenant", "t" * 129)):
            with self.subTest(bad=bad):
                self.cli("bind-provider-tenant", "--organization", ORG, "--issuer", "ok-iss", "--provider-tenant", "ten_ok", *bad, expect=1)
        unknown = self.cli("bind-provider-tenant", "--organization", "00000000-0000-4000-8000-0000000000ff", "--issuer", "ok-iss", "--provider-tenant", "ten_ok", expect=1)
        self.assertEqual(json.loads(unknown.stderr), {"refused": "organization_unknown"})
        # An inactive binding is not silently reactivated.
        with self.connect("postgres", autocommit=True) as c:
            c.execute("UPDATE genui.provider_tenant_bindings SET active=false WHERE organization_id=%s AND issuer='synthetic-iss'", (ORG,))
        inactive = self.cli(*args, expect=1)
        self.assertEqual(json.loads(inactive.stderr), {"refused": "provider_binding_inactive"})
        with self.connect("postgres", autocommit=True) as c:
            c.execute("DELETE FROM genui.provider_tenant_bindings WHERE organization_id=%s AND issuer='synthetic-iss'", (ORG,))

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
        self.assertEqual(set(out), {"code_id", "code", "kind", "max_redemptions", "expires_at", "role"})
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
                                      "expires_at", "revoked_at", "provider_invitations", "redemptions", "note"})
        self.assertIsNone(entry["note"])  # issued without --note
        self.assertEqual((entry["status"], entry["redeemed_count"], entry["live_pending_admissions"], entry["lapsed_pending_admissions"],
                          entry["denied_for_reconciliation"]), ("active", 0, 0, 0, 0))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_preminted_issue_stores_the_given_digest_and_prints_no_secret(self):
        # APP-078 managed channel: the operator minted the code; the command sees only its digest.
        raw = os.urandom(32)
        code, digest = op.encode_code(raw), op.code_digest(raw)
        out = self.issue("--code-digest", digest)
        self.assertEqual(set(out), {"code_id", "code_digest", "kind", "max_redemptions", "expires_at", "role"})
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

    def test_issue_note_is_stored_once_0023_is_applied(self):
        """The real-user trial (W3, release B): this tree's schema carries 0023's access_codes.note, so a note
        is stored on the code, returned by the issue and shown by list. Before 0023 a note refuses
        note_unsupported before any write: test_issue_with_a_note_binds_it_to_the_insert_and_the_result
        (the column probe mocked absent), whose probe test_issue_note_is_bounded_and_chooses_the_0023_insert pins."""
        self.ensure_backfill()
        self.assertEqual(self.query("SELECT count(*) FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class t ON t.oid=a.attrelid "
                                    "JOIN pg_catalog.pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname='genui' "
                                    "AND t.relname='access_codes' AND a.attname='note'"), [(1,)])
        out = self.issue("--note", "Ada L.")
        self.assertEqual(out["note"], "Ada L.")
        self.assertEqual(self.query("SELECT note FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (ORG, out["code_id"])),
                         [("Ada L.",)])
        self.assertEqual(self.operator_events()[-1], ("code.issued",))
        entry = next(item for item in self.cli("list", "--organization", ORG)["codes"] if item["code_id"] == out["code_id"])
        self.assertEqual(entry["note"], "Ada L.")

    def test_set_role_is_an_operator_event_and_keeps_the_last_owner(self):
        """ADR-021 (migration 0025): set-role changes an active member's role, advances the epoch and appends one
        membership.role_changed event with a recomputable digest; the same role again changes nothing; a revoked
        or unknown member, an unknown role and admin_policy as a permission refuse with nothing written; the
        organization never loses its last active owner."""
        self.ensure_backfill()
        admin, revoked = self.member(), self.member("revoked")
        epoch = self.epoch(admin)
        self.assertEqual(self.query("SELECT role FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, admin)), [("member",)])
        promoted = self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", "admin")
        self.assertEqual(promoted, {"principal_id": admin, "role": "admin", "role_before": "member", "changed": True, "membership_epoch": epoch + 1})
        self.assertEqual(self.query("SELECT role,membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, admin)),
                         [("admin", epoch + 1)])
        self.assertEqual(self.last_operator_event(), ("membership.role_changed", admin, None, None))
        digest = "sha256:" + hashlib.sha256(b"forma:member-role:v1\x00" + admin.encode() + b"\x00member\x00admin").hexdigest()
        self.assertEqual(self.query("""SELECT outcome_digest::text FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator'
                                       AND kind='membership.role_changed' ORDER BY chain_sequence DESC LIMIT 1""", (ORG,)), [(digest,)])
        events = len(self.operator_events())
        again = self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", "admin")
        self.assertEqual((again["changed"], again["membership_epoch"], len(self.operator_events())), (False, epoch + 1, events))
        before = self.dump()
        for principal in (revoked, str(uuid.uuid4())):
            self.cli("set-role", "--organization", ORG, "--principal", principal, "--role", "member", expect=1)
        for bad in ("teacher", "ADMIN", "owner "):
            self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", bad, expect=1)
        self.cli("grant-permission", "--organization", ORG, "--principal", admin, "--permission", "admin_policy", expect=1)
        self.cli("revoke-permission", "--organization", ORG, "--principal", admin, "--permission", "admin_policy", expect=1)
        self.assertEqual(self.dump(), before)
        # The last owner: the first owner cannot be demoted until a second one exists.
        owner = self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", "owner")
        self.assertEqual((owner["role_before"], owner["role"], owner["membership_epoch"]), ("admin", "owner", epoch + 2))
        p = self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", "member", expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "last_owner"})
        second = self.member()
        self.cli("set-role", "--organization", ORG, "--principal", second, "--role", "owner")
        demoted = self.cli("set-role", "--organization", ORG, "--principal", admin, "--role", "viewer")
        self.assertEqual((demoted["role_before"], demoted["role"], demoted["membership_epoch"]), ("owner", "viewer", epoch + 3))
        self.assertEqual(self.operator_events()[-3:], [("membership.role_changed",)] * 3)
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_issue_with_a_role_admits_a_viewer_and_list_shows_each_role(self):
        """ADR-021: issue --role viewer (default member) is stored on the code and admits with it; list names
        the redeemer's role; another role or admin_policy as a grant refuses."""
        self.ensure_backfill()
        out = self.issue("--role", "viewer")
        self.assertEqual(out["role"], "viewer")
        self.assertEqual(self.query("SELECT role FROM genui.access_codes WHERE organization_id=%s AND code_id=%s", (ORG, out["code_id"])), [("viewer",)])
        self.assertEqual(self.issue()["role"], "member")
        for bad in (["--role", "admin"], ["--role", "owner"], ["--role", "Viewer"]):
            self.cli("issue", "--organization", ORG, "--issued-by", "unit-operator", "--grant", "source_read:" + self.source, *bad, expect=1)
        digest = op.code_digest(op.normalize_code(out["code"]))
        outcome, pending, _, _ = self.pending_for(digest, "usr_viewer_" + uuid.uuid4().hex[:8], "7")
        self.assertEqual(outcome, "pending")
        with self.workspace() as c:
            principal, member_epoch, _, _, outcome = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                                                               (ORG, pending, ISSUER, TENANT, "usr_viewer_" + uuid.uuid4().hex[:8], "sha256:" + "7" * 64)).fetchone()
            self.assertEqual(outcome, "mismatch")  # the subject must match the pending admission
        subject = self.query("SELECT provider_subject FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s", (ORG, pending))[0][0]
        with self.workspace() as c:
            principal, member_epoch, _, _, outcome = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                                                               (ORG, pending, ISSUER, TENANT, subject, "sha256:" + "7" * 64)).fetchone()
            c.commit()
        self.assertEqual((outcome, member_epoch), ("admitted", 1))
        self.assertEqual(self.query("SELECT role,status FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, principal)),
                         [("viewer", "active")])
        entry = next(item for item in self.cli("list", "--organization", ORG)["codes"] if item["code_id"] == out["code_id"])
        self.assertEqual([(r["principal_id"], r["membership_role"]) for r in entry["redemptions"]], [(str(principal), "viewer")])

    def test_list_box_files_names_only_the_members_uploads(self):
        """The real-user trial (deletion): the keys of one member's uploads, read-only, in a stable order;
        another member's files and an unknown member are not listed."""
        self.ensure_backfill()
        uploader, other = self.member(), self.member()
        box, other_box = str(uuid.uuid4()), str(uuid.uuid4())
        files = [str(uuid.uuid4()) for _ in range(3)]
        with self.connect("postgres") as c:
            for owner, identifier in ((uploader, box), (other, other_box)):
                c.execute("INSERT INTO genui.boxes VALUES (%s,%s,%s,'Uploads','uploads',NULL,now())", (ORG, identifier, owner))
            for n, file in enumerate(files):
                c.execute("""INSERT INTO genui.box_files VALUES (%s,%s,%s,%s,'pdf','application/pdf',%s,%s,%s,%s,now()+make_interval(secs=>%s))""",
                          (ORG, box, file, "file-%d.pdf" % n, 1000 + n, "sha256:" + ("%02x" % n) * 32, "o/%s/b/%s/f/%s" % (ORG, box, file),
                           uploader, n))
            c.execute("""INSERT INTO genui.box_files VALUES (%s,%s,%s,'other.csv','csv','text/csv',5,%s,%s,%s,now())""",
                      (ORG, other_box, str(uuid.uuid4()), "sha256:" + "ab" * 32, "o/%s/b/%s/f/%s" % (ORG, other_box, str(uuid.uuid4())), other))
        listed = self.cli("list-box-files", "--organization", ORG, "--principal", uploader)
        self.assertEqual((listed["organization_id"], listed["principal_id"], listed["count"], listed["truncated"]), (ORG, uploader, 3, False))
        self.assertEqual([f["file_id"] for f in listed["files"]], files)
        for n, entry in enumerate(listed["files"]):
            self.assertEqual(set(entry), {"box_id", "file_id", "object_key", "byte_length", "content_digest", "kind"})
            self.assertEqual((entry["box_id"], entry["object_key"], entry["byte_length"], entry["kind"]),
                             (box, "o/%s/b/%s/f/%s" % (ORG, box, files[n]), 1000 + n, "pdf"))
        self.assertEqual(self.cli("list-box-files", "--organization", ORG, "--principal", self.member())["files"], [])
        p = self.cli("list-box-files", "--organization", ORG, "--principal", str(uuid.uuid4()), expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "member_unknown"})
        # Read-only: no operator event and no row changed.
        self.assertNotIn(("permission.granted",), self.operator_events()[-1:])
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)
        # stats counts every closed table for the organization and names the proved schema version,
        # including 0023's consent records (release B integration review, NIT-5).
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.member_consents VALUES (%s,%s,1,now(),%s)", (ORG, uploader, str(uuid.uuid4())))
        counted = self.cli("stats", "--organization", ORG)
        consents = self.query("SELECT count(*) FROM genui.member_consents WHERE organization_id=%s", (ORG,))[0][0]
        self.assertGreaterEqual(consents, 1)
        self.assertEqual(counted["counts"]["member_consents"], consents)
        self.assertEqual(set(counted), {"organization_id", "schema_version", "counts", "audit_heads", "audit_events"})
        self.assertEqual(counted["audit_events"], self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (ORG,))[0][0])
        self.assertEqual(set(counted["counts"]), set(op.STATS_TABLES))
        self.assertEqual(counted["schema_version"], runtime.migration_manifest(runtime.migration_directory())["database_version"])
        self.assertEqual((counted["counts"]["boxes"], counted["counts"]["box_files"]), (2, 4))
        self.assertGreaterEqual(counted["counts"]["memberships"], 3)
        self.assertEqual(set(counted["audit_heads"]), set(op.CHAINS))
        self.cli("stats", "--organization", str(uuid.uuid4()), expect=1)

    def test_purge_empties_every_content_table_of_a_suspended_memberless_organization_and_keeps_the_chains(self):
        """The end-of-trial purge (ADR-008 amendment, W3): refuses while active or while a member is active;
        then one transaction empties every content table and the code digests, keeps the organization row,
        its binding, its handle, its sequence and the audit chains, and never touches another organization."""
        org = str(uuid.uuid4()); tenant = "ten_" + org[-12:]
        self.ensure_backfill()
        fixture_before = self.cli("stats", "--organization", ORG)
        self.cli("provision", "--organization", org, "--display-name", "Purge test", "--issuer", ISSUER, "--provider-tenant", tenant)
        issued = self.cli("issue", "--organization", org, "--issued-by", "operator-test", "--kind", "member_invite", "--authoring-only")
        digest = op.code_digest(op.normalize_code(issued["code"]))
        members = []
        with self.connect("postgres") as c:
            for n in range(2):
                principal = str(uuid.uuid4()); members.append(principal)
                c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (org, principal, ISSUER, tenant, "usr_" + principal))
                c.execute("INSERT INTO genui.memberships VALUES (%s,%s,'active',1)", (org, principal))
            space = c.execute("SELECT space_id::text FROM genui.spaces WHERE organization_id=%s", (org,)).fetchone()[0]
            box, file = str(uuid.uuid4()), str(uuid.uuid4())
            c.execute("INSERT INTO genui.boxes VALUES (%s,%s,%s,'Uploads','uploads',NULL,now())", (org, box, members[0]))
            c.execute("INSERT INTO genui.box_files VALUES (%s,%s,%s,'notes.pdf','pdf','application/pdf',12,%s,%s,%s,now())",
                      (org, box, file, "sha256:" + "cd" * 32, "o/%s/b/%s/f/%s" % (org, box, file), members[0]))
        app = str(uuid.uuid4())
        self.cli("create-app", "--organization", org, "--app", app, "--space", space, "--owner", members[0])
        self.cli("set-role", "--organization", org, "--principal", members[1], "--role", "admin")
        populated = self.cli("stats", "--organization", org)["counts"]
        for table, expected in (("principals", 2), ("memberships", 2), ("access_codes", 1), ("apps", 1), ("app_permissions", 1),
                                ("boxes", 1), ("box_files", 1), ("spaces", 1)):
            self.assertEqual(populated[table], expected, table)
        self.assertEqual(self.query("SELECT count(*) FROM genui_private.code_index WHERE organization_id=%s", (org,)), [(1,)])
        # Refusals, each writing nothing: active organization, then an active member.
        before = self.dump()
        p = self.cli("purge-organization", "--organization", org, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "organization_not_suspended"})
        self.assertEqual(self.dump(), before)
        suspended = self.cli("suspend-organization", "--organization", org)
        self.assertEqual(suspended, {"organization_id": org, "status": "suspended", "changed": True, "audit_event": None})
        self.assertEqual(self.cli("suspend-organization", "--organization", org)["changed"], False)
        self.assertEqual(self.query("SELECT status FROM genui.organizations WHERE organization_id=%s", (org,)), [("suspended",)])
        before = self.dump()
        p = self.cli("purge-organization", "--organization", org, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "active_memberships_remain"})
        self.assertEqual(self.dump(), before)
        for principal in members:
            self.assertTrue(self.cli("revoke-membership", "--organization", org, "--principal", principal)["changed"])
        events_before = self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (org,))[0][0]
        heads_before = self.query("SELECT writer_role,chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s ORDER BY 1", (org,))
        # The purge.
        purged = self.cli("purge-organization", "--organization", org)
        self.assertEqual((purged["status"], purged["audit_events_kept"], purged["audit_event"]), ("purged", events_before, None))
        self.assertGreaterEqual(purged["rows_deleted"], 2 + 2 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1)  # the rows above plus the code's index row
        self.assertEqual(purged["deleted"]["genui_private.code_index"], 1)
        self.assertEqual(sorted(purged["kept"]), sorted(["organizations", "provider_tenant_bindings", "organization_sequences", "audit_heads",
                                                        "audit_events", "genui_private.handle_index", "genui_private.provider_tenant_index",
                                                        "genui_identity.accounts", "genui_identity.shared_sessions"]))
        self.assertEqual(purged["identity"], {"shared_auth_flows_deleted": 0, "guest_rows": {"genui_identity.linked_apps": 0, "genui_identity.share_events": 0},
                                              "linked_apps_deleted": 0, "share_events_kept": 0})
        after = self.cli("stats", "--organization", org)
        self.assertEqual(set(after["counts"].values()), {0})
        self.assertEqual(after["audit_events"], events_before)
        self.assertEqual(after["audit_heads"], {role: sequence for role, sequence, _ in heads_before})
        self.assertEqual(self.query("SELECT status FROM genui.organizations WHERE organization_id=%s", (org,)), [("suspended",)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.provider_tenant_bindings WHERE organization_id=%s", (org,)), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_private.handle_index WHERE organization_id=%s", (org,)), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_private.provider_tenant_index WHERE organization_id=%s", (org,)), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_private.code_index WHERE organization_id=%s", (org,)), [(0,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.organization_sequences WHERE organization_id=%s", (org,)), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (org,)), [(events_before,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_private.code_index WHERE code_digest=%s", (digest,)), [(0,)])
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)
        # Idempotent on an already purged organization, and the fixture organization is untouched.
        self.assertEqual(self.cli("purge-organization", "--organization", org)["rows_deleted"], 0)
        self.assertEqual(self.cli("stats", "--organization", ORG)["counts"], fixture_before["counts"])
        self.cli("purge-organization", "--organization", str(uuid.uuid4()), expect=1)

    def shared_work(self, org, sponsor):
        """An app with one published saved work for `sponsor` (the live-share fixture's parents)."""
        app, revision, publication, policy, work = (str(uuid.uuid4()) for _ in range(5))
        digest = "sha256:" + "e" * 64
        with self.connect("postgres") as c:
            space = c.execute("SELECT space_id::text FROM genui.spaces WHERE organization_id=%s", (org,)).fetchone()[0]
            c.execute("INSERT INTO genui.apps VALUES (%s,%s,%s,%s)", (org, app, space, sponsor))
            c.execute("INSERT INTO genui.definition_revisions VALUES (%s,%s,%s,1,%s,'\\x7b7d')", (org, app, revision, digest))
            c.execute("INSERT INTO genui.publication_policies VALUES (%s,%s,1,'owner_publish')", (org, policy))
            c.execute("""INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,
                         policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,
                         published_at,canonical_envelope,publication_sequence)
                         VALUES (%s,%s,%s,%s,%s,%s,1,'owner_publish',%s,%s,1,1,1,%s,now(),'\\x7b7d',1)""",
                      (org, app, publication, revision, digest, policy, sponsor, sponsor, digest))
            c.execute("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context) VALUES (%s,%s,%s,%s,1,'')",
                      (org, app, work, publication))
        return app, work, publication

    def live_share(self, org, sponsor, app, work, publication, accepted_by=None):
        share = str(uuid.uuid4())
        import datetime as dt
        accepted_at = None if accepted_by is None else dt.datetime.now(dt.timezone.utc)
        with self.connect("postgres") as c:
            c.execute("""INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,
                         accepted_account_id,accepted_at,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,
                         version,idempotency_key,request_digest,created_at,expires_at)
                         VALUES (%s,%s,%s,%s,%s,1,'guest@example.test',%s,%s,%s,%s,1,'{}','{}','Shared app',false,1,%s,%s,now(),now()+interval '7 days')""",
                      (org, share, app, work, sponsor, accepted_by, accepted_at, publication, "sha256:" + "e" * 64, "idem-" + share[:24], "sha256:" + "f" * 64))
            c.execute("""INSERT INTO genui_identity.shared_auth_flows(flow_id,owner_organization_id,share_id,provider_organization_id,cookie_digest,
                         state_digest,sealed_verifier) VALUES (%s,%s,%s,%s,%s,%s,'\\x01')""",
                      (str(uuid.uuid4()), org, share, org, "sha256:" + uuid.uuid4().hex * 2, "sha256:" + "1" * 64))
        return share

    def guest_event(self, guest, org, share, sequence, previous_hash):
        """One share.accepted row on the guest's account chain naming the organization's share; the head moves to it."""
        canonical = json.dumps({"account_id": guest, "organization_id": org, "share_id": share, "kind": "share.accepted", "share_version": 1,
                                "sequence": sequence, "previous_hash": previous_hash}, sort_keys=True, separators=(",", ":")).encode()
        event_hash = "sha256:" + hashlib.sha256(b"forma:share-account-event:v1\x00" + canonical).hexdigest()
        with self.connect("postgres") as c:
            c.execute("""INSERT INTO genui_identity.share_events(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at)
                         VALUES (%s,%s,%s,%s,%s,'share.accepted',1,%s,%s,%s,now())""",
                      (guest, str(uuid.uuid4()), sequence, org, share, canonical, previous_hash, event_hash))
            c.execute("UPDATE genui_identity.accounts SET audit_sequence=%s,audit_hash=%s WHERE account_id=%s", (sequence, event_hash, guest))
        return event_hash

    def guest_chain(self, guest):
        """The guest's account chain, verified: each hash recomputed from its canonical bytes, each previous_hash the
        hash before it (genesis first), the account head at the last row. Returns (head, [(sequence, hash, organization)])."""
        head = self.query("SELECT audit_sequence,audit_hash FROM genui_identity.accounts WHERE account_id=%s", (guest,))[0]
        events = self.query("SELECT sequence,previous_hash,hash,canonical,organization_id::text FROM genui_identity.share_events WHERE account_id=%s ORDER BY sequence",
                            (guest,))
        for _, _, digest, canonical, _ in events:
            self.assertEqual("sha256:" + hashlib.sha256(b"forma:share-account-event:v1\x00" + bytes(canonical)).hexdigest(), digest)
        self.assertEqual([e[1] for e in events], ["sha256:" + "0" * 64] + [e[2] for e in events[:-1]])
        self.assertEqual((head[0], head[1]), (events[-1][0], events[-1][2]))
        return (head[0], head[1]), [(e[0], e[2], e[4]) for e in events]

    def identity_forced(self):
        """relforcerowsecurity of every genui_identity table: all five must be back on after any purge or refusal."""
        rows = self.query("""SELECT t.relname,t.relforcerowsecurity FROM pg_catalog.pg_class t JOIN pg_catalog.pg_namespace n ON n.oid=t.relnamespace
                             WHERE n.nspname='genui_identity' AND t.relkind='r' ORDER BY 1""")
        self.assertEqual(len(rows), 5, rows)
        return all(f for _, f in rows)

    def test_purge_deletes_guest_links_at_schema_24_keeps_their_chains_and_refuses_below_it(self):
        """APP-212 (APP-185 recheck R1; ADR-008 "Guest shares"): at schema 24 the purge deletes the organization's
        linked_apps rows under the lifted FORCE, before the live_work_shares their non-deferrable key reaches, keeps
        every share_events row (the guest's hash chain still verifies from its head) and reports both; the same guest's
        link to another organization's share is untouched. Below schema 24 (the operator's version read, simulated
        in-process where the gate reads it) a guest's link refuses guest_linked_shares_present and nothing is deleted,
        not even the flow. An organization whose shares nobody accepted is purged with its sign-in flows, as before.
        FORCE row security is restored after every outcome."""
        links, events = "genui_identity.linked_apps", "genui_identity.share_events"
        guest = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui_identity.accounts(account_id,issuer,provider_tenant_id,provider_subject,verified_email) VALUES (%s,%s,%s,%s,%s)",
                      (guest, ISSUER, TENANT, "usr_guest_" + guest[:8], "guest@example.test"))
        def organization():
            org = str(uuid.uuid4())
            self.cli("provision", "--organization", org, "--display-name", "Share purge test", "--issuer", ISSUER, "--provider-tenant", "ten_" + org[-12:])
            sponsor = str(uuid.uuid4())
            with self.connect("postgres") as c:
                c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (org, sponsor, ISSUER, "ten_" + org[-12:], "usr_" + sponsor))
                c.execute("INSERT INTO genui.memberships VALUES (%s,%s,'active',1)", (org, sponsor))
            return org, sponsor
        def linked_share(org, sponsor, sequence, previous_hash):
            """A share of `org` the guest accepted and linked, with its row on the guest's chain."""
            app, work, publication = self.shared_work(org, sponsor)
            share = self.live_share(org, sponsor, app, work, publication, accepted_by=guest)
            with self.connect("postgres") as c:
                c.execute("INSERT INTO genui_identity.linked_apps(account_id,organization_id,share_id,created_at,grant_version) VALUES (%s,%s,%s,now(),1)",
                          (guest, org, share))
            return self.guest_event(guest, org, share, sequence, previous_hash)
        # Organizations A and C: one share of each, accepted and linked by the same guest, chained A then C on its account.
        (org_a, sponsor_a), (org_c, sponsor_c) = organization(), organization()
        hash_a = linked_share(org_a, sponsor_a, 1, "sha256:" + "0" * 64)
        hash_c = linked_share(org_c, sponsor_c, 2, hash_a)
        chain_before = self.guest_chain(guest)
        self.assertEqual(chain_before, ((2, hash_c), [(1, hash_a, org_a), (2, hash_c, org_c)]))
        for org, sponsor in ((org_a, sponsor_a), (org_c, sponsor_c)):
            self.cli("revoke-membership", "--organization", org, "--principal", sponsor)
            self.cli("suspend-organization", "--organization", org)
        self.assertGreaterEqual(self.cli("stats", "--organization", org_a)["schema_version"], op.GATED_PURGE_DELETIONS[links])
        # Schema 24: A's link goes (before its share), A's chain row stays, C's link and row are untouched.
        purged = self.cli("purge-organization", "--organization", org_a)
        self.assertEqual((purged["status"], purged["identity"]),
                         ("purged", {"shared_auth_flows_deleted": 1, "guest_rows": {links: 1, events: 1}, "linked_apps_deleted": 1, "share_events_kept": 1}))
        self.assertEqual((purged["deleted"][links], purged["deleted"]["live_work_shares"], events in purged["deleted"]), (1, 1, False))
        self.assertEqual(self.query("SELECT organization_id::text FROM genui_identity.linked_apps WHERE account_id=%s", (guest,)), [(org_c,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.share_events WHERE organization_id=%s", (org_a,)), [(1,)])
        self.assertEqual(self.guest_chain(guest), chain_before)  # verifies, head unchanged, A's row still in it
        self.assertEqual(self.cli("stats", "--organization", org_a)["counts"]["live_work_shares"], 0)
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.shared_auth_flows WHERE owner_organization_id=%s", (org_a,)), [(0,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.shared_auth_flows WHERE owner_organization_id=%s", (org_c,)), [(1,)])  # C's flow stays with C
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.accounts WHERE account_id=%s", (guest,)), [(1,)])
        self.assertTrue(self.identity_forced())
        # Below schema 24, C's link refuses before any row goes: the refusal rolls the transaction back, FORCE included.
        before = self.dump()
        args = op.validate(op.parser().parse_args(["purge-organization", "--organization", org_c]))
        with self.connect() as c, mock.patch.object(op, "schema_version", return_value=23):
            with self.assertRaises(op.Refused) as refused:
                op.purge_organization(c, args)
            c.rollback()
        self.assertEqual(str(refused.exception), "guest_linked_shares_present")
        self.assertEqual(self.dump(), before)  # nothing deleted: not even the flow
        self.assertTrue(self.identity_forced())
        self.assertEqual(self.guest_chain(guest), chain_before)
        # At 24 again, C is purged the same way, and the guest's two-row chain still verifies with no link left.
        purged = self.cli("purge-organization", "--organization", org_c)
        self.assertEqual(purged["identity"], {"shared_auth_flows_deleted": 1, "guest_rows": {links: 1, events: 1}, "linked_apps_deleted": 1, "share_events_kept": 1})
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.linked_apps WHERE account_id=%s", (guest,)), [(0,)])
        self.assertEqual(self.guest_chain(guest), chain_before)
        self.assertTrue(self.identity_forced())
        # Organization B: a share nobody accepted, plus its sign-in flow: both go with the purge, as before.
        org_b, sponsor_b = organization()
        app, work, publication = self.shared_work(org_b, sponsor_b)
        self.live_share(org_b, sponsor_b, app, work, publication)
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.shared_auth_flows WHERE owner_organization_id=%s", (org_b,)), [(1,)])
        self.cli("revoke-membership", "--organization", org_b, "--principal", sponsor_b)
        self.cli("suspend-organization", "--organization", org_b)
        purged = self.cli("purge-organization", "--organization", org_b)
        self.assertEqual((purged["status"], purged["identity"]),
                         ("purged", {"shared_auth_flows_deleted": 1, "guest_rows": {links: 0, events: 0}, "linked_apps_deleted": 0, "share_events_kept": 0}))
        self.assertEqual(purged["deleted"]["live_work_shares"], 1)
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.shared_auth_flows WHERE owner_organization_id=%s OR provider_organization_id=%s", (org_b, org_b)), [(0,)])
        self.assertEqual(self.cli("stats", "--organization", org_b)["counts"]["live_work_shares"], 0)
        self.assertEqual(self.guest_chain(guest), chain_before)
        self.assertTrue(self.identity_forced())
        # The key map the purge orders by: linked_apps still keys to the shares (so the link goes first, in the lifted
        # block); migration 0024 dropped share_events' key (the purge's recorded exception, W3 R1), so those rows stay.
        with self.connect("postgres") as c:
            edges = op.foreign_keys(c)
        self.assertNotIn(("genui_identity.share_events", "live_work_shares"), edges)
        self.assertIn(("genui_identity.linked_apps", "live_work_shares"), edges)
        self.assertIn(("genui_identity.shared_auth_flows", "live_work_shares"), edges)
        with self.connect("postgres") as c:
            self.assertEqual(op.foreign_children(op.content_tables(c), edges),
                             sorted(set(op.IDENTITY_TABLES) - {"genui_identity.share_events"}))

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
                                 (other, ["--permission", "admin_policy"])):  # no organization permission since 0025 (ADR-021)
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

    def test_list_names_the_principal_of_a_redeemed_code(self):
        """APP-076 G5: `list` shows each redemption's principal and entitlement ids so revocation can target the redeemer."""
        out = self.issue("--max-redemptions", "1")
        digest = op.code_digest(op.normalize_code(out["code"]))
        subject = "usr_" + uuid.uuid4().hex
        before = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual(before["redemptions"], [])
        with self.workspace() as c:
            reservation, _, _ = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s,1)", (ORG, digest, "sha256:" + "a" * 64)).fetchone()
            pending, _, outcome = c.execute("SELECT * FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)",
                                            (ORG, reservation, ISSUER, TENANT, subject)).fetchone()
            self.assertEqual(outcome, "pending")
            principal, _, entitlement, redemption, outcome = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                                                                       (ORG, pending, ISSUER, TENANT, subject, "sha256:" + "3" * 64)).fetchone()
            self.assertEqual(outcome, "admitted")
        listed = self.cli("list", "--organization", ORG)
        entry = next(i for i in listed["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual((entry["status"], entry["redeemed_count"]), ("exhausted", 1))
        self.assertEqual(len(entry["redemptions"]), 1)
        shown = entry["redemptions"][0]
        self.assertEqual(set(shown), {"redemption_id", "principal_id", "entitlement_id", "redeemed_at", "membership_status", "membership_role", "entitlement_status"})
        self.assertEqual(shown["membership_role"], "member")
        self.assertEqual((shown["redemption_id"], shown["principal_id"], shown["entitlement_id"], shown["membership_status"], shown["entitlement_status"]),
                         (str(redemption), str(principal), str(entitlement), "active", "active"))
        # Identifiers only: no provider subject, address or secret in the listing.
        self.assertNotIn(subject, json.dumps(listed))
        self.assertNotIn(out["code"], json.dumps(listed))
        # The shown principal is what revoke-entitlement targets (runbook 6.3/6.4).
        revoked = self.cli("revoke-entitlement", "--organization", ORG, "--principal", shown["principal_id"])
        self.assertEqual((revoked["changed"], revoked["entitlements_revoked"]), (True, 1))
        entry = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual((entry["redemptions"][0]["membership_status"], entry["redemptions"][0]["entitlement_status"]), ("active", "revoked"))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_list_never_hides_a_redemption_whose_rows_are_absent(self):
        """APP-078 review M5: `list` LEFT JOINs memberships and entitlements, so a redemption shows with null statuses."""
        out = self.issue("--max-redemptions", "1")
        digest = op.code_digest(op.normalize_code(out["code"]))
        subject = "usr_" + uuid.uuid4().hex
        with self.workspace() as c:
            reservation, _, _ = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s,1)", (ORG, digest, "sha256:" + "b" * 64)).fetchone()
            pending, _, _ = c.execute("SELECT * FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)",
                                      (ORG, reservation, ISSUER, TENANT, subject)).fetchone()
            principal, _, entitlement, redemption, outcome = c.execute("SELECT * FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)",
                                                                       (ORG, pending, ISSUER, TENANT, subject, "sha256:" + "4" * 64)).fetchone()
            self.assertEqual(outcome, "admitted")
        # The foreign keys make this unreachable through any role; the fixture superuser removes the
        # rows with referential triggers off (replica) and puts them back byte-for-byte afterwards.
        key = (ORG, principal)
        with self.connect("postgres") as c:
            c.execute("SET LOCAL session_replication_role = replica")
            entitlements = c.execute("DELETE FROM genui.entitlements WHERE organization_id=%s AND principal_id=%s RETURNING *", key).fetchall()
            memberships = c.execute("DELETE FROM genui.memberships WHERE organization_id=%s AND principal_id=%s RETURNING *", key).fetchall()
        self.assertEqual((len(entitlements), len(memberships)), (1, 1))
        try:
            entry = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
            self.assertEqual([(r["redemption_id"], r["principal_id"], r["entitlement_id"], r["membership_status"], r["entitlement_status"])
                              for r in entry["redemptions"]],
                             [(str(redemption), str(principal), str(entitlement), None, None)])
        finally:
            with self.connect("postgres") as c:
                c.execute("SET LOCAL session_replication_role = replica")
                for row in memberships:
                    c.execute("INSERT INTO genui.memberships VALUES (%s)" % ",".join(["%s"] * len(row)), row)
                for row in entitlements:
                    c.execute("INSERT INTO genui.entitlements VALUES (%s)" % ",".join(["%s"] * len(row)), row)
        entry = next(i for i in self.cli("list", "--organization", ORG)["codes"] if i["code_id"] == out["code_id"])
        self.assertEqual((entry["redemptions"][0]["membership_status"], entry["redemptions"][0]["entitlement_status"]), ("active", "active"))
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def test_connect_refuses_any_host_but_one_name_before_connecting(self):
        """APP-084 review: '@' is an abstract socket and ',' a host list to libpq; neither uses TLS."""
        for host, code in (("/var/run/postgresql", "socket_host_refused"), ("@abstract", "socket_host_refused"), ("", "socket_host_refused"),
                           ("db.example.test,/var/run/postgresql", "host_refused"), ("db.example.test,@x", "host_refused"),
                           ("db example.test", "host_refused"), ("db.example.test.", "host_refused"), ("[::1]", "host_refused")):
            with self.subTest(host=host), mock.patch.dict(os.environ, {"PGHOST": host, "PGUSER": "genui_migration", "PGSSLROOTCERT": "/x"}), \
                 mock.patch("psycopg.connect") as connect, self.assertRaises(op.Refused) as refused:
                op.connect()
            self.assertEqual(str(refused.exception), code); connect.assert_not_called()

    @unittest.skipUnless(importlib.util.find_spec("jsonschema") is not None,
                         "the seed validates its bundles with contracts/validate.py, which needs jsonschema")
    def test_the_seed_claims_each_tenant_in_the_cross_organization_index(self):
        """APP-084 review: a seeded binding is claimed in genui_private.provider_tenant_index in the same
        transaction, so no other organization can bind the same tenant first; an owned tenant refuses."""
        import tenant_seed
        with self.connect() as c:
            tenant_seed.seed(c, tenant_seed.make_bundles())
        index = dict(((r[0], r[1]), r[2]) for r in self.query(
            "SELECT issuer, provider_tenant_id, organization_id::text FROM genui_private.provider_tenant_index"))
        for org in tenant_seed.ORGANIZATIONS:
            self.assertEqual(index.get((tenant_seed.ISSUER, "ten_" + org)), org)
        with self.connect() as c:  # an exact repeat changes nothing
            changes = []
            tenant_seed.seed(c, tenant_seed.make_bundles(), changes)
            self.assertEqual(changes, [])
        northstar, canary = tenant_seed.ORGANIZATIONS
        move = "UPDATE genui_private.provider_tenant_index SET organization_id=%s WHERE issuer=%s AND provider_tenant_id=%s"
        with self.connect("postgres") as c:
            c.execute(move, (canary, tenant_seed.ISSUER, "ten_" + northstar))
        try:
            with self.connect() as c, self.assertRaises(ValueError):
                tenant_seed.seed(c, tenant_seed.make_bundles())
        finally:  # the class shares one database
            with self.connect("postgres") as c:
                c.execute(move, (northstar, tenant_seed.ISSUER, "ten_" + northstar))

    @unittest.skipUnless(importlib.util.find_spec("jsonschema") is not None,
                         "seed-spend-review validates the bundle with contracts/validate.py; jsonschema is in the operator image's "
                         "lock (app/operator/requirements.lock) and in tenant-foundation CI, not in the runtime-only lock")
    def test_seed_spend_review_places_the_fixture_bundle_in_northstar_once(self):
        """APP-076 G3: the Spend Review bundle in the first fixture organization through the operator seed, idempotent."""
        import tenant_seed
        northstar, canary = tenant_seed.ORGANIZATIONS
        with self.connect() as c:
            tenant_seed.seed(c, tenant_seed.make_bundles())
        bundle = tenant_seed.make_bundle("spend-review", northstar)
        app, work = bundle["definition"]["app_id"], bundle["work"]["work_id"]
        rows = lambda org: self.query("""SELECT (SELECT count(*) FROM genui.apps WHERE organization_id=%s AND app_id=%s),
              (SELECT count(*) FROM genui.saved_work WHERE organization_id=%s AND app_id=%s),
              (SELECT count(*) FROM genui.publications WHERE organization_id=%s AND app_id=%s),
              (SELECT count(*) FROM genui.internal_records WHERE organization_id=%s AND app_id=%s)""", (org, app) * 4)[0]
        self.assertEqual((rows(northstar), rows(canary)), ((0, 0, 0, 0), (1, 1, 1, 1)))
        # Not a fixture organization: refused before any write.
        self.assertIn("organization_refused", self.cli("seed-spend-review", "--organization", ORG, expect=1).stderr)
        self.cli("backfill", "--organization", northstar)
        first = self.cli("seed-spend-review", "--organization", northstar)
        self.assertEqual((first["changed"], first["label"], first["edition"]), (True, "seeded definition", "impl-2026-10-28.1"))
        self.assertRegex(first["audit_event"], r"^[0-9a-f-]{36}$")
        self.assertGreater(first["rows_inserted"], 0)
        self.assertEqual((first["app_id"], first["work_id"], first["publication_id"], first["definition_revision_id"], first["definition_digest"]),
                         (app, work, bundle["publication"]["publication_id"], bundle["definition"]["revision_id"], bundle["definition"]["digest"]))
        self.assertEqual(first["collection_ids"], [bundle["work"]["record_collections"][0]["id"]])
        self.assertEqual((rows(northstar), rows(canary)), ((1, 1, 1, 1), (1, 1, 1, 1)))
        # The same bytes and digests as the seed's: the definition row is canonical(content) under the fixture digest.
        stored = self.query("SELECT digest::text,canonical_content FROM genui.definition_revisions WHERE organization_id=%s AND app_id=%s", (northstar, app))
        self.assertEqual(stored, [(bundle["definition"]["digest"], tenant_seed.canonical(bundle["definition"]["content"]))])
        self.assertEqual(self.query("SELECT space_id::text,owner_principal_id::text FROM genui.apps WHERE organization_id=%s AND app_id=%s", (northstar, app)),
                         [(tenant_seed.SPACE, bundle["context"]["principal_id"])])
        # The Northstar seed rows the bundle shares (organization, space, policy, shared actors) are untouched.
        self.assertEqual(self.query("SELECT count(*) FROM genui.apps WHERE organization_id=%s", (northstar,)), [(2,)])
        snapshot = self.dump()
        again = self.cli("seed-spend-review", "--organization", northstar)
        self.assertEqual((again["changed"], again["rows_inserted"]), (False, 0))
        self.assertEqual(self.dump(), snapshot)
        # A differing existing row refuses whole and writes nothing (the seed's exact-match rule).
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.saved_work SET version=2 WHERE organization_id=%s AND app_id=%s AND work_id=%s", (northstar, app, work))
        tampered = self.dump()
        self.assertIn("seed_refused", self.cli("seed-spend-review", "--organization", northstar, expect=1).stderr)
        self.assertEqual(self.dump(), tampered)
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.saved_work SET version=1 WHERE organization_id=%s AND app_id=%s AND work_id=%s", (northstar, app, work))
        # D7.7 and SR10 hold afterwards: the backfilled sequence row is ahead of the seeded revision and the chains verify.
        self.cli("backfill", "--organization", northstar)
        verified = self.cli("verify", "--organization", northstar)
        self.assertEqual((verified["verified"], verified["next_commit_sequence"]), (True, 2))
        self.assertEqual(self.cli("seed-spend-review", "--organization", northstar)["changed"], False)

    @unittest.skipUnless(importlib.util.find_spec("jsonschema") is not None, "seed-library validates every bundle with contracts/validate.py")
    def test_seed_spend_review_then_library_places_every_library_app_in_northstar_once(self):
        """ADR-011 library (APP-094): all library bundles in the first fixture organization, idempotent, exact-match.

        Named to run after `test_seed_spend_review_places_the_fixture_bundle_in_northstar_once` (the class
        shares one database and that test counts Northstar's apps), so the counts here are relative.
        """
        import tenant_seed
        northstar, canary = tenant_seed.ORGANIZATIONS
        with self.connect() as c:
            tenant_seed.seed(c, tenant_seed.make_bundles())
        bundles = tenant_seed.make_library_bundles(northstar)
        catalog = tenant_seed.library_catalog()
        self.assertEqual(len(bundles), 20)
        self.assertEqual([b["definition"]["app_id"] for b in bundles], [a["app_id"] for a in catalog["apps"]])
        apps = lambda org: self.query("SELECT count(*) FROM genui.apps WHERE organization_id=%s", (org,))[0][0]
        before = (apps(northstar), apps(canary))
        self.assertIn("organization_refused", self.cli("seed-library", "--organization", ORG, expect=1).stderr)
        first = self.cli("seed-library", "--organization", northstar)
        self.assertEqual((first["changed"], first["label"], first["edition"], first["synthetic"], first["audit_event"]),
                         (True, "seeded definition", "impl-2026-10-28.1", True, None))
        self.assertEqual([a["app_id"] for a in first["apps"]], [a["app_id"] for a in catalog["apps"]])
        self.assertEqual((apps(northstar), apps(canary)), (before[0] + 20, before[1]))
        for b, reported in zip(bundles, first["apps"]):
            app, work = b["definition"]["app_id"], b["work"]["work_id"]
            self.assertEqual((reported["work_id"], reported["publication_id"], reported["definition_digest"]),
                             (work, b["publication"]["publication_id"], b["definition"]["digest"]))
            stored = self.query("SELECT digest::text,canonical_content FROM genui.definition_revisions WHERE organization_id=%s AND app_id=%s", (northstar, app))
            self.assertEqual(stored, [(b["definition"]["digest"], tenant_seed.canonical(b["definition"]["content"]))])
            # The source revision row pins the real normalized digest of the generated data (library_sources.py).
            revision = self.query("SELECT normalized_digest::text FROM genui.source_revisions WHERE organization_id=%s AND source_id=%s AND revision_id=%s",
                                  (northstar, b["sources"][0]["source_id"], b["sources"][0]["revision_id"]))
            self.assertEqual(revision, [(b["sources"][0]["normalized_digest"],)])
            self.assertEqual(self.query("SELECT count(*) FROM genui.work_bindings WHERE organization_id=%s AND app_id=%s AND work_id=%s", (northstar, app, work)), [(2,)])
            self.assertEqual(self.query("SELECT count(*) FROM genui.internal_records WHERE organization_id=%s AND app_id=%s", (northstar, app)), [(1,)])
            self.assertEqual(self.query("""SELECT count(*) FROM genui.app_permissions WHERE organization_id=%s AND app_id=%s AND principal_id=%s
                  AND permission IN ('discover','run')""", (northstar, app, b["context"]["principal_id"])), [(2,)])
        snapshot = self.dump()
        again = self.cli("seed-library", "--organization", northstar)
        self.assertEqual((again["changed"], again["rows_inserted"]), (False, 0))
        self.assertEqual(self.dump(), snapshot)
        # A differing existing row refuses whole and writes nothing.
        app, work = bundles[3]["definition"]["app_id"], bundles[3]["work"]["work_id"]
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.saved_work SET version=2 WHERE organization_id=%s AND app_id=%s AND work_id=%s", (northstar, app, work))
        tampered = self.dump()
        self.assertIn("seed_refused", self.cli("seed-library", "--organization", northstar, expect=1).stderr)
        self.assertEqual(self.dump(), tampered)
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.saved_work SET version=1 WHERE organization_id=%s AND app_id=%s AND work_id=%s", (northstar, app, work))
        self.cli("backfill", "--organization", northstar)
        verified = self.cli("verify", "--organization", northstar)
        self.assertEqual(verified["verified"], True)
        self.assertEqual(self.cli("seed-library", "--organization", northstar)["changed"], False)

    @unittest.skipUnless(importlib.util.find_spec("jsonschema") is not None, "seed-library validates every bundle with contracts/validate.py")
    def test_seed_then_grant_library_access_gives_a_demo_member_the_library_and_an_owned_app_once(self):
        """ADR-011 pitch (APP-093 debt 1, APP-096): a freshly invited member gains discover/run on every library app,
        record_read(/record_write) on each library work, source_read on their sources and an owned authoring app with
        edit_definition and run, idempotently, with one permission.granted event per new row. Named to run after the
        library seed test (the class shares one database)."""
        import tenant_seed
        northstar = tenant_seed.ORGANIZATIONS[0]
        with self.connect() as c:
            tenant_seed.seed(c, tenant_seed.make_bundles())
        self.cli("seed-library", "--organization", northstar)
        self.cli("backfill", "--organization", northstar)
        catalog = tenant_seed.library_catalog()
        def member_in(org, status="active"):
            principal = str(uuid.uuid4())
            with self.connect("postgres") as c:
                issuer, tenant = c.execute("SELECT issuer,provider_tenant_id FROM genui.provider_tenant_bindings WHERE organization_id=%s ORDER BY issuer LIMIT 1",
                                           (org,)).fetchone()
                c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (org, principal, issuer, tenant, "usr_" + principal))
                c.execute("INSERT INTO genui.memberships VALUES (%s,%s,%s,1)", (org, principal, status))
            return principal
        rows = lambda table, principal, *extra: self.query("SELECT count(*) FROM genui.%s WHERE organization_id=%%s AND principal_id=%%s%s" % (table, "".join(extra)),
                                                           (northstar, principal))[0][0]
        events = lambda: self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator' AND kind='permission.granted'", (northstar,))[0][0]
        sources = {r[0] for r in self.query("""SELECT DISTINCT source_id::text FROM genui.work_bindings b JOIN genui.saved_work w USING (organization_id,app_id,work_id)
              WHERE b.organization_id=%s AND b.kind='source' AND w.app_id = ANY(%s::uuid[])""", (northstar, [a["app_id"] for a in catalog["apps"]]))}
        self.assertEqual(len(sources), 19)

        demo, authoring = member_in(northstar), str(uuid.uuid4())
        before_events = events()
        out = self.cli("grant-library-access", "--organization", northstar, "--principal", demo, "--record-write", "--authoring-app", authoring)
        self.assertEqual([a["app_id"] for a in out["library_apps"]], [a["app_id"] for a in catalog["apps"]])
        self.assertEqual([a["library_id"] for a in out["library_apps"]], [a["library_id"] for a in catalog["apps"]])
        self.assertEqual((out["changed"], out["record_write"], out["membership_epoch"]), (True, True, 1))
        self.assertEqual(out["authoring_app"], {"app_id": authoring, "created": True, "owner_principal_id": demo})
        # 20 x (discover, run) + (discover, edit_definition, run) on the owned app; 20 x (record_read, record_write); 19 sources.
        self.assertEqual(rows("app_permissions", demo), 43)
        self.assertEqual(rows("work_permissions", demo), 40)
        self.assertEqual(rows("source_permissions", demo), 19)
        self.assertEqual({r[0] for r in self.query("SELECT source_id::text FROM genui.source_permissions WHERE principal_id=%s", (demo,))}, sources)
        self.assertEqual(self.query("SELECT space_id::text,owner_principal_id::text FROM genui.apps WHERE organization_id=%s AND app_id=%s", (northstar, authoring)),
                         [(tenant_seed.SPACE, demo)])
        self.assertEqual(sorted(r[0] for r in self.query("SELECT permission FROM genui.app_permissions WHERE app_id=%s AND principal_id=%s", (authoring, demo))),
                         ["discover", "edit_definition", "run"])
        self.assertEqual(len(out["granted"]), 43 + 40 + 19)
        self.assertEqual(events(), before_events + len(out["granted"]))
        # A work row's event names the work; an app row's names the app alone; a source row's neither.
        kinds = self.query("""SELECT app_id::text,work_id::text FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator'
              AND kind='permission.granted' AND actor_principal_id=%s ORDER BY chain_sequence""", (northstar, demo))
        self.assertEqual(len(kinds), len(out["granted"]))
        self.assertEqual([(g["app_id"], g["work_id"]) for g in out["granted"]], kinds)
        # Idempotent: a repeat writes nothing and no event.
        snapshot = self.dump()
        again = self.cli("grant-library-access", "--organization", northstar, "--principal", demo, "--record-write", "--authoring-app", authoring)
        self.assertEqual((again["changed"], again["granted"], again["authoring_app"]), (False, [], {"app_id": authoring, "created": False, "owner_principal_id": demo}))
        self.assertEqual(self.dump(), snapshot)
        # A reader without --record-write and no authoring app: library reads only.
        reader = member_in(northstar)
        out = self.cli("grant-library-access", "--organization", northstar, "--principal", reader)
        self.assertEqual((out["authoring_app"], out["record_write"], rows("app_permissions", reader), rows("work_permissions", reader),
                          rows("source_permissions", reader)), (None, False, 40, 20, 19))
        self.assertEqual(self.query("SELECT count(*) FROM genui.work_permissions WHERE principal_id=%s AND permission='record_write'", (reader,)), [(0,)])
        # Refusals write nothing: an inactive or unknown member, another member's app as the authoring app, a space
        # without an authoring app, an unknown space, and an organization without the library.
        revoked, other = member_in(northstar, "revoked"), member_in(northstar)
        legacy = self.member()
        snapshot = self.dump()
        for argv, reason in (((northstar, revoked), "member_inactive"), ((northstar, str(uuid.uuid4())), "member_unknown"),
                             ((northstar, other, "--authoring-app", authoring), "app_owner_mismatch"),
                             ((northstar, other, "--space", tenant_seed.SPACE), "space_without_authoring_app"),
                             ((northstar, other, "--authoring-app", str(uuid.uuid4()), "--space", str(uuid.uuid4())), "space_unknown"),
                             ((ORG, legacy), "library_not_seeded")):
            with self.subTest(reason=reason):
                refused = self.cli("grant-library-access", "--organization", argv[0], "--principal", *argv[1:], expect=1)
                self.assertEqual(json.loads(refused.stderr), {"refused": reason})
        self.assertEqual(self.dump(), snapshot)
        self.assertEqual(self.cli("verify", "--organization", northstar)["verified"], True)

    def test_verify_recomputes_sql_written_admission_chain_and_detects_tamper(self):
        out = self.issue("--max-redemptions", "1")
        digest = op.code_digest(op.normalize_code(out["code"]))
        subject = "usr_" + uuid.uuid4().hex
        with self.connect("genui_workspace") as c:
            c.execute("SELECT set_config('genui.organization_id',%s,true)", (ORG,))
            reservation, _, outcome = c.execute("SELECT * FROM genui_private.reserve_code(%s,%s,%s,1)", (ORG, digest, "sha256:" + "1" * 64)).fetchone()
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


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class RoleMigrationTests(OperatorDatabase, unittest.TestCase):
    """ADR-021 (migration 0025): the data migration from admin_policy rows, on a database stopped at
    schema 24 and then migrated by the runner. Zero, one and two (here three) holders per organization."""
    migrate_through = 24

    def test_the_earliest_admitted_admin_becomes_owner_and_the_others_admins(self):
        orgs = {name: str(uuid.uuid4()) for name in ("none", "one", "many", "tie")}
        people = {}
        with self.connect("postgres") as c:
            self.assertEqual(c.execute("SELECT version FROM genui_meta.schema_version").fetchone(), (24,))
            self.assertEqual(c.execute("SELECT count(*) FROM pg_attribute a JOIN pg_class t ON t.oid=a.attrelid JOIN pg_namespace n ON n.oid=t.relnamespace "
                                       "WHERE n.nspname='genui' AND t.relname='memberships' AND a.attname='role'").fetchone(), (0,))
            for name, org in orgs.items():
                c.execute("INSERT INTO genui.organizations VALUES (%s,%s,'active')", (org, "Role migration " + name))
                c.execute("INSERT INTO genui.provider_tenant_bindings VALUES (%s,%s,%s,true)", (org, ISSUER, "ten_" + org[-12:]))
                code = str(uuid.uuid4())
                c.execute("""INSERT INTO genui.access_codes(organization_id,code_id,code_digest,kind,status,destination_organization_id,max_redemptions,
                             redeemed_count,issued_at,expires_at,trial_days,issued_by,revoked_at) VALUES (%s,%s,%s,'member_invite','active',%s,10,0,now(),now()+interval '1 day',30,'test',NULL)""",
                          (org, code, "sha256:" + hashlib.sha256(org.encode()).hexdigest(), org))
                members = {}
                # Six members per organization: three would-be admins, a plain member, a revoked holder, a revoked member.
                for label, status in (("p1", "active"), ("p2", "active"), ("p3", "active"), ("plain", "active"), ("gone_admin", "revoked"), ("gone", "revoked")):
                    principal = str(uuid.uuid4()); members[label] = principal
                    c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (org, principal, ISSUER, "ten_" + org[-12:], "usr_" + principal))
                    c.execute("INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES (%s,%s,%s,1)", (org, principal, status))
                    c.execute("""INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at)
                                 VALUES (%s,%s,%s,'active','member_invite',now(),now()+interval '30 days')""", (org, principal, "0000e000-0000-4000-8000-" + principal[-12:]))
                # Admission times: p2 redeemed first, p3 an hour later, p1 never (an operator-admitted member).
                def redeemed(label, when):
                    principal = members[label]; reservation = str(uuid.uuid4())
                    c.execute("""INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at,consumed_at)
                                 VALUES (%s,%s,%s,'pending',%s,%s,%s,%s,'consumed',now()+interval '1 day',now())""",
                              (org, reservation, code, "sha256:" + "a" * 64, ISSUER, "ten_" + org[-12:], "usr_" + principal))
                    c.execute("""INSERT INTO genui.code_redemptions(organization_id,redemption_id,code_id,principal_id,pending_admission_id,entitlement_id,payload_digest,committed_at,audit_event_id)
                                 VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s)""", (org, str(uuid.uuid4()), code, principal, reservation, "0000e000-0000-4000-8000-" + principal[-12:],
                                                                      "sha256:" + "b" * 64, when, str(uuid.uuid4())))
                holders = {"none": [], "one": ["p2"], "many": ["p1", "p2", "p3", "gone_admin"], "tie": ["p1", "p2"]}[name]
                for label in holders:
                    c.execute("INSERT INTO genui.organization_permissions VALUES (%s,%s,'admin_policy')", (org, members[label]))
                if name == "many":
                    redeemed("p2", "2026-11-01T10:00:00Z")
                    redeemed("p3", "2026-11-01T11:00:00Z")
                    redeemed("gone_admin", "2026-10-01T09:00:00Z")  # the earliest, but revoked: never an owner
                people[name] = members
        # The runner resumes from 24 and applies 0025.
        with self.connect() as c:
            runtime.apply_migrations(c, runtime.migration_directory())
            c.commit()
        with self.connect("postgres") as c:
            # The runner applies every landed migration, not only 0025 (0026 followed on 2026-11-15).
            self.assertEqual(c.execute("SELECT version FROM genui_meta.schema_version").fetchone(),
                             (runtime.migration_manifest(runtime.migration_directory())["database_version"],))
            self.assertGreaterEqual(c.execute("SELECT version FROM genui_meta.schema_version").fetchone()[0], 25)
            self.assertEqual(c.execute("SELECT to_regclass('genui.organization_permissions')").fetchone(), (None,))
            roles = {name: dict(c.execute("SELECT principal_id::text,role FROM genui.memberships WHERE organization_id=%s", (org,)).fetchall())
                     for name, org in orgs.items()}
        none, one, many = (people[n] for n in ("none", "one", "many"))
        self.assertEqual(set(roles["none"].values()), {"member"}, "no holder: no owner, everyone a member")
        self.assertEqual(roles["one"][one["p2"]], "owner")
        self.assertEqual({roles["one"][one[l]] for l in ("p1", "p3", "plain", "gone_admin", "gone")}, {"member"})
        self.assertEqual(roles["many"][many["p2"]], "owner", "the earliest admitted active holder")
        self.assertEqual(roles["many"][many["p3"]], "admin", "admitted later")
        self.assertEqual(roles["many"][many["p1"]], "admin", "no admission time sorts last")
        self.assertEqual(roles["many"][many["gone_admin"]], "member", "a revoked holder keeps member")
        self.assertEqual((roles["many"][many["plain"]], roles["many"][many["gone"]]), ("member", "member"))
        # The tie: two operator-admitted holders (no redemption at all) resolve by the smaller principal id.
        tie = people["tie"]
        lower, higher = sorted([tie["p1"], tie["p2"]])
        self.assertEqual((roles["tie"][lower], roles["tie"][higher]), ("owner", "admin"), "the smaller principal id is the owner")
        self.assertEqual({roles["tie"][tie[l]] for l in ("p3", "plain", "gone_admin", "gone")}, {"member"})
        with self.connect("postgres") as c:
            self.assertEqual(c.execute("""SELECT count(*) FROM genui.memberships m WHERE m.organization_id=%s AND m.role='owner'""", (orgs["many"],)).fetchone(), (1,))
            self.assertEqual(c.execute("""SELECT count(*) FROM genui.memberships m WHERE m.organization_id=%s AND m.role='owner'""", (orgs["tie"],)).fetchone(), (1,))
            self.assertEqual(c.execute("SELECT count(*) FROM genui.memberships WHERE role NOT IN ('owner','admin','manager','member','viewer')").fetchone(), (0,))
            self.assertEqual(c.execute("SELECT relforcerowsecurity FROM pg_class WHERE oid='genui.memberships'::regclass").fetchone(), (True,))
            self.assertEqual(c.execute("SELECT relforcerowsecurity FROM pg_class WHERE oid='genui.code_redemptions'::regclass").fetchone(), (True,))


class LifecycleFixture:
    """Rows a member accumulates (APP-230's acceptance fixture), inserted as the superuser the way the
    seed and the other tests do; every id is returned so the test can prove what stayed and what went."""

    def member(self, status="active"):
        principal = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (ORG, principal, ISSUER, TENANT, "usr_" + principal))
            c.execute("INSERT INTO genui.memberships VALUES (%s,%s,%s,1)", (ORG, principal, status))
        return principal

    def space(self):
        space = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.spaces VALUES (%s,%s,'Lifecycle test space')", (ORG, space))
        return space

    def last_operator_event(self):
        return self.query("""SELECT kind,actor_principal_id::text,app_id::text,work_id::text FROM genui.audit_events
                             WHERE organization_id=%s AND writer_role='operator' ORDER BY chain_sequence DESC LIMIT 1""", (ORG,))[0]

    def epoch(self, principal):
        return self.query("SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, principal))[0][0]

    def entitle(self, principal, org=ORG, expires="now()+interval '30 days'", status="active"):
        entitlement = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at,revoked_at) "
                      "VALUES (%s,%s,%s,%s,'member_invite',now()-interval '40 days'," + expires + ",CASE WHEN %s='revoked' THEN now() END)",
                      (org, principal, entitlement, status, status))
        return entitlement

    def expires(self, principal, org=ORG):
        return self.query("SELECT status,expires_at FROM genui.entitlements WHERE organization_id=%s AND principal_id=%s ORDER BY expires_at", (org, principal))

    def until(self, days):
        return (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")

    def operator_kind_count(self, kind, org=ORG):
        return self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator' AND kind=%s", (org, kind))[0][0]


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class LifecycleTests(OperatorDatabase, LifecycleFixture, unittest.TestCase):
    """Migration 0026 (CLASS-CONTINUATION-PLAN §2.2/§2.3; APP-229, APP-230) on the latest schema."""

    def test_extend_entitlement_moves_dates_later_never_earlier_revives_a_lapse_and_is_audited(self):
        """APP-229: --principal moves one active member's entitlement to --until and appends one entitlement.extended
        event with a recomputable digest; an earlier --until changes nothing and appends nothing; --all-active reaches
        every active membership in one transaction, a lapsed-but-not-revoked member included, never a revoked membership
        or a revoked entitlement; every refusal writes nothing."""
        self.ensure_backfill()
        current, lapsed, revoked_member, revoked_entitlement, bare = (self.member() for _ in range(5))
        self.entitle(current, expires="now()+interval '10 days'")
        self.entitle(lapsed, expires="now()-interval '1 day'")
        with self.connect("postgres") as c:
            c.execute("UPDATE genui.memberships SET status='revoked' WHERE organization_id=%s AND principal_id=%s", (ORG, revoked_member))
        self.entitle(revoked_member)
        self.entitle(revoked_entitlement, status="revoked")
        events = len(self.operator_events())
        first = self.until(60)
        out = self.cli("extend-entitlement", "--organization", ORG, "--principal", current, "--until", first)
        self.assertEqual((out["organization_id"], out["until"], out["members"], out["changed"], out["unchanged"]), (ORG, first, 1, 1, 0))
        entry = out["entries"][0]
        self.assertEqual((entry["principal_id"], entry["changed"], entry["expires_at"]), (current, True, first))
        self.assertEqual(self.expires(current)[0][1].strftime("%Y-%m-%dT%H:%M:%SZ"), first)
        self.assertEqual(self.last_operator_event(), ("entitlement.extended", current, None, None))
        digest = "sha256:" + hashlib.sha256(b"forma:entitlement-extended:v1\x00" + current.encode() + b"\x00" + first.encode()).hexdigest()
        self.assertEqual(self.query("""SELECT outcome_digest::text FROM genui.audit_events WHERE organization_id=%s AND writer_role='operator'
                                       ORDER BY chain_sequence DESC LIMIT 1""", (ORG,)), [(digest,)])
        self.assertEqual(len(self.operator_events()), events + 1)
        # Never lowered: an earlier date changes nothing and is not audited; the epoch never moves.
        epoch = self.epoch(current)
        again = self.cli("extend-entitlement", "--organization", ORG, "--principal", current, "--until", self.until(30))
        self.assertEqual((again["changed"], again["unchanged"], again["entries"][0]["changed"], again["entries"][0]["expires_at"]), (0, 1, False, first))
        self.assertEqual((len(self.operator_events()), self.epoch(current)), (events + 1, epoch))
        # The whole organization: the current and the lapsed member move, the member without an entitlement is
        # reported unchanged, the revoked membership is not a target, the revoked entitlement is untouched.
        lapsed_before = self.expires(lapsed)
        self.assertLess(lapsed_before[0][1], datetime.datetime.now(datetime.timezone.utc))
        second = self.until(90)
        everyone = self.cli("extend-entitlement", "--organization", ORG, "--all-active", "--until", second)
        by_principal = {e["principal_id"]: e for e in everyone["entries"]}
        self.assertNotIn(revoked_member, by_principal)
        self.assertEqual((by_principal[current]["changed"], by_principal[current]["expires_at"], by_principal[current]["expires_before"]), (True, second, first))
        self.assertEqual((by_principal[lapsed]["changed"], by_principal[lapsed]["expires_at"]), (True, second))
        self.assertEqual((by_principal[bare]["changed"], by_principal[bare]["expires_at"]), (False, None))
        self.assertEqual((by_principal[revoked_entitlement]["changed"], by_principal[revoked_entitlement]["expires_at"]), (False, None))
        self.assertEqual((everyone["changed"], everyone["members"] - everyone["unchanged"]), (2, 2))
        self.assertEqual(self.expires(lapsed)[0][1].strftime("%Y-%m-%dT%H:%M:%SZ"), second)
        self.assertEqual(self.expires(revoked_member)[0][1].strftime("%Y-%m-%dT%H:%M:%SZ") < second, True)
        self.assertEqual(self.expires(revoked_entitlement)[0][0], "revoked")
        self.assertEqual(self.operator_events()[-2:], [("entitlement.extended",)] * 2)
        self.assertEqual(self.operator_kind_count("entitlement.extended"), 3)
        # Refusals, each before any write: the clock bounds, the target shape, an unknown or revoked member,
        # a member with no active entitlement named alone, and a suspended organization.
        other = str(uuid.uuid4())
        self.cli("provision", "--organization", other, "--display-name", "Suspended", "--issuer", ISSUER, "--provider-tenant", "ten_" + other[-12:])
        with self.connect("postgres") as c:
            principal = str(uuid.uuid4())
            c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (other, principal, ISSUER, "ten_" + other[-12:], "usr_" + principal))
            c.execute("INSERT INTO genui.memberships VALUES (%s,%s,'active',1)", (other, principal))
        self.entitle(principal, org=other)
        self.cli("suspend-organization", "--organization", other)
        before = self.dump()
        for args, reason in [(["--principal", current, "--until", self.until(-1)], "until_in_past"),
                             (["--principal", current, "--until", self.until(401)], "until_too_far"),
                             (["--principal", current, "--until", "2027-02-15 00:00:00"], "malformed_until"),
                             (["--principal", current, "--until", "2027-02-30T00:00:00Z"], "malformed_until"),
                             (["--principal", current, "--all-active", "--until", second], "target_refused"),
                             (["--until", second], "target_refused"),
                             (["--principal", str(uuid.uuid4()), "--until", second], "member_unknown"),
                             (["--principal", revoked_member, "--until", second], "member_inactive"),
                             (["--principal", bare, "--until", second], "no_active_entitlement")]:
            p = self.cli("extend-entitlement", "--organization", ORG, *args, expect=1)
            self.assertEqual(json.loads(p.stderr), {"refused": reason}, args)
        p = self.cli("extend-entitlement", "--organization", other, "--principal", principal, "--until", second, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "organization_inactive"})
        p = self.cli("extend-entitlement", "--organization", other, "--all-active", "--until", second, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "organization_inactive"})
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)

    def member_world(self, erased, other, owner_app=True):
        """What `erased` leaves behind: an app of their own with a work, records, a share, a live share accepted by a
        guest, a folder with two uploads, two sources from them (one bound by `other`'s work, one not), records
        they entered in `other`'s app, a session, an agent token with an attribution and a log row, a Slack link,
        a card, a label, an artifact kind, grants and a name. Returns the ids the assertions need."""
        ids = {"space": self.space()}
        with self.connect("postgres") as c:
            def put(table, values):
                c.execute("INSERT INTO genui." + table + " VALUES (" + ",".join(["%s"] * len(values)) + ")", values)
            digest = "sha256:" + "e" * 64
            for name, person in (("mine", erased), ("theirs", other)):
                app, revision, policy, publication, work, collection, record = (str(uuid.uuid4()) for _ in range(7))
                ids[name] = {"app": app, "revision": revision, "publication": publication, "work": work, "collection": collection, "record": record}
                put("apps", (ORG, app, ids["space"], person))
                put("definition_revisions", (ORG, app, revision, 1, digest, b"{}"))
                c.execute("INSERT INTO genui.definition_inputs VALUES (%s,%s,%s,'data','source',%s,%s)", (ORG, app, revision, digest, b"{}"))
                c.execute("INSERT INTO genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema) VALUES (%s,%s,%s,'entries',1,%s,%s)",
                          (ORG, app, revision, digest, b"{}"))
                put("publication_policies", (ORG, policy, 1, "owner_publish"))
                c.execute("""INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,
                             author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope,publication_sequence)
                             VALUES (%s,%s,%s,%s,%s,%s,1,'owner_publish',%s,%s,1,1,1,%s,now(),'\\x7b7d',%s)""",
                          (ORG, app, publication, revision, digest, policy, person, person, digest, 1 if name == "mine" else 2))
                c.execute("INSERT INTO genui.publication_inputs VALUES (%s,%s,%s,%s,'data','source',%s)", (ORG, app, publication, revision, digest))
                c.execute("INSERT INTO genui.publication_record_schemas VALUES (%s,%s,%s,%s,'entries',1,%s)", (ORG, app, publication, revision, digest))
                c.execute("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES (%s,%s,%s,%s,1,'',%s)",
                          (ORG, app, work, publication, person))
                put("record_collections", (ORG, app, work, publication, collection, "entries", 1))
                put("internal_records", (ORG, app, work, collection, record, 1))
                # The erased member authored the first version of every record, in their own app and in the other's.
                c.execute("""INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values)
                             VALUES (%s,%s,%s,%s,%s,1,%s,%s,%s,now(),'\\x7b7d')""", (ORG, app, work, collection, record, erased, erased, 100 if name == "mine" else 200))
                put("app_permissions", (ORG, person, app, "edit_definition"))
                put("work_permissions", (ORG, person, app, work, "record_write"))
            # Grants to the erased member on the other's app, and a share each way.
            put("app_permissions", (ORG, erased, ids["theirs"]["app"], "discover"))
            put("work_permissions", (ORG, erased, ids["theirs"]["app"], ids["theirs"]["work"], "record_read"))
            ids["share_out"], ids["share_in"] = str(uuid.uuid4()), str(uuid.uuid4())
            c.execute("INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at,role) VALUES (%s,%s,%s,%s,%s,%s,'{record_read}',%s,now(),'viewer')",
                      (ORG, ids["share_out"], erased, other, ids["mine"]["app"], ids["mine"]["work"], str(uuid.uuid4())))
            c.execute("INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at,role) VALUES (%s,%s,%s,%s,%s,%s,'{record_read}',%s,now(),'viewer')",
                      (ORG, ids["share_in"], other, erased, ids["theirs"]["app"], ids["theirs"]["work"], str(uuid.uuid4())))
            # A live share the erased member sponsors, accepted and linked by a guest.
            ids["guest"], ids["live"] = str(uuid.uuid4()), str(uuid.uuid4())
            c.execute("INSERT INTO genui_identity.accounts(account_id,issuer,provider_tenant_id,provider_subject,verified_email) VALUES (%s,'guest-issuer','ten_guest',%s,'guest@example.test')",
                      (ids["guest"], "usr_guest_" + ids["guest"][-12:]))
            c.execute("""INSERT INTO genui.live_work_shares(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,accepted_account_id,accepted_at,
                         publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at)
                         VALUES (%s,%s,%s,%s,%s,1,'guest@example.test',%s,now(),%s,%s,1,'{}','{}','Shared',false,1,%s,%s,now(),now()+interval '7 days')""",
                      (ORG, ids["live"], ids["mine"]["app"], ids["mine"]["work"], erased, ids["guest"], ids["mine"]["publication"], digest, "idem-" + ids["live"][:24], digest))
            c.execute("INSERT INTO genui_identity.linked_apps(account_id,organization_id,share_id,created_at,grant_version) VALUES (%s,%s,%s,now(),1)", (ids["guest"], ORG, ids["live"]))
            c.execute("""INSERT INTO genui_identity.share_events(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at)
                         VALUES (%s,%s,1,%s,%s,'share.accepted',1,'\\x7b7d',%s,%s,now())""", (ids["guest"], str(uuid.uuid4()), ORG, ids["live"], "sha256:" + "0" * 64, "sha256:" + "1" * 64))
            # The folder, two uploads, and the sources registered from them: one bound by the other's work.
            ids["box"], ids["files"], ids["sources"] = str(uuid.uuid4()), [str(uuid.uuid4()) for _ in range(3)], [str(uuid.uuid4()) for _ in range(3)]
            ids["revisions"] = [str(uuid.uuid4()) for _ in range(3)]
            put("boxes", (ORG, ids["box"], erased, "Uploads", "uploads", None, datetime.datetime.now(datetime.timezone.utc)))
            for n in range(3):
                normalized = "sha256:" + ("%02x" % (0xa0 + n)) * 32
                c.execute("INSERT INTO genui.box_files VALUES (%s,%s,%s,%s,'csv','text/csv',12,%s,%s,%s,now())",
                          (ORG, ids["box"], ids["files"][n], "data%d.csv" % n, "sha256:" + ("%02x" % (0xc0 + n)) * 32, "o/%s/b/%s/f/%s" % (ORG, ids["box"], ids["files"][n]), erased))
                put("sources", (ORG, ids["sources"][n]))
                c.execute("INSERT INTO genui.source_revisions(organization_id,source_id,revision_id,content_digest,normalized_digest,contract_digest,canonical_envelope) VALUES (%s,%s,%s,%s,%s,%s,'\\x7b7d')",
                          (ORG, ids["sources"][n], ids["revisions"][n], "sha256:" + ("%02x" % (0xc0 + n)) * 32, normalized, digest))
                c.execute("INSERT INTO genui.box_file_sources VALUES (%s,%s,%s,%s,%s,%s,%s,3,%s,now())",
                          (ORG, ids["box"], ids["files"][n], "sha256:" + ("%02x" % (0xc0 + n)) * 32, ids["sources"][n], ids["revisions"][n], normalized, erased))
                put("source_permissions", (ORG, erased, ids["sources"][n], "source_read"))
            c.execute("""INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,source_id,source_revision_id,source_digest)
                         VALUES (%s,%s,%s,%s,'data','source',%s,%s,%s,%s)""", (ORG, ids["theirs"]["app"], ids["theirs"]["work"], ids["theirs"]["publication"], digest,
                                                                           ids["sources"][1], ids["revisions"][1], "sha256:" + "a1" * 32))
            put("source_permissions", (ORG, other, ids["sources"][1], "source_read"))
            # The third source is bound only by a saved work the erased member created inside the other's app (review MINOR-4):
            # the work stays (an entry in someone else's app), the source and its binding go.
            ids["my_work_in_theirs"] = str(uuid.uuid4())
            c.execute("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES (%s,%s,%s,%s,1,'',%s)",
                      (ORG, ids["theirs"]["app"], ids["my_work_in_theirs"], ids["theirs"]["publication"], erased))
            c.execute("""INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,source_id,source_revision_id,source_digest)
                         VALUES (%s,%s,%s,%s,'data','source',%s,%s,%s,%s)""", (ORG, ids["theirs"]["app"], ids["my_work_in_theirs"], ids["theirs"]["publication"], digest,
                                                                           ids["sources"][2], ids["revisions"][2], "sha256:" + "a2" * 32))
            # Someone else's folder for the erased member's app, with that person's file: detached, never deleted (review MINOR-3).
            ids["foreign_box"], ids["foreign_file"] = str(uuid.uuid4()), str(uuid.uuid4())
            put("boxes", (ORG, ids["foreign_box"], other, "Their folder", "app", ids["mine"]["app"], datetime.datetime.now(datetime.timezone.utc)))
            c.execute("INSERT INTO genui.box_files VALUES (%s,%s,%s,'theirs.csv','csv','text/csv',12,%s,%s,%s,now())",
                      (ORG, ids["foreign_box"], ids["foreign_file"], "sha256:" + "d9" * 32, "o/%s/b/%s/f/%s" % (ORG, ids["foreign_box"], ids["foreign_file"]), other))
            # The admission the member came in through: a consumed pending reservation naming their subject (review MAJOR-1),
            # and an abandoned guest sign-in flow for the live share, never swept (review MAJOR-2).
            ids["code"], ids["reservation"] = str(uuid.uuid4()), str(uuid.uuid4())
            c.execute("""INSERT INTO genui.access_codes(organization_id,code_id,code_digest,kind,status,destination_organization_id,max_redemptions,redeemed_count,
                         issued_at,expires_at,trial_days,issued_by) VALUES (%s,%s,%s,'member_invite','exhausted',%s,1,1,now()-interval '2 days',now()+interval '10 days',30,'test')""",
                      (ORG, ids["code"], "sha256:" + hashlib.sha256(ids["code"].encode()).hexdigest(), ORG))
            c.execute("""INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at,consumed_at,notice_version)
                         VALUES (%s,%s,%s,'pending',%s,%s,%s,%s,'consumed',now()+interval '1 day',now()-interval '1 day',1)""",
                      (ORG, ids["reservation"], ids["code"], "sha256:" + "c" * 64, ISSUER, TENANT, "usr_" + erased))
            c.execute("""INSERT INTO genui_identity.shared_auth_flows(flow_id,owner_organization_id,share_id,provider_organization_id,cookie_digest,state_digest,sealed_verifier,created_at,expires_at)
                         VALUES (%s,%s,%s,%s,%s,%s,'\\x01',now()-interval '1 day',now()-interval '1 day'+interval '5 minutes')""",
                      (str(uuid.uuid4()), ORG, ids["live"], ORG, "sha256:" + uuid.uuid4().hex * 2, "sha256:" + "5" * 64))
            # Sessions, tokens, links, cards, labels, kinds, a name, a role, a consent.
            ids["entitlement"] = self_entitlement = str(uuid.uuid4())
            c.execute("INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) VALUES (%s,%s,%s,'active','member_invite',now()-interval '1 day',now()+interval '30 days')",
                      (ORG, erased, self_entitlement))
            c.execute("""INSERT INTO genui.sessions(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,refresh_token_ref,created_at,last_seen_at,absolute_expires_at)
                         VALUES (%s,%s,%s,%s,1,%s,%s,%s,now(),now(),now()+interval '1 hour')""", (ORG, str(uuid.uuid4()), "sha256:" + uuid.uuid4().hex * 2, erased, self_entitlement, "0" * 64, str(uuid.uuid4())))
            c.execute("""INSERT INTO genui.pre_access_sessions(organization_id,session_id,cookie_digest,issuer,provider_tenant_id,provider_subject,email_verified,flow_kind,refresh_token_ref,created_at,last_seen_at,expires_at)
                         VALUES (%s,%s,%s,%s,%s,%s,true,'signin',%s,now(),now(),now()+interval '10 minutes')""", (ORG, str(uuid.uuid4()), "sha256:" + uuid.uuid4().hex * 2, ISSUER, TENANT, "usr_" + erased, str(uuid.uuid4())))
            ids["token"] = str(uuid.uuid4())
            c.execute("INSERT INTO genui.agent_tokens(organization_id,token_id,token_digest,principal_id,label,membership_epoch_at_mint,registry_sha256,created_at,expires_at) VALUES (%s,%s,%s,%s,'my laptop',1,%s,now(),now()+interval '30 days')",
                      (ORG, ids["token"], "sha256:" + uuid.uuid4().hex * 2, erased, "0" * 64))
            c.execute("INSERT INTO genui.agent_attributions(organization_id,attribution_id,token_id,app_id,work_id,created_at) VALUES (%s,%s,%s,%s,%s,now())",
                      (ORG, str(uuid.uuid4()), ids["token"], ids["theirs"]["app"], ids["theirs"]["work"]))
            c.execute("""INSERT INTO genui.agent_access_log(organization_id,entry_id,chain_sequence,token_id,principal_id,agent_label,tool,touched_apps,touched_works,touched_records,
                         touched_folders,touched_files,outcome,reason,space,resource_label,duration_ms,occurred_at,previous_hash,hash)
                         VALUES (%s,%s,1,%s,%s,'my laptop','forma_list_apps','{}','{}','{}','{}','{}','allowed',NULL,NULL,NULL,1,now(),%s,%s)""",
                      (ORG, str(uuid.uuid4()), ids["token"], erased, "sha256:" + "0" * 64, "sha256:" + "2" * 64))
            c.execute("INSERT INTO genui.slack_links(organization_id,link_id,principal_id,team_id,slack_user_id,membership_epoch_at_link,linked_at) VALUES (%s,%s,%s,'T1','U1',1,now())",
                      (ORG, str(uuid.uuid4()), erased))
            c.execute("""INSERT INTO genui.member_cards(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,everyone,created_at,updated_at)
                         VALUES (%s,%s,%s,'card',%s,%s,'cards','','A card','\\x7b7d',true,now(),now())""", (ORG, str(uuid.uuid4()), erased, ids["theirs"]["app"], ids["theirs"]["work"]))
            c.execute("INSERT INTO genui.member_labels(organization_id,principal_id,app_id,card,name,updated_at) VALUES (%s,%s,%s,'','Their app',now())", (ORG, erased, ids["theirs"]["app"]))
            c.execute("INSERT INTO genui.app_labels(organization_id,app_id,card,name,updated_by,updated_at) VALUES (%s,%s,'','Our app',%s,now())", (ORG, ids["theirs"]["app"], erased))
            c.execute("INSERT INTO genui.work_artifact_kinds(organization_id,principal_id,app_id,work_id,kind) VALUES (%s,%s,%s,%s,'card')", (ORG, erased, ids["theirs"]["app"], ids["theirs"]["work"]))
            c.execute("UPDATE genui.memberships SET display_name='Alice',role='admin' WHERE organization_id=%s AND principal_id=%s", (ORG, erased))
            # D7.7: the organization's sequence stays ahead of every committed revision (verify checks it).
            c.execute("INSERT INTO genui.organization_sequences VALUES (%s,1000) ON CONFLICT (organization_id) DO UPDATE SET next_commit_sequence=greatest(genui.organization_sequences.next_commit_sequence,1000)", (ORG,))
            c.execute("INSERT INTO genui.member_consents VALUES (%s,%s,1,now(),%s)", (ORG, erased, str(uuid.uuid4())))
        return ids

    def naming(self, principal, org=ORG):
        """Every genui row still naming the principal through a key column, by table.column (the proof's own query)."""
        found = {}
        with self.connect("postgres") as c:
            for table, column in op.principal_columns(c):
                n = c.execute("SELECT count(*) FROM genui." + table + " WHERE organization_id=%s AND " + column + "=%s", (org, principal)).fetchone()[0]
                if n:
                    found[table + "." + column] = n
        return found

    def test_erase_member_removes_their_own_world_keeps_their_entries_elsewhere_nameless_and_is_audited_once(self):
        """APP-230: after revoke-membership and erase-member, no content row names the member except the principal
        (subject tombstoned), the revoked nameless membership, the consent, the revoked entitlement, the revoked agent
        token and its log, and the entries they made in someone else's app; their own app, folder, uploads, unbound
        source, sessions, token attributions, Slack link, cards, labels, grants and shares are gone; the guest's chain
        row stays; the other member's app, records and bound source still read; one member.erased event; a repeat is
        harmless; the refusals write nothing."""
        self.ensure_backfill()
        erased, other = self.member(), self.member()
        ids = self.member_world(erased, other)
        before_naming = self.naming(erased)
        self.assertGreaterEqual(len(before_naming), 15, before_naming)
        theirs_before = self.query("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id=%s AND app_id=%s", (ORG, ids["theirs"]["app"]))
        guest_chain = self.query("SELECT count(*) FROM genui_identity.share_events WHERE account_id=%s", (ids["guest"],))
        # Refusals before any write: an active membership, a wrong confirmation, an unknown principal.
        before = self.dump()
        p = self.cli("erase-member", "--organization", ORG, "--principal", erased, "--confirm", erased, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "member_active"})
        p = self.cli("erase-member", "--organization", ORG, "--principal", erased, "--confirm", other, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "confirm_mismatch"})
        unknown = str(uuid.uuid4())
        p = self.cli("erase-member", "--organization", ORG, "--principal", unknown, "--confirm", unknown, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "member_unknown"})
        self.assertEqual(self.dump(), before)
        # The procedure: remove, then erase.
        self.assertTrue(self.cli("revoke-membership", "--organization", ORG, "--principal", erased)["changed"])
        epoch = self.epoch(erased)
        events = self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (ORG,))[0][0]
        out = self.cli("erase-member", "--organization", ORG, "--principal", erased, "--confirm", erased)
        self.assertEqual((out["status"], out["changed"], out["principal_id"], out["membership_epoch"]), ("erased", True, erased, epoch + 1))
        self.assertEqual((out["apps"], out["boxes"], out["boxes_detached"], out["sources"], out["sources_kept"], out["live_shares"]), (1, 1, 1, 2, 1, 1))
        self.assertEqual(sorted(out["object_keys"]), sorted("o/%s/b/%s/f/%s" % (ORG, ids["box"], f) for f in ids["files"]))
        self.assertEqual(out["source_revisions"], sorted([[ids["sources"][0], ids["revisions"][0]], [ids["sources"][2], ids["revisions"][2]]]))
        self.assertEqual((out["agent_tokens_revoked"], out["agent_tokens_kept"], out["entitlements_revoked"], out["provider_subject"]), (1, 1, 1, "usr_" + erased))
        self.assertEqual((out["audit_events_kept"], out["deleted"]["apps"], out["deleted"]["boxes"], out["deleted"]["box_files"], out["deleted"]["sources"]), (events, 1, 1, 3, 2))
        self.assertEqual((out["deleted"]["genui_identity.shared_auth_flows"], out["deleted"]["genui_identity.linked_apps"], out["deleted"]["live_work_shares"], out["deleted"]["work_shares"],
                          out["deleted"]["sessions"], out["deleted"]["pre_access_sessions"], out["deleted"]["slack_links"], out["deleted"]["agent_attributions"],
                          out["deleted"]["internal_record_revisions"], out["deleted"]["work_bindings"]), (1, 1, 1, 2, 1, 1, 1, 1, 1, 1))
        # The reservation names the tombstone, never the subject; the abandoned flow is gone; the foreign folder is detached with its file.
        self.assertEqual(self.query("SELECT provider_subject FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s", (ORG, ids["reservation"])), [("erased:" + erased,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.shared_auth_flows WHERE owner_organization_id=%s AND share_id=%s", (ORG, ids["live"])), [(0,)])
        self.assertEqual(self.query("SELECT kind,app_id::text,owner_principal_id::text FROM genui.boxes WHERE organization_id=%s AND box_id=%s", (ORG, ids["foreign_box"])), [("box", None, other)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.box_files WHERE organization_id=%s AND box_id=%s AND file_id=%s", (ORG, ids["foreign_box"], ids["foreign_file"])), [(1,)])
        # The member's own saved work inside the other's app stays, unbound; its source went.
        self.assertEqual(self.query("SELECT count(*) FROM genui.saved_work WHERE organization_id=%s AND work_id=%s", (ORG, ids["my_work_in_theirs"])), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.work_bindings WHERE organization_id=%s AND work_id=%s", (ORG, ids["my_work_in_theirs"])), [(0,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.sources WHERE organization_id=%s AND source_id=%s", (ORG, ids["sources"][2])), [(0,)])
        # What names the member now: exactly the kept set.
        self.assertEqual(self.naming(erased), {"memberships.principal_id": 1, "member_consents.principal_id": 1,
                                               "entitlements.principal_id": 1, "agent_tokens.principal_id": 1, "internal_record_revisions.creator_id": 1,
                                               "internal_record_revisions.updater_id": 1, "app_labels.updated_by": 1, "saved_work.created_by": 1})
        self.assertEqual(self.query("SELECT status,display_name,role,membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, erased)),
                         [("revoked", None, "member", epoch + 1)])
        self.assertEqual(self.query("SELECT provider_subject FROM genui.principals WHERE organization_id=%s AND principal_id=%s", (ORG, erased)), [("erased:" + erased,)])
        self.assertEqual(self.query("SELECT status FROM genui.entitlements WHERE organization_id=%s AND principal_id=%s", (ORG, erased)), [("revoked",)])
        self.assertEqual(self.query("SELECT label,revoked_at IS NOT NULL FROM genui.agent_tokens WHERE organization_id=%s AND token_id=%s", (ORG, ids["token"])), [("erased", True)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.agent_access_log WHERE organization_id=%s AND token_id=%s", (ORG, ids["token"])), [(1,)])
        # Their own world is gone; the other member's app, records (authored by the nameless principal) and bound source stay.
        for table, column, value in (("apps", "app_id", ids["mine"]["app"]), ("saved_work", "app_id", ids["mine"]["app"]), ("internal_record_revisions", "app_id", ids["mine"]["app"]),
                                     ("boxes", "box_id", ids["box"]), ("box_files", "box_id", ids["box"]), ("sources", "source_id", ids["sources"][0]),
                                     ("source_revisions", "source_id", ids["sources"][0]), ("live_work_shares", "share_id", ids["live"]), ("work_shares", "share_id", ids["share_out"]),
                                     ("work_shares", "share_id", ids["share_in"])):
            self.assertEqual(self.query("SELECT count(*) FROM genui." + table + " WHERE organization_id=%s AND " + column + "=%s", (ORG, value)), [(0,)], table)
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.linked_apps WHERE organization_id=%s AND share_id=%s", (ORG, ids["live"])), [(0,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui_identity.share_events WHERE account_id=%s", (ids["guest"],)), guest_chain)
        self.assertEqual(self.query("SELECT count(*) FROM genui.apps WHERE organization_id=%s AND app_id=%s AND owner_principal_id=%s", (ORG, ids["theirs"]["app"], other)), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.internal_record_revisions WHERE organization_id=%s AND app_id=%s", (ORG, ids["theirs"]["app"])), theirs_before)
        self.assertEqual(self.query("""SELECT r.creator_id::text,m.display_name FROM genui.internal_record_revisions r JOIN genui.memberships m
                                       ON m.organization_id=r.organization_id AND m.principal_id=r.creator_id WHERE r.organization_id=%s AND r.app_id=%s""", (ORG, ids["theirs"]["app"])),
                         [(erased, None)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.sources WHERE organization_id=%s AND source_id=%s", (ORG, ids["sources"][1])), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.work_bindings WHERE organization_id=%s AND source_id=%s", (ORG, ids["sources"][1])), [(1,)])
        self.assertEqual(self.query("SELECT count(*) FROM genui.box_file_sources WHERE organization_id=%s AND source_id=%s", (ORG, ids["sources"][1])), [(0,)])
        # Audited once, on the operator chain, with a recomputable digest; the chains verify.
        self.assertEqual(self.last_operator_event(), ("member.erased", erased, None, None))
        self.assertEqual(self.query("SELECT count(*) FROM genui.audit_events WHERE organization_id=%s", (ORG,)), [(events + 1,)])
        digest = "sha256:" + hashlib.sha256(b"forma:member-erased:v1\x00" + erased.encode()).hexdigest()
        self.assertEqual(self.query("SELECT outcome_digest::text FROM genui.audit_events WHERE organization_id=%s AND event_id=%s", (ORG, out["audit_event"])), [(digest,)])
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)
        # A repeat is harmless (review MAJOR-1: the consumed reservation now names the tombstone and must not match again):
        # nothing to do, no epoch move, no second member.erased, every byte the same.
        before, erased_events = self.dump(), self.operator_kind_count("member.erased")
        again = self.cli("erase-member", "--organization", ORG, "--principal", erased, "--confirm", erased)
        self.assertEqual((again["changed"], again["rows_deleted"], again["audit_event"], again["provider_subject"], again["membership_epoch"]), (False, 0, None, None, epoch + 1))
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.operator_kind_count("member.erased"), erased_events)

    def test_erase_member_refuses_the_last_owner_unless_the_organization_is_being_purged(self):
        """APP-230: a revoked owner with no other active owner refuses last_owner while the organization is active;
        once another owner exists it goes; in a suspended organization (the purge path) the last owner is erased."""
        self.ensure_backfill()
        owner = self.member()
        self.cli("set-role", "--organization", ORG, "--principal", owner, "--role", "owner")
        self.assertEqual(self.query("SELECT count(*) FROM genui.memberships WHERE organization_id=%s AND role='owner' AND status='active' AND principal_id<>%s", (ORG, owner)), [(0,)])
        self.cli("revoke-membership", "--organization", ORG, "--principal", owner)
        before = self.dump()
        p = self.cli("erase-member", "--organization", ORG, "--principal", owner, "--confirm", owner, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "last_owner"})
        self.assertEqual(self.dump(), before)
        successor = self.member()
        self.cli("set-role", "--organization", ORG, "--principal", successor, "--role", "owner")
        out = self.cli("erase-member", "--organization", ORG, "--principal", owner, "--confirm", owner)
        self.assertEqual((out["changed"], out["rows_deleted"]), (True, 0))
        self.assertEqual(self.query("SELECT role,display_name FROM genui.memberships WHERE organization_id=%s AND principal_id=%s", (ORG, owner)), [("member", None)])
        self.assertEqual(self.last_operator_event(), ("member.erased", owner, None, None))
        # The purge path: a suspended organization's sole owner.
        org = str(uuid.uuid4()); tenant = "ten_" + org[-12:]
        self.cli("provision", "--organization", org, "--display-name", "Purging", "--issuer", ISSUER, "--provider-tenant", tenant)
        sole = str(uuid.uuid4())
        with self.connect("postgres") as c:
            c.execute("INSERT INTO genui.principals VALUES (%s,%s,%s,%s,%s)", (org, sole, ISSUER, tenant, "usr_" + sole))
            c.execute("INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch,role) VALUES (%s,%s,'revoked',2,'owner')", (org, sole))
        p = self.cli("erase-member", "--organization", org, "--principal", sole, "--confirm", sole, expect=1)
        self.assertEqual(json.loads(p.stderr), {"refused": "last_owner"})
        self.cli("suspend-organization", "--organization", org)
        purged = self.cli("erase-member", "--organization", org, "--principal", sole, "--confirm", sole)
        self.assertEqual((purged["changed"], purged["provider_subject"]), (True, "usr_" + sole))
        self.assertEqual(self.query("SELECT provider_subject FROM genui.principals WHERE organization_id=%s AND principal_id=%s", (org, sole)), [("erased:" + sole,)])
        self.assertEqual(self.cli("verify", "--organization", org)["verified"], True)
        # The purge still works afterwards.
        self.assertEqual(self.cli("purge-organization", "--organization", org)["status"], "purged")


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class LifecycleMigrationTests(OperatorDatabase, LifecycleFixture, unittest.TestCase):
    """Migration 0026 on a database stopped at schema 25 and then migrated by the runner: below it both commands
    refuse (the catalog is not the manifest's) and write nothing; after it the kinds, the policy and the two
    genui_owner definers exist and extend-entitlement works."""
    migrate_through = 25

    def test_the_runner_takes_25_to_26_and_the_commands_refuse_before_it(self):
        member = self.member()
        self.entitle(member, expires="now()-interval '1 day'")
        with self.connect("postgres") as c:
            self.assertEqual(c.execute("SELECT version FROM genui_meta.schema_version").fetchone(), (25,))
            self.assertEqual(c.execute("SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='genui_private' AND p.proname IN ('extend_entitlement','erase_member')").fetchone(), (0,))
            self.assertNotIn("entitlement.extended", c.execute("SELECT pg_get_expr(polwithcheck,polrelid) FROM pg_policy WHERE polname='operator_chain' AND polrelid='genui.audit_events'::regclass").fetchone()[0])
        before = self.dump()
        until = self.until(60)
        self.cli("extend-entitlement", "--organization", ORG, "--principal", member, "--until", until, expect=1)
        self.cli("erase-member", "--organization", ORG, "--principal", member, "--confirm", member, expect=1)
        self.assertEqual(self.dump(), before)
        with self.connect() as c:
            runtime.apply_migrations(c, runtime.migration_directory())
            c.commit()
        self.ensure_backfill()
        with self.connect("postgres") as c:
            self.assertEqual(c.execute("SELECT version FROM genui_meta.schema_version").fetchone(), (26,))
            functions = c.execute("""SELECT p.proname,pg_get_userbyid(p.proowner),p.prosecdef,p.proisstrict FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
                                     WHERE n.nspname='genui_private' AND p.proname IN ('extend_entitlement','erase_member') ORDER BY 1""").fetchall()
            self.assertEqual(functions, [("erase_member", "genui_owner", True, True), ("extend_entitlement", "genui_owner", True, True)])
            check = c.execute("SELECT pg_get_expr(polwithcheck,polrelid) FROM pg_policy WHERE polname='operator_chain' AND polrelid='genui.audit_events'::regclass").fetchone()[0]
            self.assertIn("entitlement.extended", check); self.assertIn("member.erased", check)
            self.assertEqual(c.execute("SELECT polroles::regrole[]::text[] FROM pg_policy WHERE polname='operator_scope' AND polrelid='genui_identity.linked_apps'::regclass").fetchone(), (["genui_owner"],))
            self.assertEqual(c.execute("SELECT relforcerowsecurity FROM pg_class WHERE oid='genui_identity.linked_apps'::regclass").fetchone(), (True,))
            # EXECUTE is the owner's alone: no application role may call the operator definers.
            self.assertEqual(c.execute("SELECT bool_or(has_function_privilege(r.rolname,'genui_private.erase_member(uuid,uuid)','EXECUTE')) FROM pg_roles r WHERE r.rolname IN ('genui_workspace','genui_control','genui_data','genui_worker','genui_admit')").fetchone(), (False,))
        revived = self.cli("extend-entitlement", "--organization", ORG, "--principal", member, "--until", until)
        self.assertEqual((revived["changed"], revived["entries"][0]["expires_at"]), (1, until))
        self.assertEqual(self.expires(member)[0][0], "active")
        self.assertEqual(self.cli("verify", "--organization", ORG)["verified"], True)
