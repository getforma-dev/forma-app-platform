-- Trial plan (ADR-008 amendment 2026-11-14), the three data capabilities a
-- real cohort needs from a general platform: D5 durable consent to the beta
-- notice, D6 member display names, D4 an in-product organization admin area,
-- plus the operator's note on an access code. Additive: three nullable
-- columns, one insert-only table, three new genui_admit definers, two
-- replaced in place, and no stored value is rewritten; with one recorded
-- deviation from ADR-009 D9.15's additive rule: reserve_code(uuid,text,text)
-- is DROPPED and recreated with p_notice_version, not overloaded, because an
-- overload without a version would still let a caller reserve (and so admit)
-- without the consent the notice version records. Every caller in this tree
-- passes the version (storage::admission, storage::sessions, the Rust and
-- Python campaigns); nothing deployed calls the three-argument form.
--
-- Consent. The invitation page shows notice version N and refuses without the
-- checkbox; reserve_code stores N on the presignup reservation (the row the
-- consented form created), record_pending_admission copies it onto the
-- pending admission, and admit_redemption records the consent in
-- genui.member_consents and appends `admission.consented` in the same
-- transaction as the admission. A pending admission without a version (one
-- recorded before this migration) is refused as malformed: no row, no write.
--
-- Names. memberships.display_name is set only by its member through
-- set_display_name (bounded, printable, trimmed), audited as `member.named`
-- on the admission chain with the name's digest; the DS-44 persona file
-- (crate::directory) stays demo-only.
--
-- Admin. org_admin_overview and admin_remove_member require the caller's
-- live membership at its epoch plus `admin_policy` (0002
-- organization_permissions), re-checked here under the member fence. The
-- overview never reads a provider subject or an email. Removal follows the
-- operator's semantics (admission_operator.py revoke_membership and
-- revoke_entitlement): status revoked, entitlements revoked, epoch + 1 so
-- every session dies at its next request, `entitlement.revoked` and
-- `membership.revoked` appended on the admission chain with the admin as
-- actor and the removed member's digest as outcome (reconstructable). It
-- refuses the caller itself, the last active admin and a non-member.
--
-- Recorded debt, as 0008 did: ADR-007 D7.1 names six definers and D7.8 lists
-- `membership.revoked` and `entitlement.revoked` on the operator chain only;
-- this migration lets the admission chain carry them for an admin's action
-- and adds three definers to genui_admit.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 23);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 23);

-- (a) Consent -----------------------------------------------------------------
-- The notice version the invitation page showed when this reservation was
-- made; NULL only on rows from before this migration.
ALTER TABLE genui.code_reservations ADD COLUMN notice_version integer
 CHECK (notice_version IS NULL OR notice_version BETWEEN 1 AND 1000000);
GRANT SELECT(notice_version),INSERT(notice_version) ON genui.code_reservations TO genui_admit;

-- One row per member and notice version, written only by admit_redemption
-- (genui_admit, INSERT) and read only through org_admin_overview; no
-- application role holds any privilege on it (review m3: a workspace read is
-- granted when a surface consumes it). The event id names the
-- `admission.consented` event (no foreign key into the insert-only audit
-- log, 0007's decision).
CREATE TABLE genui.member_consents (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL,
 notice_version integer NOT NULL CHECK (notice_version BETWEEN 1 AND 1000000),
 consented_at timestamptz NOT NULL,
 audit_event_id uuid NOT NULL,
 PRIMARY KEY (organization_id,principal_id,notice_version),
 UNIQUE (organization_id,audit_event_id),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships
);
ALTER TABLE genui.member_consents ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.member_consents FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.member_consents USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);
GRANT SELECT(organization_id,principal_id,notice_version,consented_at),
 INSERT(organization_id,principal_id,notice_version,consented_at,audit_event_id) ON genui.member_consents TO genui_admit;

-- (b) Display names -----------------------------------------------------------
-- Bounded, printable, single-line, trimmed (crate::directory::usable_name).
-- The 0002 table-level SELECT of the application roles covers the column.
ALTER TABLE genui.memberships ADD COLUMN display_name text
 CHECK (display_name IS NULL OR (length(display_name) BETWEEN 1 AND 80 AND display_name !~ '[[:cntrl:]]' AND display_name = btrim(display_name)));
GRANT SELECT(display_name),UPDATE(display_name,status) ON genui.memberships TO genui_admit;

-- (c) The operator's note on a code (W3 writes it through the operator channel).
ALTER TABLE genui.access_codes ADD COLUMN note text
 CHECK (note IS NULL OR (length(note) BETWEEN 1 AND 80 AND note !~ '[[:cntrl:]]'));
