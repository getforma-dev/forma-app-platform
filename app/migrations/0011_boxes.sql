-- ADR-011 (APP-108, pitch sub-branch pivot/boxes): boxes. A member keeps
-- files in a box (a canvas card): PDFs, images, CSV/XLSX and text, at most 20
-- files of at most 20 MB each per box. Each original is an immutable object
-- under the organization's prefix (`o/<org>/b/<box>/f/<file>`, SSE-KMS with
-- the tenant key, If-None-Match, its SHA-256 recorded here); the row records
-- name, kind, media type, size and digest. A CSV/XLSX in a box is parsed by
-- the sealed parser (ADR-009) and, once the member confirms its column types,
-- becomes an immutable source revision (APP-016/018 lite) whose `original` is
-- that file: genui_private.register_box_source (owned by genui_admit, as
-- 0008's share_work) inserts the source, the revision, the owner's
-- `source_read` and the link `box_file_sources`, so genui_workspace still
-- cannot write the source ledger or a permission row itself. An app built
-- from a box records `box_derivations` (the box, the saved work and the file
-- digests used); a teammate a derived app is shared with receives
-- `box_file_grants` on exactly the files used (not the box) and a whole-box
-- share is `box_shares`. `box_explanations` keeps the validated, typed
-- summary template of a derived app (placeholders and refs only; every number
-- is filled from the governed results at read time).
--
-- Every relation is organization-leading, states ENABLE/FORCE RLS and the
-- tenant_scope policy (D7.3, D7.5); genui_workspace gets column SELECT and
-- INSERT, plus UPDATE of a box's name (rename): no other UPDATE, no DELETE
-- (deleting a box file is APP-097 debt).
-- Uploads, data registrations, derivations and shares are audited on the
-- workspace chain (its closed kind list widened in place below). Recorded
-- debt (ADR-011 §Consequences, APP-097): an eighth genui_admit definer that
-- writes the source ledger and a permission row for a member. Executed only
-- by the checksummed, transactional migration runner (D7.12: numbered in
-- landing order; a parallel branch's migration of the same number is
-- renumbered at merge).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 11);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 11);

-- One box: its owner, a short name, its kind (a named box, the member's
-- implicit "Uploads" box, or the box attached to one of the member's apps).
CREATE TABLE genui.boxes (
 organization_id uuid NOT NULL, box_id uuid NOT NULL, owner_principal_id uuid NOT NULL,
 name text NOT NULL CHECK (length(name) BETWEEN 1 AND 60 AND name !~ '[[:cntrl:]]'),
 kind text NOT NULL CHECK (kind IN ('box','uploads','app')),
 app_id uuid, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,box_id),
 FOREIGN KEY (organization_id,owner_principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps,
 CHECK ((kind='app') = (app_id IS NOT NULL))
);
CREATE INDEX boxes_owner ON genui.boxes (organization_id,owner_principal_id,created_at);
CREATE UNIQUE INDEX boxes_one_uploads ON genui.boxes (organization_id,owner_principal_id) WHERE kind='uploads';
CREATE UNIQUE INDEX boxes_one_per_app ON genui.boxes (organization_id,owner_principal_id,app_id) WHERE kind='app';

-- One file of a box: an immutable object and its identity.
CREATE TABLE genui.box_files (
 organization_id uuid NOT NULL, box_id uuid NOT NULL, file_id uuid NOT NULL,
 name text NOT NULL CHECK (length(name) BETWEEN 1 AND 200 AND name !~ '[[:cntrl:]/\\]'),
 kind text NOT NULL CHECK (kind IN ('pdf','image','csv','xlsx','text')),
 media_type text NOT NULL CHECK (media_type IN ('application/pdf','image/png','image/jpeg','image/webp','text/csv',
  'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet','text/plain','text/markdown')),
 byte_length bigint NOT NULL CHECK (byte_length BETWEEN 1 AND 20971520),
 content_digest genui.digest NOT NULL,
 object_key text NOT NULL CHECK (object_key ~ '^o/[0-9a-f-]{36}/b/[0-9a-f-]{36}/f/[0-9a-f-]{36}$'),
 uploaded_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,box_id,file_id),
 UNIQUE (organization_id,box_id,file_id,content_digest),
 FOREIGN KEY (organization_id,box_id) REFERENCES genui.boxes,
 FOREIGN KEY (organization_id,uploaded_by) REFERENCES genui.memberships(organization_id,principal_id)
);
CREATE INDEX box_files_digest ON genui.box_files (organization_id,content_digest);

