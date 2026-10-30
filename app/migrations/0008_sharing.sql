-- ADR-011 (APP-095, pitch sub-branch pivot/realtime): a member shares a saved
-- work with a teammate of the SAME organization. One definer function,
-- genui_private.share_work, owned by genui_admit (ADR-007 D7.1's authority
-- writer), COPIES onto the grantee exactly the subset of the sharer's own
-- current permissions on that work: `discover` and `run` on the app that the
-- sharer holds, `record_read` on the work (the sharer holds it as the
-- unrevoked creator or by a work grant), `record_write` only when the sharer
-- asks for it and holds it, and `source_read` on each of the work's source
-- dependencies the sharer holds. The teammate therefore opens the work under
-- the sharer's permissions, copied (ADR-011 amendment 2026-10-30; APP-095
-- review M3), not under grants of its own. `source_read` is copied because
-- the work's read clause requires it on every dependency; by the 0002 model
-- it is keyed (organization, principal, source), so the copied row is
-- organization-wide (the grantee may afterwards read that source through any
-- app it can `run`) and is not removed when the share is reversed. Nothing
-- else is ever written: no `edit_definition`, `review_publish`, `share_work`,
-- `export` or `admin_policy`, and no row for a principal outside the
-- transaction's organization scope (RLS WITH CHECK). The sharer must be the
-- work's unrevoked creator or hold `share_work` on it, and must hold `run` on
-- the app. The function takes no permission names from its caller and exposes
-- no bypass. When at least one row was inserted it writes one
-- genui.work_shares row naming exactly the inserted set and appends exactly
-- one `permission.granted` event on the admission chain (actor the sharer, app
-- and work named, outcome_digest the digest of the grantee and that inserted
-- set, event_id referenced by the work_shares row), so every share is
-- reconstructable and reversible row by row; a repeat share inserts nothing
-- and writes neither. This migration has been applied only to disposable
-- databases and is amended in place (review M3).
--
-- Recorded debt (ADR-011 §Consequences, APP-095, APP-097): ADR-007 D7.3 says
-- permission rows are written only by admit_redemption and the operator
-- channel, D7.8 lists `permission.granted` on the operator chain only, and
-- D7.1 names six definer functions. This migration adds a seventh definer
-- that writes permission rows and lets the admission chain carry
-- `permission.granted`; the November 3 revisit decides whether ADR-007 is
-- amended (seventh function, the kind on two chains, the A4 SELECT grants
-- below) or the path is reworked. Executed only by the checksummed,
-- transactional migration runner (D7.12: numbered in landing order).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 8);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 8);

-- The record of each share that wrote a row: who shared which work with whom,
-- exactly the permission rows the share inserted (`scope:resource:permission`,
-- the set the audit event's outcome_digest covers) and the event that recorded
-- it. Written only by share_work; no application role reads it (an unshare
-- route is recorded debt, APP-097). No foreign key to audit_events (0007's
-- decision: the audit log is insert-only and a referencing row would need a
-- KEY SHARE lock on it); the operator `verify` pairs them as a set.
CREATE TABLE genui.work_shares (
 organization_id uuid NOT NULL, share_id uuid NOT NULL,
 sharer_id uuid NOT NULL, grantee_id uuid NOT NULL, app_id uuid NOT NULL, work_id uuid NOT NULL,
 granted text[] NOT NULL CHECK (cardinality(granted) BETWEEN 1 AND 64),
 audit_event_id uuid NOT NULL, shared_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,share_id),
 UNIQUE (organization_id,audit_event_id),
 FOREIGN KEY (organization_id,sharer_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,grantee_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,app_id,work_id) REFERENCES genui.saved_work,
 CHECK (sharer_id<>grantee_id)
);
-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
ALTER TABLE genui.work_shares ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.work_shares FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.work_shares USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid) WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);
GRANT INSERT(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at) ON genui.work_shares TO genui_admit;

-- D7.1 A4 extension: the reads share_work needs, by column. INSERT on the
-- three permission tables is 0003's grant; no UPDATE or DELETE is added.
GRANT SELECT(status) ON genui.memberships TO genui_admit;
GRANT SELECT(organization_id,app_id,work_id,created_by,creator_revoked_at) ON genui.saved_work TO genui_admit;
GRANT SELECT(organization_id,app_id,work_id,source_id) ON genui.work_source_dependencies TO genui_admit;
GRANT SELECT(organization_id,principal_id,app_id,permission) ON genui.app_permissions TO genui_admit;
GRANT SELECT(organization_id,principal_id,app_id,work_id,permission) ON genui.work_permissions TO genui_admit;
GRANT SELECT(organization_id,principal_id,source_id,permission) ON genui.source_permissions TO genui_admit;

-- D7.8 role binding, widened in place: the admission chain also carries
-- `permission.granted` (the existing operator kind, written here by a member
-- through share_work). Every other kind list is unchanged.
ALTER POLICY admission_chain ON genui.audit_events
 WITH CHECK (writer_role = 'admission' AND kind = ANY (ARRAY['admission.pending','admission.redeemed',
  'admission.denied_for_reconciliation','session.sign_out','session.sign_out_all','permission.granted']));

