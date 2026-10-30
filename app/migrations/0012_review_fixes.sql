-- ADR-011 pitch, APP-113 (code review of 2026-10-30, storage findings).
-- Additive except three dropped indexes no query or foreign key needs:
--
-- * box_file_sources: one registration per file and normalized content,
--   UNIQUE (organization_id,box_id,file_id,normalized_digest), and
--   genui_private.register_box_source returns the existing registration on a
--   repeat (a retried or double-submitted "use as data" no longer mints a
--   second source, revision and source_read). The definer is replaced in
--   place under its owner genui_admit (same signature, result, attributes and
--   EXECUTE grant), and genui_admit gains the column reads the lookup needs.
--   Existing duplicates make the UNIQUE fail and the whole migration roll
--   back: they are checked per organization before this runs (APP-113 Log).
-- * saved_work: a member's slash-command names are unique in the database
--   (a unique partial index on the creator and the context's marker line,
--   `forma-command:/<slug>`, case-folded), so two concurrent "Save as /name"
--   requests, in one task or several, cannot both create the name; the
--   second insert is refused and the name route answers `name_taken`.
--   Existing duplicates are checked before this runs, as above.
-- * agent_tokens.expires_at: a token lives 30 days from minting (the mint
--   writes it); a token minted before this migration expires 30 days after
--   it. storage::sessions refuses an expired token like a revoked one.
-- * The workspace chain also carries `box.explained` (a derived app's
--   summary template saved) and `box.briefed` (a folder brief saved).
-- * Dropped: box_files_digest (no query filters box files by digest alone;
--   the box_file_sources foreign key uses the (organization_id,box_id,
--   file_id,content_digest) UNIQUE), box_shares_principal (every box_shares
--   read names the box: its primary key) and slack_links_principal (links are
--   read by (team, user) or listed per organization). The three indexes
--   lead with (organization_id, principal_id) for the foreign keys to
--   genui.memberships, which only a membership DELETE or key UPDATE would
--   use: no role holds DELETE on memberships and no statement changes its
--   key (revocation is a status), so they are pure write cost.
--
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 12);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 12);

ALTER TABLE genui.box_file_sources
 ADD CONSTRAINT box_file_sources_one_per_content UNIQUE (organization_id,box_id,file_id,normalized_digest);

CREATE UNIQUE INDEX saved_work_one_command_name ON genui.saved_work
 (organization_id,created_by,lower(split_part(context,E'\n',1))) WHERE context LIKE 'forma-command:/%';

ALTER TABLE genui.agent_tokens ADD COLUMN expires_at timestamptz NOT NULL DEFAULT (now() + interval '30 days');
ALTER TABLE genui.agent_tokens ALTER COLUMN expires_at DROP DEFAULT;
ALTER TABLE genui.agent_tokens ADD CONSTRAINT agent_tokens_expires_after_creation CHECK (expires_at > created_at);
GRANT SELECT(expires_at), INSERT(expires_at) ON genui.agent_tokens TO genui_workspace;

DROP INDEX genui.box_files_digest;
DROP INDEX genui.box_shares_principal;
DROP INDEX genui.slack_links_principal;

-- D7.8 role binding, widened in place: the two box writes that had no event.
ALTER POLICY workspace_chain ON genui.audit_events
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared',
  'box.explained','box.briefed']));

-- D7.1 A4 extension: the definer's idempotency lookup reads the existing link.
GRANT SELECT(organization_id,box_id,file_id,normalized_digest,source_id,revision_id,created_at)
 ON genui.box_file_sources TO genui_admit;

-- The definer replaced in place, as its owner (genui_migration may SET ROLE
-- genui_admit, never inherit it); CREATE on genui_private is granted for the
-- replacement only and revoked below (as 0003, 0008 and 0011 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
SET LOCAL ROLE genui_admit;
-- The member registers a confirmed CSV/XLSX of a box they own as a new source
-- with one immutable revision, holds `source_read` on it, and the link to the
-- file is recorded (0011). Since 0012 a registration of the same file with
-- the same normalized digest returns the existing link instead (first by a
-- lookup, and on a concurrent registration's unique violation after it
-- commits), writing nothing. The context rule is share_work's: the
-- transaction's scope is the member's own organization, principal and
-- current epoch; a refusal returns no row.
CREATE OR REPLACE FUNCTION genui_private.register_box_source(p_org uuid,p_principal uuid,p_box uuid,p_file uuid,p_source uuid,p_revision uuid,
 p_content_digest text,p_normalized_digest text,p_contract_digest text,p_envelope bytea,p_row_count bigint)
 RETURNS TABLE(source_id uuid,revision_id uuid,created_at timestamptz)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; v_owner uuid; v_kind text; v_digest text; v_at timestamptz;
 v_source uuid; v_revision uuid;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO m_status,m_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR SHARE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 SELECT b.owner_principal_id INTO v_owner FROM genui.boxes b WHERE b.organization_id=p_org AND b.box_id=p_box;
 IF NOT FOUND OR v_owner<>p_principal THEN RETURN; END IF;
 SELECT f.kind,f.content_digest::text INTO v_kind,v_digest FROM genui.box_files f
  WHERE f.organization_id=p_org AND f.box_id=p_box AND f.file_id=p_file;
 IF NOT FOUND OR v_kind NOT IN ('csv','xlsx') OR v_digest<>p_content_digest THEN RETURN; END IF;
 SELECT s.source_id,s.revision_id,s.created_at INTO v_source,v_revision,v_at FROM genui.box_file_sources s
  WHERE s.organization_id=p_org AND s.box_id=p_box AND s.file_id=p_file AND s.normalized_digest::text=p_normalized_digest;
 IF FOUND THEN
  source_id:=v_source; revision_id:=v_revision; created_at:=v_at; RETURN NEXT;
  RETURN;
 END IF;
 v_at:=date_trunc('second',now());
 BEGIN
  INSERT INTO genui.sources(organization_id,source_id) VALUES (p_org,p_source);
  INSERT INTO genui.source_revisions(organization_id,source_id,revision_id,content_digest,normalized_digest,contract_digest,deleted_at,canonical_envelope)
   VALUES (p_org,p_source,p_revision,p_content_digest,p_normalized_digest,p_contract_digest,NULL,p_envelope);
  INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission) VALUES (p_org,p_principal,p_source,'source_read')
   ON CONFLICT DO NOTHING;
  INSERT INTO genui.box_file_sources(organization_id,box_id,file_id,content_digest,source_id,revision_id,normalized_digest,row_count,registered_by,created_at)
   VALUES (p_org,p_box,p_file,p_content_digest,p_source,p_revision,p_normalized_digest,p_row_count,p_principal,v_at);
 EXCEPTION WHEN unique_violation THEN
  -- A concurrent registration of the same content committed first: its
  -- link is the answer and this call's rows are rolled back with the block.
  SELECT s.source_id,s.revision_id,s.created_at INTO v_source,v_revision,v_at FROM genui.box_file_sources s
   WHERE s.organization_id=p_org AND s.box_id=p_box AND s.file_id=p_file AND s.normalized_digest::text=p_normalized_digest;
  IF NOT FOUND THEN RAISE; END IF;
  source_id:=v_source; revision_id:=v_revision; created_at:=v_at; RETURN NEXT;
  RETURN;
 END;
 source_id:=p_source; revision_id:=p_revision; created_at:=v_at; RETURN NEXT;
 RETURN;
END $$;
RESET ROLE;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
