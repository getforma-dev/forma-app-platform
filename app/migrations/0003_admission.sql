-- ADR-007 (APP-073 slice 1): admission ledger, entitlements, audit chains and
-- the genui_admit definer principal (D7.1, D7.2, D7.3, D7.8, D7.12, D7.17, D7.19).
-- Executed only by the checksummed, transactional migration runner. No
-- application role gains a table write; only genui_workspace may EXECUTE the
-- six definer functions. Revocation, issuance and provisioning stay with the
-- operator channel (genui_migration acting as genui_owner, operator chain).
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname='genui_admit') THEN
    RAISE EXCEPTION 'Unexpected preexisting admission role' USING ERRCODE='42501';
  END IF;
  CREATE ROLE genui_admit NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
END $$;
-- The migration operator may transfer function ownership; applications never may (A3).
GRANT genui_admit TO genui_migration WITH INHERIT FALSE, SET TRUE;
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 3);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 3);

-- D7.2 (A2): the only relations without RLS; readable only through
-- resolve_code/resolve_handle. Written only by the operator channel (D7.19).
CREATE TABLE genui_private.code_index (
 code_digest genui.digest PRIMARY KEY, organization_id uuid NOT NULL REFERENCES genui.organizations
);
CREATE TABLE genui_private.handle_index (
 handle_digest genui.digest PRIMARY KEY, organization_id uuid NOT NULL REFERENCES genui.organizations,
 retired_at timestamptz
);