GRANT SELECT(note) ON genui.access_codes TO genui_admit;

-- (d) The reads and writes the three definers need (D7.1 A4, by column).
GRANT SELECT(organization_id,actor_principal_id,occurred_at) ON genui.audit_events TO genui_admit;
GRANT SELECT(organization_id,principal_id,last_seen_at) ON genui.sessions TO genui_admit;
GRANT SELECT(organization_id,creator_id,version) ON genui.internal_record_revisions TO genui_admit;
GRANT SELECT(uploaded_by) ON genui.box_files TO genui_admit;
GRANT SELECT(code_id,committed_at) ON genui.code_redemptions TO genui_admit;
GRANT SELECT(organization_id,principal_id,entitlement_id,status),UPDATE(status,revoked_at) ON genui.entitlements TO genui_admit;

-- D7.8 role binding, widened in place: the admission chain also carries the
-- consent, the member's own name change and an admin's removal.
ALTER POLICY admission_chain ON genui.audit_events
 WITH CHECK (writer_role = 'admission' AND kind = ANY (ARRAY['admission.pending','admission.redeemed',
  'admission.denied_for_reconciliation','session.sign_out','session.sign_out_all','permission.granted',
  'admission.consented','member.named','membership.revoked','entitlement.revoked']));

-- The definers replaced and created in place, as their owner (genui_migration
-- may SET ROLE genui_admit, never inherit it); CREATE on genui_private is
-- granted for this block only and revoked below (as 0003, 0008 and 0012 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
SET LOCAL ROLE genui_admit;

-- 0003's reserve_code with the notice version the form consented to. The old
-- three-argument form is dropped: every reservation records a version.
DROP FUNCTION genui_private.reserve_code(uuid,text,text);
CREATE FUNCTION genui_private.reserve_code(p_org uuid,p_code_digest text,p_context_digest text,p_notice_version integer)
 RETURNS TABLE(reservation_id uuid,expires_at timestamptz,outcome text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; v_code record; v_live bigint; v_id uuid; v_expires timestamptz;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR p_context_digest !~ '^sha256:[0-9a-f]{64}$' OR p_notice_version NOT BETWEEN 1 AND 1000000 THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT a.code_id,a.status,a.max_redemptions,a.redeemed_count,a.expires_at INTO v_code FROM genui.access_codes a
  WHERE a.organization_id=p_org AND a.code_digest=p_code_digest FOR UPDATE;
 IF NOT FOUND THEN RETURN; END IF;
 IF v_code.status='revoked' THEN RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'revoked'::text; RETURN; END IF;
 IF v_code.status='expired' OR v_code.expires_at<=now() THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'expired'::text; RETURN;
 END IF;
 -- D7.17: remaining counts redemptions and live pending admissions only.
 SELECT count(*) INTO v_live FROM genui.code_reservations r WHERE r.organization_id=p_org AND r.code_id=v_code.code_id
  AND r.kind='pending' AND r.state='live' AND r.expires_at>now();
 IF v_code.status='exhausted' OR v_code.max_redemptions-v_code.redeemed_count-v_live<1 THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'exhausted'::text; RETURN;
 END IF;
 SELECT count(*) INTO v_live FROM genui.code_reservations r WHERE r.organization_id=p_org AND r.code_id=v_code.code_id
  AND r.kind='presignup' AND r.state='live' AND r.expires_at>now();
 IF v_live>=10 THEN RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'too_many_reservations'::text; RETURN; END IF;
 v_id:=gen_random_uuid(); v_expires:=now()+interval '10 minutes';
 INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,state,expires_at,notice_version)
  VALUES (p_org,v_id,v_code.code_id,'presignup',p_context_digest,'live',v_expires,p_notice_version);
 RETURN QUERY SELECT v_id,v_expires,'reserved'::text;
END $$;

-- 0003's record_pending_admission; the pending row inherits the presignup
-- reservation's notice version. Everything else is unchanged.
CREATE OR REPLACE FUNCTION genui_private.record_pending_admission(p_org uuid,p_reservation uuid,p_issuer text,p_provider_tenant text,p_provider_subject text)
 RETURNS TABLE(pending_admission_id uuid,expires_at timestamptz,outcome text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; v_code_id uuid; v_code record; v_res record; v_live bigint; v_id uuid; v_expires timestamptz;
 v_kind text:='admission.pending'; v_actor uuid; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 -- Malformed identity input is a storage error, not a business refusal: no row
 -- (the 0002 column bounds). Binding existence and active state are the
 -- storage module's registry and binding check before this call.
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR length(p_issuer) NOT BETWEEN 1 AND 512 OR length(p_provider_tenant) NOT BETWEEN 1 AND 256
  OR length(p_provider_subject) NOT BETWEEN 1 AND 256 THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT r.code_id INTO v_code_id FROM genui.code_reservations r
  WHERE r.organization_id=p_org AND r.reservation_id=p_reservation AND r.kind='presignup';
 IF NOT FOUND THEN RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'reservation_expired'::text; RETURN; END IF;
 -- Lock order: organization, code, then the reservation (D7.1).
 SELECT a.status,a.max_redemptions,a.redeemed_count,a.expires_at INTO v_code FROM genui.access_codes a
  WHERE a.organization_id=p_org AND a.code_id=v_code_id FOR UPDATE;
 SELECT r.state,r.expires_at,r.context_digest,r.notice_version INTO v_res FROM genui.code_reservations r
  WHERE r.organization_id=p_org AND r.reservation_id=p_reservation FOR UPDATE;
 IF v_res.state<>'live' OR v_res.expires_at<=now() THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'reservation_expired'::text; RETURN;
 END IF;
 IF v_code.status='revoked' THEN RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'revoked'::text; RETURN; END IF;
 IF v_code.status='expired' OR v_code.expires_at<=now() THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'expired'::text; RETURN;
 END IF;
 SELECT count(*) INTO v_live FROM genui.code_reservations r WHERE r.organization_id=p_org AND r.code_id=v_code_id
  AND r.kind='pending' AND r.state='live' AND r.expires_at>now();
 IF v_code.status='exhausted' OR v_code.max_redemptions-v_code.redeemed_count-v_live<1 THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'exhausted'::text; RETURN;
 END IF;
 IF EXISTS (SELECT 1 FROM genui.principals p WHERE p.organization_id=p_org AND p.issuer=p_issuer
   AND p.provider_tenant_id=p_provider_tenant AND p.provider_subject=p_provider_subject) THEN
  RETURN QUERY SELECT NULL::uuid,NULL::timestamptz,'already_member'::text; RETURN;
 END IF;
 -- One live pending admission per identity and code (APP-073 review m3): a
 -- repeated callback consumes its presignup and answers the live row, with no
 -- new slot hold and no event. The code row lock above serializes this check.
 SELECT r.reservation_id,r.expires_at INTO v_id,v_expires FROM genui.code_reservations r
  WHERE r.organization_id=p_org AND r.code_id=v_code_id AND r.kind='pending' AND r.state='live' AND r.expires_at>now()
   AND r.issuer=p_issuer AND r.provider_tenant_id=p_provider_tenant AND r.provider_subject=p_provider_subject
  ORDER BY r.expires_at DESC,r.reservation_id LIMIT 1;
 IF FOUND THEN
  UPDATE genui.code_reservations SET state='consumed',consumed_at=now() WHERE organization_id=p_org AND reservation_id=p_reservation;
  RETURN QUERY SELECT v_id,v_expires,'pending'::text; RETURN;
 END IF;
 v_id:=gen_random_uuid(); v_expires:=now()+interval '24 hours';
 INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at,notice_version)
  VALUES (p_org,v_id,v_code_id,'pending',v_res.context_digest,p_issuer,p_provider_tenant,p_provider_subject,'live',v_expires,v_res.notice_version);
 UPDATE genui.code_reservations SET state='consumed',consumed_at=now() WHERE organization_id=p_org AND reservation_id=p_reservation;
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
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 RETURN QUERY SELECT v_id,v_expires,'pending'::text;
END $$;

-- 0003's admit_redemption; an admission also records the consent the pending
-- admission carries and appends `admission.consented` after
-- `admission.redeemed` on the same chain, in the same transaction. A pending
-- admission without a notice version is malformed: no row, no write.
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
 SELECT a.kind,a.status,a.max_redemptions,a.redeemed_count,a.expires_at INTO v_code FROM genui.access_codes a
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
   INSERT INTO genui.memberships(organization_id,principal_id,status,membership_epoch) VALUES (p_org,v_principal,'active',v_epoch);
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

-- D6: the member sets the name others see. The context rule is share_work's:
-- the transaction's scope is the member's own organization, principal and
-- current epoch; a refusal (or an unusable name) returns no row. The same
-- name again changes nothing and writes no event. The event carries the
-- name's digest, never the name.
CREATE FUNCTION genui_private.set_display_name(p_org uuid,p_principal uuid,p_name text)
 RETURNS TABLE(changed boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; v_current text;
 v_kind text:='member.named'; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL
  OR length(p_name) NOT BETWEEN 1 AND 80 OR p_name ~ '[[:cntrl:]]' OR p_name<>btrim(p_name) THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch,m.display_name INTO m_status,m_epoch,v_current FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF v_current IS NOT DISTINCT FROM p_name THEN RETURN QUERY SELECT false; RETURN; END IF;
 UPDATE genui.memberships SET display_name=p_name WHERE organization_id=p_org AND principal_id=p_principal;
 v_digest:='sha256:'||encode(sha256(convert_to('forma:member-name:v1','UTF8')||'\x00'::bytea||convert_to(p_name,'UTF8')),'hex');
 -- D7.8 append on the admission chain; the head row lock serializes appenders.
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||to_json(p_principal::text)::text||',"app_id":null,"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,v_kind,p_principal,NULL,NULL,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 RETURN QUERY SELECT true;
END $$;

-- D4: every member of the caller's organization, for an admin. The caller
-- must be an active member at the transaction's epoch holding admin_policy;
-- otherwise no row. Per member: the chosen name, status, whether an admin,
-- the latest consent (version, time), the last activity (the latest audit
-- event the member caused or the latest session touch, whichever is later),
-- apps owned, works created, records created, files uploaded, and the status
-- and operator note of the code the member redeemed. No provider subject,
-- no email, no digest.
CREATE FUNCTION genui_private.org_admin_overview(p_org uuid,p_caller uuid)
 RETURNS TABLE(principal_id uuid,display_name text,status text,is_admin boolean,consent_version integer,consented_at timestamptz,
  last_active timestamptz,apps bigint,works bigint,records bigint,uploads bigint,code_status text,code_note text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; m_status text; m_epoch bigint;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_caller::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO m_status,m_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_caller FOR SHARE;
 IF NOT FOUND OR m_status<>'active' OR m_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.organization_permissions x WHERE x.organization_id=p_org AND x.principal_id=p_caller AND x.permission='admin_policy') THEN RETURN; END IF;
 RETURN QUERY
  SELECT m.principal_id,m.display_name,m.status,
   EXISTS (SELECT 1 FROM genui.organization_permissions x WHERE x.organization_id=m.organization_id AND x.principal_id=m.principal_id AND x.permission='admin_policy'),
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

-- D4: an admin removes a member. Lock order: organization FOR SHARE, the
-- caller FOR SHARE, the member FOR UPDATE (the operator's lock_member order).
-- Outcomes: removed, self, last_admin, not_member; no row when the caller is
-- not an active admin at its epoch. A sole admin removing itself is refused
-- as last_admin; with another admin present, as self.
CREATE FUNCTION genui_private.admin_remove_member(p_org uuid,p_caller uuid,p_principal uuid)
 RETURNS TABLE(outcome text,membership_epoch bigint)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; c_status text; c_epoch bigint; t_status text; t_epoch bigint; v_others bigint; v_revoked bigint;
 v_kind text; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_caller::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO c_status,c_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_caller FOR SHARE;
 IF NOT FOUND OR c_status<>'active' OR c_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.organization_permissions x WHERE x.organization_id=p_org AND x.principal_id=p_caller AND x.permission='admin_policy') THEN RETURN; END IF;
 SELECT count(*) INTO v_others FROM genui.organization_permissions x JOIN genui.memberships m
  ON m.organization_id=x.organization_id AND m.principal_id=x.principal_id
  WHERE x.organization_id=p_org AND x.permission='admin_policy' AND m.status='active' AND x.principal_id<>p_principal;
 IF p_principal=p_caller THEN
  IF v_others=0 THEN RETURN QUERY SELECT 'last_admin'::text,NULL::bigint; ELSE RETURN QUERY SELECT 'self'::text,NULL::bigint; END IF;
  RETURN;
 END IF;
 SELECT m.status,m.membership_epoch INTO t_status,t_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND OR t_status<>'active' THEN RETURN QUERY SELECT 'not_member'::text,NULL::bigint; RETURN; END IF;
 -- Unreachable while the caller is itself an active admin; kept as the guard
 -- the operator's semantics name.
 IF v_others=0 THEN RETURN QUERY SELECT 'last_admin'::text,NULL::bigint; RETURN; END IF;
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
-- replaced functions keep 0003's grants; the four created here start from
-- PostgreSQL's PUBLIC default and are closed the same way).
REVOKE ALL ON FUNCTION genui_private.reserve_code(uuid,text,text,integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.set_display_name(uuid,uuid,text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.org_admin_overview(uuid,uuid) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.admin_remove_member(uuid,uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.reserve_code(uuid,text,text,integer),genui_private.set_display_name(uuid,uuid,text),
 genui_private.org_admin_overview(uuid,uuid),genui_private.admin_remove_member(uuid,uuid,uuid) TO genui_workspace;
RESET ROLE;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
