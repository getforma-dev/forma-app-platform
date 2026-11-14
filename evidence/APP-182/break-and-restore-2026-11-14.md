# Break-and-restore evidence: W1 consent, names and organization admin (2026-11-14)

Branch `trial/w1-consent-admin`, worktree `forma-work`. Each break edits one rule
in place, runs the test that guards it, records the failing assertion, and
restores the file from git. The runs below are the ones the review (M2) asked
for, plus the SQL breaks redone correctly (see "a finding" at the end). Logs are
local only (`break-*.log` in the session scratchpad); the lines quoted here are
copied from them.

## (a) `consented_code` accepts a lone `code` (`app/src/auth/invite.rs`)

Break: a third match arm `[(a, code)] if a == "code" => Some(code.as_str())`.

- `cargo test --locked -p forma-app-platform --lib auth::invite::tests::the_form_is_exactly_a_code_and_the_consent`
  fails: `assertion left == right failed: [("code", "ABCD")]  left: Some("ABCD")  right: None`
  (`invite.rs:547`).
- `just campaign invite` fails at the D5 check (`invite_tests.rs:1039`): the
  consent-less post of a valid code is handed off instead of refused,
  `left: (303, "")  right: (400, "page=invite;failed=true;limited=false")`.
- The router-level extra-field test (`invite_entry_refuses_before_any_work`)
  still passes with this break: an unissued code and a missing consent are the
  same neutral page by design, so that test cannot distinguish them. The rule
  is proven by the unit test and the campaign.

Restored: `git checkout -- app/src/auth/invite.rs`; both tests pass again.

## (b) `admit_redemption` records no consent (`0023_consent_names_org_admin.sql`)

Break: the whole consent block of the admitted branch removed (the
`admission.consented` append and the `member_consents` INSERT), the manifest
repinned and `catalog-v23.json` regenerated from the broken SQL in a disposable
PostgreSQL, so the break reaches the tests instead of the catalog pin.

- `just campaign org-admin`: fails at check 1 (`org_admin_campaign.rs:238`), the consent row count: `left: 0  right: 1`.
- Schema qualifier (`qualify_tenant_schema.py`): fails at the admission footprint (`qualify_tenant_schema.py:827`), the `audit_events: 2` delta (one event, not two; no consent row).

Restored: `git checkout -- app/migrations/0023_consent_names_org_admin.sql app/migrations/manifest.json app/migrations/catalog-v23.json`.

## (c) The sole admin removing itself answers `self`, not `last_admin`

Break: the `IF v_others=0 THEN 'last_admin' ELSE 'self'` branch of
`admin_remove_member` replaced by `'self'`; catalog regenerated as in (b).

- `just campaign org-admin`: fails at `org_admin_campaign.rs:522`, `the sole admin removing itself: left: Refused("self")  right: Refused("last_admin")`.

## (d) Removal leaves the membership active and the epoch unmoved

Break: `UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1`
replaced by `SET status='active'`; catalog regenerated as in (b).

- `just campaign org-admin`: fails at `org_admin_campaign.rs:608`: `left: "active"  right: "revoked"` (the membership row after the removal).

## Also proven in the first pass (Rust and page rules, no catalog involved)

- `is_org_admin` answering `Ok(true)` on `Denied` (`org_admin.rs`): the
  campaign fails at `assertion failed: !stores.workspace.is_org_admin(&principal)`
  (`org_admin_campaign.rs:252`).
- The notice text without the age sentence: `auth::notice::tests` fails.
- The directory ignoring member names: `directory::tests::member_names_are_per_organization_and_bounded`
  fails (`left: "Member 00000000"  right: "Ada Lovelace"`).
- The page without the consent checkbox (`app.tsx`, `dist` rebuilt): the SSR
  test fails at `body.contains("name=\"consent\"") && body.contains("type=\"checkbox\"")`
  (`main.rs:726`).

## A finding

The first pass broke the SQL without regenerating the catalog. All three SQL
breaks then failed at fixture start (`qualification.rs:50`, the migration
runner's catalog check), not at the intended assertion: `catalog-v23.json`
pins every definer's source, so a changed function body is refused before any
test runs. Those three runs proved the pin, not the rule; they were redone as
(b), (c) and (d) above with the catalog regenerated for the broken SQL.
