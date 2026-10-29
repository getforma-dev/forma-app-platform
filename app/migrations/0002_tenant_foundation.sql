-- ADR-004: real tenant relations; application writes remain closed.
-- Executed only by the checksummed, transactional migration runner.
DO $$
DECLARE r record;
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='genui_lock') THEN
    RAISE EXCEPTION 'Unexpected preexisting lock role' USING ERRCODE='42501';
  END IF;
  CREATE ROLE genui_lock NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
END $$;
-- The migration operator may transfer function ownership; applications never may.
GRANT genui_lock TO genui_migration WITH INHERIT FALSE, SET TRUE;
CREATE SCHEMA genui AUTHORIZATION genui_owner;
CREATE SCHEMA genui_private AUTHORIZATION genui_owner;
SET LOCAL ROLE genui_owner;

CREATE DOMAIN genui.safe_counter AS bigint CHECK (VALUE BETWEEN 1 AND 9007199254740991);
CREATE DOMAIN genui.digest AS text CHECK (octet_length(VALUE)=71 AND VALUE ~ '^sha256:[0-9a-f]{64}$');
CREATE DOMAIN genui.local_id AS text CHECK (octet_length(VALUE) BETWEEN 1 AND 64 AND VALUE ~ '^[a-z][a-z0-9_]{0,63}$' AND VALUE !~ '[^a-z0-9_]');

