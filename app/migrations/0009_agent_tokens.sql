-- ADR-011 (APP-099, pitch sub-branch pivot/mcp): Forma as an MCP server.
-- A signed-in member mints an agent token from the canvas ("Connect an
-- agent"); an MCP client (Claude Code, the OpenAI Responses API `mcp` tool,
-- any Streamable HTTP client) presents it as a bearer on POST /v1/mcp. The
-- token value is a capability `fmat_` + base64url(organization ‖ 32 random
-- bytes), exactly the shape of a workspace session cookie: the leading
-- organization only selects the RLS scope and the row is found by the
-- SHA-256 digest of the whole value, so a forged or foreign prefix finds
-- nothing. Only the digest is stored (D7.11's rule for every capability).
-- A token becomes a TrustedRequest only through storage::sessions, under the
-- member fence, at the member's current epoch, which must still equal the
-- epoch at minting; a revoked token, a revoked membership, an inactive
-- organization, a lapsed entitlement, an inactive provider binding or another
-- registry identity each refuse it (the uniform 404). A token carries no
-- right of its own: every tool calls the storage operation the canvas route
-- calls, as the member.
--
-- genui.agent_attributions records which agent token produced a committed
-- record version or a composed saved work, for the canvas's "via <label>"
-- chip. It is display provenance only, never authority; the audit chain is
-- unchanged (the commit's command.committed event names the member).
--
-- Both relations are organization-leading, state ENABLE/FORCE RLS and the
-- tenant_scope policy (D7.3, D7.5) and grant genui_workspace column SELECT and
-- INSERT plus the column-limited UPDATEs below; no DELETE (a revoked token row
-- stays so its attributions keep their label). Recorded debt (APP-097,
-- APP-098 DS-48): bearer tokens stand in for OAuth 2.1. Executed only by the
-- checksummed, transactional migration runner (D7.12: numbered in landing
-- order; a parallel branch's migration of the same number is renumbered at
-- merge).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 9);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 9);

-- One member-minted agent token: its digest, the member, a short label the
-- member chose ("Claude", "OpenAI"), the epoch and registry identity at
-- minting, and its use and revocation times.
CREATE TABLE genui.agent_tokens (
 organization_id uuid NOT NULL, token_id uuid NOT NULL, token_digest genui.digest NOT NULL,
 principal_id uuid NOT NULL, label text NOT NULL CHECK (length(label) BETWEEN 1 AND 40 AND label ~ '^[A-Za-z0-9][A-Za-z0-9 ._-]*$'),
 membership_epoch_at_mint genui.safe_counter NOT NULL,
 registry_sha256 text NOT NULL CHECK (registry_sha256 ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL, last_used_at timestamptz, revoked_at timestamptz,
 PRIMARY KEY (organization_id,token_id), UNIQUE (organization_id,token_digest),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 CHECK (last_used_at IS NULL OR last_used_at >= created_at),
 CHECK (revoked_at IS NULL OR revoked_at >= created_at)
);
CREATE INDEX agent_tokens_principal ON genui.agent_tokens (organization_id,principal_id);

-- Display provenance: the agent token that committed a record version or
-- composed a saved work (record_id and record_version absent).
CREATE TABLE genui.agent_attributions (
 organization_id uuid NOT NULL, attribution_id uuid NOT NULL, token_id uuid NOT NULL,
 app_id uuid NOT NULL, work_id uuid NOT NULL, record_id uuid, record_version genui.safe_counter,
 created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,attribution_id),
 FOREIGN KEY (organization_id,token_id) REFERENCES genui.agent_tokens,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 CHECK ((record_id IS NULL) = (record_version IS NULL))
);
CREATE INDEX agent_attributions_work ON genui.agent_attributions (organization_id,app_id,work_id);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['agent_tokens','agent_attributions'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;

-- genui_workspace (the session role): column reads and inserts, the two
-- column-limited token updates (use, revocation); no DELETE.
GRANT SELECT(organization_id,token_id,token_digest,principal_id,label,membership_epoch_at_mint,registry_sha256,
 created_at,last_used_at,revoked_at),
 INSERT(organization_id,token_id,token_digest,principal_id,label,membership_epoch_at_mint,registry_sha256,created_at),
 UPDATE(last_used_at,revoked_at) ON genui.agent_tokens TO genui_workspace;
GRANT SELECT(organization_id,attribution_id,token_id,app_id,work_id,record_id,record_version,created_at),
 INSERT(organization_id,attribution_id,token_id,app_id,work_id,record_id,record_version,created_at)
 ON genui.agent_attributions TO genui_workspace;
RESET ROLE;
