-- Trial plan (ADR-008 amendment 2026-11-14), decisions D7, D8 and D9
-- (APP-191, APP-192): private entries, reviewers and sharing with roles or
-- with the whole organization. Additive. No stored value is rewritten.
--
-- D7, own collections. A record schema declared `visibility: "own"` (contract
-- edition impl-2026-11-14.1) makes a collection whose entries each member sees
-- and changes only when they created them; the work's reviewers see and
-- change all of them. `definition_record_schemas.owner_only` is a stored
-- generated column derived from the canonical schema bytes the admission
-- wrote, so no writer changes and nothing can drift; the one storage helper
-- (`authorization::own_rows`) reads it through the work's pinned publication
-- (record_collections -> publication_record_schemas -> definition_record_schemas).
--
-- Reviewers. A new work permission `record_review`. A reviewer of a work is
-- the app's owner, the work's unrevoked creator (Victor, 2026-11-14), a
-- `record_review` holder, or every active member while an organization-wide
-- reviewer share is live (below). Only reviewers run a `reviewer_only`
-- command (D8, judged by storage's `compose`).
--
-- D9, roles and the organization-wide share. `genui_private.share_work` is
-- replaced in place: its boolean becomes a role (viewer: record_read;
-- contributor: also record_write; reviewer: also record_review), each
-- permission copied only when the sharer holds it PERSONALLY (as the app's
-- owner, the work's unrevoked creator or by a work grant; standing that
-- comes only from a live organization-wide share is never copied into a
-- durable personal grant), and the reviewer role only when the sharer is a
-- reviewer (the owner always is; product advisor, 2026-11-14). Share
-- authority is unchanged (ADR-011: the unrevoked creator or a `share_work`
-- holder, with `run` on the app; `share_work` itself is never copied), so a
-- reviewer who is neither cannot share or grant anything. A share with a
-- lower role than the grantee holds takes the surplus away in the same
-- transaction (reviewer to contributor drops record_review; to viewer also
-- record_write), audited as `permission.revoked`, by a sharer who could have
-- granted those rows (Tier A follow-up f1).
-- `genui_private.revoke_work_share` is the owner's revocation of a member's
-- personal share (Tier A review M1): the app's owner removes every
-- record_read/record_write/record_review row the grantee holds on the work in
-- one unit, marks the grantee's share records revoked and appends
-- `permission.revoked`; a removed grant leaves no row behind, so no later
-- share can revive it. It never touches the organization-wide share, the
-- app-level discover/run rows (they may serve other works of the app) or the
-- owner's own standing; the operator's channel stays as it is.
-- `genui_private.set_work_everyone` writes the organization-wide share: one
-- live `work_shares` row with no grantee and a role, which every work-level
-- clause honors for every active member (a member admitted later is covered)
-- and which one call revokes as a unit (`'none'`) or replaces (a new role
-- revokes and re-inserts). Concurrent calls for one work serialize on a
-- transaction advisory lock, so each answers a result and the partial unique
-- index below is never hit. The sharer must hold `discover` and `run` on the
-- app and `record_read` on the work personally, so the role's set is only
-- ever what the sharer holds; a live reviewer share is replaced or revoked
-- only by a reviewer. It is refused for a work with source dependencies
-- (`source_read` is a personal, organization-wide copy); storage also refuses
-- a work built from folder files (personal `box_file_grants`). Every grant
-- appends `permission.granted` and every revocation `permission.revoked` on
-- the admission chain, each digest recomputable from the `work_shares` row.
--
-- Recorded exception to ADR-009's additive rule (W3's purge review R1,
-- 2026-11-14): the foreign key from genui_identity.share_events
-- (organization_id, share_id) to genui.live_work_shares (0017) is dropped.
-- The end-of-test purge deletes an organization's live_work_shares while each
-- guest account's share_events hash chain must stay verifiable; those rows
-- hold only {account_id, organization_id, share_id, kind, share_version,
-- sequence, previous_hash} and their hash, no content, and with the key in
-- place a guest-accepted share could never be deleted. linked_apps keeps its
-- key: a linked app is removed before its share goes. Nothing else is
-- dropped and no row is rewritten.
--
-- Recorded debt, as 0008 and 0023 did: ADR-007 D7.1 names six definers and
-- D7.3 says permission rows are written only by admit_redemption and the
-- operator channel; this migration replaces one genui_admit definer and adds
-- two that write (and, for the owner's revocation, delete) permission rows
-- (the ADR-007 amendment of 2026-11-14).
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 24);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 24);