CREATE TABLE genui.organizations (
 organization_id uuid PRIMARY KEY, display_name text NOT NULL CHECK (length(display_name) BETWEEN 1 AND 200),
 status text NOT NULL CHECK (status IN ('active','suspended'))
);
CREATE TABLE genui.provider_tenant_bindings (
 organization_id uuid NOT NULL REFERENCES genui.organizations,
 issuer text NOT NULL CHECK (length(issuer) BETWEEN 1 AND 512),
 provider_tenant_id text NOT NULL CHECK (length(provider_tenant_id) BETWEEN 1 AND 256), active boolean NOT NULL,
 PRIMARY KEY (organization_id,issuer,provider_tenant_id)
);
CREATE TABLE genui.principals (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, issuer text NOT NULL, provider_tenant_id text NOT NULL,
 provider_subject text NOT NULL CHECK (length(provider_subject) BETWEEN 1 AND 256),
 PRIMARY KEY (organization_id,principal_id),
 UNIQUE (organization_id,issuer,provider_tenant_id,provider_subject),
 FOREIGN KEY (organization_id,issuer,provider_tenant_id) REFERENCES genui.provider_tenant_bindings
);
CREATE TABLE genui.memberships (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL,
 status text NOT NULL CHECK (status IN ('active','revoked')), membership_epoch genui.safe_counter NOT NULL,
 PRIMARY KEY (organization_id,principal_id), FOREIGN KEY (organization_id,principal_id) REFERENCES genui.principals
);
CREATE TABLE genui.spaces (
 organization_id uuid NOT NULL REFERENCES genui.organizations, space_id uuid NOT NULL,
 display_name text NOT NULL CHECK (length(display_name) BETWEEN 1 AND 200), PRIMARY KEY (organization_id,space_id)
);
CREATE TABLE genui.apps (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, space_id uuid NOT NULL, owner_principal_id uuid NOT NULL,
 PRIMARY KEY (organization_id,app_id), FOREIGN KEY (organization_id,space_id) REFERENCES genui.spaces,
 FOREIGN KEY (organization_id,owner_principal_id) REFERENCES genui.principals
);
CREATE TABLE genui.definition_revisions (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, revision_id uuid NOT NULL,
 schema_version integer NOT NULL CHECK (schema_version=1), digest genui.digest NOT NULL,
 canonical_content bytea NOT NULL CHECK (octet_length(canonical_content) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,app_id,revision_id), UNIQUE (organization_id,app_id,revision_id,digest),
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps
);
CREATE TABLE genui.definition_inputs (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, revision_id uuid NOT NULL,
 input_id genui.local_id NOT NULL, kind text NOT NULL CHECK (kind IN ('source','records')),
 contract_digest genui.digest NOT NULL, canonical_contract bytea NOT NULL CHECK (octet_length(canonical_contract) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,app_id,revision_id,input_id),
 UNIQUE (organization_id,app_id,revision_id,input_id,kind,contract_digest),
 FOREIGN KEY (organization_id,app_id,revision_id) REFERENCES genui.definition_revisions
);
CREATE TABLE genui.definition_record_schemas (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, revision_id uuid NOT NULL,
 schema_id genui.local_id NOT NULL, schema_version integer NOT NULL CHECK (schema_version=1), digest genui.digest NOT NULL,
 canonical_schema bytea NOT NULL CHECK (octet_length(canonical_schema) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,app_id,revision_id,schema_id,schema_version),
 UNIQUE (organization_id,app_id,revision_id,schema_id,schema_version,digest),
 FOREIGN KEY (organization_id,app_id,revision_id) REFERENCES genui.definition_revisions
);
CREATE TABLE genui.publication_policies (
 organization_id uuid NOT NULL REFERENCES genui.organizations, policy_id uuid NOT NULL,
 policy_version genui.safe_counter NOT NULL, kind text NOT NULL CHECK (kind IN ('owner_publish','independent_review')),
 PRIMARY KEY (organization_id,policy_id,policy_version), UNIQUE (organization_id,policy_id,policy_version,kind)
);
CREATE TABLE genui.publications (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, publication_id uuid NOT NULL,
 definition_revision_id uuid NOT NULL, definition_digest genui.digest NOT NULL,
 policy_id uuid NOT NULL, policy_version genui.safe_counter NOT NULL, policy_kind text NOT NULL,
 author_id uuid NOT NULL, approver_id uuid NOT NULL, approver_epoch genui.safe_counter NOT NULL,
 renderer_version integer NOT NULL CHECK (renderer_version=1), operator_version integer NOT NULL CHECK (operator_version=1),
 admission_digest genui.digest NOT NULL, published_at timestamptz NOT NULL,
 canonical_envelope bytea NOT NULL CHECK (octet_length(canonical_envelope) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,app_id,publication_id),
 UNIQUE (organization_id,app_id,publication_id,definition_revision_id),
 FOREIGN KEY (organization_id,app_id,definition_revision_id,definition_digest) REFERENCES genui.definition_revisions(organization_id,app_id,revision_id,digest),
 FOREIGN KEY (organization_id,policy_id,policy_version,policy_kind) REFERENCES genui.publication_policies(organization_id,policy_id,policy_version,kind),
 FOREIGN KEY (organization_id,author_id) REFERENCES genui.principals(organization_id,principal_id),
 FOREIGN KEY (organization_id,approver_id) REFERENCES genui.principals(organization_id,principal_id),
 CHECK (policy_kind='owner_publish' OR author_id<>approver_id)
);
CREATE TABLE genui.publication_inputs (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, publication_id uuid NOT NULL, definition_revision_id uuid NOT NULL,
 input_id genui.local_id NOT NULL, kind text NOT NULL, contract_digest genui.digest NOT NULL,
 PRIMARY KEY (organization_id,app_id,publication_id,input_id),
 UNIQUE (organization_id,app_id,publication_id,input_id,kind,contract_digest),
 FOREIGN KEY (organization_id,app_id,publication_id,definition_revision_id) REFERENCES genui.publications(organization_id,app_id,publication_id,definition_revision_id),
 FOREIGN KEY (organization_id,app_id,definition_revision_id,input_id,kind,contract_digest) REFERENCES genui.definition_inputs(organization_id,app_id,revision_id,input_id,kind,contract_digest)
);
CREATE TABLE genui.publication_record_schemas (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, publication_id uuid NOT NULL, definition_revision_id uuid NOT NULL,
 schema_id genui.local_id NOT NULL, schema_version integer NOT NULL, digest genui.digest NOT NULL,
 PRIMARY KEY (organization_id,app_id,publication_id,schema_id,schema_version),
 FOREIGN KEY (organization_id,app_id,publication_id,definition_revision_id) REFERENCES genui.publications(organization_id,app_id,publication_id,definition_revision_id),
 FOREIGN KEY (organization_id,app_id,definition_revision_id,schema_id,schema_version,digest) REFERENCES genui.definition_record_schemas(organization_id,app_id,revision_id,schema_id,schema_version,digest)
);
CREATE TABLE genui.saved_work (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, publication_id uuid NOT NULL,
 version genui.safe_counter NOT NULL, context text NOT NULL CHECK (length(context)<=2000),
 PRIMARY KEY (organization_id,app_id,work_id), UNIQUE (organization_id,app_id,work_id,publication_id),
 FOREIGN KEY (organization_id,app_id,publication_id) REFERENCES genui.publications
);
CREATE TABLE genui.sources (
 organization_id uuid NOT NULL REFERENCES genui.organizations, source_id uuid NOT NULL,
 PRIMARY KEY (organization_id,source_id)
);
CREATE TABLE genui.source_revisions (
 organization_id uuid NOT NULL, source_id uuid NOT NULL, revision_id uuid NOT NULL,
 content_digest genui.digest NOT NULL, normalized_digest genui.digest NOT NULL, contract_digest genui.digest NOT NULL,
 deleted_at timestamptz,
 canonical_envelope bytea NOT NULL CHECK (octet_length(canonical_envelope) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,source_id,revision_id), UNIQUE (organization_id,source_id,revision_id,normalized_digest),
 UNIQUE (organization_id,source_id,revision_id,normalized_digest,contract_digest),
 FOREIGN KEY (organization_id,source_id) REFERENCES genui.sources
);
CREATE TABLE genui.record_collections (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, publication_id uuid NOT NULL,
 collection_id uuid NOT NULL, schema_id genui.local_id NOT NULL, schema_version integer NOT NULL,
 PRIMARY KEY (organization_id,app_id,work_id,collection_id),
 UNIQUE (organization_id,app_id,work_id,collection_id,schema_id,schema_version),
 FOREIGN KEY (organization_id,app_id,work_id,publication_id) REFERENCES genui.saved_work(organization_id,app_id,work_id,publication_id) DEFERRABLE INITIALLY IMMEDIATE,
 FOREIGN KEY (organization_id,app_id,publication_id,schema_id,schema_version) REFERENCES genui.publication_record_schemas
);
CREATE TABLE genui.work_bindings (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, publication_id uuid NOT NULL,
 input_id genui.local_id NOT NULL, kind text NOT NULL CHECK (kind IN ('source','records')), contract_digest genui.digest NOT NULL,
 source_id uuid, source_revision_id uuid, source_digest genui.digest,
 collection_id uuid, schema_id genui.local_id, schema_version integer, revision_policy text,
 PRIMARY KEY (organization_id,app_id,work_id,input_id),
 UNIQUE (organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest),
 FOREIGN KEY (organization_id,app_id,work_id,publication_id) REFERENCES genui.saved_work(organization_id,app_id,work_id,publication_id) DEFERRABLE INITIALLY IMMEDIATE,
 FOREIGN KEY (organization_id,app_id,publication_id,input_id,kind,contract_digest) REFERENCES genui.publication_inputs(organization_id,app_id,publication_id,input_id,kind,contract_digest),
 FOREIGN KEY (organization_id,source_id,source_revision_id,source_digest,contract_digest) REFERENCES genui.source_revisions(organization_id,source_id,revision_id,normalized_digest,contract_digest),
 FOREIGN KEY (organization_id,app_id,work_id,collection_id,schema_id,schema_version) REFERENCES genui.record_collections(organization_id,app_id,work_id,collection_id,schema_id,schema_version),
 CHECK ((kind='source' AND source_id IS NOT NULL AND source_revision_id IS NOT NULL AND source_digest IS NOT NULL
   AND collection_id IS NULL AND schema_id IS NULL AND schema_version IS NULL AND revision_policy IS NULL)
 OR (kind='records' AND source_id IS NULL AND source_revision_id IS NULL AND source_digest IS NULL
   AND collection_id IS NOT NULL AND schema_id IS NOT NULL AND schema_version IS NOT NULL AND revision_policy IS NOT NULL AND revision_policy='committed_snapshot'))
);
CREATE TABLE genui.work_source_dependencies (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, input_id genui.local_id NOT NULL,
 source_id uuid NOT NULL, source_revision_id uuid NOT NULL, source_digest genui.digest NOT NULL,
 PRIMARY KEY (organization_id,app_id,work_id,input_id,source_id,source_revision_id),
 UNIQUE (organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest),
 FOREIGN KEY (organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest) REFERENCES genui.work_bindings(organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest),
 FOREIGN KEY (organization_id,source_id,source_revision_id,source_digest) REFERENCES genui.source_revisions(organization_id,source_id,revision_id,normalized_digest)
);
CREATE TABLE genui.internal_records (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, collection_id uuid NOT NULL, record_id uuid NOT NULL,
 current_version genui.safe_counter NOT NULL,
 PRIMARY KEY (organization_id,app_id,work_id,collection_id,record_id),
 FOREIGN KEY (organization_id,app_id,work_id,collection_id) REFERENCES genui.record_collections
);
CREATE TABLE genui.internal_record_revisions (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, collection_id uuid NOT NULL, record_id uuid NOT NULL,
 version genui.safe_counter NOT NULL, creator_id uuid NOT NULL, updater_id uuid NOT NULL,
 commit_sequence genui.safe_counter NOT NULL, committed_at timestamptz NOT NULL,
 canonical_values bytea NOT NULL CHECK (octet_length(canonical_values) BETWEEN 1 AND 262144),
 PRIMARY KEY (organization_id,app_id,work_id,collection_id,record_id,version),
 FOREIGN KEY (organization_id,app_id,work_id,collection_id,record_id) REFERENCES genui.internal_records DEFERRABLE INITIALLY DEFERRED,
 FOREIGN KEY (organization_id,creator_id) REFERENCES genui.principals(organization_id,principal_id),
 FOREIGN KEY (organization_id,updater_id) REFERENCES genui.principals(organization_id,principal_id)
);
ALTER TABLE genui.internal_records ADD FOREIGN KEY (organization_id,app_id,work_id,collection_id,record_id,current_version)
 REFERENCES genui.internal_record_revisions DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE genui.record_source_dependencies (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, collection_id uuid NOT NULL, record_id uuid NOT NULL,
 record_version genui.safe_counter NOT NULL, input_id genui.local_id NOT NULL,
 source_id uuid NOT NULL, source_revision_id uuid NOT NULL, source_digest genui.digest NOT NULL,
 PRIMARY KEY (organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id),
 FOREIGN KEY (organization_id,app_id,work_id,collection_id,record_id,record_version) REFERENCES genui.internal_record_revisions,
 FOREIGN KEY (organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest) REFERENCES genui.work_source_dependencies(organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest),
 FOREIGN KEY (organization_id,source_id,source_revision_id,source_digest) REFERENCES genui.source_revisions(organization_id,source_id,revision_id,normalized_digest)
);
CREATE TABLE genui.organization_permissions (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, permission text NOT NULL CHECK (permission='admin_policy'),
 PRIMARY KEY (organization_id,principal_id,permission), FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships
);
CREATE TABLE genui.app_permissions (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, app_id uuid NOT NULL,
 permission text NOT NULL CHECK (permission IN ('discover','run','edit_definition','review_publish')),
 PRIMARY KEY (organization_id,principal_id,app_id,permission), FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps
);
CREATE TABLE genui.work_permissions (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL,
 permission text NOT NULL CHECK (permission IN ('record_read','record_write','share_work','export')),
 PRIMARY KEY (organization_id,principal_id,app_id,work_id,permission), FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work
);
CREATE TABLE genui.source_permissions (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, source_id uuid NOT NULL,
 permission text NOT NULL CHECK (permission='source_read'),
 PRIMARY KEY (organization_id,principal_id,source_id,permission), FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,source_id) REFERENCES genui.sources
);

