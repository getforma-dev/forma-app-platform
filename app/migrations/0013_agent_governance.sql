-- ADR-011 pitch, APP-115 quick wins (pivot/governance): agent access
-- governance. Additive only:
--
-- * agent_tokens gains the token's scope, chosen by the member at minting:
--   scope_kind 'all' (every space and app the member may use: today's
--   behavior, the default for every existing token) or 'selected' (the
--   spaces in scope_spaces, the library pane's categories plus "mine" and
--   "folders", and the apps in scope_apps); scope_actions, the actions the
--   token may take (read, draft, commit, decide, share, build; every action
--   by default). A scope never grants anything: every MCP tool still acts as
--   the member with the member's rights, and the MCP layer refuses a call
--   outside the scope before any storage call and again where the tool
--   resolves its app, record or folder. The expiry (at most 30 days) is the
--   0012 expires_at, now chosen at minting.
-- * agent_access_log: one append-only row per MCP tools/call: the token,
--   its label, the member, the tool, the ids it touched, the outcome
--   (allowed, or refused with a reason code such as scope_space or
--   scope_action), a display label of the resource, the duration and the
--   time. Rows are hash-chained per organization (chain_sequence,
--   previous_hash, hash: SHA-256 under "forma:agent-access:v1" over the
--   canonical row, the audit chains' construction), written under a
--   per-organization transaction advisory lock. genui_workspace holds column
--   SELECT and INSERT only: no UPDATE, no DELETE.
--
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 13);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 13);

ALTER TABLE genui.agent_tokens
 ADD COLUMN scope_kind text NOT NULL DEFAULT 'all' CHECK (scope_kind IN ('all','selected')),
 ADD COLUMN scope_spaces text[] NOT NULL DEFAULT '{}'::text[]
  CHECK (cardinality(scope_spaces) <= 9 AND array_position(scope_spaces,NULL) IS NULL
   AND scope_spaces <@ ARRAY['finance','security','operations','sales','support','people','utilities','mine','folders']::text[]),
 ADD COLUMN scope_apps uuid[] NOT NULL DEFAULT '{}'::uuid[]
  CHECK (cardinality(scope_apps) <= 20 AND array_position(scope_apps,NULL) IS NULL),
 ADD COLUMN scope_actions text[] NOT NULL DEFAULT ARRAY['read','draft','commit','decide','share','build']::text[]
  CHECK (cardinality(scope_actions) BETWEEN 1 AND 6 AND array_position(scope_actions,NULL) IS NULL
   AND scope_actions <@ ARRAY['read','draft','commit','decide','share','build']::text[]);
ALTER TABLE genui.agent_tokens ADD CONSTRAINT agent_tokens_scope_shape CHECK (
 (scope_kind = 'all' AND cardinality(scope_spaces) = 0 AND cardinality(scope_apps) = 0)
 OR (scope_kind = 'selected' AND cardinality(scope_spaces) + cardinality(scope_apps) >= 1));
GRANT SELECT(scope_kind,scope_spaces,scope_apps,scope_actions), INSERT(scope_kind,scope_spaces,scope_apps,scope_actions)
 ON genui.agent_tokens TO genui_workspace;

-- One MCP tools/call by an agent token, hash-chained per organization.
CREATE TABLE genui.agent_access_log (
 organization_id uuid NOT NULL, entry_id uuid NOT NULL, chain_sequence genui.safe_counter NOT NULL,
 token_id uuid NOT NULL, principal_id uuid NOT NULL,
 agent_label text NOT NULL CHECK (length(agent_label) BETWEEN 1 AND 40),
 tool text NOT NULL CHECK (tool ~ '^forma_[a-z_]{1,40}$'),
 touched_apps uuid[] NOT NULL CHECK (cardinality(touched_apps) <= 20),
 touched_works uuid[] NOT NULL CHECK (cardinality(touched_works) <= 20),
 touched_records uuid[] NOT NULL CHECK (cardinality(touched_records) <= 20),
 touched_folders uuid[] NOT NULL CHECK (cardinality(touched_folders) <= 20),
 touched_files uuid[] NOT NULL CHECK (cardinality(touched_files) <= 20),
 outcome text NOT NULL CHECK (outcome IN ('allowed','refused')),
 reason text CHECK (reason ~ '^[a-z][a-z_]{0,39}$'),
 space text CHECK (space ~ '^[a-z][a-z_]{0,39}$'),
 resource_label text CHECK (length(resource_label) BETWEEN 1 AND 120),
 duration_ms integer NOT NULL CHECK (duration_ms >= 0),
 occurred_at timestamptz NOT NULL,
 previous_hash text NOT NULL CHECK (previous_hash ~ '^sha256:[0-9a-f]{64}$'),
 hash text NOT NULL CHECK (hash ~ '^sha256:[0-9a-f]{64}$'),
 PRIMARY KEY (organization_id,entry_id), UNIQUE (organization_id,chain_sequence),
 FOREIGN KEY (organization_id,token_id) REFERENCES genui.agent_tokens,
 CHECK ((outcome = 'refused') = (reason IS NOT NULL))
);
CREATE INDEX agent_access_log_token ON genui.agent_access_log (organization_id,token_id,chain_sequence);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
ALTER TABLE genui.agent_access_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.agent_access_log FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.agent_access_log
 USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid)
 WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- genui_workspace: column reads and inserts; append-only (no UPDATE, no DELETE).
GRANT SELECT(organization_id,entry_id,chain_sequence,token_id,principal_id,agent_label,tool,touched_apps,touched_works,
 touched_records,touched_folders,touched_files,outcome,reason,space,resource_label,duration_ms,occurred_at,previous_hash,hash),
 INSERT(organization_id,entry_id,chain_sequence,token_id,principal_id,agent_label,tool,touched_apps,touched_works,
 touched_records,touched_folders,touched_files,outcome,reason,space,resource_label,duration_ms,occurred_at,previous_hash,hash)
 ON genui.agent_access_log TO genui_workspace;
RESET ROLE;
