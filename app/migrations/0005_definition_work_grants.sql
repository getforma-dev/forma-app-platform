-- ADR-007 D7.3 and D7.8 (APP-010, plan Task 6): the business writes that the
-- typed definition, publication and saved-work transactions perform, and the
-- workspace and control audit chains they append to. Every table below is
-- 0002's (or 0003's audit tables) and keeps its ENABLE/FORCE RLS and
-- tenant_scope policy. Grants are column-limited, exactly the columns each
-- transaction writes or reads beyond 0002's reads; nothing here grants UPDATE
-- or DELETE on a business table, UPDATE on saved_work.version or
-- creator_revoked_at, or any write to the permission tables (D7.3). App shells
-- and the owner_publish policy rows are operator writes (D7.19; review m2:
-- "never earlier"), never an application role's. APP-009's session relations
-- arrive with 0006. Executed only by the checksummed, transactional migration
-- runner (D7.12: numbered in landing order).
--
-- ADR-007 amendment 2026-10-29 (APP-010 review M1, M2), applied to this
-- migration in place because it has been applied only to disposable
-- databases: saved_work records its creator, whose authority equals
-- record_read and record_write on that work until the operator revokes it;
-- publications carry a per-app publication_sequence assigned under the
-- control-head lock, so the newest publication is unambiguous.
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 5);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 5);

-- ADR-007 amendment (M2): the saved-work half of ADR-009 B13. A NULL creator
-- (a pre-amendment or operator-seeded work) carries no creator authority;
-- creator_revoked_at is written only by the operator's
-- revoke-creator-authority command.
ALTER TABLE genui.saved_work ADD COLUMN created_by uuid NULL, ADD COLUMN creator_revoked_at timestamptz NULL,
 ADD FOREIGN KEY (organization_id,created_by) REFERENCES genui.memberships;
-- ADR-007 amendment (M1): an app's publications in a total order.
ALTER TABLE genui.publications ADD COLUMN publication_sequence genui.safe_counter NOT NULL,
 ADD UNIQUE (organization_id,app_id,publication_sequence);

-- D7.3 genui_workspace (APP-010 part): drafts and saved work. 0002 already
-- grants SELECT on every one of these relations.
GRANT INSERT(organization_id,app_id,revision_id,schema_version,digest,canonical_content)
 ON genui.definition_revisions TO genui_workspace;
GRANT INSERT(organization_id,app_id,revision_id,input_id,kind,contract_digest,canonical_contract)
 ON genui.definition_inputs TO genui_workspace;
GRANT INSERT(organization_id,app_id,revision_id,schema_id,schema_version,digest,canonical_schema)
 ON genui.definition_record_schemas TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,version,context,created_by) ON genui.saved_work TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,collection_id,schema_id,schema_version)
 ON genui.record_collections TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest,source_id,source_revision_id,
 source_digest,collection_id,schema_id,schema_version,revision_policy) ON genui.work_bindings TO genui_workspace;
GRANT INSERT(organization_id,app_id,work_id,input_id,source_id,source_revision_id,source_digest)
 ON genui.work_source_dependencies TO genui_workspace;
-- ADR-007 amendment (M2): the workspace saves work only as its own creator.
CREATE POLICY saved_work_creator ON genui.saved_work AS RESTRICTIVE FOR INSERT TO genui_workspace
 WITH CHECK (created_by = nullif(current_setting('genui.principal_id',true),'')::uuid);

-- D7.3 genui_control (APP-010): publication. 0002 already grants SELECT on
-- these relations and on publication_policies, which only the operator writes.
GRANT INSERT(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,
 policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,
 canonical_envelope,publication_sequence) ON genui.publications TO genui_control;
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
-- ADR-007 amendment (M2): the operator chain's closed kinds gain the grant,
-- grant-revocation and creator-authority-revocation events of the operator
-- package's permission subcommands. 0003's policy is widened here, in place,
-- so that 0003 and catalogs v1-v4 stay byte-identical.
ALTER POLICY operator_chain ON genui.audit_events
 WITH CHECK (writer_role = 'operator' AND kind = ANY (ARRAY['organization.provisioned','handle.rotated',
  'code.issued','code.revoked','code.extended','membership.revoked','entitlement.revoked','permission.granted',
  'permission.revoked','creator_authority.revoked']));
RESET ROLE;
