-- Organization roles (ADR-021, Victor's decisions of 2026-11-15; APP-225,
-- APP-226): every membership carries one of five platform-level roles,
-- `owner`, `admin`, `manager`, `member` or `viewer`, and the role is the one
-- source of truth for organization administration. The 0002 governance row
-- (`organization_permissions.permission = 'admin_policy'`) is folded into it
-- and the table is dropped: it never held another permission, and a second
-- source of admin standing would drift from the role.
--
-- Data migration, deterministic. In each organization, the active
-- `admin_policy` holder admitted earliest becomes `owner`: order by the
-- earliest `code_redemptions.committed_at` of the principal, NULLS LAST (a
-- seeded or operator-admitted member has no redemption), then by the
-- smallest principal id, so two holders with the same or no admission time
-- resolve the same way on every database. Every other active holder becomes
-- `admin`. Everyone else keeps the column default, `member`; a revoked
-- membership keeps `member` whatever it held. An organization with no
-- holder gets no owner (operator-managed synthetic organizations stay
-- valid): the last-owner rule below governs transitions only. The three
-- tables the UPDATE reads are owner-held with FORCE RLS and this migration
-- has no organization scope, so FORCE is lifted for the one statement and
-- restored right after; the catalog this migration leaves is the same.
--
-- Capability ceiling (ADR-021). Build, write, share, export, connect an
-- agent or Slack and use any AI: owner, admin, manager and member. Read:
-- every role. See `/admin` and manage members: owner, admin and manager.
-- Who may set which role on whom:
--   owner    sets any role on anyone (owner and admin included) and removes anyone;
--   admin    sets manager, member or viewer on a manager, member or viewer, and removes them;
--   manager  sets member or viewer on a member or viewer, and removes them.
-- Nobody raises their own role (`self`); nobody acts on a role above what
-- their own role manages (`not_allowed`); an organization never loses its
-- last owner (`last_owner`). A helper (an assistant, a shift lead) is not a
-- role: a member, plus `record_review` on the apps they help with.
--
-- Definers (genui_admit, D7.1 A4 by column). `set_member_role` is new;
-- `admin_remove_member` is replaced in place with the same table and the
-- `last_owner` code (0023's `last_admin`); `org_admin_overview` answers the
-- role per member and is callable by owner, admin and manager, so (one
-- recorded deviation from ADR-009 D9.15's additive rule, as 0023's
-- reserve_code) it is DROPPED and recreated: its `is_admin` column becomes
-- `role`, and a return type cannot change in place. `steward_card` (0015)
-- and `admit_redemption` (0023) are replaced in place: the steward is the
-- app's owner or an organization owner or admin; an admission takes the
-- code's role. Every role change advances the member's epoch (sessions
-- re-read authority at their next request) and appends
-- `membership.role_changed` on the admission chain (the admin's actions) or
-- the operator chain (the operator's `set-role`), its digest recomputable
-- from the target id, the role before and the role after.
--
-- Access codes carry the role they admit with (`member` or `viewer`;
-- managers, admins and owners are promoted after joining, never by code).
--
-- Viewer read-only, in the database as well as in Rust (defense in depth):
-- the RESTRICTIVE policy `viewer_read_only` on every table a write, share,
-- export, build, upload or connection reaches refuses INSERT, UPDATE and
-- DELETE in a member transaction whose scoped principal is a viewer. A
-- transaction with no principal (admission, the operator, a guest) is not
-- judged by it. Storage's own SQL clauses (`authorization::not_viewer`)
-- refuse first; this policy holds if a Rust gate is ever missed.
--
-- Recorded debt, as 0008, 0023 and 0024 did: ADR-007 D7.1 names six
-- definers and D7.8's closed kind lists gain `membership.role_changed` on
-- the admission and operator chains.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 25);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 25);

-- (a) The role ------------------------------------------------------------------
ALTER TABLE genui.memberships ADD COLUMN role text NOT NULL DEFAULT 'member'
 CHECK (role IN ('owner','admin','manager','member','viewer'));
-- The 0002 table-level SELECT of the application roles covers the column.
GRANT SELECT(role),INSERT(role),UPDATE(role) ON genui.memberships TO genui_admit;

-- (b) The data migration (see the header). FORCE is lifted for one statement.
ALTER TABLE genui.memberships NO FORCE ROW LEVEL SECURITY;
ALTER TABLE genui.organization_permissions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE genui.code_redemptions NO FORCE ROW LEVEL SECURITY;
WITH holders AS (
 SELECT x.organization_id,x.principal_id,
  row_number() OVER (PARTITION BY x.organization_id ORDER BY
   (SELECT min(d.committed_at) FROM genui.code_redemptions d WHERE d.organization_id=x.organization_id AND d.principal_id=x.principal_id) NULLS LAST,
   x.principal_id) AS admitted
 FROM genui.organization_permissions x JOIN genui.memberships m ON m.organization_id=x.organization_id AND m.principal_id=x.principal_id
 WHERE x.permission='admin_policy' AND m.status='active')
UPDATE genui.memberships m SET role=CASE WHEN h.admitted=1 THEN 'owner' ELSE 'admin' END
 FROM holders h WHERE m.organization_id=h.organization_id AND m.principal_id=h.principal_id;
ALTER TABLE genui.memberships FORCE ROW LEVEL SECURITY;
ALTER TABLE genui.code_redemptions FORCE ROW LEVEL SECURITY;
-- The governance row has moved into the role; the table held nothing else.
DROP TABLE genui.organization_permissions;

-- (c) Access codes admit with a role.
ALTER TABLE genui.access_codes ADD COLUMN role text NOT NULL DEFAULT 'member' CHECK (role IN ('member','viewer'));
GRANT SELECT(role) ON genui.access_codes TO genui_admit;

-- (d) D7.8 role bindings, restated whole: both chains carry a role change.
ALTER POLICY admission_chain ON genui.audit_events
 WITH CHECK (writer_role = 'admission' AND kind = ANY (ARRAY['admission.pending','admission.redeemed',
  'admission.denied_for_reconciliation','session.sign_out','session.sign_out_all','permission.granted',
  'admission.consented','member.named','membership.revoked','entitlement.revoked','permission.revoked',
  'membership.role_changed']));
ALTER POLICY operator_chain ON genui.audit_events
 WITH CHECK (writer_role = 'operator' AND kind = ANY (ARRAY['organization.provisioned','handle.rotated',
  'code.issued','code.revoked','code.extended','membership.revoked','entitlement.revoked','permission.granted',
  'permission.revoked','creator_authority.revoked','provider.invitation.created','provider.invitation.reconciled',
  'provider.invitation.revoked','source_revision.registered','work.seeded','provider_tenant.bound','app.seeded',
  'membership.role_changed']));

-- (e) Viewer read-only in the database: one RESTRICTIVE policy per write
-- table and command, for the application roles and the definers' role. The
-- expression names the transaction's scoped principal; with none set
-- (admission, operator, guest) it is TRUE.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY[
  'apps','definition_revisions','definition_inputs','definition_record_schemas','publications','publication_inputs',
  'publication_record_schemas','publication_policies','app_labels',
  'saved_work','work_bindings','work_source_dependencies','work_data_copies','record_collections',
  'internal_records','internal_record_revisions','record_source_dependencies',
  'command_claims','command_outcomes',
  'work_shares','live_work_shares','app_permissions','work_permissions','source_permissions','box_shares','box_file_grants',
  'sources','source_revisions','boxes','box_files','box_file_sources','box_derivations','box_explanations','box_briefs',
  'agent_tokens','agent_attributions','slack_links'] LOOP
  EXECUTE format('CREATE POLICY viewer_read_only_insert ON genui.%I AS RESTRICTIVE FOR INSERT TO genui_workspace,genui_control,genui_data,genui_admit WITH CHECK (%s)',t,
   $v$(nullif(current_setting('genui.principal_id',true),'') IS NULL OR NOT EXISTS (SELECT 1 FROM genui.memberships vr WHERE vr.organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid AND vr.principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid AND vr.role = 'viewer'))$v$);
  EXECUTE format('CREATE POLICY viewer_read_only_update ON genui.%I AS RESTRICTIVE FOR UPDATE TO genui_workspace,genui_control,genui_data,genui_admit USING (%s) WITH CHECK (%s)',t,
   $v$(nullif(current_setting('genui.principal_id',true),'') IS NULL OR NOT EXISTS (SELECT 1 FROM genui.memberships vr WHERE vr.organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid AND vr.principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid AND vr.role = 'viewer'))$v$,
   $v$(nullif(current_setting('genui.principal_id',true),'') IS NULL OR NOT EXISTS (SELECT 1 FROM genui.memberships vr WHERE vr.organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid AND vr.principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid AND vr.role = 'viewer'))$v$);
  EXECUTE format('CREATE POLICY viewer_read_only_delete ON genui.%I AS RESTRICTIVE FOR DELETE TO genui_workspace,genui_control,genui_data,genui_admit USING (%s)',t,
   $v$(nullif(current_setting('genui.principal_id',true),'') IS NULL OR NOT EXISTS (SELECT 1 FROM genui.memberships vr WHERE vr.organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid AND vr.principal_id = nullif(current_setting('genui.principal_id',true),'')::uuid AND vr.role = 'viewer'))$v$);
 END LOOP;
END $$;

-- The definers replaced and created in place, as their owner (genui_migration
-- may SET ROLE genui_admit, never inherit it); CREATE on genui_private is
-- granted for this block only and revoked below (as 0003, 0008 and 0023 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
SET LOCAL ROLE genui_admit;

-- 0023's admit_redemption; the new membership takes the code's role.
-- Everything else is unchanged.
CREATE OR REPLACE FUNCTION genui_private.admit_redemption(p_org uuid,p_pending_admission uuid,p_issuer text,p_provider_tenant text,p_provider_subject text,p_payload_digest text)
 RETURNS TABLE(principal_id uuid,membership_epoch bigint,entitlement_id uuid,redemption_id uuid,outcome text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; v_code_id uuid; v_code record; v_pend record; v_red record; v_others bigint;
 v_principal uuid; v_epoch bigint; v_entitlement uuid; v_redemption uuid; v_outcome text;
 v_kind text; v_actor uuid; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
 v_consent_event uuid;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR p_payload_digest !~ '^sha256:[0-9a-f]{64}$'
  OR length(p_issuer) NOT BETWEEN 1 AND 512 OR length(p_provider_tenant) NOT BETWEEN 1 AND 256
  OR length(p_provider_subject) NOT BETWEEN 1 AND 256 THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT r.code_id INTO v_code_id FROM genui.code_reservations r
  WHERE r.organization_id=p_org AND r.reservation_id=p_pending_admission AND r.kind='pending';
 IF NOT FOUND THEN RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'mismatch'::text; RETURN; END IF;
 -- Lock order: organization, code, then the pending admission (D7.1).
 SELECT a.kind,a.status,a.max_redemptions,a.redeemed_count,a.expires_at,a.role INTO v_code FROM genui.access_codes a
  WHERE a.organization_id=p_org AND a.code_id=v_code_id FOR UPDATE;
 SELECT r.state,r.expires_at,r.issuer,r.provider_tenant_id,r.provider_subject,r.notice_version INTO v_pend FROM genui.code_reservations r
  WHERE r.organization_id=p_org AND r.reservation_id=p_pending_admission FOR UPDATE;
 IF v_pend.issuer<>p_issuer OR v_pend.provider_tenant_id<>p_provider_tenant OR v_pend.provider_subject<>p_provider_subject THEN
  RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'mismatch'::text; RETURN;
 END IF;
 IF v_pend.state='consumed' THEN
  -- An identical replay returns the stored redemption and consumes nothing.
  SELECT d.redemption_id,d.principal_id,d.entitlement_id,d.payload_digest INTO v_red FROM genui.code_redemptions d
   WHERE d.organization_id=p_org AND d.pending_admission_id=p_pending_admission;
  IF NOT FOUND OR v_red.payload_digest<>p_payload_digest THEN
   RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'mismatch'::text; RETURN;
  END IF;
  SELECT m.membership_epoch INTO v_epoch FROM genui.memberships m WHERE m.organization_id=p_org AND m.principal_id=v_red.principal_id;
  RETURN QUERY SELECT v_red.principal_id,v_epoch,v_red.entitlement_id,v_red.redemption_id,'replayed'::text; RETURN;
 END IF;
 IF v_pend.state<>'live' THEN
  RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'expired'::text; RETURN;
 END IF;
 -- 0023: a live pending admission must carry the consented notice version.
 IF v_pend.notice_version IS NULL THEN RETURN; END IF;
 IF v_pend.expires_at<=now() THEN
  -- ADR-006: an expired pending admission is retained for reconciliation.
  UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  v_outcome:='expired'; v_kind:='admission.denied_for_reconciliation';
 ELSIF EXISTS (SELECT 1 FROM genui.principals p WHERE p.organization_id=p_org AND p.issuer=p_issuer
   AND p.provider_tenant_id=p_provider_tenant AND p.provider_subject=p_provider_subject) THEN
  RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'already_member'::text; RETURN;
 ELSIF v_code.status='revoked' THEN
  -- ADR-006 Flow 2: a pending admission whose code was meanwhile revoked,
  -- expired or exhausted is denied and recorded for operator reconciliation.
  UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  v_outcome:='revoked'; v_kind:='admission.denied_for_reconciliation';
 ELSIF v_code.status='expired' OR v_code.expires_at<=now() THEN
  UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  v_outcome:='expired'; v_kind:='admission.denied_for_reconciliation';
 ELSE
  -- D7.17: remaining excludes the pending admission being judged.
  SELECT count(*) INTO v_others FROM genui.code_reservations r WHERE r.organization_id=p_org AND r.code_id=v_code_id
   AND r.kind='pending' AND r.state='live' AND r.expires_at>now() AND r.reservation_id<>p_pending_admission;
  IF v_code.status='exhausted' OR v_code.max_redemptions-v_code.redeemed_count-v_others<1 THEN
   UPDATE genui.code_reservations SET state='denied_for_reconciliation' WHERE organization_id=p_org AND reservation_id=p_pending_admission;
   v_outcome:='exhausted'; v_kind:='admission.denied_for_reconciliation';
  ELSE
   v_principal:=gen_random_uuid(); v_entitlement:=gen_random_uuid(); v_redemption:=gen_random_uuid(); v_epoch:=1;
   -- A concurrent admission of the same identity through another code waits on
   -- the unique key and then answers already_member; this is the branch's first
   -- write, so returning writes nothing and needs no savepoint.
   INSERT INTO genui.principals(organization_id,principal_id,issuer,provider_tenant_id,provider_subject)
    VALUES (p_org,v_principal,p_issuer,p_provider_tenant,p_provider_subject)
    ON CONFLICT (organization_id,issuer,provider_tenant_id,provider_subject) DO NOTHING;
   IF NOT FOUND THEN RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,'already_member'::text; RETURN; END IF;
   -- 0025: the membership takes the code's role (member or viewer).
   INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch,role) VALUES (p_org,v_principal,'active',v_epoch,v_code.role);
   INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission)
    SELECT p_org,v_principal,t.app_id,t.permission FROM genui.code_grant_templates t
    WHERE t.organization_id=p_org AND t.code_id=v_code_id AND t.permission IN ('discover','run','edit_definition');
   INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission)
    SELECT p_org,v_principal,t.app_id,t.work_id,t.permission FROM genui.code_grant_templates t
    WHERE t.organization_id=p_org AND t.code_id=v_code_id AND t.permission IN ('record_read','record_write','share_work','export');
   INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission)
    SELECT p_org,v_principal,t.source_id,t.permission FROM genui.code_grant_templates t
    WHERE t.organization_id=p_org AND t.code_id=v_code_id AND t.permission='source_read';
   INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at)
    VALUES (p_org,v_principal,v_entitlement,'active',v_code.kind,now(),now()+interval '30 days');
   v_outcome:='admitted'; v_kind:='admission.redeemed'; v_actor:=v_principal; v_digest:=p_payload_digest;
  END IF;
 END IF;
 -- D7.8 append on the admission chain; the head row lock serializes appenders.
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||coalesce(to_json(v_actor::text)::text,'null')||',"app_id":null,"chain_sequence":'||v_seq::text
  ||',"code_id":'||coalesce(to_json(v_code_id::text)::text,'null')||',"event_id":'||to_json(v_event::text)::text
  ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||coalesce(to_json(v_digest)::text,'null')
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,v_kind,v_actor,NULL,NULL,NULL,v_code_id,v_digest,v_at,v_prev,v_hash);
 IF v_outcome='admitted' THEN
  INSERT INTO genui.code_redemptions(organization_id,redemption_id,code_id,principal_id,pending_admission_id,entitlement_id,payload_digest,committed_at,audit_event_id)
   VALUES (p_org,v_redemption,v_code_id,v_principal,p_pending_admission,v_entitlement,p_payload_digest,now(),v_event);
  UPDATE genui.access_codes SET redeemed_count=redeemed_count+1,
   status=CASE WHEN redeemed_count+1=max_redemptions THEN 'exhausted' ELSE status END
   WHERE organization_id=p_org AND code_id=v_code_id;
  UPDATE genui.code_reservations SET state='consumed',consumed_at=now() WHERE organization_id=p_org AND reservation_id=p_pending_admission;
  -- 0023: the consent, as the second event of this admission. Its outcome
  -- digest is the notice version under its own domain, so the chain alone
  -- tells which version was agreed to.
  v_prev:=v_hash; v_seq:=v_seq+1; v_consent_event:=gen_random_uuid(); v_kind:='admission.consented';
  v_digest:='sha256:'||encode(sha256(convert_to('forma:notice-consent:v1','UTF8')||'\x00'::bytea||convert_to(v_pend.notice_version::text,'UTF8')),'hex');
  v_body:='{"actor_principal_id":'||to_json(v_principal::text)::text||',"app_id":null,"chain_sequence":'||v_seq::text
   ||',"code_id":'||to_json(v_code_id::text)::text||',"event_id":'||to_json(v_consent_event::text)::text
   ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
   ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
   ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
  v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
  INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
   VALUES (p_org,v_consent_event,'admission',v_seq,v_kind,v_principal,NULL,NULL,NULL,v_code_id,v_digest,v_at,v_prev,v_hash);
  INSERT INTO genui.member_consents(organization_id,principal_id,notice_version,consented_at,audit_event_id)
   VALUES (p_org,v_principal,v_pend.notice_version,v_at,v_consent_event);
 END IF;
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 IF v_outcome='admitted' THEN
  RETURN QUERY SELECT v_principal,v_epoch,v_entitlement,v_redemption,'admitted'::text;
 ELSE
  RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,v_outcome;
 END IF;
