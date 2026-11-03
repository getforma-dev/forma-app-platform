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
--   `read_apps`/`read_works` are the (app, work) pairs a pinned answer's
--   receipts cite (empty for a made card); a listing shows a card only while
--   the reader holds `record_read` on its work and on every pair.
--   A card is the member's own ("made by you") until the app's owner or an
--   admin who made it adds it for everyone (`everyone`, audited
--   `card.shared`); then every member who can read the app's work sees it.
--   Removing is a soft delete (`removed_at`, audited `card.removed`): there
--   is no DELETE. Made and pinned are audited `card.made` and
--   `answer.pinned`. Defence in depth: a RESTRICTIVE policy binds
--   genui_workspace to the scoped principal's own rows for every write, and
--   to its own rows or the rows added for everyone for reads
--   (`genui.principal_id`, set by every member transaction), as
--   member_labels_principal does for labels; `everyone` is not in the INSERT
--   grant, so a card starts private and only the audited share UPDATE adds
--   it for everyone. The steward path (the app's owner or an admin takes
--   another member's card added for everyone back to its maker, or removes
--   it, also when its maker left) is the genui_admit definer
--   genui_private.steward_card, as share_work is for sharing a work.
--
-- The relation is organization-leading, states ENABLE/FORCE RLS and the
-- tenant_scope policy (D7.3, D7.5). genui_workspace gets column SELECT,
-- INSERT without `everyone` and `removed_at`, the UPDATE of `everyone`,
-- `updated_at` and `removed_at` only, and EXECUTE on steward_card.
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
 read_apps uuid[] NOT NULL DEFAULT '{}', read_works uuid[] NOT NULL DEFAULT '{}',
 created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL, removed_at timestamptz,
 PRIMARY KEY (organization_id,card_id),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.principals,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 CHECK ((app_id IS NULL) = (work_id IS NULL)),
 CHECK (kind = 'answer' OR (app_id IS NOT NULL AND source <> '')),
 CHECK (kind = 'card' OR NOT everyone),
 CHECK (cardinality(read_apps) = cardinality(read_works) AND cardinality(read_apps) <= 12
  AND array_position(read_apps,NULL) IS NULL AND array_position(read_works,NULL) IS NULL),
 CHECK (kind = 'answer' OR cardinality(read_apps) = 0)
);
CREATE INDEX member_cards_by_principal ON genui.member_cards (organization_id,principal_id) WHERE removed_at IS NULL;

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
ALTER TABLE genui.member_cards ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.member_cards FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.member_cards USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- The workspace writes only the scoped principal's own cards, and reads its
-- own and those added for everyone (the command narrows those to the works
-- the member can read). The steward path is the definer below.
CREATE POLICY member_cards_principal ON genui.member_cards AS RESTRICTIVE FOR ALL TO genui_workspace
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid OR everyone)
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);

-- genui_workspace: column reads; inserts without the sharing and removal
-- columns (a card starts private and live); UPDATE of the sharing and
-- removal columns; no DELETE.
GRANT SELECT(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,everyone,read_apps,read_works,created_at,updated_at,removed_at),
 INSERT(organization_id,card_id,principal_id,kind,app_id,work_id,source,address,title,spec,read_apps,read_works,created_at,updated_at),
 UPDATE(everyone,updated_at,removed_at) ON genui.member_cards TO genui_workspace;

-- D7.8 role binding, widened in place: the workspace chain also carries the
-- four card events. Every other kind list is unchanged.
ALTER POLICY workspace_chain ON genui.audit_events
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared',
  'box.explained','box.briefed','app.labeled','app.aliased','card.made','answer.pinned','card.shared','card.removed']));

-- D7.1 A4 extension for the ninth definer: the reads and writes it needs, by column.
GRANT SELECT(organization_id,card_id,kind,app_id,work_id,everyone,removed_at),UPDATE(everyone,updated_at,removed_at)
 ON genui.member_cards TO genui_admit;
GRANT SELECT(organization_id,app_id,owner_principal_id) ON genui.apps TO genui_admit;
GRANT SELECT(organization_id,principal_id,permission) ON genui.organization_permissions TO genui_admit;

-- The steward path (APP-123 review): the app's owner or an admin_policy
-- holder who can open the app takes another member's card added for
-- everyone back to its maker (`everyone` false) or removes it (a soft
-- delete), also when its maker has left the organization. Only a live made
-- card added for everyone qualifies; a private card or a pinned answer
-- returns no row. A card already removed returns its row with `changed`
-- false. The context rule is share_work's: the transaction's scope is the
-- actor's own organization, principal and current epoch; a refusal returns
-- no row. The caller appends the audit event in the same transaction.
CREATE FUNCTION genui_private.steward_card(p_org uuid,p_actor uuid,p_card uuid,p_remove boolean)
 RETURNS TABLE(app_id uuid,work_id uuid,changed boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; v_app uuid; v_work uuid; v_kind text; v_everyone boolean;
 v_removed timestamptz; v_owner uuid;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_actor::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO m_status,m_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_actor FOR SHARE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 SELECT c.app_id,c.work_id,c.kind,c.everyone,c.removed_at INTO v_app,v_work,v_kind,v_everyone,v_removed
  FROM genui.member_cards c WHERE c.organization_id=p_org AND c.card_id=p_card FOR UPDATE;
 IF NOT FOUND OR v_kind<>'card' OR NOT v_everyone THEN RETURN; END IF;
 SELECT a.owner_principal_id INTO v_owner FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=v_app;
 IF NOT FOUND THEN RETURN; END IF;
 IF v_owner IS DISTINCT FROM p_actor AND NOT EXISTS (SELECT 1 FROM genui.organization_permissions o
  WHERE o.organization_id=p_org AND o.principal_id=p_actor AND o.permission='admin_policy') THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.app_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_actor
  AND p.app_id=v_app AND p.permission='discover') THEN RETURN; END IF;
 app_id:=v_app; work_id:=v_work; changed:=false;
 IF v_removed IS NOT NULL THEN RETURN NEXT; RETURN; END IF;
 IF p_remove THEN
  UPDATE genui.member_cards c SET removed_at=date_trunc('second',now()),updated_at=date_trunc('second',now())
   WHERE c.organization_id=p_org AND c.card_id=p_card;
 ELSE
  UPDATE genui.member_cards c SET everyone=false,updated_at=date_trunc('second',now())
   WHERE c.organization_id=p_org AND c.card_id=p_card;
 END IF;
 changed:=true; RETURN NEXT;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.steward_card(uuid,uuid,uuid,boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.steward_card(uuid,uuid,uuid,boolean) TO genui_workspace;
-- Temporary CREATE for the ownership transfer only; revoked below (as 0003, 0008 and 0011 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.steward_card(uuid,uuid,uuid,boolean) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
