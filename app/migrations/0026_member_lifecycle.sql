-- Membership lifecycle (Victor's decisions of 2026-11-15, CLASS-CONTINUATION-PLAN
-- §2.2 and §2.3; APP-229, APP-230): an audited, never-lowering entitlement
-- extension and a per-member erasure on request, both operator-only.
--
-- Two operator kinds join the closed list of D7.8 (`entitlement.extended`,
-- `member.erased`), and the database work of both commands moves into two
-- SECURITY DEFINER functions owned by genui_owner, the NOLOGIN owner of every
-- tenant relation: the operator (genui_migration acting as genui_owner under
-- organization scope, D7.19) calls them inside its own transaction and appends
-- the operator-chain event itself, so a refusal rolls everything back. FORCE
-- row-level security is respected throughout: genui_owner has no BYPASSRLS,
-- every statement runs under `tenant_scope`, and the two guest tables a
-- member's erasure reaches (genui_identity.linked_apps and
-- genui_identity.shared_auth_flows, whose non-deferrable keys reach the
-- member's live shares) get an organization-scoped policy for genui_owner
-- instead of the purge's lifted FORCE.
--
-- Why genui_owner and SECURITY DEFINER (review MINOR-2): the operator chain's
-- RESTRICTIVE insert policy is bound to genui_owner, and only the relations'
-- owner may delete across every tenant table, so the functions must run as
-- genui_owner whatever the caller's SET ROLE state; SECURITY DEFINER fixes
-- that identity at creation (this block's role) rather than trusting the
-- caller's. EXECUTE is revoked from PUBLIC and must never be granted to an
-- application role or to genui_admit: a caller with EXECUTE deletes a
-- member's world in the scoped organization. The qualifier pins both.
--
-- extend_entitlement(p_org, p_principal, p_until): the organization (active)
-- FOR SHARE, the member FOR UPDATE (active), then
-- `expires_at = greatest(expires_at, p_until)` on the member's active
-- entitlements with `expires_at < p_until`: a date is only ever moved later,
-- never earlier, and a member already past `p_until` answers `unchanged`. A
-- lapsed entitlement is an `active` row whose `expires_at` is in the past
-- (nothing in the schema ever sets `expired`): sign-in (sessions.rs) and
-- every guest share the member sponsors (shared_access.rs) require
-- `status='active' AND expires_at>clock_timestamp()`, so the same UPDATE
-- revives it, and no epoch advances (an extension revokes nothing). Revoked
-- entitlements are never touched. Outcomes: changed, unchanged,
-- no_active_entitlement, member_unknown, member_inactive,
-- organization_inactive, until_refused (in the past or more than 400 days
-- ahead by the database clock); no row when the scope is not the operator's.
--
-- erase_member(p_org, p_principal): deletion on request. Requires the
-- membership already revoked (`/admin` Remove or `revoke-membership`), so
-- the sessions and epochs are already dead; refuses `member_active`,
-- `member_unknown`, and `last_owner` when the member's role is still owner,
-- no other active owner exists and the organization is active (a suspended
-- organization is being purged, and the purge path stays as it was).
-- Then, in the caller's transaction, children before parents:
--   * the apps the member owns and everything under them (definition
--     revisions, publications, saved work, collections, records and their
--     revisions, bindings, dependencies, data copies, commands and outcomes,
--     permissions, shares, live shares and the guests' links to them, labels,
--     cards, artifact kinds, derivations, explanations, file grants, the
--     app's folder);
--   * the folders the member owns and every file in them, every file the
--     member uploaded anywhere, and the sources registered from those files
--     unless a saved work created by someone else binds the source (then the
--     source stays with that work, like an entry; a source bound only by the
--     member's own works goes, and those bindings with it). A folder someone
--     else owns for one of the member's apps is detached (kind `box`, no
--     app), never deleted, and keeps its files;
--   * sessions and pre-access sessions, Slack links, agent attributions,
--     member cards and labels, artifact kinds, box shares and file grants,
--     app, work and source permissions, work shares made by or to the
--     member, live shares the member sponsors with the guests' links and
--     sign-in flows naming them;
--   * entitlements revoked; agent tokens revoked with the label replaced by
--     `erased` (the row stays: agent_access_log, the hash-chained MCP log,
--     keys it); the membership's display_name NULL and role member, its
--     epoch advanced; the principal's provider_subject replaced by
--     `erased:<principal_id>` (unique, so the identity key holds), and the
--     same on the member's consumed pending admission, both only while the
--     subject is not yet the tombstone, so a repeat changes nothing
--     (review MAJOR-1).
-- What stays, and why: the principal row (internal_record_revisions.creator_id
-- and updater_id, publications.author_id and approver_id, saved_work.created_by,
-- work_data_copies.actor_principal_id and app_labels.updated_by reference it,
-- so deleting it would delete other owners' records), the membership row
-- (revoked, nameless), member_consents, code_redemptions and the reservation
-- (identifiers and times: the legal record), entitlements (revoked), the
-- audit chains (actor ids, kinds, hashes), agent_access_log (the same, per
-- token), and every entry the member made in an app someone else owns, now
-- attributed to a nameless principal (Victor, Q3). The S3 objects of the
-- deleted files and sources are the caller's (ops/promotion/admission.py
-- erase-member), as at the purge. Outcomes: erased, unchanged (a repeat finds
-- nothing to do), and the refusals above; the report names every count and
-- the ids the caller proves against pg_catalog afterwards.
--
-- Recorded debt, as 0008, 0023, 0024 and 0025 did: ADR-007 D7.8's closed
-- operator kind list gains the two kinds, and D7.1 names six definers.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 26);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 26);