-- (0) The purge's exception (above): share_events keeps only its account key.
ALTER TABLE genui_identity.share_events DROP CONSTRAINT share_events_organization_id_share_id_fkey;

-- (a) Own collections ----------------------------------------------------------
-- Derived from the canonical schema bytes (forma-json-v1 UTF-8 text) through a
-- real JSON parse. `convert_from` is STABLE in the catalog because it reads
-- the server encoding; this database is UTF-8 and the bytes are canonical
-- UTF-8 JSON, so the wrapper is declared IMMUTABLE (a generated column needs
-- it) and refuses nothing a canonical writer produces: invalid UTF-8 or
-- non-JSON bytes raise and the insert fails closed; an escaped spelling is
-- decoded, so `"\u006fwn"` reads as own (admission never stores one: the
-- canonical encoder writes identifiers and enum words literally). Executable
-- by the writers of definition_record_schemas (genui_workspace) and the owner;
-- PUBLIC is revoked. A schema that is not `own` reads false; nothing is
-- rewritten.
CREATE FUNCTION genui.canonical_jsonb(p_canonical bytea) RETURNS jsonb
 LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
 SET search_path=pg_catalog,pg_temp
 RETURN pg_catalog.convert_from(p_canonical,'UTF8')::jsonb;
REVOKE ALL ON FUNCTION genui.canonical_jsonb(bytea) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui.canonical_jsonb(bytea) TO genui_workspace;
ALTER TABLE genui.definition_record_schemas ADD COLUMN owner_only boolean
 GENERATED ALWAYS AS ((genui.canonical_jsonb(canonical_schema)->>'visibility') IS NOT DISTINCT FROM 'own') STORED;

-- (b) The reviewer permission ----------------------------------------------------
ALTER TABLE genui.work_permissions DROP CONSTRAINT work_permissions_permission_check,
 ADD CONSTRAINT work_permissions_permission_check
 CHECK (permission IN ('record_read','record_write','share_work','export','record_review'));

-- (c) Shares with a role, and with everyone ------------------------------------
-- A NULL grantee means every active member of the organization; such a row
-- carries a role and may be revoked (revoked_at, by whom, which event).
ALTER TABLE genui.work_shares ALTER COLUMN grantee_id DROP NOT NULL;
ALTER TABLE genui.work_shares
 ADD COLUMN role text CHECK (role IN ('viewer','contributor','reviewer')),
 ADD COLUMN revoked_at timestamptz,
 ADD COLUMN revoked_by uuid,
 ADD COLUMN revoke_event_id uuid,
 ADD CONSTRAINT work_shares_grantee_or_role CHECK (grantee_id IS NOT NULL OR role IS NOT NULL),
 ADD CONSTRAINT work_shares_revocation_whole CHECK ((revoked_at IS NULL)=(revoked_by IS NULL) AND (revoked_at IS NULL)=(revoke_event_id IS NULL)),
 ADD CONSTRAINT work_shares_organization_id_revoked_by_fkey FOREIGN KEY (organization_id,revoked_by) REFERENCES genui.memberships;
-- At most one live organization-wide share per work.
CREATE UNIQUE INDEX work_shares_everyone_live ON genui.work_shares (organization_id,app_id,work_id)
 WHERE grantee_id IS NULL AND revoked_at IS NULL;

-- The definers' reads and writes on work_shares (0008 granted the INSERT
-- columns of a personal share), and the application roles' read of the live
-- organization-wide rows (every work-level clause consults them).
GRANT INSERT(role) ON genui.work_shares TO genui_admit;
GRANT UPDATE(revoked_at,revoked_by,revoke_event_id) ON genui.work_shares TO genui_admit;
GRANT SELECT(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,role,revoked_at,granted) ON genui.work_shares TO genui_admit;
GRANT SELECT(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,role,revoked_at,shared_at) ON genui.work_shares
 TO genui_workspace,genui_control,genui_data;