-- D7.17: expiry at most 30 days after issuance, 1..10 redemptions, trial 30 days.
CREATE TABLE genui.access_codes (
 organization_id uuid NOT NULL REFERENCES genui.organizations, code_id uuid NOT NULL,
 code_digest genui.digest NOT NULL,
 kind text NOT NULL CHECK (kind IN ('trial','member_invite')),
 status text NOT NULL CHECK (status IN ('active','revoked','exhausted','expired')),
 destination_organization_id uuid NOT NULL,
 max_redemptions genui.safe_counter NOT NULL CHECK (max_redemptions BETWEEN 1 AND 10),
 redeemed_count bigint NOT NULL,
 issued_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
 trial_days integer NOT NULL CHECK (trial_days = 30),
 issued_by text NOT NULL CHECK (length(issued_by) BETWEEN 1 AND 200),
 revoked_at timestamptz,
 PRIMARY KEY (organization_id,code_id), UNIQUE (organization_id,code_digest),
 CHECK (destination_organization_id = organization_id),
 CHECK (redeemed_count BETWEEN 0 AND max_redemptions),
 CHECK (status <> 'exhausted' OR redeemed_count = max_redemptions),
 CHECK ((status = 'revoked') = (revoked_at IS NOT NULL)),
 CHECK (expires_at > issued_at),
 CHECK (expires_at <= issued_at + interval '30 days')
);
-- Explicit (resource, permission) pairs; admin_policy and review_publish are refused at issuance.
CREATE TABLE genui.code_grant_templates (
 organization_id uuid NOT NULL, code_id uuid NOT NULL,
 permission text NOT NULL CHECK (permission IN ('discover','run','edit_definition','record_read','record_write','share_work','export','source_read')),
 app_id uuid, work_id uuid, source_id uuid,
 UNIQUE NULLS NOT DISTINCT (organization_id,code_id,permission,app_id,work_id,source_id),
 FOREIGN KEY (organization_id,code_id) REFERENCES genui.access_codes,
 FOREIGN KEY (organization_id,app_id) REFERENCES genui.apps,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY (organization_id,source_id) REFERENCES genui.sources,
 CHECK ((permission IN ('discover','run','edit_definition') AND app_id IS NOT NULL AND work_id IS NULL AND source_id IS NULL)
  OR (permission IN ('record_read','record_write','share_work','export') AND app_id IS NOT NULL AND work_id IS NOT NULL AND source_id IS NULL)
  OR (permission = 'source_read' AND app_id IS NULL AND work_id IS NULL AND source_id IS NOT NULL))
);
-- D7.1/D7.17: presignup reservations (10 minutes, never counted, at most 10
-- live per code) and pending admissions (24 hours, bound to the verified
-- identity, counted against remaining redemptions).
CREATE TABLE genui.code_reservations (
 organization_id uuid NOT NULL, reservation_id uuid NOT NULL, code_id uuid NOT NULL,
 kind text NOT NULL CHECK (kind IN ('presignup','pending')),
 context_digest genui.digest NOT NULL,
 issuer text, provider_tenant_id text,
 provider_subject text CHECK (length(provider_subject) BETWEEN 1 AND 256),
 state text NOT NULL CHECK (state IN ('live','consumed','expired','denied_for_reconciliation')),
 expires_at timestamptz NOT NULL, consumed_at timestamptz,
 PRIMARY KEY (organization_id,reservation_id),
 FOREIGN KEY (organization_id,code_id) REFERENCES genui.access_codes,
 FOREIGN KEY (organization_id,issuer,provider_tenant_id) REFERENCES genui.provider_tenant_bindings,
 CHECK ((kind = 'presignup' AND issuer IS NULL AND provider_tenant_id IS NULL AND provider_subject IS NULL
   AND state IN ('live','consumed','expired'))
  OR (kind = 'pending' AND issuer IS NOT NULL AND provider_tenant_id IS NOT NULL AND provider_subject IS NOT NULL
   AND state IN ('live','consumed','denied_for_reconciliation'))),
 CHECK ((state = 'consumed') = (consumed_at IS NOT NULL))
);
CREATE TABLE genui.entitlements (
 organization_id uuid NOT NULL, principal_id uuid NOT NULL, entitlement_id uuid NOT NULL,
 status text NOT NULL CHECK (status IN ('active','expired','revoked')),
 kind text NOT NULL CHECK (kind IN ('trial','member_invite')),
 starts_at timestamptz NOT NULL, expires_at timestamptz NOT NULL, revoked_at timestamptz,
 PRIMARY KEY (organization_id,principal_id,entitlement_id),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 CHECK (expires_at > starts_at),
 CHECK ((status = 'revoked') = (revoked_at IS NOT NULL))
);
-- D7.8: one insert-only chain per organization and writer role; every hash is
-- typed genui.digest, so previous_hash || canonical bytes has a fixed-length prefix.
CREATE TABLE genui.audit_heads (
 organization_id uuid NOT NULL REFERENCES genui.organizations,
 writer_role text NOT NULL CHECK (writer_role IN ('workspace','control','data','admission','operator')),
 chain_sequence genui.safe_counter NOT NULL, head_hash genui.digest NOT NULL,
 PRIMARY KEY (organization_id,writer_role)
);
CREATE TABLE genui.audit_events (
 organization_id uuid NOT NULL REFERENCES genui.organizations, event_id uuid NOT NULL,
 writer_role text NOT NULL CHECK (writer_role IN ('workspace','control','data','admission','operator')),
 chain_sequence genui.safe_counter NOT NULL, kind text NOT NULL,
 actor_principal_id uuid, app_id uuid, work_id uuid, record_id uuid, code_id uuid,
 outcome_digest genui.digest, occurred_at timestamptz NOT NULL,
 previous_hash genui.digest NOT NULL, hash genui.digest NOT NULL,
 PRIMARY KEY (organization_id,event_id),
 UNIQUE (organization_id,writer_role,chain_sequence),
 UNIQUE (organization_id,writer_role,previous_hash)
);
CREATE TABLE genui.code_redemptions (
 organization_id uuid NOT NULL, redemption_id uuid NOT NULL, code_id uuid NOT NULL, principal_id uuid NOT NULL,
 pending_admission_id uuid NOT NULL, entitlement_id uuid NOT NULL,
 payload_digest genui.digest NOT NULL, committed_at timestamptz NOT NULL, audit_event_id uuid NOT NULL,
 PRIMARY KEY (organization_id,redemption_id), UNIQUE (organization_id,pending_admission_id),
 FOREIGN KEY (organization_id,code_id) REFERENCES genui.access_codes,
 FOREIGN KEY (organization_id,pending_admission_id) REFERENCES genui.code_reservations,
 FOREIGN KEY (organization_id,principal_id,entitlement_id) REFERENCES genui.entitlements
 -- audit_event_id carries no foreign key: a referential check row-locks the
 -- referenced row, which needs UPDATE on audit_events, and no role holds it (D7.8).
);
-- APP-073: durable provider-invitation operation state; operator-only.
CREATE TABLE genui.provider_invitations (
 organization_id uuid NOT NULL, invitation_id uuid NOT NULL,
 issuer text NOT NULL, provider_tenant_id text NOT NULL,
 remote_invitation_id text CHECK (length(remote_invitation_id) BETWEEN 1 AND 256),
 intended_email_digest genui.digest NOT NULL,
 provider_role text NOT NULL CHECK (length(provider_role) BETWEEN 1 AND 64),
 delivery_mode text NOT NULL CHECK (delivery_mode = 'server_only'),
 operation text NOT NULL CHECK (operation IN ('create','revoke')),
 state text NOT NULL CHECK (state IN ('pending','confirmed','failed','unknown')),
 code_id uuid NOT NULL, created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,invitation_id),
 FOREIGN KEY (organization_id,issuer,provider_tenant_id) REFERENCES genui.provider_tenant_bindings,
 FOREIGN KEY (organization_id,code_id) REFERENCES genui.access_codes,
 CHECK (updated_at >= created_at)
);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here;
-- 0002's loop covered only 0002's tables.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['access_codes','code_grant_templates','code_reservations','entitlements',
   'audit_heads','audit_events','code_redemptions','provider_invitations'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;
-- D7.8 role binding: every role holding INSERT on audit_events (genui_admit by
-- column grant, genui_owner by ownership for the operator channel) is bound to
-- its chain and closed kind list; its head UPDATE is bound to its own chain.
-- workspace/control/data policies arrive with their INSERT grants (D7.3).
CREATE POLICY admission_chain ON genui.audit_events AS RESTRICTIVE FOR INSERT TO genui_admit
 WITH CHECK (writer_role = 'admission' AND kind = ANY (ARRAY['admission.pending','admission.redeemed',
  'admission.denied_for_reconciliation','session.sign_out','session.sign_out_all']));
CREATE POLICY operator_chain ON genui.audit_events AS RESTRICTIVE FOR INSERT TO genui_owner
 WITH CHECK (writer_role = 'operator' AND kind = ANY (ARRAY['organization.provisioned','handle.rotated',
  'code.issued','code.revoked','code.extended','membership.revoked','entitlement.revoked']));
CREATE POLICY admission_chain ON genui.audit_heads AS RESTRICTIVE FOR UPDATE TO genui_admit
 USING (writer_role = 'admission');
CREATE POLICY operator_chain ON genui.audit_heads AS RESTRICTIVE FOR UPDATE TO genui_owner
 USING (writer_role = 'operator');
-- D7.8: audit_events is insert-only for every role, its owner included.
REVOKE UPDATE,DELETE,TRUNCATE ON genui.audit_events FROM genui_owner;

-- D7.1 grants of genui_admit (A4). organizations.status is a lock-enabling
-- column only: no genui_admit function body updates genui.organizations.
REVOKE ALL ON genui_private.code_index,genui_private.handle_index FROM PUBLIC;
GRANT USAGE ON SCHEMA genui,genui_private TO genui_admit;
GRANT SELECT(organization_id,status),UPDATE(status) ON genui.organizations TO genui_admit;
GRANT SELECT(code_digest,organization_id) ON genui_private.code_index TO genui_admit;
GRANT SELECT(handle_digest,organization_id,retired_at) ON genui_private.handle_index TO genui_admit;
GRANT SELECT(organization_id,code_id,code_digest,kind,status,max_redemptions,redeemed_count,expires_at),
 UPDATE(redeemed_count,status) ON genui.access_codes TO genui_admit;
GRANT SELECT(organization_id,code_id,permission,app_id,work_id,source_id) ON genui.code_grant_templates TO genui_admit;
GRANT SELECT(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at),
 INSERT(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at),
 UPDATE(state,consumed_at) ON genui.code_reservations TO genui_admit;
GRANT SELECT(organization_id,issuer,provider_tenant_id,provider_subject),
 INSERT(organization_id,principal_id,issuer,provider_tenant_id,provider_subject) ON genui.principals TO genui_admit;
GRANT SELECT(organization_id,principal_id,membership_epoch),
 INSERT(organization_id,principal_id,status,membership_epoch),UPDATE(membership_epoch) ON genui.memberships TO genui_admit;
GRANT INSERT(organization_id,principal_id,permission) ON genui.organization_permissions TO genui_admit;
GRANT INSERT(organization_id,principal_id,app_id,permission) ON genui.app_permissions TO genui_admit;
GRANT INSERT(organization_id,principal_id,app_id,work_id,permission) ON genui.work_permissions TO genui_admit;
GRANT INSERT(organization_id,principal_id,source_id,permission) ON genui.source_permissions TO genui_admit;
GRANT INSERT(organization_id,principal_id,entitlement_id,status,kind,starts_at,expires_at) ON genui.entitlements TO genui_admit;
GRANT SELECT(organization_id,redemption_id,principal_id,pending_admission_id,entitlement_id,payload_digest),
 INSERT(organization_id,redemption_id,code_id,principal_id,pending_admission_id,entitlement_id,payload_digest,committed_at,audit_event_id)
 ON genui.code_redemptions TO genui_admit;
GRANT INSERT(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,
 outcome_digest,occurred_at,previous_hash,hash) ON genui.audit_events TO genui_admit;
GRANT SELECT(organization_id,writer_role,chain_sequence,head_hash),UPDATE(chain_sequence,head_hash) ON genui.audit_heads TO genui_admit;

-- D7.1: six SECURITY DEFINER STRICT VOLATILE functions owned by genui_admit.
-- Each returns a typed outcome and never raises for a business refusal; a
-- transaction scope that does not match p_org (or an unknown or inactive
-- organization, or an unknown code under scope) returns no row, as the 0002
-- fences do. A missing audit head is a configuration error and raises 55000.
CREATE FUNCTION genui_private.resolve_code(p_code_digest text) RETURNS uuid
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE v_org uuid;
BEGIN
 SELECT i.organization_id INTO v_org FROM genui_private.code_index i WHERE i.code_digest=p_code_digest;
 RETURN v_org;
END $$;
CREATE FUNCTION genui_private.resolve_handle(p_handle_digest text) RETURNS uuid
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE v_org uuid;
BEGIN
 SELECT h.organization_id INTO v_org FROM genui_private.handle_index h
  WHERE h.handle_digest=p_handle_digest AND (h.retired_at IS NULL OR h.retired_at>now());
 RETURN v_org;
END $$;
CREATE FUNCTION genui_private.reserve_code(p_org uuid,p_code_digest text,p_context_digest text)
 RETURNS TABLE(reservation_id uuid,expires_at timestamptz,outcome text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; v_code record; v_live bigint; v_id uuid; v_expires timestamptz;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR p_context_digest !~ '^sha256:[0-9a-f]{64}$' THEN RETURN; END IF;
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
 INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,state,expires_at)
  VALUES (p_org,v_id,v_code.code_id,'presignup',p_context_digest,'live',v_expires);
 RETURN QUERY SELECT v_id,v_expires,'reserved'::text;
END $$;
CREATE FUNCTION genui_private.record_pending_admission(p_org uuid,p_reservation uuid,p_issuer text,p_provider_tenant text,p_provider_subject text)
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
 SELECT r.state,r.expires_at,r.context_digest INTO v_res FROM genui.code_reservations r
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
 INSERT INTO genui.code_reservations(organization_id,reservation_id,code_id,kind,context_digest,issuer,provider_tenant_id,provider_subject,state,expires_at)
  VALUES (p_org,v_id,v_code_id,'pending',v_res.context_digest,p_issuer,p_provider_tenant,p_provider_subject,'live',v_expires);
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
CREATE FUNCTION genui_private.admit_redemption(p_org uuid,p_pending_admission uuid,p_issuer text,p_provider_tenant text,p_provider_subject text,p_payload_digest text)
 RETURNS TABLE(principal_id uuid,membership_epoch bigint,entitlement_id uuid,redemption_id uuid,outcome text)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
DECLARE o_status text; v_code_id uuid; v_code record; v_pend record; v_red record; v_others bigint;
 v_principal uuid; v_epoch bigint; v_entitlement uuid; v_redemption uuid; v_outcome text;
 v_kind text; v_actor uuid; v_digest text; v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
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
 SELECT r.state,r.expires_at,r.issuer,r.provider_tenant_id,r.provider_subject INTO v_pend FROM genui.code_reservations r
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
 END IF;
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 IF v_outcome='admitted' THEN
  RETURN QUERY SELECT v_principal,v_epoch,v_entitlement,v_redemption,'admitted'::text;
 ELSE
  RETURN QUERY SELECT NULL::uuid,NULL::bigint,NULL::uuid,NULL::uuid,v_outcome;
 END IF;
END $$;
CREATE FUNCTION genui_private.advance_member_epoch(p_org uuid,p_principal uuid,p_reason text) RETURNS bigint
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; v_epoch bigint; v_code_id uuid; v_kind text; v_actor uuid; v_digest text;
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text;
BEGIN
 -- Same context rule as lock_member_authority (0002:247-249): a member can
 -- advance only its own epoch, and only for the two sign-out reasons.
 IF p_reason NOT IN ('session.sign_out','session.sign_out_all')
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN NULL; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND THEN RETURN NULL; END IF;
 SELECT m.membership_epoch INTO v_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND OR v_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN NULL; END IF;
 UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id=p_org AND principal_id=p_principal;
 v_kind:=p_reason; v_actor:=p_principal;
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
 RETURN v_epoch+1;
END $$;
REVOKE ALL ON FUNCTION genui_private.resolve_code(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.resolve_handle(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.reserve_code(uuid,text,text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.record_pending_admission(uuid,uuid,text,text,text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.admit_redemption(uuid,uuid,text,text,text,text) FROM PUBLIC;
REVOKE ALL ON FUNCTION genui_private.advance_member_epoch(uuid,uuid,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.resolve_code(text),genui_private.resolve_handle(text),
 genui_private.reserve_code(uuid,text,text),genui_private.record_pending_admission(uuid,uuid,text,text,text),
 genui_private.admit_redemption(uuid,uuid,text,text,text,text),genui_private.advance_member_epoch(uuid,uuid,text)
 TO genui_workspace;
-- Temporary CREATE for the ownership transfer only; revoked below (A3, as 0002 did for genui_lock).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.resolve_code(text) OWNER TO genui_admit;
ALTER FUNCTION genui_private.resolve_handle(text) OWNER TO genui_admit;
ALTER FUNCTION genui_private.reserve_code(uuid,text,text) OWNER TO genui_admit;
ALTER FUNCTION genui_private.record_pending_admission(uuid,uuid,text,text,text) OWNER TO genui_admit;
ALTER FUNCTION genui_private.admit_redemption(uuid,uuid,text,text,text,text) OWNER TO genui_admit;
ALTER FUNCTION genui_private.advance_member_epoch(uuid,uuid,text) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
