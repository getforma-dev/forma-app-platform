-- ADR-011 pitch, APP-121 (pivot/naming): user naming. Additive only.
--
-- * app_labels: the organization's labels of an app (card '') or of one of
--   its cards (card = the card's name within the app, `ledger`, `totals`):
--   a display name and a description, and for the app its address
--   (`/team-spend`) and its kept addresses (`kept`: the addresses it had
--   before a rename, kept automatically so nothing breaks; never edited
--   directly, and there are no other organization aliases). Written by
--   the app's owner or a holder of `admin_policy`, audited `app.labeled`
--   on the workspace chain. A label never touches the definition, its
--   revision, its publication or any receipt; without a row everything
--   falls back to the catalog's names.
--   Addresses are unique in an organization (the index below; addresses and
--   kept addresses are lowercase by shape, so the uniqueness is
--   case-insensitive); kept addresses, the catalog's names and saved works'
--   names are checked by the command under a per-organization advisory lock.
-- * member_labels: one member's own shortcuts (`/exp` → an app, `/q3` → a
--   card; one letter is enough, `/e`) and own card names, visible to that
--   member only, audited `app.aliased`. An organization address always
--   wins over a personal alias.
--   Defence in depth: a RESTRICTIVE policy binds genui_workspace to the
--   scoped principal's own rows (`genui.principal_id`, set by every member
--   transaction), as saved_work_creator does for saved work.
--
-- Both relations are organization-leading, state ENABLE/FORCE RLS and the
-- tenant_scope policy (D7.3, D7.5). genui_workspace gets column SELECT and
-- INSERT and the UPDATE of the label columns: no DELETE (clearing a label
-- writes NULLs). Executed only by the checksummed, transactional migration
-- runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 14);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 14);

CREATE TABLE genui.app_labels (
 organization_id uuid NOT NULL, app_id uuid NOT NULL,
 card text NOT NULL CHECK (card = '' OR card ~ '^[a-z0-9][a-z0-9-]{0,47}$'),
 name text CHECK (length(name) BETWEEN 1 AND 60 AND name !~ '[[:cntrl:]]'),
 description text CHECK (length(description) BETWEEN 1 AND 280 AND description !~ '[[:cntrl:]]'),
 address text CHECK (address ~ '^[a-z][a-z0-9-]{1,31}$'),
 kept text[] NOT NULL DEFAULT '{}'::text[] CHECK (cardinality(kept) <= 8
  AND array_to_string(kept,',') ~ '^([a-z][a-z0-9-]{1,31}(,[a-z][a-z0-9-]{1,31})*)?$'),
 updated_by uuid NOT NULL, updated_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,app_id,card),
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps,
 FOREIGN KEY (organization_id,updated_by) REFERENCES genui.principals,
 CHECK (card = '' OR (address IS NULL AND cardinality(kept) = 0))
);
CREATE UNIQUE INDEX app_labels_one_address ON genui.app_labels (organization_id,address) WHERE address IS NOT NULL;

CREATE TABLE genui.member_labels (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, app_id uuid NOT NULL,
 card text NOT NULL CHECK (card = '' OR card ~ '^[a-z0-9][a-z0-9-]{0,47}$'),
 name text CHECK (length(name) BETWEEN 1 AND 60 AND name !~ '[[:cntrl:]]'),
 aliases text[] NOT NULL DEFAULT '{}'::text[] CHECK (cardinality(aliases) <= 8
  AND array_to_string(aliases,',') ~ '^([a-z][a-z0-9-]{0,31}(,[a-z][a-z0-9-]{0,31})*)?$'),
 updated_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,principal_id,app_id,card),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.principals,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps
);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['app_labels','member_labels'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;

-- The workspace reads and writes only the scoped principal's own personal labels.
CREATE POLICY member_labels_principal ON genui.member_labels AS RESTRICTIVE FOR ALL TO genui_workspace
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid)
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);

-- genui_workspace: column reads and inserts, the label columns' UPDATE; no DELETE.
GRANT SELECT(organization_id,app_id,card,name,description,address,kept,updated_by,updated_at),
 INSERT(organization_id,app_id,card,name,description,address,kept,updated_by,updated_at),
 UPDATE(name,description,address,kept,updated_by,updated_at) ON genui.app_labels TO genui_workspace;
GRANT SELECT(organization_id,principal_id,app_id,card,name,aliases,updated_at),
 INSERT(organization_id,principal_id,app_id,card,name,aliases,updated_at),
 UPDATE(name,aliases,updated_at) ON genui.member_labels TO genui_workspace;

-- D7.8 role binding, widened in place: the workspace chain also carries the
-- two naming events. Every other kind list is unchanged.
ALTER POLICY workspace_chain ON genui.audit_events
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared',
  'box.explained','box.briefed','app.labeled','app.aliased']));
RESET ROLE;