END $$;

-- 0015's steward_card: the steward is the app's owner or an organization
-- owner or admin (the former admin_policy holder). Everything else is unchanged.
CREATE OR REPLACE FUNCTION genui_private.steward_card(p_org uuid,p_actor uuid,p_card uuid,p_remove boolean)
 RETURNS TABLE(app_id uuid,work_id uuid,changed boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; m_role text; v_app uuid; v_work uuid; v_kind text; v_everyone boolean;
 v_removed timestamptz; v_owner uuid;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_actor::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch,m.role INTO m_status,m_epoch,m_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_actor FOR SHARE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 SELECT c.app_id,c.work_id,c.kind,c.everyone,c.removed_at INTO v_app,v_work,v_kind,v_everyone,v_removed
  FROM genui.member_cards c WHERE c.organization_id=p_org AND c.card_id=p_card FOR UPDATE;
 IF NOT FOUND OR v_kind<>'card' OR NOT v_everyone THEN RETURN; END IF;
 SELECT a.owner_principal_id INTO v_owner FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=v_app;
 IF NOT FOUND THEN RETURN; END IF;
 IF v_owner IS DISTINCT FROM p_actor AND m_role NOT IN ('owner','admin') THEN RETURN; END IF;
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

-- 0023's org_admin_overview, with the role per member in place of the admin
-- flag (dropped and recreated: the return type changes). The caller must be
-- an active owner, admin or manager at the transaction's epoch; otherwise no
-- row. Everything else is unchanged.
DROP FUNCTION genui_private.org_admin_overview(uuid,uuid);
CREATE FUNCTION genui_private.org_admin_overview(p_org uuid,p_caller uuid)
 RETURNS TABLE(principal_id uuid,display_name text,status text,role text,consent_version integer,consented_at timestamptz,
  last_active timestamptz,apps bigint,works bigint,records bigint,uploads bigint,code_status text,code_note text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; m_status text; m_epoch bigint; m_role text;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_caller::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch,m.role INTO m_status,m_epoch,m_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_caller FOR SHARE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF m_role NOT IN ('owner','admin','manager') THEN RETURN; END IF;
 RETURN QUERY
  SELECT m.principal_id,m.display_name,m.status,m.role,
   c.notice_version,c.consented_at,
   greatest(
    (SELECT max(e.occurred_at) FROM genui.audit_events e WHERE e.organization_id=m.organization_id AND e.actor_principal_id=m.principal_id),
    (SELECT max(s.last_seen_at) FROM genui.sessions s WHERE s.organization_id=m.organization_id AND s.principal_id=m.principal_id)),
   (SELECT count(*) FROM genui.apps a WHERE a.organization_id=m.organization_id AND a.owner_principal_id=m.principal_id),
   (SELECT count(*) FROM genui.saved_work w WHERE w.organization_id=m.organization_id AND w.created_by=m.principal_id),
   (SELECT count(*) FROM genui.internal_record_revisions r WHERE r.organization_id=m.organization_id AND r.creator_id=m.principal_id AND r.version=1),
   (SELECT count(*) FROM genui.box_files f WHERE f.organization_id=m.organization_id AND f.uploaded_by=m.principal_id),
   k.status,k.note
  FROM genui.memberships m
  LEFT JOIN LATERAL (SELECT c.notice_version,c.consented_at FROM genui.member_consents c
   WHERE c.organization_id=m.organization_id AND c.principal_id=m.principal_id ORDER BY c.notice_version DESC LIMIT 1) c ON true
  LEFT JOIN LATERAL (SELECT a.status,a.note FROM genui.code_redemptions d JOIN genui.access_codes a ON a.organization_id=d.organization_id AND a.code_id=d.code_id
   WHERE d.organization_id=m.organization_id AND d.principal_id=m.principal_id ORDER BY d.committed_at DESC LIMIT 1) k ON true
  WHERE m.organization_id=p_org
  ORDER BY m.display_name NULLS LAST,m.principal_id;
END $$;

-- An owner, admin or manager sets a member's role (ADR-021). Lock order:
-- organization FOR SHARE, the caller FOR SHARE, the target FOR UPDATE (the
-- operator's lock_member order). Outcomes: `changed` (the role, the epoch
-- advanced, one `membership.role_changed` event), `unchanged` (the same role
-- again: nothing written), or a refusal from the closed set `unknown_role`,
-- `not_member` (the target is not an active member), `self` (raising one's
-- own role), `not_allowed` (the target's role or the new role is above what
-- the caller's role manages), `last_owner` (the organization would be left
-- without an active owner); no row when the caller is not an active owner,
-- admin or manager at its epoch. The ladder: owner 4, admin 3, manager 2,
-- member 1, viewer 0; an owner manages up to 4, an admin up to 2, a manager
-- up to 1. The event's digest is recomputable: the target id, the role
-- before and the role after, under `forma:member-role:v1`.
CREATE FUNCTION genui_private.set_member_role(p_org uuid,p_caller uuid,p_principal uuid,p_role text)
 RETURNS TABLE(outcome text,changed boolean,membership_epoch bigint)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; c_status text; c_epoch bigint; c_role text; t_status text; t_role text; t_epoch bigint;
 v_manages integer; v_rank_caller integer; v_rank_target integer; v_rank_new integer; v_other_owners bigint;
 v_kind text:='membership.role_changed'; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_caller::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch,m.role INTO c_status,c_epoch,c_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_caller FOR SHARE;
 IF NOT FOUND OR c_status<>'active' OR c_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF c_role NOT IN ('owner','admin','manager') THEN RETURN; END IF;
 IF p_role NOT IN ('owner','admin','manager','member','viewer') THEN RETURN QUERY SELECT 'unknown_role'::text,false,NULL::bigint; RETURN; END IF;
 -- The target FOR UPDATE (the caller's own row, when it is the target, is
 -- upgraded from this transaction's FOR SHARE).
 SELECT m.status,m.membership_epoch,m.role INTO t_status,t_epoch,t_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND OR t_status<>'active' THEN RETURN QUERY SELECT 'not_member'::text,false,NULL::bigint; RETURN; END IF;
 v_rank_caller:=CASE c_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'manager' THEN 2 WHEN 'member' THEN 1 ELSE 0 END;
 v_rank_target:=CASE t_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'manager' THEN 2 WHEN 'member' THEN 1 ELSE 0 END;
 v_rank_new:=CASE p_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'manager' THEN 2 WHEN 'member' THEN 1 ELSE 0 END;
 v_manages:=CASE c_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 2 ELSE 1 END;
 IF p_principal=p_caller AND v_rank_new>v_rank_caller THEN RETURN QUERY SELECT 'self'::text,false,NULL::bigint; RETURN; END IF;
 IF v_rank_target>v_manages OR v_rank_new>v_manages THEN RETURN QUERY SELECT 'not_allowed'::text,false,NULL::bigint; RETURN; END IF;
 IF t_role=p_role THEN RETURN QUERY SELECT 'unchanged'::text,false,t_epoch; RETURN; END IF;
 IF t_role='owner' THEN
  SELECT count(*) INTO v_other_owners FROM genui.memberships m
   WHERE m.organization_id=p_org AND m.role='owner' AND m.status='active' AND m.principal_id<>p_principal;
  IF v_other_owners=0 THEN RETURN QUERY SELECT 'last_owner'::text,false,NULL::bigint; RETURN; END IF;
 END IF;
 UPDATE genui.memberships SET role=p_role,membership_epoch=membership_epoch+1
  WHERE organization_id=p_org AND principal_id=p_principal RETURNING membership_epoch INTO t_epoch;
 v_digest:='sha256:'||encode(sha256(convert_to('forma:member-role:v1','UTF8')||'\x00'::bytea||convert_to(p_principal::text,'UTF8')||'\x00'::bytea||convert_to(t_role,'UTF8')||'\x00'::bytea||convert_to(p_role,'UTF8')),'hex');
 -- D7.8 append on the admission chain; the head row lock serializes appenders.
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||to_json(p_caller::text)::text||',"app_id":null,"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,v_kind,p_caller,NULL,NULL,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 RETURN QUERY SELECT 'changed'::text,true,t_epoch;
END $$;

-- 0023's admin_remove_member under the same ladder. Lock order as before.
-- Outcomes: removed, self, last_owner, not_member, not_allowed; no row when
-- the caller is not an active owner, admin or manager at its epoch. A sole
-- owner removing itself is refused as last_owner; anyone else removing
-- itself as self; a target above what the caller manages as not_allowed.
CREATE OR REPLACE FUNCTION genui_private.admin_remove_member(p_org uuid,p_caller uuid,p_principal uuid)
 RETURNS TABLE(outcome text,membership_epoch bigint)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; c_status text; c_epoch bigint; c_role text; t_status text; t_role text; t_epoch bigint; v_other_owners bigint; v_revoked bigint;
 v_manages integer; v_rank_target integer;
 v_kind text; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_caller::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch,m.role INTO c_status,c_epoch,c_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_caller FOR SHARE;
 IF NOT FOUND OR c_status<>'active' OR c_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF c_role NOT IN ('owner','admin','manager') THEN RETURN; END IF;
 IF p_principal=p_caller THEN
  IF c_role='owner' THEN
   SELECT count(*) INTO v_other_owners FROM genui.memberships m
    WHERE m.organization_id=p_org AND m.role='owner' AND m.status='active' AND m.principal_id<>p_caller;
   IF v_other_owners=0 THEN RETURN QUERY SELECT 'last_owner'::text,NULL::bigint; RETURN; END IF;
  END IF;
  RETURN QUERY SELECT 'self'::text,NULL::bigint; RETURN;
 END IF;
 SELECT m.status,m.membership_epoch,m.role INTO t_status,t_epoch,t_role FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND OR t_status<>'active' THEN RETURN QUERY SELECT 'not_member'::text,NULL::bigint; RETURN; END IF;
 v_rank_target:=CASE t_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'manager' THEN 2 WHEN 'member' THEN 1 ELSE 0 END;
 v_manages:=CASE c_role WHEN 'owner' THEN 4 WHEN 'admin' THEN 2 ELSE 1 END;
 IF v_rank_target>v_manages THEN RETURN QUERY SELECT 'not_allowed'::text,NULL::bigint; RETURN; END IF;
 IF t_role='owner' THEN
  -- Unreachable while the caller is itself an active owner; kept as the
  -- guard the rule names.
  SELECT count(*) INTO v_other_owners FROM genui.memberships m
   WHERE m.organization_id=p_org AND m.role='owner' AND m.status='active' AND m.principal_id<>p_principal;
  IF v_other_owners=0 THEN RETURN QUERY SELECT 'last_owner'::text,NULL::bigint; RETURN; END IF;
 END IF;
 UPDATE genui.entitlements SET status='revoked',revoked_at=now()
  WHERE organization_id=p_org AND principal_id=p_principal AND status='active';
 GET DIAGNOSTICS v_revoked=ROW_COUNT;
 UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1
  WHERE organization_id=p_org AND principal_id=p_principal RETURNING membership_epoch INTO t_epoch;
 v_digest:='sha256:'||encode(sha256(convert_to('forma:member-removal:v1','UTF8')||'\x00'::bytea||convert_to(p_principal::text,'UTF8')),'hex');
 -- D7.8 append on the admission chain; the head row lock serializes appenders.
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_at:=date_trunc('second',now());
 FOREACH v_kind IN ARRAY (CASE WHEN v_revoked>0 THEN ARRAY['entitlement.revoked','membership.revoked'] ELSE ARRAY['membership.revoked'] END) LOOP
  v_seq:=v_seq+1; v_event:=gen_random_uuid();
  v_body:='{"actor_principal_id":'||to_json(p_caller::text)::text||',"app_id":null,"chain_sequence":'||v_seq::text
   ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
   ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
   ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
   ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
  v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
  INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
   VALUES (p_org,v_event,'admission',v_seq,v_kind,p_caller,NULL,NULL,NULL,NULL,v_digest,v_at,v_prev,v_hash);
  v_prev:=v_hash;
 END LOOP;
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 RETURN QUERY SELECT 'removed'::text,t_epoch;
END $$;
-- EXECUTE for genui_workspace only, granted by the functions' owner (the
-- replaced functions keep their grants; the two created here start from
-- PostgreSQL's PUBLIC default and are closed the same way).
REVOKE ALL ON FUNCTION genui_private.org_admin_overview(uuid,uuid) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.set_member_role(uuid,uuid,uuid,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.org_admin_overview(uuid,uuid),genui_private.set_member_role(uuid,uuid,uuid,text) TO genui_workspace;
RESET ROLE;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
