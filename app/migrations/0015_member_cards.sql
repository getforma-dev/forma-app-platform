-- ADR-011 pitch, APP-123 (pivot/app123): cards made by you. Additive only.
--
-- * member_cards: a card a member made with Shape (`kind` 'card': "Add as
--   new card", or "Make it a card" on an Ask answer) or an Ask answer they
--   pinned to the canvas (`kind` 'answer'). A made card belongs to its app
--   (`app_id`, `work_id`) and stores only a view: which card it came from
--   (`source`), its address and title, and its closed view spec (`spec`,
--   forma-json-v1 bytes, judged by the route against the work's published
--   schema; receipts are never dropped). It stores no record value: the
--   canvas reads the records live, with their receipts. A pinned answer
--   stores what Ask answered (question, text, references and their storage
--   receipts, each receipt checked against the record revisions by the
--   route) and never runs again; an answer over several apps has no app.
--   A card is the member's own ("made by you") until the app's owner or an
--   admin who made it adds it for everyone (`everyone`, audited
--   `card.shared`); then every member who can open the app sees it. Removing
--   is a soft delete (`removed_at`, audited `card.removed`): there is no
--   DELETE. Made and pinned are audited `card.made` and `answer.pinned`.
--   Defence in depth: a RESTRICTIVE policy binds genui_workspace to the
--   scoped principal's own rows for every write, and to its own rows or the
--   rows added for everyone for reads (`genui.principal_id`, set by every
--   member transaction), as member_labels_principal does for labels.
--
-- The relation is organization-leading, states ENABLE/FORCE RLS and the
-- tenant_scope policy (D7.3, D7.5). genui_workspace gets column SELECT and
-- INSERT and the UPDATE of `everyone`, `updated_at` and `removed_at` only.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 15);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 15);

CREATE TABLE genui.member_cards (
 organization_id uuid NOT NULL, card_id uuid NOT NULL, principal_id uuid NOT NULL,
 kind text NOT NULL CHECK (kind IN ('card','answer')),
 app_id uuid, work_id uuid,
 source text NOT NULL CHECK (source = '' OR source ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
 address text NOT NULL CHECK (length(address) <= 88 AND address ~ '^[a-z0-9:.-]*$'),
 title text NOT NULL CHECK (length(title) BETWEEN 1 AND 120 AND title !~ '[[:cntrl:]]'),
 spec bytea NOT NULL CHECK (octet_length(spec) BETWEEN 2 AND 262144),
 everyone boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL, removed_at timestamptz,
 PRIMARY KEY (organization_id,card_id),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.principals,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 CHECK ((app_id IS NULL) = (work_id IS NULL)),
 CHECK (kind = 'answer' OR (app_id IS NOT NULL AND source <> '')),
 CHECK (kind = 'card' OR NOT everyone)
);
CREATE INDEX member_cards_by_principal ON genui.member_cards (organization_id,principal_id) WHERE removed_at IS NULL;

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
ALTER TABLE genui.member_cards ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.member_cards FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.member_cards USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- The workspace writes only the scoped principal's own cards, and reads its
-- own and those added for everyone (the command narrows those to the apps
-- the member can open).
CREATE POLICY member_cards_principal ON genui.member_cards AS RESTRICTIVE FOR ALL TO genui_workspace
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid OR everyone)
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);

-- genui_workspace: column reads and inserts; UPDATE of the sharing and removal columns; no DELETE.
GRANT SELECT(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,everyone,created_at,updated_at,removed_at),
 INSERT(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,everyone,created_at,updated_at),
 UPDATE(everyone,updated_at,removed_at) ON genui.member_cards TO genui_workspace;

-- D7.8 role binding, widened in place: the workspace chain also carries the
-- four card events. Every other kind list is unchanged.
ALTER POLICY workspace_chain ON genui.audit_events
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared',
  'box.explained','box.briefed','app.labeled','app.aliased','card.made','answer.pinned','card.shared','card.removed']));
RESET ROLE;