-- (a) D7.8 role binding, restated whole: the operator chain carries the two kinds.
ALTER POLICY operator_chain ON genui.audit_events
 WITH CHECK (writer_role = 'operator' AND kind = ANY (ARRAY['organization.provisioned','handle.rotated',
  'code.issued','code.revoked','code.extended','membership.revoked','entitlement.revoked','permission.granted',
  'permission.revoked','creator_authority.revoked','provider.invitation.created','provider.invitation.reconciled',
  'provider.invitation.revoked','source_revision.registered','work.seeded','provider_tenant.bound','app.seeded',
  'membership.role_changed','entitlement.extended','member.erased']));

-- (b) The operator's organization-scoped reach into the guests' links and
-- the share sign-in flows, under FORCE: a permissive policy for genui_owner
-- only, beside account_scope. Both tables' keys reach live_work_shares and
-- neither is deferrable (an abandoned flow is never swept; review MAJOR-2).
CREATE POLICY operator_scope ON genui_identity.linked_apps TO genui_owner
 USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid)
 WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);
CREATE POLICY operator_scope ON genui_identity.shared_auth_flows TO genui_owner
 USING (owner_organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid)
 WITH CHECK (owner_organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- (c) The two operator definers, owned by genui_owner (this block's role).
CREATE FUNCTION genui_private.extend_entitlement(p_org uuid,p_principal uuid,p_until timestamptz)
 RETURNS TABLE(outcome text,changed boolean,expires_at timestamptz,expires_before timestamptz)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
-- genui_owner's, SECURITY DEFINER, EXECUTE never granted (0026 header).
DECLARE o_status text; m_status text; v_before timestamptz; v_after timestamptz; v_count bigint;
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS NOT NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND OR o_status<>'active' THEN RETURN QUERY SELECT 'organization_inactive'::text,false,NULL::timestamptz,NULL::timestamptz; RETURN; END IF;
 IF p_until<=now() OR p_until>now()+interval '400 days' THEN
  RETURN QUERY SELECT 'until_refused'::text,false,NULL::timestamptz,NULL::timestamptz; RETURN;
 END IF;
 SELECT m.status INTO m_status FROM genui.memberships m WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND THEN RETURN QUERY SELECT 'member_unknown'::text,false,NULL::timestamptz,NULL::timestamptz; RETURN; END IF;
 IF m_status<>'active' THEN RETURN QUERY SELECT 'member_inactive'::text,false,NULL::timestamptz,NULL::timestamptz; RETURN; END IF;
 SELECT max(e.expires_at) INTO v_before FROM genui.entitlements e
  WHERE e.organization_id=p_org AND e.principal_id=p_principal AND e.status='active';
 IF v_before IS NULL THEN RETURN QUERY SELECT 'no_active_entitlement'::text,false,NULL::timestamptz,NULL::timestamptz; RETURN; END IF;
 -- Never lowered: greatest() keeps a later date, and the predicate touches
 -- only rows that are earlier than the target. Revoked rows are untouched.
 UPDATE genui.entitlements e SET expires_at=greatest(e.expires_at,p_until)
  WHERE e.organization_id=p_org AND e.principal_id=p_principal AND e.status='active' AND e.expires_at<p_until;
 GET DIAGNOSTICS v_count=ROW_COUNT;
 SELECT max(e.expires_at) INTO v_after FROM genui.entitlements e
  WHERE e.organization_id=p_org AND e.principal_id=p_principal AND e.status='active';
 IF v_count=0 THEN RETURN QUERY SELECT 'unchanged'::text,false,v_after,v_before; RETURN; END IF;
 RETURN QUERY SELECT 'changed'::text,true,v_after,v_before;
END $$;
REVOKE ALL ON FUNCTION genui_private.extend_entitlement(uuid,uuid,timestamptz) FROM PUBLIC;

CREATE FUNCTION genui_private.erase_member(p_org uuid,p_principal uuid)
 RETURNS TABLE(outcome text,changed boolean,report jsonb)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
-- genui_owner's, SECURITY DEFINER, EXECUTE never granted (0026 header).
DECLARE o_status text; m_status text; m_role text; m_name text; v_subject text; v_issuer text; v_tenant text; v_tomb text;
 v_apps uuid[]; v_boxes uuid[]; v_sources uuid[]; v_kept bigint; v_shares uuid[]; v_tokens uuid[]; v_keys text[]; v_revs jsonb;
 n bigint; c bigint[]:='{}'; v_changed boolean; v_tok bigint; v_ent bigint; v_res bigint:=0; v_det bigint; v_scrub boolean:=false; i integer;
 -- Children before parents: the relation and its predicate ($1 org, $2 principal, $3 apps, $4 boxes, $5 sources,
 -- $6 shares, $7 tokens, $8 issuer, $9 tenant, $10 subject), constants of this body.
 names text[]:=ARRAY['genui_identity.shared_auth_flows','genui_identity.linked_apps','live_work_shares','agent_attributions','box_file_grants',
  'box_derivations','box_explanations','box_briefs','box_shares','box_file_sources','box_files','boxes','member_cards','member_labels',
  'work_artifact_kinds','work_data_copies','command_outcomes','command_claims','record_source_dependencies','internal_record_revisions',
  'internal_records','work_source_dependencies','work_bindings','record_collections','work_permissions','work_shares','code_grant_templates',
  'saved_work','publication_inputs','publication_record_schemas','publications','definition_inputs','definition_record_schemas',
  'definition_revisions','app_permissions','app_labels','apps','source_permissions','source_revisions','sources','sessions',
  'pre_access_sessions','slack_links'];
 preds text[]:=ARRAY['owner_organization_id=$1 AND share_id=ANY($6)','organization_id=$1 AND share_id=ANY($6)','organization_id=$1 AND share_id=ANY($6)',
  'organization_id=$1 AND (token_id=ANY($7) OR app_id=ANY($3))',
  'organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3) OR box_id=ANY($4) OR EXISTS (SELECT 1 FROM genui.box_files f WHERE f.organization_id=$1 AND f.box_id=t.box_id AND f.file_id=t.file_id AND f.uploaded_by=$2))',
  'organization_id=$1 AND (box_id=ANY($4) OR app_id=ANY($3))','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND box_id=ANY($4)',
  'organization_id=$1 AND (box_id=ANY($4) OR principal_id=$2)',
  'organization_id=$1 AND (box_id=ANY($4) OR source_id=ANY($5) OR EXISTS (SELECT 1 FROM genui.box_files f WHERE f.organization_id=$1 AND f.box_id=t.box_id AND f.file_id=t.file_id AND f.uploaded_by=$2))',
  'organization_id=$1 AND (box_id=ANY($4) OR uploaded_by=$2)','organization_id=$1 AND box_id=ANY($4)',
  'organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3))','organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3))',
  'organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3))','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)',
  'organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND (app_id=ANY($3) OR source_id=ANY($5))','organization_id=$1 AND app_id=ANY($3)',
  'organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND (app_id=ANY($3) OR source_id=ANY($5))',
  'organization_id=$1 AND (app_id=ANY($3) OR source_id=ANY($5))','organization_id=$1 AND app_id=ANY($3)',
  'organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3))','organization_id=$1 AND (sharer_id=$2 OR grantee_id=$2 OR app_id=ANY($3))',
  'organization_id=$1 AND (app_id=ANY($3) OR source_id=ANY($5))','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)',
  'organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)',
  'organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND (principal_id=$2 OR app_id=ANY($3))',
  'organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND app_id=ANY($3)','organization_id=$1 AND (principal_id=$2 OR source_id=ANY($5))',
  'organization_id=$1 AND source_id=ANY($5)','organization_id=$1 AND source_id=ANY($5)','organization_id=$1 AND principal_id=$2',
  'organization_id=$1 AND issuer=$8 AND provider_tenant_id=$9 AND provider_subject=$10','organization_id=$1 AND principal_id=$2'];
BEGIN
 IF nullif(current_setting('genui.organization_id',true),'') IS DISTINCT FROM p_org::text
  OR nullif(current_setting('genui.principal_id',true),'') IS NOT NULL THEN RETURN; END IF;
 SELECT o.status INTO o_status FROM genui.organizations o WHERE o.organization_id=p_org FOR SHARE;
 IF NOT FOUND THEN RETURN QUERY SELECT 'organization_unknown'::text,false,NULL::jsonb; RETURN; END IF;
 SELECT m.status,m.role,m.display_name INTO m_status,m_role,m_name FROM genui.memberships m
  WHERE m.organization_id=p_org AND m.principal_id=p_principal FOR UPDATE;
 IF NOT FOUND THEN RETURN QUERY SELECT 'member_unknown'::text,false,NULL::jsonb; RETURN; END IF;
 IF m_status='active' THEN RETURN QUERY SELECT 'member_active'::text,false,NULL::jsonb; RETURN; END IF;
 IF m_role='owner' AND o_status='active' THEN
  -- The last-owner rule, serialized with 0025's owner transitions.
  PERFORM pg_advisory_xact_lock(hashtextextended('forma:owner-change:'||p_org::text,0));
  IF NOT EXISTS (SELECT 1 FROM genui.memberships m
   WHERE m.organization_id=p_org AND m.role='owner' AND m.status='active' AND m.principal_id<>p_principal) THEN
   RETURN QUERY SELECT 'last_owner'::text,false,NULL::jsonb; RETURN;
  END IF;
 END IF;
 SELECT p.provider_subject,p.issuer,p.provider_tenant_id INTO v_subject,v_issuer,v_tenant FROM genui.principals p
  WHERE p.organization_id=p_org AND p.principal_id=p_principal FOR UPDATE;
 v_tomb:='erased:'||p_principal::text;
 -- What the member owns. A source from the member's files stays when a saved work created by someone else binds it.
 v_apps:=ARRAY(SELECT app_id FROM genui.apps WHERE organization_id=p_org AND owner_principal_id=p_principal ORDER BY 1);
 v_boxes:=ARRAY(SELECT box_id FROM genui.boxes WHERE organization_id=p_org AND owner_principal_id=p_principal ORDER BY 1);
 v_tokens:=ARRAY(SELECT token_id FROM genui.agent_tokens WHERE organization_id=p_org AND principal_id=p_principal ORDER BY 1);
 v_sources:=ARRAY(SELECT DISTINCT s.source_id FROM genui.box_file_sources s
  JOIN genui.box_files f ON f.organization_id=s.organization_id AND f.box_id=s.box_id AND f.file_id=s.file_id
  WHERE s.organization_id=p_org AND (f.box_id=ANY(v_boxes) OR f.uploaded_by=p_principal)
  AND NOT EXISTS (SELECT 1 FROM genui.work_bindings b JOIN genui.saved_work w ON w.organization_id=b.organization_id AND w.app_id=b.app_id AND w.work_id=b.work_id
   WHERE b.organization_id=p_org AND b.source_id=s.source_id AND w.created_by IS DISTINCT FROM p_principal));
 SELECT count(DISTINCT s.source_id) INTO v_kept FROM genui.box_file_sources s
  JOIN genui.box_files f ON f.organization_id=s.organization_id AND f.box_id=s.box_id AND f.file_id=s.file_id
  WHERE s.organization_id=p_org AND (f.box_id=ANY(v_boxes) OR f.uploaded_by=p_principal) AND NOT (s.source_id=ANY(v_sources));
 v_shares:=ARRAY(SELECT share_id FROM genui.live_work_shares WHERE organization_id=p_org AND (sponsor_id=p_principal OR app_id=ANY(v_apps)) ORDER BY 1);
 v_keys:=ARRAY(SELECT object_key FROM genui.box_files WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR uploaded_by=p_principal) ORDER BY 1);
 v_revs:=coalesce((SELECT jsonb_agg(jsonb_build_array(source_id::text,revision_id::text) ORDER BY source_id,revision_id)
  FROM genui.source_revisions WHERE organization_id=p_org AND source_id=ANY(v_sources)),'[]'::jsonb);
 -- Someone else's folder for one of the member's apps is detached, never deleted (review MINOR-3).
 UPDATE genui.boxes SET app_id=NULL,kind='box' WHERE organization_id=p_org AND app_id=ANY(v_apps) AND NOT (box_id=ANY(v_boxes)); GET DIAGNOSTICS v_det=ROW_COUNT;
 FOR i IN 1..array_length(names,1) LOOP
  EXECUTE format('DELETE FROM %s t WHERE %s',CASE WHEN position('.' IN names[i])>0 THEN names[i] ELSE 'genui.'||names[i] END,preds[i])
   USING p_org,p_principal,v_apps,v_boxes,v_sources,v_shares,v_tokens,v_issuer,v_tenant,v_subject;
  GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 END LOOP;
 -- Kept as identifiers: revoked or scrubbed, each only while not yet so (a repeat changes nothing).
 UPDATE genui.agent_tokens SET revoked_at=coalesce(revoked_at,now()),label='erased'
  WHERE organization_id=p_org AND principal_id=p_principal AND (revoked_at IS NULL OR label<>'erased'); GET DIAGNOSTICS v_tok=ROW_COUNT;
 UPDATE genui.entitlements SET status='revoked',revoked_at=now()
  WHERE organization_id=p_org AND principal_id=p_principal AND status='active'; GET DIAGNOSTICS v_ent=ROW_COUNT;
 IF v_subject<>v_tomb THEN
  UPDATE genui.code_reservations SET provider_subject=v_tomb
   WHERE organization_id=p_org AND kind='pending' AND issuer=v_issuer AND provider_tenant_id=v_tenant AND provider_subject=v_subject; GET DIAGNOSTICS v_res=ROW_COUNT;
  UPDATE genui.principals SET provider_subject=v_tomb WHERE organization_id=p_org AND principal_id=p_principal; v_scrub:=true;
 END IF;
 v_changed:=v_scrub OR m_name IS NOT NULL OR m_role<>'member' OR v_tok>0 OR v_ent>0 OR v_res>0 OR v_det>0 OR EXISTS (SELECT 1 FROM unnest(c) x WHERE x>0);
 IF v_changed THEN
  UPDATE genui.memberships SET display_name=NULL,role='member',membership_epoch=membership_epoch+1 WHERE organization_id=p_org AND principal_id=p_principal;
 END IF;
 report:=jsonb_build_object('deleted',(SELECT jsonb_object_agg(names[i],c[i]) FROM generate_subscripts(names,1) i),
  'app_ids',to_jsonb(v_apps),'box_ids',to_jsonb(v_boxes),'boxes_detached',v_det,'source_ids',to_jsonb(v_sources),'sources_kept',v_kept,'share_ids',to_jsonb(v_shares),
  'object_keys',to_jsonb(v_keys),'source_revisions',v_revs,'agent_tokens_revoked',v_tok,'agent_tokens_kept',coalesce(array_length(v_tokens,1),0),
  'entitlements_revoked',v_ent,'reservations_scrubbed',v_res,'subject_scrubbed',v_scrub,'provider_subject',CASE WHEN v_scrub THEN v_subject END,
  'display_name_cleared',m_name IS NOT NULL,'role_before',m_role);
 outcome:=CASE WHEN v_changed THEN 'erased' ELSE 'unchanged' END; changed:=v_changed;
 RETURN NEXT;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.erase_member(uuid,uuid) FROM PUBLIC;
RESET ROLE;
