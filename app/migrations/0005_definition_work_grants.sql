-- ADR-007 D7.3 and D7.8 (APP-010, plan Task 6): the business writes that the
-- typed definition, publication and saved-work transactions perform, and the
-- workspace and control audit chains they append to. No new relation: every
-- table below is 0002's (or 0003's audit tables) and keeps its ENABLE/FORCE
-- RLS and tenant_scope policy. Grants are column-limited, exactly the columns
-- each transaction writes or reads beyond 0002's reads; nothing here grants
-- UPDATE or DELETE on a business table, UPDATE on saved_work.version, or any
-- write to the permission tables (D7.3). APP-009's session relations arrive
-- with 0006. The owner_publish policy rows are operator-seeded under scope
-- (plan Task 6; D7.19), never written here. Executed only by the checksummed,
-- transactional migration runner (D7.12: numbered in landing order).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 5);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 5);

-- D7.3 genui_workspace (APP-010 part): drafts and saved work. 0002 already
-- grants SELECT on every one of these relations.
GRANT INSERT(organization_id,app_id,space_id,owner_principal_id) ON genui.apps TO genui_workspace;
GRANT INSERT(organization_id,app_id,revision_id,schema_version,digest,canonical_content)
 ON genui.definition_revisions TO genui_workspace;
GRANT INSERT(organization_id,app_id,revision_id,input_id,kind,contract_digest,canonical_contract)
 ON genui.definition_inputs TO genui_workspace;
GRANT INSERT(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema)
 ON genui.definition_record_schemas TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,version,context) ON genui.saved_work TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,collection_id,schema_id,schema_version)
 ON genui.record_collections TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,source_id,source_revision_id,
 source_digest,collection_id,schema_id,schema_version,revision_policy) ON genui.work_bindings TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest)
 ON genui.work_source_dependencies TO genui_workspace;

-- D7.3 genui_control (APP-010): publication and policy. 0002 already grants
-- SELECT on these four relations.
GRANT INSERT(organization_id,policy_id,policy_version,kind) ON genui.publication_policies TO genui_control;
GRANT INSERT(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,
 policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,
 canonical_envelope) ON genui.publications TO genui_control;
GRANT INSERT(organization_id,app_id,publication_id,definition_revision_id,input_id,kind,contract_digest)
 ON genui.publication_inputs TO genui_control;
GRANT INSERT(organization_id,app_id,publication_id,definition_revision_id,schema_id,schema_version,digest)
 ON genui.publication_record_schemas TO genui_control;

-- D7.8 append protocol for both chains: INSERT on audit_events (never UPDATE
-- or DELETE), and the head row read, locked and advanced for the role's own
-- chain only.
GRANT INSERT(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,
 outcome_digest,occurred_at,previous_hash,hash) ON genui.audit_events TO genui_workspace,genui_control;
GRANT SELECT(organization_id,writer_role,chain_sequence,head_hash),UPDATE(chain_sequence,head_hash)
 ON genui.audit_heads TO genui_workspace,genui_control;

-- D7.8 role binding: each new INSERT holder on audit_events is bound to its
-- chain and closed kind list, and its head UPDATE to its own chain. D7.3's
-- "audit_heads (own chain)" read is a RESTRICTIVE SELECT policy as well, so a
-- role neither reads nor locks another chain's head (SELECT ... FOR UPDATE
-- also applies the UPDATE policy).
CREATE POLICY workspace_chain ON genui.audit_events AS RESTRICTIVE FOR INSERT TO genui_workspace
 WITH CHECK (writer_role = 'workspace' AND kind = ANY (ARRAY['session.signed_in','app.created','definition.drafted',
  'work.saved','work.bound']));
CREATE POLICY control_chain ON genui.audit_events AS RESTRICTIVE FOR INSERT TO genui_control
 WITH CHECK (writer_role = 'control' AND kind = ANY (ARRAY['publication.published','policy.changed']));
CREATE POLICY workspace_chain ON genui.audit_heads AS RESTRICTIVE FOR UPDATE TO genui_workspace
 USING (writer_role = 'workspace');
CREATE POLICY control_chain ON genui.audit_heads AS RESTRICTIVE FOR UPDATE TO genui_control
 USING (writer_role = 'control');
CREATE POLICY workspace_chain_read ON genui.audit_heads AS RESTRICTIVE FOR SELECT TO genui_workspace
 USING (writer_role = 'workspace');
CREATE POLICY control_chain_read ON genui.audit_heads AS RESTRICTIVE FOR SELECT TO genui_control
 USING (writer_role = 'control');
RESET ROLE;