-- Catalog loop only covers the fixed tables just created; no app DDL grant.
DO $$
DECLARE t record;
BEGIN
 FOR t IN SELECT tablename FROM pg_tables WHERE schemaname='genui' LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t.tablename);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t.tablename);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t.tablename);
 END LOOP;
END $$;
REVOKE ALL ON SCHEMA genui,genui_private FROM PUBLIC;
GRANT USAGE ON SCHEMA genui TO genui_workspace,genui_control,genui_data,genui_worker,genui_lock;
GRANT USAGE ON SCHEMA genui_private TO genui_workspace,genui_control,genui_data,genui_lock;
GRANT SELECT ON genui.organizations TO genui_workspace,genui_control,genui_data,genui_worker;
GRANT SELECT ON genui.memberships,genui.organization_permissions,genui.app_permissions,genui.work_permissions,genui.source_permissions
 TO genui_workspace,genui_control,genui_data;
GRANT SELECT ON genui.provider_tenant_bindings,genui.principals TO genui_workspace;
GRANT SELECT ON genui.spaces,genui.apps,genui.definition_revisions,genui.definition_inputs,genui.definition_record_schemas,
 genui.publication_policies,genui.publications,genui.publication_inputs,genui.publication_record_schemas
 TO genui_workspace,genui_control,genui_data;