-- A confirmed CSV/XLSX: the source revision registered from the file.
CREATE TABLE genui.box_file_sources (
 organization_id uuid NOT NULL, box_id uuid NOT NULL, file_id uuid NOT NULL, content_digest genui.digest NOT NULL,
 source_id uuid NOT NULL, revision_id uuid NOT NULL, normalized_digest genui.digest NOT NULL,
 row_count bigint NOT NULL CHECK (row_count BETWEEN 0 AND 1000000),
 registered_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,box_id,file_id,source_id,revision_id),
 UNIQUE (organization_id,source_id,revision_id),
 FOREIGN KEY (organization_id,box_id,file_id,content_digest) REFERENCES genui.box_files(organization_id,box_id,file_id,content_digest),
 FOREIGN KEY (organization_id,source_id,revision_id,normalized_digest) REFERENCES genui.source_revisions(organization_id,source_id,revision_id,normalized_digest),
 FOREIGN KEY (organization_id,registered_by) REFERENCES genui.memberships(organization_id,principal_id)
);

-- An app built from a box (`derived`, through the composer) or a file's data
-- card (`data`): the saved work, the files and their digests at the time.
CREATE TABLE genui.box_derivations (
 organization_id uuid NOT NULL, derivation_id uuid NOT NULL, box_id uuid NOT NULL,
 app_id uuid NOT NULL, work_id uuid NOT NULL, kind text NOT NULL CHECK (kind IN ('derived','data')),
 file_ids uuid[] NOT NULL CHECK (cardinality(file_ids) BETWEEN 1 AND 20),
 file_digests text[] NOT NULL CHECK (cardinality(file_digests) BETWEEN 1 AND 20),
 created_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,derivation_id),
 UNIQUE (organization_id,app_id,work_id),
 FOREIGN KEY (organization_id,box_id) REFERENCES genui.boxes,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY (organization_id,created_by) REFERENCES genui.memberships(organization_id,principal_id),
 CHECK (cardinality(file_ids) = cardinality(file_digests))
);
CREATE INDEX box_derivations_box ON genui.box_derivations (organization_id,box_id);

-- A whole box shared with a teammate.
CREATE TABLE genui.box_shares (
 organization_id uuid NOT NULL, box_id uuid NOT NULL, principal_id uuid NOT NULL,
 granted_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,box_id,principal_id),
 FOREIGN KEY (organization_id,box_id) REFERENCES genui.boxes,
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,granted_by) REFERENCES genui.memberships(organization_id,principal_id),
 CHECK (principal_id<>granted_by)
);
CREATE INDEX box_shares_principal ON genui.box_shares (organization_id,principal_id);

-- One file of a box readable by a teammate (a derived app shared with them).
CREATE TABLE genui.box_file_grants (
 organization_id uuid NOT NULL, box_id uuid NOT NULL, file_id uuid NOT NULL, principal_id uuid NOT NULL,
 app_id uuid NOT NULL, work_id uuid NOT NULL, granted_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,box_id,file_id,principal_id),
 FOREIGN KEY (organization_id,box_id,file_id) REFERENCES genui.box_files,
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY (organization_id,granted_by) REFERENCES genui.memberships(organization_id,principal_id)
);
CREATE INDEX box_file_grants_principal ON genui.box_file_grants (organization_id,principal_id);

-- A derived app's validated summary template (forma-json-v1: findings of
-- text parts and refs; no number is stored).
CREATE TABLE genui.box_explanations (
 organization_id uuid NOT NULL, explanation_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL,
 template bytea NOT NULL CHECK (octet_length(template) BETWEEN 1 AND 16384),
 created_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,explanation_id),
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY (organization_id,created_by) REFERENCES genui.memberships(organization_id,principal_id)
);
CREATE INDEX box_explanations_work ON genui.box_explanations (organization_id,app_id,work_id,created_at);

