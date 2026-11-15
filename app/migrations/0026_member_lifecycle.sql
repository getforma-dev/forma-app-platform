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
-- every statement runs under `tenant_scope`, and the one guest table a
-- member's erasure reaches (genui_identity.linked_apps, whose non-deferrable
-- key reaches the member's live shares) gets an organization-scoped policy
-- for genui_owner instead of the purge's lifted FORCE.
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
--     unless another owner's saved work binds the source (then the source
--     stays with that owner's app, like an entry);
--   * sessions and pre-access sessions, Slack links, agent attributions,
--     member cards and labels, artifact kinds, box shares and file grants,
--     app, work and source permissions, work shares made by or to the
--     member, live shares the member sponsors;
--   * entitlements revoked; agent tokens revoked with the label replaced by
--     `erased` (the row stays: agent_access_log, the hash-chained MCP log,
--     keys it); the membership's display_name NULL and role member, its
--     epoch advanced; the principal's provider_subject replaced by
--     `erased:<principal_id>` (unique, so the identity key holds), and the
--     same on the member's consumed pending admission.
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

-- (b) The operator's organization-scoped reach into the guests' links, under
-- FORCE: a permissive policy for genui_owner only, beside account_scope.
CREATE POLICY operator_scope ON genui_identity.linked_apps TO genui_owner
 USING (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid)
 WITH CHECK (organization_id = nullif(current_setting('genui.organization_id',true),'')::uuid);