-- The same context rule as lock_member_authority and advance_member_epoch:
-- the transaction's scope must be the sharer's own organization, principal
-- and current epoch. A refusal returns no row (as the 0002 fences do); a
-- missing audit head raises 55000.
CREATE FUNCTION genui_private.share_work(p_org uuid,p_sharer uuid,p_grantee uuid,p_app uuid,p_work uuid,p_record_write boolean)
 RETURNS TABLE(scope text,resource_id uuid,permission text,inserted boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; s_status text; s_epoch bigint; g_status text; v_creator boolean; v_share boolean;
 v_count integer; v_any boolean:=false; v_inserted text[]:='{}'; r record;
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text; v_digest text;
BEGIN
 IF p_sharer=p_grantee
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_sharer::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO s_status,s_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_sharer FOR SHARE;
 IF NOT FOUND OR s_status<>'active' OR s_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 -- The grantee: an active member of this organization (RLS scopes the read),
 -- locked FOR SHARE too (review m3) so that an operator revocation of the
 -- grantee (member FOR UPDATE) cannot interleave between this read and the
 -- inserts below. Lock order unchanged: organizations, then member rows.
 SELECT m.status INTO g_status FROM genui.memberships m WHERE m.organization_id=p_org AND m.principal_id=p_grantee FOR SHARE;
 IF NOT FOUND OR g_status<>'active' THEN RETURN; END IF;
 -- The sharer's authority over the work: its unrevoked creator (ADR-007
 -- amendment 2026-10-29) or a `share_work` grant; and `run` on the app.
 SELECT (w.created_by=p_sharer AND w.creator_revoked_at IS NULL) INTO v_creator FROM genui.saved_work w
  WHERE w.organization_id=p_org AND w.app_id=p_app AND w.work_id=p_work;
 IF NOT FOUND THEN RETURN; END IF;
 v_creator:=coalesce(v_creator,false);
 v_share:=EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='share_work');
 IF NOT (v_creator OR v_share) THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission='run') THEN RETURN; END IF;
 IF NOT (v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_read')) THEN RETURN; END IF;
 IF p_record_write AND NOT (v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org
  AND p.principal_id=p_sharer AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_write')) THEN RETURN; END IF;
 -- The app permissions the sharer holds among discover and run, in order.
 FOR r IN SELECT a.permission FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission IN ('discover','run') ORDER BY a.permission LOOP
  INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission) VALUES (p_org,p_grantee,p_app,r.permission)
   ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS v_count=ROW_COUNT;
  scope:='app'; resource_id:=p_app; permission:=r.permission; inserted:=v_count=1; v_any:=v_any OR inserted;
  IF inserted THEN v_inserted:=v_inserted||('app:'||p_app::text||':'||r.permission); END IF; RETURN NEXT;
 END LOOP;
 -- record_read always (the sharer holds it, judged above); record_write on request.
 FOR r IN SELECT x.permission FROM (VALUES ('record_read'),('record_write')) AS x(permission)
  WHERE x.permission='record_read' OR p_record_write ORDER BY x.permission LOOP
  INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission) VALUES (p_org,p_grantee,p_app,p_work,r.permission)
   ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS v_count=ROW_COUNT;
  scope:='work'; resource_id:=p_work; permission:=r.permission; inserted:=v_count=1; v_any:=v_any OR inserted;
  IF inserted THEN v_inserted:=v_inserted||('work:'||p_work::text||':'||r.permission); END IF; RETURN NEXT;
 END LOOP;
 -- source_read on each source dependency of the work that the sharer holds.
 FOR r IN SELECT DISTINCT d.source_id FROM genui.work_source_dependencies d
  JOIN genui.source_permissions s ON s.organization_id=d.organization_id AND s.source_id=d.source_id
   AND s.principal_id=p_sharer AND s.permission='source_read'
  WHERE d.organization_id=p_org AND d.app_id=p_app AND d.work_id=p_work ORDER BY d.source_id LOOP
  INSERT INTO genui.source_permissions(organization_id,principal_id,source_id,permission) VALUES (p_org,p_grantee,r.source_id,'source_read')
   ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS v_count=ROW_COUNT;
  scope:='source'; resource_id:=r.source_id; permission:='source_read'; inserted:=v_count=1; v_any:=v_any OR inserted;
  IF inserted THEN v_inserted:=v_inserted||('source:'||r.source_id::text||':source_read'); END IF; RETURN NEXT;
 END LOOP;
 IF NOT v_any THEN RETURN; END IF;
 -- D7.8 append on the admission chain; the head row lock serializes appenders.
 -- The digest covers the grantee and exactly the inserted set, which the
 -- work_shares row stores, so it is recomputable from that row.
 v_digest:='sha256:'||encode(sha256(convert_to('forma:share-grant:v1','UTF8')||'\x00'::bytea||convert_to(p_grantee::text,'UTF8')
  ||'\x00'::bytea||convert_to(array_to_string(v_inserted,','),'UTF8')),'hex');
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||to_json(p_sharer::text)::text||',"app_id":'||to_json(p_app::text)::text||',"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":"permission.granted","occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":'||to_json(p_work::text)::text||',"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,'permission.granted',p_sharer,p_app,p_work,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 -- The share's record: exactly what this call inserted, referencing the event.
 INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at)
  VALUES (p_org,gen_random_uuid(),p_sharer,p_grantee,p_app,p_work,v_inserted,v_event,v_at);
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,boolean) TO genui_workspace;
-- Temporary CREATE for the ownership transfer only; revoked below (as 0003 did).
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,boolean) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
