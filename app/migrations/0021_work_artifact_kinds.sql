-- APP-169: a member's deliberate Card or App classification of one whole saved
-- work. Absence means ambiguous and is shown as an App. Nothing here counts
-- components. The row is personal: another member does not read it.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 21);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 21);

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
CREATE POLICY tenant_scope ON genui.work_artifact_kinds
 USING (organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid
        AND principal_id=nullif(current_setting('genui.principal_id',true),'')::uuid)
 WITH CHECK (organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid
        AND principal_id=nullif(current_setting('genui.principal_id',true),'')::uuid);
GRANT SELECT (organization_id, principal_id, app_id, work_id, kind, updated_at),
      INSERT (organization_id, principal_id, app_id, work_id, kind),
      UPDATE (kind, updated_at)
 ON genui.work_artifact_kinds TO genui_workspace;
RESET ROLE;