GRANT SELECT ON genui.saved_work,genui.record_collections,genui.work_bindings,genui.work_source_dependencies,genui.sources,genui.source_revisions
 TO genui_workspace,genui_data;
GRANT SELECT ON genui.internal_records,genui.internal_record_revisions,genui.record_source_dependencies TO genui_data;

-- Row-locking SELECT needs one UPDATE column; only the non-login definer gets it.
GRANT SELECT(organization_id,status),UPDATE(status) ON genui.organizations TO genui_lock;
GRANT SELECT(organization_id,principal_id,status,membership_epoch),UPDATE(membership_epoch) ON genui.memberships TO genui_lock;
GRANT SELECT(organization_id,app_id,work_id,publication_id,version),UPDATE(version) ON genui.saved_work TO genui_lock;

CREATE FUNCTION genui_private.lock_member_authority(p_org uuid,p_principal uuid,p_epoch bigint)
 RETURNS TABLE(organization_active boolean,member_active boolean,membership_epoch bigint)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,genui,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint;
BEGIN
 IF p_epoch NOT BETWEEN 1 AND 9007199254740991 OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS DISTINCT FROM p_epoch::text THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO m_status,m_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR SHARE;
 IF NOT FOUND OR m_epoch<>p_epoch THEN RETURN; END IF;
 RETURN QUERY SELECT o_status='active',m_status='active',m_epoch;