-- The application roles see the organization-wide rows only (RESTRICTIVE);
-- personal share records stay the definers' (Tier A review n2).
CREATE POLICY work_shares_everyone ON genui.work_shares AS RESTRICTIVE FOR SELECT TO genui_workspace,genui_control,genui_data
 USING (grantee_id IS NULL);
-- The owner's revocation deletes a grantee's work rows and marks the share
-- records (Tier A review M1); the operator channel is unchanged.
GRANT DELETE ON genui.work_permissions TO genui_admit;

-- D7.8 role binding, restated whole: 0023's kinds plus `permission.revoked`.
ALTER POLICY admission_chain ON genui.audit_events
 WITH CHECK (writer_role = 'admission' AND kind = ANY (ARRAY['admission.pending','admission.redeemed',
  'admission.denied_for_reconciliation','session.sign_out','session.sign_out_all','permission.granted',
  'admission.consented','member.named','membership.revoked','entitlement.revoked','permission.revoked']));

-- The definers, as their owner (genui_migration may SET ROLE genui_admit,
-- never inherit it); CREATE on genui_private for this block only.
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
SET LOCAL ROLE genui_admit;

-- 0008's share_work with a role instead of a boolean. The old form is
-- dropped: every share names its role.
DROP FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,boolean);
CREATE FUNCTION genui_private.share_work(p_org uuid,p_sharer uuid,p_grantee uuid,p_app uuid,p_work uuid,p_role text)
 RETURNS TABLE(scope text,resource_id uuid,permission text,inserted boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; s_status text; s_epoch bigint; g_status text; v_creator boolean; v_share boolean;
 v_owner boolean; v_read boolean; v_write boolean; v_review boolean; v_count integer; v_any boolean:=false; v_inserted text[]:='{}'; r record;
 v_demote text[]:='{}'; v_removed text[]:='{}';
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text; v_digest text;
BEGIN
 IF p_sharer=p_grantee OR p_role NOT IN ('viewer','contributor','reviewer')
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_sharer::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO s_status,s_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_sharer FOR SHARE;
 IF NOT FOUND OR s_status<>'active' OR s_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 SELECT m.status INTO g_status FROM genui.memberships m WHERE m.organization_id=p_org AND m.principal_id=p_grantee FOR SHARE;
 IF NOT FOUND OR g_status<>'active' THEN RETURN; END IF;
 -- Share authority (ADR-011, unchanged): the unrevoked creator or a
 -- `share_work` holder, with `run` on the app.
 SELECT (w.created_by=p_sharer AND w.creator_revoked_at IS NULL) INTO v_creator FROM genui.saved_work w
  WHERE w.organization_id=p_org AND w.app_id=p_app AND w.work_id=p_work;
 IF NOT FOUND THEN RETURN; END IF;
 v_creator:=coalesce(v_creator,false);
 v_share:=EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='share_work');
 IF NOT (v_creator OR v_share) THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission='run') THEN RETURN; END IF;
 -- The role is copied, never conferred, from the sharer's PERSONAL standing
 -- (the app's owner, the work's unrevoked creator or a work grant; a live
 -- organization-wide share confers nothing durable, Tier A review m2): the
 -- sharer must hold record_read (0008's rule, review m1); a contributor needs
 -- the sharer's record_write; a reviewer needs the sharer to be a reviewer.
 v_owner:=EXISTS (SELECT 1 FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=p_app AND a.owner_principal_id=p_sharer);
 v_read:=v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_read');
 v_write:=v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_write');
 v_review:=v_creator OR v_owner
  OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_review');
 IF NOT v_read THEN RETURN; END IF;
 IF p_role IN ('contributor','reviewer') AND NOT v_write THEN RETURN; END IF;
 IF p_role='reviewer' AND NOT v_review THEN RETURN; END IF;
 -- A lower role takes away what it lacks (Tier A follow-up f1): a reviewer
 -- re-shared as contributor loses record_review, as viewer also record_write,
 -- in this transaction, by a sharer who could have granted those rows (else
 -- the whole call is refused before any write); the grantee's earlier personal
 -- share records are marked revoked and one permission.revoked names the
 -- removed set, appended before the grant below.
 IF p_role<>'reviewer' AND EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_grantee
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_review') THEN
  IF NOT v_review THEN RETURN; END IF;
  v_demote:=v_demote||'record_review'::text;
 END IF;
 IF p_role='viewer' AND EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_grantee
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_write') THEN
  IF NOT v_write THEN RETURN; END IF;
  v_demote:=v_demote||'record_write'::text;
 END IF;
 IF cardinality(v_demote)>0 THEN
  FOREACH permission IN ARRAY v_demote LOOP
   DELETE FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_grantee AND p.app_id=p_app AND p.work_id=p_work AND p.permission=share_work.permission;
   v_removed:=v_removed||('work:'||p_work::text||':'||permission);
   scope:='revoked'; resource_id:=p_work; inserted:=false; RETURN NEXT;
  END LOOP;
  v_digest:='sha256:'||encode(sha256(convert_to('forma:share-revoke:v1','UTF8')||'\x00'::bytea||convert_to(p_grantee::text,'UTF8')
   ||'\x00'::bytea||convert_to(array_to_string(v_removed,','),'UTF8')),'hex');
  SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
   WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
  IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
  v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
  v_body:='{"actor_principal_id":'||to_json(p_sharer::text)::text||',"app_id":'||to_json(p_app::text)::text||',"chain_sequence":'||v_seq::text
   ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
   ||',"kind":"permission.revoked","occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
   ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
   ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":'||to_json(p_work::text)::text||',"writer_role":"admission"}';
  v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
  INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
   VALUES (p_org,v_event,'admission',v_seq,'permission.revoked',p_sharer,p_app,p_work,NULL,NULL,v_digest,v_at,v_prev,v_hash);
  UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
  UPDATE genui.work_shares SET revoked_at=v_at,revoked_by=p_sharer,revoke_event_id=v_event
   WHERE organization_id=p_org AND app_id=p_app AND work_id=p_work AND grantee_id=p_grantee AND revoked_at IS NULL;
 END IF;
 -- Copy discover/run on the app that the sharer holds.
 FOR r IN SELECT a.permission FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission IN ('discover','run') ORDER BY a.permission LOOP
  INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission) VALUES (p_org,p_grantee,p_app,r.permission)
   ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS v_count=ROW_COUNT;
  scope:='app'; resource_id:=p_app; permission:=r.permission; inserted:=v_count=1; v_any:=v_any OR inserted;
  IF inserted THEN v_inserted:=v_inserted||('app:'||p_app::text||':'||r.permission); END IF; RETURN NEXT;
 END LOOP;
 -- record_read always; record_write for a contributor or reviewer;
 -- record_review for a reviewer.
 FOR r IN SELECT x.permission FROM (VALUES ('record_read'),('record_review'),('record_write')) AS x(permission)
  WHERE x.permission='record_read' OR (x.permission='record_write' AND p_role IN ('contributor','reviewer'))
   OR (x.permission='record_review' AND p_role='reviewer') ORDER BY x.permission LOOP
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
 -- D7.8 append on the admission chain (0008's digest: the grantee and
 -- exactly the inserted set, recomputable from the work_shares row).
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
 INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at,role)
  VALUES (p_org,gen_random_uuid(),p_sharer,p_grantee,p_app,p_work,v_inserted,v_event,v_at,p_role);
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,text) FROM PUBLIC;