-- (c) The two operator definers, owned by genui_owner (this block's role).
CREATE FUNCTION genui_private.extend_entitlement(p_org uuid,p_principal uuid,p_until timestamptz)
 RETURNS TABLE(outcome text,changed boolean,expires_at timestamptz,expires_before timestamptz)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
#variable_conflict use_column
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
DECLARE o_status text; m_status text; m_role text; m_name text; v_subject text; v_issuer text; v_tenant text; v_tomb text;
 v_apps uuid[]; v_boxes uuid[]; v_sources uuid[]; v_kept bigint; v_shares uuid[]; v_tokens uuid[]; v_keys text[]; v_revs jsonb;
 n bigint; c bigint[]:='{}'; v_changed boolean; v_tok bigint; v_ent bigint; v_res bigint; v_scrub boolean:=false;
 names text[]:=ARRAY['genui_identity.linked_apps','live_work_shares','agent_attributions','box_file_grants','box_derivations','box_explanations',
  'box_briefs','box_shares','box_file_sources','box_files','boxes','member_cards','member_labels','work_artifact_kinds','work_data_copies',
  'command_outcomes','command_claims','record_source_dependencies','internal_record_revisions','internal_records','work_source_dependencies',
  'work_bindings','record_collections','work_permissions','work_shares','code_grant_templates','saved_work','publication_inputs',
  'publication_record_schemas','publications','definition_inputs','definition_record_schemas','definition_revisions','app_permissions',
  'app_labels','apps','source_permissions','source_revisions','sources','sessions','pre_access_sessions','slack_links'];
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
 -- What the member owns; sources from the member's files stay when another owner's work binds them.
 v_apps:=ARRAY(SELECT app_id FROM genui.apps WHERE organization_id=p_org AND owner_principal_id=p_principal ORDER BY 1);
 v_boxes:=ARRAY(SELECT box_id FROM genui.boxes WHERE organization_id=p_org AND (owner_principal_id=p_principal OR app_id=ANY(v_apps)) ORDER BY 1);
 v_tokens:=ARRAY(SELECT token_id FROM genui.agent_tokens WHERE organization_id=p_org AND principal_id=p_principal ORDER BY 1);
 v_sources:=ARRAY(SELECT DISTINCT s.source_id FROM genui.box_file_sources s
  JOIN genui.box_files f ON f.organization_id=s.organization_id AND f.box_id=s.box_id AND f.file_id=s.file_id
  WHERE s.organization_id=p_org AND (f.box_id=ANY(v_boxes) OR f.uploaded_by=p_principal)
  AND NOT EXISTS (SELECT 1 FROM genui.work_bindings b JOIN genui.apps a ON a.organization_id=b.organization_id AND a.app_id=b.app_id
   WHERE b.organization_id=p_org AND b.source_id=s.source_id AND a.owner_principal_id<>p_principal));
 SELECT count(DISTINCT s.source_id) INTO v_kept FROM genui.box_file_sources s
  JOIN genui.box_files f ON f.organization_id=s.organization_id AND f.box_id=s.box_id AND f.file_id=s.file_id
  WHERE s.organization_id=p_org AND (f.box_id=ANY(v_boxes) OR f.uploaded_by=p_principal) AND NOT (s.source_id=ANY(v_sources));
 v_shares:=ARRAY(SELECT share_id FROM genui.live_work_shares WHERE organization_id=p_org AND (sponsor_id=p_principal OR app_id=ANY(v_apps)) ORDER BY 1);
 v_keys:=ARRAY(SELECT object_key FROM genui.box_files WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR uploaded_by=p_principal) ORDER BY 1);
 v_revs:=coalesce((SELECT jsonb_agg(jsonb_build_array(source_id::text,revision_id::text) ORDER BY source_id,revision_id)
  FROM genui.source_revisions WHERE organization_id=p_org AND source_id=ANY(v_sources)),'[]'::jsonb);
 -- Children before parents, in the order of `names`; every count is reported.
 DELETE FROM genui_identity.linked_apps WHERE organization_id=p_org AND share_id=ANY(v_shares); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.live_work_shares WHERE organization_id=p_org AND share_id=ANY(v_shares); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.agent_attributions WHERE organization_id=p_org AND (token_id=ANY(v_tokens) OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_file_grants g WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps) OR box_id=ANY(v_boxes)
  OR EXISTS (SELECT 1 FROM genui.box_files f WHERE f.organization_id=g.organization_id AND f.box_id=g.box_id AND f.file_id=g.file_id AND f.uploaded_by=p_principal)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_derivations WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_explanations WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_briefs WHERE organization_id=p_org AND box_id=ANY(v_boxes); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_shares WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR principal_id=p_principal); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_file_sources s WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR source_id=ANY(v_sources)
  OR EXISTS (SELECT 1 FROM genui.box_files f WHERE f.organization_id=s.organization_id AND f.box_id=s.box_id AND f.file_id=s.file_id AND f.uploaded_by=p_principal)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.box_files WHERE organization_id=p_org AND (box_id=ANY(v_boxes) OR uploaded_by=p_principal); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.boxes WHERE organization_id=p_org AND box_id=ANY(v_boxes); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.member_cards WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.member_labels WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_artifact_kinds WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_data_copies WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.command_outcomes WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.command_claims WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.record_source_dependencies WHERE organization_id=p_org AND (app_id=ANY(v_apps) OR source_id=ANY(v_sources)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.internal_record_revisions WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.internal_records WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_source_dependencies WHERE organization_id=p_org AND (app_id=ANY(v_apps) OR source_id=ANY(v_sources)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_bindings WHERE organization_id=p_org AND (app_id=ANY(v_apps) OR source_id=ANY(v_sources)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.record_collections WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_permissions WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.work_shares WHERE organization_id=p_org AND (sharer_id=p_principal OR grantee_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.code_grant_templates WHERE organization_id=p_org AND (app_id=ANY(v_apps) OR source_id=ANY(v_sources)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.saved_work WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.publication_inputs WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.publication_record_schemas WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.publications WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.definition_inputs WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.definition_record_schemas WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.definition_revisions WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.app_permissions WHERE organization_id=p_org AND (principal_id=p_principal OR app_id=ANY(v_apps)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.app_labels WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.apps WHERE organization_id=p_org AND app_id=ANY(v_apps); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.source_permissions WHERE organization_id=p_org AND (principal_id=p_principal OR source_id=ANY(v_sources)); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.source_revisions WHERE organization_id=p_org AND source_id=ANY(v_sources); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.sources WHERE organization_id=p_org AND source_id=ANY(v_sources); GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.sessions WHERE organization_id=p_org AND principal_id=p_principal; GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.pre_access_sessions WHERE organization_id=p_org AND issuer=v_issuer AND provider_tenant_id=v_tenant AND provider_subject=v_subject; GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 DELETE FROM genui.slack_links WHERE organization_id=p_org AND principal_id=p_principal; GET DIAGNOSTICS n=ROW_COUNT; c:=c||n;
 -- Kept as identifiers: revoked or scrubbed.
 UPDATE genui.agent_tokens SET revoked_at=coalesce(revoked_at,now()),label='erased'
  WHERE organization_id=p_org AND principal_id=p_principal AND (revoked_at IS NULL OR label<>'erased'); GET DIAGNOSTICS v_tok=ROW_COUNT;
 UPDATE genui.entitlements SET status='revoked',revoked_at=now()
  WHERE organization_id=p_org AND principal_id=p_principal AND status='active'; GET DIAGNOSTICS v_ent=ROW_COUNT;
 UPDATE genui.code_reservations SET provider_subject=v_tomb
  WHERE organization_id=p_org AND kind='pending' AND issuer=v_issuer AND provider_tenant_id=v_tenant AND provider_subject=v_subject; GET DIAGNOSTICS v_res=ROW_COUNT;
 IF v_subject<>v_tomb THEN
  UPDATE genui.principals SET provider_subject=v_tomb WHERE organization_id=p_org AND principal_id=p_principal; v_scrub:=true;
 END IF;
 v_changed:=v_scrub OR m_name IS NOT NULL OR m_role<>'member' OR v_tok>0 OR v_ent>0 OR v_res>0 OR EXISTS (SELECT 1 FROM unnest(c) x WHERE x>0);
 IF v_changed THEN
  UPDATE genui.memberships SET display_name=NULL,role='member',membership_epoch=membership_epoch+1 WHERE organization_id=p_org AND principal_id=p_principal;
 END IF;
 report:=jsonb_build_object('deleted',(SELECT jsonb_object_agg(names[i],c[i]) FROM generate_subscripts(names,1) i),
  'app_ids',to_jsonb(v_apps),'box_ids',to_jsonb(v_boxes),'source_ids',to_jsonb(v_sources),'sources_kept',v_kept,'share_ids',to_jsonb(v_shares),
  'object_keys',to_jsonb(v_keys),'source_revisions',v_revs,'agent_tokens_revoked',v_tok,'agent_tokens_kept',coalesce(array_length(v_tokens,1),0),
  'entitlements_revoked',v_ent,'reservations_scrubbed',v_res,'subject_scrubbed',v_scrub,'provider_subject',CASE WHEN v_scrub THEN v_subject END,
  'display_name_cleared',m_name IS NOT NULL,'role_before',m_role);
 outcome:=CASE WHEN v_changed THEN 'erased' ELSE 'unchanged' END; changed:=v_changed;
 RETURN NEXT;
 RETURN;
END $$;
REVOKE ALL ON FUNCTION genui_private.erase_member(uuid,uuid) FROM PUBLIC;
RESET ROLE;