-- A folder's brief: typed paragraphs (text, governed-value references,
-- statements cited to a file page); no number of a data file is stored.
CREATE TABLE genui.box_briefs (
 organization_id uuid NOT NULL, brief_id uuid NOT NULL, box_id uuid NOT NULL,
 body bytea NOT NULL CHECK (octet_length(body) BETWEEN 1 AND 131072),
 created_by uuid NOT NULL, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,brief_id),
 FOREIGN KEY (organization_id,box_id) REFERENCES genui.boxes,
 FOREIGN KEY (organization_id,created_by) REFERENCES genui.memberships(organization_id,principal_id)
);
CREATE INDEX box_briefs_box ON genui.box_briefs (organization_id,box_id,created_at);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['boxes','box_files','box_file_sources','box_derivations','box_shares','box_file_grants','box_explanations','box_briefs'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;

-- genui_workspace (the session role): column reads and inserts, the box
-- name's UPDATE (rename, audited); no DELETE. box_file_sources is read here
-- and written by the definer only.
GRANT SELECT(organization_id,box_id,owner_principal_id,name,kind,app_id,created_at),
 INSERT(organization_id,box_id,owner_principal_id,name,kind,app_id,created_at),
 UPDATE(name) ON genui.boxes TO genui_workspace;
GRANT SELECT(organization_id,box_id,file_id,name,kind,media_type,byte_length,content_digest,object_key,uploaded_by,created_at),
 INSERT(organization_id,box_id,file_id,name,kind,media_type,byte_length,content_digest,object_key,uploaded_by,created_at)
 ON genui.box_files TO genui_workspace;
GRANT SELECT(organization_id,box_id,file_id,content_digest,source_id,revision_id,normalized_digest,row_count,registered_by,created_at)
 ON genui.box_file_sources TO genui_workspace;
GRANT SELECT(organization_id,derivation_id,box_id,app_id,work_id,kind,file_ids,file_digests,created_by,created_at),
 INSERT(organization_id,derivation_id,box_id,app_id,work_id,kind,file_ids,file_digests,created_by,created_at)
 ON genui.box_derivations TO genui_workspace;
GRANT SELECT(organization_id,box_id,principal_id,granted_by,created_at),
 INSERT(organization_id,box_id,principal_id,granted_by,created_at) ON genui.box_shares TO genui_workspace;
GRANT SELECT(organization_id,box_id,file_id,principal_id,app_id,work_id,granted_by,created_at),
 INSERT(organization_id,box_id,file_id,principal_id,app_id,work_id,granted_by,created_at) ON genui.box_file_grants TO genui_workspace;
GRANT SELECT(organization_id,explanation_id,app_id,work_id,template,created_by,created_at),
 INSERT(organization_id,explanation_id,app_id,work_id,template,created_by,created_at) ON genui.box_explanations TO genui_workspace;
GRANT SELECT(organization_id,brief_id,box_id,body,created_by,created_at),
 INSERT(organization_id,brief_id,box_id,body,created_by,created_at) ON genui.box_briefs TO genui_workspace;

-- D7.8 role binding, widened in place: the workspace chain also carries the
-- box events (one per audited box command). Every other kind list is unchanged.
ALTER POLICY workspace_chain ON genui.audit_events
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared']));

-- D7.1 A4 extension for the eighth definer: the reads and writes it needs, by column.
GRANT SELECT(organization_id,box_id,owner_principal_id) ON genui.boxes TO genui_admit;
GRANT SELECT(organization_id,box_id,file_id,kind,content_digest) ON genui.box_files TO genui_admit;
GRANT INSERT(organization_id,source_id) ON genui.sources TO genui_admit;
GRANT INSERT(organization_id,source_id,revision_id,content_digest,normalized_digest,contract_digest,deleted_at,canonical_envelope)
 ON genui.source_revisions TO genui_admit;
GRANT INSERT(organization_id,box_id,file_id,content_digest,source_id,revision_id,normalized_digest,row_count,registered_by,created_at)
 ON genui.box_file_sources TO genui_admit;

-- The member registers a confirmed CSV/XLSX of a box they own as a new source
-- with one immutable revision, holds `source_read` on it, and the link to the
-- file is recorded. The same context rule as share_work: the transaction's
-- scope is the member's own organization, principal and current epoch; a
-- refusal returns no row. The envelope, digests and identity are the
-- caller's (checked again by the stored constraints and the engine's digest
-- checks); the file must be a csv or xlsx row with exactly that content digest.
CREATE FUNCTION genui_private.register_box_source(p_org uuid,p_principal uuid,p_box uuid,p_file uuid,p_source uuid,p_revision uuid,
 p_content_digest text,p_normalized_digest text,p_contract_digest text,p_envelope bytea,p_row_count bigint)
 RETURNS TABLE(source_id uuid,revision_id uuid,created_at timestamptz)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; v_owner uuid; v_kind text; v_digest text; v_at timestamptz;
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
 v_at:=date_trunc('second',now());
 INSERT INTO genui.sources(organization_id,source_id) VALUES (p_org,p_source);
 INSERT INTO genui.source_revisions(organization_id,source_id,revision_id,content_digest,normalized_digest,contract_digest,deleted_at,canonical_envelope)
  VALUES (p_org,p_source,p_revision,p_content_digest,p_normalized_digest,p_contract_digest,NULL,p_envelope);
 INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission) VALUES (p_org,p_principal,p_source,'source_read')
  ON CONFLICT DO NOTHING;
 INSERT INTO genui.box_file_sources(organization_id,box_id,file_id,content_digest,source_id,revision_id,normalized_digest,row_count,registered_by,created_at)
  VALUES (p_org,p_box,p_file,p_content_digest,p_source,p_revision,p_normalized_digest,p_row_count,p_principal,v_at);
 source_id:=p_source; revision_id:=p_revision; created_at:=v_at; RETURN NEXT;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.register_box_source(uuid,uuid,uuid,uuid,uuid,uuid,text,text,text,bytea,bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.register_box_source(uuid,uuid,uuid,uuid,uuid,uuid,text,text,text,bytea,bigint) TO genui_workspace;
-- Temporary CREATE for the ownership transfer only; revoked below (as 0003 and 0008 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.register_box_source(uuid,uuid,uuid,uuid,uuid,uuid,text,text,text,bytea,bigint) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
