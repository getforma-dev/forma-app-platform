-- APP-145. One authoring shell for a member who owns none.
--
-- ADR-007 D7.3 keeps app creation an operator action. This is the one
-- exception: the member's own compose, after identity and the member fence,
-- may create one empty app they own. The shell is not a library app and it
-- has no publication. Grants are edit_definition, run, and discover. One
-- permission.granted event on the admission chain names the app. A second
-- call returns the existing unpublished shell and writes nothing.
-- No publication policy is inserted. Executed only by the checksummed runner.
SET LOCAL ROLE genui_owner;

ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 16);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 16);

-- genui_admit could write permissions and the admission chain, not an app
-- row, and could not read the unpublished check or the space pick. Column
-- grants only: a table INSERT would fail the closed grant set.
GRANT INSERT(organization_id,app_id,space_id,owner_principal_id) ON genui.apps TO genui_admit;
GRANT SELECT(organization_id,app_id) ON genui.publications TO genui_admit;
GRANT SELECT(organization_id,space_id) ON genui.spaces TO genui_admit;

CREATE FUNCTION genui_private.ensure_authoring_shell(p_org uuid, p_principal uuid)
 RETURNS uuid
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
DECLARE o_status text; m_status text; m_epoch bigint; v_app uuid; v_space uuid;
 v_seq bigint; v_prev text; v_event uuid; v_at timestamptz; v_body text; v_hash text; v_digest text;
BEGIN
 IF p_org IS NULL OR p_principal IS NULL
  OR nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS DISTINCT FROM p_principal::text
  OR nullif(current_setting('genui.membership_epoch',true),'') IS NULL THEN RETURN NULL; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN NULL; END IF;
 SELECT m.status,m.membership_epoch INTO m_status,m_epoch FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR SHARE;
 IF NOT FOUND OR m_status<>'active'
  OR m_epoch::text IS DISTINCT FROM current_setting('genui.membership_epoch',true) THEN RETURN NULL; END IF;
 -- The fence already holds FOR SHARE on this membership. FOR UPDATE here
 -- deadlocks a second first compose. This lock serializes the insert; the
 -- waiter re-reads under READ COMMITTED.
 PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended('genui:authoring-shell:'||p_org::text||':'||p_principal::text,0));
 SELECT a.app_id INTO v_app FROM genui.apps a
  WHERE a.organization_id=p_org AND a.owner_principal_id=p_principal
   AND NOT EXISTS (SELECT 1 FROM genui.publications p WHERE p.organization_id=a.organization_id AND p.app_id=a.app_id)
   AND EXISTS (SELECT 1 FROM genui.app_permissions e WHERE e.organization_id=a.organization_id AND e.principal_id=p_principal AND e.app_id=a.app_id AND e.permission='edit_definition')
   AND EXISTS (SELECT 1 FROM genui.app_permissions e WHERE e.organization_id=a.organization_id AND e.principal_id=p_principal AND e.app_id=a.app_id AND e.permission='run')
  ORDER BY a.app_id LIMIT 1;
 IF FOUND THEN RETURN v_app; END IF;
 SELECT s.space_id INTO v_space FROM genui.spaces s WHERE s.organization_id=p_org ORDER BY s.space_id LIMIT 1;
 IF NOT FOUND THEN RETURN NULL; END IF;
 v_app:=gen_random_uuid();
 INSERT INTO genui.apps(organization_id,app_id,space_id,owner_principal_id) VALUES (p_org,v_app,v_space,p_principal);
 INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission)
  SELECT p_org,p_principal,v_app,x.permission FROM (VALUES ('discover'),('edit_definition'),('run')) AS x(permission);
 v_digest:='sha256:'||encode(sha256(convert_to('forma:authoring-shell:v1','UTF8')||'\x00'::bytea
  ||convert_to(p_principal::text,'UTF8')||'\x00'::bytea||convert_to(v_app::text,'UTF8')),'hex');
 SELECT h.chain_sequence,h.head_hash INTO v_seq,v_prev FROM genui.audit_heads h
  WHERE h.organization_id=p_org AND h.writer_role='admission' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'admission audit chain is not provisioned' USING ERRCODE='55000'; END IF;
 v_seq:=v_seq+1; v_event:=gen_random_uuid(); v_at:=date_trunc('second',now());
 v_body:='{"actor_principal_id":'||to_json(p_principal::text)::text||',"app_id":'||to_json(v_app::text)::text||',"chain_sequence":'||v_seq::text
  ||',"code_id":null,"event_id":'||to_json(v_event::text)::text
  ||',"kind":"permission.granted","occurred_at":'||to_json(to_char(v_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'))::text
  ||',"organization_id":'||to_json(p_org::text)::text||',"outcome_digest":'||to_json(v_digest)::text
  ||',"previous_hash":'||to_json(v_prev)::text||',"record_id":null,"work_id":null,"writer_role":"admission"}';
 v_hash:='sha256:'||encode(sha256(convert_to('forma:audit-event:v1','UTF8')||'\x00'::bytea||convert_to(v_prev,'UTF8')||convert_to(v_body,'UTF8')),'hex');
 INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,record_id,code_id,outcome_digest,occurred_at,previous_hash,hash)
  VALUES (p_org,v_event,'admission',v_seq,'permission.granted',p_principal,v_app,NULL,NULL,NULL,v_digest,v_at,v_prev,v_hash);
 UPDATE genui.audit_heads SET chain_sequence=v_seq,head_hash=v_hash WHERE organization_id=p_org AND writer_role='admission';
 RETURN v_app;
END $$;
REVOKE ALL ON FUNCTION genui_private.ensure_authoring_shell(uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.ensure_authoring_shell(uuid,uuid) TO genui_workspace;
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.ensure_authoring_shell(uuid,uuid) OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