-- The organization-wide share: p_role 'viewer', 'contributor' or 'reviewer'
-- sets it (replacing a live one of another role: one revocation, one grant,
-- in this transaction); 'none' revokes it. Rows answered: one per permission
-- the role carries (`inserted` false when the same role was already live and
-- nothing was written), or for 'none' one `('work', work, 'none', revoked)`.
-- A refusal answers no row (as 0002's fences). The granted set names the
-- role's permissions, `everyone` standing for the grantee in the digest.
CREATE FUNCTION genui_private.set_work_everyone(p_org uuid,p_sharer uuid,p_app uuid,p_work uuid,p_role text)
 RETURNS TABLE(scope text,resource_id uuid,permission text,inserted boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; s_status text; s_epoch bigint; v_creator boolean; v_share boolean; v_owner boolean; v_read boolean; v_write boolean; v_review boolean;
 v_live_share uuid; v_live_role text; v_granted text[]; r record;
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text; v_digest text; v_kind text;
BEGIN
 IF p_role NOT IN ('viewer','contributor','reviewer','none')
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_sharer::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO s_status,s_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_sharer FOR SHARE;
 IF NOT FOUND OR s_status<>'active' OR s_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 -- Share authority (ADR-011, unchanged) and `run` on the app.
 SELECT (w.created_by=p_sharer AND w.creator_revoked_at IS NULL) INTO v_creator FROM genui.saved_work w
  WHERE w.organization_id=p_org AND w.app_id=p_app AND w.work_id=p_work;
 IF NOT FOUND THEN RETURN; END IF;
 v_creator:=coalesce(v_creator,false);
 v_share:=EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='share_work');
 IF NOT (v_creator OR v_share) THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission='run') THEN RETURN; END IF;
 -- The v1 limit: a work with source dependencies is shared person by person.
 IF p_role<>'none' AND EXISTS (SELECT 1 FROM genui.work_source_dependencies d WHERE d.organization_id=p_org AND d.app_id=p_app AND d.work_id=p_work) THEN RETURN; END IF;
 -- The role is copied from the sharer's PERSONAL standing (as share_work): the
 -- sharer must hold discover on the app and record_read on the work, so the
 -- organization-wide set (discover, run, record_read, ...) is only ever what
 -- the sharer holds (review m1); a contributor share needs record_write, a
 -- reviewer share a reviewer (review m2 excludes the live org share itself).
 v_owner:=EXISTS (SELECT 1 FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=p_app AND a.owner_principal_id=p_sharer);
 IF p_role<>'none' AND NOT EXISTS (SELECT 1 FROM genui.app_permissions a WHERE a.organization_id=p_org AND a.principal_id=p_sharer
  AND a.app_id=p_app AND a.permission='discover') THEN RETURN; END IF;
 v_read:=v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_read');
 v_write:=v_creator OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_write');
 v_review:=v_creator OR v_owner
  OR EXISTS (SELECT 1 FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_sharer
  AND p.app_id=p_app AND p.work_id=p_work AND p.permission='record_review');
 IF p_role<>'none' AND NOT v_read THEN RETURN; END IF;
 IF p_role IN ('contributor','reviewer') AND NOT v_write THEN RETURN; END IF;
 IF p_role='reviewer' AND NOT v_review THEN RETURN; END IF;
 -- One call at a time per work (review m3): concurrent calls queue here and
 -- each sees the other's committed row, so every caller gets a result and the
 -- partial unique index is never the arbiter.
 PERFORM pg_advisory_xact_lock(hashtextextended('forma:work-everyone:'||p_org::text||':'||p_work::text,0));
 -- The live organization-wide share, locked (the partial unique index keeps
 -- a concurrent second insert out).
 SELECT s.share_id,s.role INTO v_live_share,v_live_role FROM genui.work_shares s
  WHERE s.organization_id=p_org AND s.app_id=p_app AND s.work_id=p_work AND s.grantee_id IS NULL AND s.revoked_at IS NULL FOR UPDATE;
 -- A live reviewer share is the owner's (or a reviewer's) to replace or
 -- revoke (review n7).
 IF v_live_role='reviewer' AND v_live_role IS DISTINCT FROM p_role AND NOT v_review THEN RETURN; END IF;
 IF v_live_role IS NOT NULL AND v_live_role=p_role THEN
  -- Unchanged: the same role is live; nothing is written.
  FOR r IN SELECT x.permission FROM (VALUES ('app','discover'),('app','run'),('work','record_read'),('work','record_write'),('work','record_review')) AS x(kind,permission)
   WHERE x.kind='app' OR x.permission='record_read' OR (x.permission='record_write' AND p_role IN ('contributor','reviewer'))
    OR (x.permission='record_review' AND p_role='reviewer') LOOP
   scope:=CASE WHEN r.permission IN ('discover','run') THEN 'app' ELSE 'work' END;
   resource_id:=CASE WHEN scope='app' THEN p_app ELSE p_work END; permission:=r.permission; inserted:=false; RETURN NEXT;
  END LOOP;
  RETURN;
 END IF;
 IF v_live_share IS NULL AND p_role='none' THEN
  scope:='work'; resource_id:=p_work; permission:='none'; inserted:=false; RETURN NEXT; RETURN;
 END IF;
 -- D7.8: the head row lock serializes appenders; one or two events follow.
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_at:=date_trunc('second',now());
 IF v_live_share IS NOT NULL THEN
  -- Revoke the live share as one unit: `permission.revoked`, its digest over
  -- the share and the role it carried.
  v_kind:='permission.revoked';
  v_digest:='sha256:'||encode(sha256(convert_to('forma:share-revoke:v1','UTF8')||'\x00'::bytea||convert_to(v_live_share::text,'UTF8')
   ||'\x00'::bytea||convert_to(v_live_role,'UTF8')),'hex');
  v_seq:=v_seq+1; v_event:=gen_random_uuid();
  v_body:='{"actor_principal_id":'||to_json(p_sharer::text)::text||',"app_id":'||to_json(p_app::text)::text||',"chain_sequence":'||v_seq::text
   ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
   ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
   ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
   ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":'||to_json(p_work::text)::text||',"writer_role":"admission"}';
  v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
  INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
   VALUES (p_org,v_event,'admission',v_seq,v_kind,p_sharer,p_app,p_work,NULL,NULL,v_digest,v_at,v_prev,v_hash);
  UPDATE genui.work_shares SET revoked_at=v_at,revoked_by=p_sharer,revoke_event_id=v_event
   WHERE organization_id=p_org AND share_id=v_live_share;
  v_prev:=v_hash;
  IF p_role='none' THEN
   UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
   scope:='work'; resource_id:=p_work; permission:='none'; inserted:=true; RETURN NEXT; RETURN;
  END IF;
 END IF;
 -- Grant: the role's permission set, named as a personal share names its rows.
 v_granted:=ARRAY['app:'||p_app::text||':discover','app:'||p_app::text||':run','work:'||p_work::text||':record_read'];
 IF p_role IN ('contributor','reviewer') THEN v_granted:=v_granted||('work:'||p_work::text||':record_write'); END IF;
 IF p_role='reviewer' THEN v_granted:=v_granted||('work:'||p_work::text||':record_review'); END IF;
 v_kind:='permission.granted';
 v_digest:='sha256:'||encode(sha256(convert_to('forma:share-grant:v1','UTF8')||'\x00'::bytea||convert_to('everyone','UTF8')
  ||'\x00'::bytea||convert_to(array_to_string(v_granted,','),'UTF8')),'hex');
 v_seq:=v_seq+1; v_event:=gen_random_uuid();
 v_body:='{"actor_principal_id":'||to_json(p_sharer::text)::text||',"app_id":'||to_json(p_app::text)::text||',"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":'||to_json(v_kind)::text||',"occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":'||to_json(p_work::text)::text||',"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,v_kind,p_sharer,p_app,p_work,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 INSERT INTO genui.work_shares(organization_id,share_id,sharer_id,grantee_id,app_id,work_id,granted,audit_event_id,shared_at,role)
  VALUES (p_org,gen_random_uuid(),p_sharer,NULL,p_app,p_work,v_granted,v_event,v_at,p_role);
 FOREACH permission IN ARRAY ARRAY['discover','run','record_read','record_write','record_review'] LOOP
  IF permission IN ('discover','run','record_read') OR (permission='record_write' AND p_role IN ('contributor','reviewer'))
   OR (permission='record_review' AND p_role='reviewer') THEN
   scope:=CASE WHEN permission IN ('discover','run') THEN 'app' ELSE 'work' END;
   resource_id:=CASE WHEN scope='app' THEN p_app ELSE p_work END; inserted:=true; RETURN NEXT;
  END IF;
 END LOOP;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.set_work_everyone(uuid,uuid,uuid,uuid,text) FROM PUBLIC;

