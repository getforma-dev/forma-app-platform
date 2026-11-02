-- Same-work CSV copies: workspace installs the admitted definition and the
-- insert-only definer materializes initial records in that same transaction.
SET LOCAL ROLE genui_owner;
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND 20);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK(version BETWEEN 1 AND 20);
CREATE TABLE genui.work_data_copies (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL,
 actor_principal_id uuid NOT NULL, idempotency_key text NOT NULL CHECK(idempotency_key ~ '^[0-9a-f]{32}$'),
 original_publication_id uuid NOT NULL, original_work_version genui.safe_counter NOT NULL, publication_id uuid NOT NULL,
 imported_rows bigint NOT NULL CHECK(imported_rows BETWEEN 0 AND 100000),
 payload_digest genui.digest NOT NULL, created_at timestamptz NOT NULL DEFAULT date_trunc('second',now()),
 PRIMARY KEY(organization_id,app_id,work_id),
 FOREIGN KEY(organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY(organization_id,actor_principal_id) REFERENCES genui.principals,
 FOREIGN KEY(organization_id,app_id,publication_id) REFERENCES genui.publications
);
ALTER TABLE genui.work_data_copies ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.work_data_copies FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.work_data_copies USING (organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK(organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid);
GRANT SELECT(organization_id,app_id,work_id,actor_principal_id,idempotency_key,original_publication_id,original_work_version,publication_id,imported_rows,payload_digest,created_at),INSERT(organization_id,app_id,work_id,actor_principal_id,idempotency_key,original_publication_id,original_work_version,publication_id,imported_rows,payload_digest) ON genui.work_data_copies TO genui_workspace;
GRANT SELECT(organization_id,app_id,work_id) ON genui.work_data_copies TO genui_data;
GRANT SELECT(organization_id,app_id,work_id,collection_id) ON genui.record_collections TO genui_admit;
GRANT SELECT(organization_id,app_id,work_id) ON genui.internal_records TO genui_admit;
GRANT SELECT(input_id,source_revision_id,source_digest) ON genui.work_source_dependencies TO genui_admit;
GRANT SELECT(organization_id,next_commit_sequence),UPDATE(next_commit_sequence) ON genui.organization_sequences TO genui_admit;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,current_version) ON genui.internal_records TO genui_admit;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values) ON genui.internal_record_revisions TO genui_admit;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id,source_digest) ON genui.record_source_dependencies TO genui_admit;
-- No update/delete capability: only a completely empty collection may be seeded.
CREATE FUNCTION genui_private.import_work_records(p_org uuid,p_principal uuid,p_app uuid,p_work uuid,p_rows jsonb)
 RETURNS bigint LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE v_status text; v_epoch bigint; v_sequence bigint; v_count bigint; v_bytes bigint;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
 OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text THEN RETURN NULL; END IF;
 SELECT o.status INTO v_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR v_status<>'active' THEN RETURN NULL; END IF;
 SELECT m.membership_epoch INTO v_epoch FROM genui.memberships m
 WHERE m.organization_id=p_org AND m.principal_id=p_principal AND m.status='active' FOR SHARE;
 IF NOT FOUND OR v_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN NULL; END IF;
 IF NOT EXISTS(SELECT 1 FROM genui.apps a JOIN genui.saved_work w USING(organization_id,app_id)
 WHERE a.organization_id=p_org AND a.app_id=p_app AND w.work_id=p_work AND a.owner_principal_id=p_principal
 AND w.created_by=p_principal AND w.creator_revoked_at IS NULL) THEN RETURN NULL; END IF;
 IF jsonb_typeof(p_rows)<>'array' THEN RETURN NULL; END IF;
 v_count:=jsonb_array_length(p_rows);
 IF v_count>100000 OR octet_length(p_rows::text)>140000000 THEN RETURN NULL; END IF;
 SELECT coalesce(sum(length(x->>'canonical')/2),0) INTO v_bytes FROM jsonb_array_elements(p_rows) x;
 IF v_bytes>67108864 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p_rows) x WHERE
 jsonb_typeof(x)<>'object' OR (SELECT count(*) FROM jsonb_object_keys(x))<>4 OR (x->>'canonical') !~ '^[0-9a-f]+$'
 OR length(x->>'canonical') NOT BETWEEN 2 AND 524288 OR length(x->>'canonical')%2<>0
 OR NOT EXISTS(SELECT 1 FROM genui.record_collections c WHERE c.organization_id=p_org AND c.app_id=p_app AND c.work_id=p_work AND c.collection_id=(x->>'collection')::uuid)
 OR NOT EXISTS(SELECT 1 FROM genui.work_source_dependencies d WHERE d.organization_id=p_org AND d.app_id=p_app AND d.work_id=p_work AND d.input_id=x->>'source_input')) THEN RETURN NULL; END IF;
 IF EXISTS(SELECT 1 FROM genui.internal_records r WHERE r.organization_id=p_org AND r.app_id=p_app AND r.work_id=p_work) THEN RETURN NULL; END IF;
 SELECT next_commit_sequence INTO v_sequence FROM genui.organization_sequences WHERE organization_id=p_org FOR UPDATE;
 IF NOT FOUND OR v_sequence>=9007199254740991 THEN RETURN NULL; END IF;
 INSERT INTO genui.internal_records(organization_id,app_id,work_id,collection_id,record_id,current_version)
 SELECT p_org,p_app,p_work,(x->>'collection')::uuid,(x->>'record')::uuid,1 FROM jsonb_array_elements(p_rows) x;
 INSERT INTO genui.internal_record_revisions(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,canonical_values)
 SELECT p_org,p_app,p_work,(x->>'collection')::uuid,(x->>'record')::uuid,1,p_principal,p_principal,v_sequence,date_trunc('second',now()),decode(x->>'canonical','hex') FROM jsonb_array_elements(p_rows) x;
 INSERT INTO genui.record_source_dependencies(organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id,source_digest)
 SELECT p_org,p_app,p_work,(x->>'collection')::uuid,(x->>'record')::uuid,1,d.input_id,d.source_id,d.source_revision_id,d.source_digest
 FROM jsonb_array_elements(p_rows) x JOIN genui.work_source_dependencies d ON d.organization_id=p_org AND d.app_id=p_app AND d.work_id=p_work AND d.input_id=x->>'source_input';
 UPDATE genui.organization_sequences SET next_commit_sequence=v_sequence+1 WHERE organization_id=p_org;
 RETURN v_count;
END $$;
REVOKE ALL ON FUNCTION genui_private.import_work_records(uuid,uuid,uuid,uuid,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.import_work_records(uuid,uuid,uuid,uuid,jsonb) TO genui_workspace;
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.import_work_records(uuid,uuid,uuid,uuid,jsonb) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
