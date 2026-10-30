-- ADR-007 D7.3, D7.7, D7.8, D7.9 and ADR-009 D9.8 (APP-013, plan Task 10): the
-- relations of the atomic record command. One command transaction of the data
-- role writes the record revision, the current-version pointer, the scoped
-- idempotency outcome and exactly one `command.committed` audit event, in the
-- D7.9 lock order (organizations FOR SHARE -> member FOR SHARE -> work FOR
-- SHARE -> organization_sequences FOR UPDATE -> record FOR UPDATE -> audit_heads
-- FOR UPDATE). The idempotency claim is taken right after the shared precheck
-- (INSERT ... ON CONFLICT DO NOTHING, no savepoint) and released with a rolled
-- back transaction; the outcome is inserted last with a foreign key to it.
-- Every new relation is organization-leading and states ENABLE/FORCE RLS and
-- tenant_scope here (D7.3, D7.5). Grants are column-limited: exactly what the
-- command transaction writes beyond 0002's reads. No role gains UPDATE or
-- DELETE on a claim, an outcome, a revision or an audit event; the only UPDATEs
-- are the organization's commit sequence, the record's current-version pointer
-- and the data role's own audit head. Executed only by the checksummed,
-- transactional migration runner (D7.12: numbered in landing order).
--
-- Decisions recorded in evidence/APP-013: command_outcomes.audit_event_id has
-- no foreign key to audit_events (D7.8: audit_events is insert-only for every
-- role, and a referencing row would need a KEY SHARE lock on it); the operator
-- `verify` and the command campaign assert the pairing as a set instead.
-- query_receipts carries ADR-009 D9.8's column set and no application grant:
-- receipts are stored only by APP-023's citation path.
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 7);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 7);

-- D7.7: one row per organization, locked FOR UPDATE inside every command after
-- both fences; it serializes commands per organization. Rows are written by the
-- operator channel only (provisioning, and the backfill for existing
-- organizations with next_commit_sequence = 1 + max(commit_sequence)).
CREATE TABLE genui.organization_sequences (
 organization_id uuid PRIMARY KEY REFERENCES genui.organizations,
 next_commit_sequence genui.safe_counter NOT NULL
);
-- D7.9: the idempotency claim, insert-only, taken before the fences.
CREATE TABLE genui.command_claims (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL,
 command_id genui.local_id NOT NULL,
 idempotency_key text NOT NULL CHECK (octet_length(idempotency_key)=32 AND idempotency_key ~ '^[0-9a-f]{32}$'),
 payload_digest genui.digest NOT NULL, claimed_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,principal_id,work_id,command_id,idempotency_key),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work
);
-- The committed outcome (contract `command_outcome`, edition 1), inserted once
-- at the end of the command transaction with a foreign key to its claim.
-- Synthetic retry horizon 24 hours (ADR-002).
CREATE TABLE genui.command_outcomes (
 organization_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL, principal_id uuid NOT NULL,
 command_id genui.local_id NOT NULL,
 idempotency_key text NOT NULL CHECK (octet_length(idempotency_key)=32 AND idempotency_key ~ '^[0-9a-f]{32}$'),
 payload_digest genui.digest NOT NULL, state text NOT NULL CHECK (state='committed'),
 record_id uuid NOT NULL, record_version genui.safe_counter NOT NULL, work_version genui.safe_counter NOT NULL,
 commit_sequence genui.safe_counter NOT NULL, audit_event_id uuid NOT NULL,
 committed_at timestamptz NOT NULL, retry_expires_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,principal_id,work_id,command_id,idempotency_key),
 UNIQUE (organization_id,commit_sequence),
 UNIQUE (organization_id,audit_event_id),
 FOREIGN KEY (organization_id,principal_id,work_id,command_id,idempotency_key) REFERENCES genui.command_claims,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 CHECK (retry_expires_at = committed_at + interval '24 hours')
);
-- ADR-009 D9.8 (M10): receipts, stored only when cited by a committed record
-- command or an explicit keep (APP-023 adds columns and the citation path).
CREATE TABLE genui.query_receipts (
 organization_id uuid NOT NULL REFERENCES genui.organizations, receipt_id uuid NOT NULL,
 digest genui.digest NOT NULL,
 canonical_receipt bytea NOT NULL CHECK (octet_length(canonical_receipt) BETWEEN 1 AND 262144),
 cited_by_outcome uuid, created_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,receipt_id), UNIQUE (organization_id,digest)
);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['organization_sequences','command_claims','command_outcomes','query_receipts'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;

-- D7.3 genui_data (APP-013): the command transaction's writes. 0002 already
-- grants SELECT on internal_records, internal_record_revisions,
-- record_source_dependencies and every relation the precheck reads.
GRANT SELECT(organization_id,next_commit_sequence),UPDATE(next_commit_sequence) ON genui.organization_sequences TO genui_data;
GRANT SELECT(organization_id,principal_id,app_id,work_id,command_id,idempotency_key,payload_digest,claimed_at),
 INSERT(organization_id,principal_id,app_id,work_id,command_id,idempotency_key,payload_digest,claimed_at)
 ON genui.command_claims TO genui_data;
GRANT SELECT(organization_id,app_id,work_id,principal_id,command_id,idempotency_key,payload_digest,state,record_id,record_version,
 work_version,commit_sequence,audit_event_id,committed_at,retry_expires_at),
 INSERT(organization_id,app_id,work_id,principal_id,command_id,idempotency_key,payload_digest,state,record_id,record_version,
 work_version,commit_sequence,audit_event_id,committed_at,retry_expires_at)
 ON genui.command_outcomes TO genui_data;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,current_version),UPDATE(current_version)
 ON genui.internal_records TO genui_data;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,version,creator_id,updater_id,commit_sequence,committed_at,
 canonical_values) ON genui.internal_record_revisions TO genui_data;