END $$;
CREATE FUNCTION genui_private.lock_work_revision(p_org uuid,p_app uuid,p_work uuid,p_publication uuid,p_version bigint)
 RETURNS TABLE(publication_id uuid,work_version bigint)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,genui,pg_temp AS $$
DECLARE actual_publication uuid; actual_version bigint;
BEGIN
 IF p_version NOT BETWEEN 1 AND 9007199254740991 OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS NULL
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 -- Parse both required context fields; malformed/zero/overflow scope cannot lock.
 PERFORM nullif(current_setting('genui.principal_id',true),'')::uuid;
 IF current_setting('genui.membership_epoch',true)::bigint NOT BETWEEN 1 AND 9007199254740991 THEN RETURN; END IF;
 SELECT w.publication_id,w.version INTO actual_publication,actual_version FROM genui.saved_work w
  WHERE w.organization_id=p_org AND w.app_id=p_app AND w.work_id=p_work FOR SHARE;
 IF NOT FOUND OR actual_publication<>p_publication OR actual_version<>p_version THEN RETURN; END IF;
 RETURN QUERY SELECT actual_publication,actual_version;
END $$;
REVOKE ALL ON FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.lock_work_revision(uuid,uuid,uuid,uuid,bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) TO genui_workspace,genui_control,genui_data;
GRANT EXECUTE ON FUNCTION genui_private.lock_work_revision(uuid,uuid,uuid,uuid,bigint) TO genui_data;
GRANT CREATE ON SCHEMA genui_private TO genui_lock;
RESET ROLE;
ALTER FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) OWNER TO genui_lock;
ALTER FUNCTION genui_private.lock_work_revision(uuid,uuid,uuid,uuid,bigint) OWNER TO genui_lock;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_lock;
RESET ROLE;