-- The owner's revocation of a member's personal share (Tier A review M1).
-- The app's owner alone (an active member at its epoch) removes every
-- record_read, record_write and record_review row p_grantee holds on the
-- work, in one unit, and marks the grantee's live personal share records
-- revoked; one `permission.revoked` names the grantee and the removed set.
-- Nothing else moves: not the organization-wide share, not the app-level
-- discover/run rows (other works of the app may rest on them), not the
-- owner's or the creator's own standing. Rows answered: one per permission
-- with `removed`; a refusal answers none; nothing to remove answers three
-- `false` rows and writes nothing.
CREATE FUNCTION genui_private.revoke_work_share(p_org uuid,p_actor uuid,p_app uuid,p_work uuid,p_grantee uuid)
 RETURNS TABLE(permission text,removed boolean)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; a_status text; a_epoch bigint; v_owner boolean; v_count integer; v_any boolean:=false; v_removed text[]:='{}'; r record;
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text; v_digest text;
BEGIN
 IF p_actor=p_grantee
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_actor::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN; END IF;
 SELECT m.status,m.membership_epoch INTO a_status,a_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_actor FOR SHARE;
 IF NOT FOUND OR a_status<>'active' OR a_epoch::text IS DISTINCT FROM nullif(current_setting('genui.membership_epoch',true),'') THEN RETURN; END IF;
 -- The owner alone; the work must exist; never the owner's own rows.
 v_owner:=EXISTS (SELECT 1 FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=p_app AND a.owner_principal_id=p_actor);
 IF NOT v_owner THEN RETURN; END IF;
 IF NOT EXISTS (SELECT 1 FROM genui.saved_work w WHERE w.organization_id=p_org AND w.app_id=p_app AND w.work_id=p_work) THEN RETURN; END IF;
 IF EXISTS (SELECT 1 FROM genui.apps a WHERE a.organization_id=p_org AND a.app_id=p_app AND a.owner_principal_id=p_grantee) THEN RETURN; END IF;
 FOREACH permission IN ARRAY ARRAY['record_read','record_review','record_write'] LOOP
  DELETE FROM genui.work_permissions p WHERE p.organization_id=p_org AND p.principal_id=p_grantee AND p.app_id=p_app AND p.work_id=p_work AND p.permission=revoke_work_share.permission;
  GET DIAGNOSTICS v_count=ROW_COUNT;
  removed:=v_count=1; v_any:=v_any OR removed;
  IF removed THEN v_removed:=v_removed||('work:'||p_work::text||':'||permission); END IF; RETURN NEXT;
 END LOOP;
 IF NOT v_any THEN RETURN; END IF;
 -- D7.8 append on the admission chain; the digest covers the grantee and
 -- exactly the removed set.
 v_digest:='sha256:'||encode(sha256(convert_to('forma:share-revoke:v1','UTF8')||'\x00'::bytea||convert_to(p_grantee::text,'UTF8')
  ||'\x00'::bytea||convert_to(array_to_string(v_removed,','),'UTF8')),'hex');
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||to_json(p_actor::text)::text||',"app_id":'||to_json(p_app::text)::text||',"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":"permission.revoked","occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":'||to_json(p_work::text)::text||',"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,'permission.revoked',p_actor,p_app,p_work,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 -- The grantee's personal share records of this work, revoked by the owner.
 UPDATE genui.work_shares SET revoked_at=v_at,revoked_by=p_actor,revoke_event_id=v_event
  WHERE organization_id=p_org AND app_id=p_app AND work_id=p_work AND grantee_id=p_grantee AND revoked_at IS NULL;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.revoke_work_share(uuid,uuid,uuid,uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.share_work(uuid,uuid,uuid,uuid,uuid,text),
 genui_private.set_work_everyone(uuid,uuid,uuid,uuid,text),
 genui_private.revoke_work_share(uuid,uuid,uuid,uuid,uuid) TO genui_workspace;
RESET ROLE;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