GRANT INSERT(organization_id,app_id,work_id,collection_id,record_id,record_version,input_id,source_id,source_revision_id,source_digest)
 ON genui.record_source_dependencies TO genui_data;
-- D7.8 append protocol for the data chain: INSERT on audit_events (never
-- UPDATE or DELETE), the own head read, locked and advanced.
GRANT INSERT(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,
 outcome_digest,occurred_at,previous_hash,hash) ON genui.audit_events TO genui_data;
GRANT SELECT(organization_id,writer_role,chain_sequence,head_hash),UPDATE(chain_sequence,head_hash)
 ON genui.audit_heads TO genui_data;

-- D7.8 role binding for the data chain (closed kind list: command.committed),
-- as 0005 binds the workspace and control chains.
CREATE POLICY data_chain ON genui.audit_events AS RESTRICTIVE FOR INSERT TO genui_data
 WITH CHECK (writer_role = 'data' AND kind = ANY (ARRAY['command.committed']));
CREATE POLICY data_chain ON genui.audit_heads AS RESTRICTIVE FOR UPDATE TO genui_data
 USING (writer_role = 'data');
CREATE POLICY data_chain_read ON genui.audit_heads AS RESTRICTIVE FOR SELECT TO genui_data
 USING (writer_role = 'data');
-- ADR-002: the idempotency scope includes the principal, and current
-- authorization precedes outcome disclosure. The data role claims, commits
-- and reads outcomes only as the scoped principal; another principal's
-- outcome under the same key is unreachable at the row level.
CREATE POLICY command_principal ON genui.command_claims AS RESTRICTIVE FOR INSERT TO genui_data
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);
CREATE POLICY command_principal_read ON genui.command_claims AS RESTRICTIVE FOR SELECT TO genui_data
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);
CREATE POLICY command_principal ON genui.command_outcomes AS RESTRICTIVE FOR INSERT TO genui_data
 WITH CHECK (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);
CREATE POLICY command_principal_read ON genui.command_outcomes AS RESTRICTIVE FOR SELECT TO genui_data
 USING (principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid);
-- A revision records the scoped principal as its updater.
CREATE POLICY record_updater ON genui.internal_record_revisions AS RESTRICTIVE FOR INSERT TO genui_data
 WITH CHECK (updater_id = nullif(current_setting('genui.principal_id',true),'')::uuid);
RESET ROLE;
