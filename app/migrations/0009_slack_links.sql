-- ADR-011 (APP-100, pitch sub-branch pivot/slack): a Slack user linked to a
-- member. A row says "Slack user U in workspace T acts as member P of this
-- organization": it is written only when P, signed in to the canvas under a
-- live workspace session, confirms a short-lived HMAC-signed link token that
-- the Slack user obtained with `/forma link` (the token names T, U and a
-- nonce; the row is written for the confirming member's own organization and
-- principal, never for one named by Slack). The member's membership epoch at
-- link time is recorded: a Slack request becomes a TrustedRequest only through
-- storage::sessions (`resolve_slack_link`), which requires the row unrevoked,
-- the member's CURRENT epoch equal to this one (a sign-out or an operator
-- revocation advances it and so ends the link), an active organization and
-- membership, a live entitlement, the principal's provider binding active and
-- this process's registry identity, exactly as the session path. A Slack user
-- id alone confers nothing (ADR-011 I1/I2).
--
-- Uniqueness is (organization_id, team_id, slack_user_id): every genui index
-- leads with organization_id (ADR-007 D7.5), so the same Slack user could be
-- linked in two organizations; resolution scans the registry's organizations
-- and refuses (link prompt) when more than one live link answers. Only
-- genui_workspace holds privileges: column SELECT and INSERT, and UPDATE of
-- the columns a re-link or an unlink rewrites. No secret, token or digest is
-- stored: the link token is single-use in process memory and never persisted.
-- Executed only by the checksummed, transactional migration runner (D7.12:
-- numbered in landing order).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 9);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 9);

CREATE TABLE genui.slack_links (
 organization_id uuid NOT NULL, link_id uuid NOT NULL, principal_id uuid NOT NULL,
 team_id text NOT NULL CHECK (team_id ~ '^[A-Z0-9]{1,32}$'),
 slack_user_id text NOT NULL CHECK (slack_user_id ~ '^[A-Z0-9]{1,32}$'),
 membership_epoch_at_link genui.safe_counter NOT NULL,
 linked_at timestamptz NOT NULL, revoked_at timestamptz,
 PRIMARY KEY (organization_id,link_id),
 UNIQUE (organization_id,team_id,slack_user_id),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 CHECK (revoked_at IS NULL OR revoked_at >= linked_at)
);
CREATE INDEX slack_links_principal ON genui.slack_links (organization_id,principal_id);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
ALTER TABLE genui.slack_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.slack_links FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.slack_links USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- genui_workspace (the role of storage::sessions): read, link, re-link, unlink.
GRANT SELECT(organization_id,link_id,principal_id,team_id,slack_user_id,membership_epoch_at_link,linked_at,revoked_at),
 INSERT(organization_id,link_id,principal_id,team_id,slack_user_id,membership_epoch_at_link,linked_at),
 UPDATE(principal_id,membership_epoch_at_link,linked_at,revoked_at) ON genui.slack_links TO genui_workspace;
RESET ROLE;
