-- APP-169: a member's deliberate Card or App classification of one whole saved
-- work. Absence means ambiguous and is shown as an App. Nothing here counts
-- components. The row is personal: another member does not read it.
-- Each change of kind is one `work.kind_set` workspace-chain event (D7.8).
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 22);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 22);

CREATE TABLE genui.work_artifact_kinds (
 organization_id uuid NOT NULL,
 principal_id uuid NOT NULL,
 app_id uuid NOT NULL,
 work_id uuid NOT NULL,
 kind text NOT NULL CHECK (kind IN ('app','card')),
 updated_at timestamptz NOT NULL DEFAULT date_trunc('second', now()),
 PRIMARY KEY (organization_id, principal_id, app_id, work_id),
 FOREIGN KEY (organization_id, principal_id) REFERENCES genui.principals,
 FOREIGN KEY (organization_id, app_id, work_id) REFERENCES genui.saved_work
);
ALTER TABLE genui.work_artifact_kinds ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.work_artifact_kinds FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.work_artifact_kinds USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- The workspace reads and writes only the scoped principal's own rows, as the
-- 0014 personal labels and the 0015 member cards do (RESTRICTIVE, workspace only).
CREATE POLICY work_artifact_kinds_principal ON genui.work_artifact_kinds AS RESTRICTIVE FOR ALL TO genui_workspace
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid)
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);

GRANT SELECT (organization_id, principal_id, app_id, work_id, kind, updated_at),
      INSERT (organization_id, principal_id, app_id, work_id, kind),
      UPDATE (kind, updated_at)
 ON genui.work_artifact_kinds TO genui_workspace;

-- 0017's workspace kinds, plus `work.kind_set`.
ALTER POLICY workspace_chain ON genui.audit_events WITH CHECK(writer_role='workspace' AND kind=ANY(ARRAY['session.signed_in','app.created','definition.drafted','work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared','box.explained','box.briefed','app.labeled','app.aliased','card.made','answer.pinned','card.shared','card.removed','share.created','share.updated','share.revoked','work.kind_set']));
RESET ROLE;
