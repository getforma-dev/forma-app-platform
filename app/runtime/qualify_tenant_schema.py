"""Bounded local PostgreSQL 16.13 schema qualification; no AWS/IAM acceptance.

Run with the existing locked runtime + contract validator environments. A unique
Docker database publishes only a random loopback port and is removed in finally.
"""
import argparse
from datetime import timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import shutil
import tempfile
import threading
import time
import uuid

import psycopg
from psycopg import sql
import runtime
from tenant_seed import ORGANIZATIONS, OWNER_PUBLISH_POLICY, make_bundles, seed, canonical

IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
MIGRATIONS = Path(__file__).resolve().parents[1] / 'migrations'
P = '00000000-0000-4000-8000-000000000066'
APP = '00000000-0000-4000-8000-000000000065'
WORK = '00000000-0000-4000-8000-000000000069'
PUB = '00000000-0000-4000-8000-000000000068'
APP_ROLES = ('genui_workspace','genui_control','genui_data','genui_worker')
# ADR-007 D7.1: the six genui_admit definer functions, exact identity arguments and results.
ADMIT_FUNCTIONS = {
    'resolve_code': ('p_code_digest text', 'uuid'),
    'resolve_handle': ('p_handle_digest text', 'uuid'),
    'reserve_code': ('p_org uuid, p_code_digest text, p_context_digest text',
                     'TABLE(reservation_id uuid, expires_at timestamp with time zone, outcome text)'),
    'record_pending_admission': ('p_org uuid, p_reservation uuid, p_issuer text, p_provider_tenant text, p_provider_subject text',
                                 'TABLE(pending_admission_id uuid, expires_at timestamp with time zone, outcome text)'),
    'admit_redemption': ('p_org uuid, p_pending_admission uuid, p_issuer text, p_provider_tenant text, p_provider_subject text, p_payload_digest text',
                         'TABLE(principal_id uuid, membership_epoch bigint, entitlement_id uuid, redemption_id uuid, outcome text)'),
    'advance_member_epoch': ('p_org uuid, p_principal uuid, p_reason text', 'bigint'),
    # Migration 0008 (ADR-011 pitch, APP-095): the seventh definer; the sharer's
    # own permission subset onto a teammate (recorded ADR-007 debt).
    'share_work': ('p_org uuid, p_sharer uuid, p_grantee uuid, p_app uuid, p_work uuid, p_record_write boolean',
                   'TABLE(scope text, resource_id uuid, permission text, inserted boolean)'),
    # Migration 0011 (ADR-011 pitch, APP-108): the eighth definer; a member's
    # confirmed folder CSV/XLSX becomes a source, revision and source_read.
    'register_box_source': ('p_org uuid, p_principal uuid, p_box uuid, p_file uuid, p_source uuid, p_revision uuid, p_content_digest text, '
                            'p_normalized_digest text, p_contract_digest text, p_envelope bytea, p_row_count bigint',
                            'TABLE(source_id uuid, revision_id uuid, created_at timestamp with time zone)'),
}
# Relations migration 0003 adds to genui (D7.3: each states RLS, FORCE and tenant_scope).
ADMISSION_TABLES = ('access_codes','audit_events','audit_heads','code_grant_templates','code_redemptions',
                    'code_reservations','entitlements','provider_invitations')
# Relation migration 0008 adds (APP-095 review M3): the record of each share that
# wrote a row, written by share_work only; no application role holds a privilege.
SHARING_TABLES = ('work_shares',)
# D7.8 closed chains and kinds for the roles that hold INSERT on audit_events in 0003.
CHAIN_KINDS = {
    # 0008 widens the admission list in place (ALTER POLICY): share_work appends
    # permission.granted (ADR-011 pitch, APP-095; recorded ADR-007 D7.8 debt).
    'genui_admit': ('admission', ('admission.pending','admission.redeemed','admission.denied_for_reconciliation',
                                  'session.sign_out','session.sign_out_all','permission.granted')),
    # 0005 widens the operator list in place (ALTER POLICY): ADR-007 amendment 2026-10-29 (APP-010 review M2).
    'genui_owner': ('operator', ('organization.provisioned','handle.rotated','code.issued','code.revoked','code.extended',
                                 'membership.revoked','entitlement.revoked','permission.granted','permission.revoked',
                                 'creator_authority.revoked')),
}


def _columns(relation, privilege, *columns):
    return {('genui_admit', relation, privilege, column) for column in columns}


# ADR-007 D7.1 (A4): genui_admit's exact column grants; nothing at table level.
ADMIT_GRANTS = set().union(
    _columns('organizations','SELECT','organization_id','status'), _columns('organizations','UPDATE','status'),
    _columns('code_index','SELECT','code_digest','organization_id'),
    _columns('handle_index','SELECT','handle_digest','organization_id','retired_at'),
    _columns('access_codes','SELECT','organization_id','code_id','code_digest','kind','status','max_redemptions','redeemed_count','expires_at'),
    _columns('access_codes','UPDATE','redeemed_count','status'),
    _columns('code_grant_templates','SELECT','organization_id','code_id','permission','app_id','work_id','source_id'),
    _columns('code_reservations','SELECT','organization_id','reservation_id','code_id','kind','context_digest','issuer',
             'provider_tenant_id','provider_subject','state','expires_at'),
    _columns('code_reservations','INSERT','organization_id','reservation_id','code_id','kind','context_digest','issuer',
             'provider_tenant_id','provider_subject','state','expires_at'),
    _columns('code_reservations','UPDATE','state','consumed_at'),
    # Review m2: no function reads principals.principal_id.
    _columns('principals','SELECT','organization_id','issuer','provider_tenant_id','provider_subject'),
    _columns('principals','INSERT','organization_id','principal_id','issuer','provider_tenant_id','provider_subject'),
    # 0008 adds status (share_work judges the grantee's membership).
    _columns('memberships','SELECT','organization_id','principal_id','status','membership_epoch'),
    # 0008 (APP-095): the reads share_work needs to copy the sharer's subset.
    _columns('saved_work','SELECT','organization_id','app_id','work_id','created_by','creator_revoked_at'),
    _columns('work_source_dependencies','SELECT','organization_id','app_id','work_id','source_id'),
    _columns('app_permissions','SELECT','organization_id','principal_id','app_id','permission'),
    _columns('work_permissions','SELECT','organization_id','principal_id','app_id','work_id','permission'),
    _columns('source_permissions','SELECT','organization_id','principal_id','source_id','permission'),
    _columns('memberships','INSERT','organization_id','principal_id','status','membership_epoch'),
    _columns('memberships','UPDATE','membership_epoch'),
    _columns('organization_permissions','INSERT','organization_id','principal_id','permission'),
    _columns('app_permissions','INSERT','organization_id','principal_id','app_id','permission'),
    _columns('work_permissions','INSERT','organization_id','principal_id','app_id','work_id','permission'),
    _columns('source_permissions','INSERT','organization_id','principal_id','source_id','permission'),
    # 0008 (APP-095 review M3): the share's record, exactly the inserted set.
    _columns('work_shares','INSERT','organization_id','share_id','sharer_id','grantee_id','app_id','work_id','granted','audit_event_id','shared_at'),
    _columns('entitlements','INSERT','organization_id','principal_id','entitlement_id','status','kind','starts_at','expires_at'),
    _columns('code_redemptions','SELECT','organization_id','redemption_id','principal_id','pending_admission_id','entitlement_id','payload_digest'),
    _columns('code_redemptions','INSERT','organization_id','redemption_id','code_id','principal_id','pending_admission_id',
             'entitlement_id','payload_digest','committed_at','audit_event_id'),
    _columns('audit_events','INSERT','organization_id','event_id','writer_role','chain_sequence','kind','actor_principal_id',
             'app_id','work_id','record_id','code_id','outcome_digest','occurred_at','previous_hash','hash'),
    _columns('audit_heads','SELECT','organization_id','writer_role','chain_sequence','head_hash'),
    _columns('audit_heads','UPDATE','chain_sequence','head_hash'),
    # 0011 (APP-108): register_box_source's reads and writes.
    _columns('boxes','SELECT','organization_id','box_id','owner_principal_id'),
    _columns('box_files','SELECT','organization_id','box_id','file_id','kind','content_digest'),
    _columns('sources','INSERT','organization_id','source_id'),
    _columns('source_revisions','INSERT','organization_id','source_id','revision_id','content_digest','normalized_digest','contract_digest',
             'deleted_at','canonical_envelope'),
    _columns('box_file_sources','INSERT','organization_id','box_id','file_id','content_digest','source_id','revision_id','normalized_digest',
             'row_count','registered_by','created_at'),
    # 0012 (APP-113 review): the idempotency lookup of register_box_source.
    _columns('box_file_sources','SELECT','organization_id','box_id','file_id','normalized_digest','source_id','revision_id','created_at'),
)


# Migration 0004 (ADR-007 D7.3, APP-073 Task 5): the entitlement columns the
# shared member precheck and fence read, for exactly these three roles.
ENTITLEMENT_READS = {(role,'entitlements','SELECT',column) for role in ('genui_workspace','genui_control','genui_data')
                     for column in ('organization_id','principal_id','entitlement_id','status','expires_at')}


def _role_columns(role, relation, privilege, *columns):
    return {(role, relation, privilege, column) for column in columns}


AUDIT_EVENT_COLUMNS = ('organization_id','event_id','writer_role','chain_sequence','kind','actor_principal_id','app_id','work_id',
                       'record_id','code_id','outcome_digest','occurred_at','previous_hash','hash')
# Migration 0005 (ADR-007 D7.3/D7.8, APP-010): column-limited INSERTs for the
# definition, publication and saved-work transactions and the two application
# audit chains. 0002 already grants the SELECTs on every business relation.
# Review m2: no INSERT on apps (workspace) or publication_policies (control):
# app shells and policy rows are operator writes in Phase 1 ("never earlier").
DEFINITION_WORK_GRANTS = set().union(
    _role_columns('genui_workspace','definition_revisions','INSERT','organization_id','app_id','revision_id','schema_version','digest',
                  'canonical_content'),
    _role_columns('genui_workspace','definition_inputs','INSERT','organization_id','app_id','revision_id','input_id','kind',
                  'contract_digest','canonical_contract'),
    _role_columns('genui_workspace','definition_record_schemas','INSERT','organization_id','app_id','revision_id','schema_id',
                  'schema_version','digest','canonical_schema'),
    # ADR-007 amendment (review M2): the creator is recorded, never creator_revoked_at.
    _role_columns('genui_workspace','saved_work','INSERT','organization_id','app_id','work_id','publication_id','version','context',
                  'created_by'),
    _role_columns('genui_workspace','record_collections','INSERT','organization_id','app_id','work_id','publication_id','collection_id',
                  'schema_id','schema_version'),
    _role_columns('genui_workspace','work_bindings','INSERT','organization_id','app_id','work_id','publication_id','input_id','kind',
                  'contract_digest','source_id','source_revision_id','source_digest','collection_id','schema_id','schema_version',
                  'revision_policy'),
    _role_columns('genui_workspace','work_source_dependencies','INSERT','organization_id','app_id','work_id','input_id','source_id',
                  'source_revision_id','source_digest'),
    # ADR-007 amendment (review M1): publication_sequence orders an app's publications.
    _role_columns('genui_control','publications','INSERT','organization_id','app_id','publication_id','definition_revision_id',
                  'definition_digest','policy_id','policy_version','policy_kind','author_id','approver_id','approver_epoch',
                  'renderer_version','operator_version','admission_digest','published_at','canonical_envelope',
                  'publication_sequence'),
    _role_columns('genui_control','publication_inputs','INSERT','organization_id','app_id','publication_id','definition_revision_id',
                  'input_id','kind','contract_digest'),
    _role_columns('genui_control','publication_record_schemas','INSERT','organization_id','app_id','publication_id',
                  'definition_revision_id','schema_id','schema_version','digest'),
    *(_role_columns(role,'audit_events','INSERT',*AUDIT_EVENT_COLUMNS) for role in ('genui_workspace','genui_control')),
    *(_role_columns(role,'audit_heads','SELECT','organization_id','writer_role','chain_sequence','head_hash')
      for role in ('genui_workspace','genui_control')),
    *(_role_columns(role,'audit_heads','UPDATE','chain_sequence','head_hash') for role in ('genui_workspace','genui_control')),
)
# D7.8 closed chains and kinds of the application roles 0005 and 0007 bind.
APPLICATION_CHAIN_KINDS = {
    # 0011 (APP-108) widens the workspace list in place with the folder commands;
    # 0012 (APP-113 review) adds the summary and brief saves; 0014 (APP-121)
    # the organization label and personal alias writes.
    'genui_workspace': ('workspace', ('session.signed_in','app.created','definition.drafted','work.saved','work.bound','box.created',
                                      'box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared',
                                      'box.explained','box.briefed','app.labeled','app.aliased')),
    'genui_control': ('control', ('publication.published','policy.changed')),
    'genui_data': ('data', ('command.committed',)),
}


# Migration 0006 (ADR-006, ADR-007 D7.3/D7.5/D7.11; APP-009): the three session
# relations, genui_workspace's column SELECT/INSERT, UPDATE(last_seen_at,
# revoked_at) and table DELETE, plus one narrow column read of
# code_reservations (the open D7.3 question recorded in evidence/APP-009).
SESSION_TABLES = ('prelogin_flows','pre_access_sessions','sessions')
PRELOGIN_COLUMNS = ('organization_id','flow_id','cookie_digest','state_digest','kind','reservation_id','signup_handoff_id',
                    'code_verifier_ciphertext','client_address_class','created_at','expires_at')
PRE_ACCESS_COLUMNS = ('organization_id','session_id','cookie_digest','issuer','provider_tenant_id','provider_subject','email_verified',
                      'flow_kind','pending_admission_id','pending_expires_at','refresh_token_ref','created_at','last_seen_at','expires_at')
SESSION_COLUMNS = ('organization_id','session_id','cookie_digest','principal_id','membership_epoch_at_mint','entitlement_id',
                   'registry_sha256','refresh_token_ref','created_at','last_seen_at','absolute_expires_at')
PENDING_LOOKUP_COLUMNS = ('organization_id','reservation_id','kind','issuer','provider_tenant_id','provider_subject','state','expires_at')
SESSION_GRANTS = set().union(
    _role_columns('genui_workspace','prelogin_flows','SELECT',*PRELOGIN_COLUMNS),
    _role_columns('genui_workspace','prelogin_flows','INSERT',*PRELOGIN_COLUMNS),
    _role_columns('genui_workspace','pre_access_sessions','SELECT',*PRE_ACCESS_COLUMNS,'revoked_at'),
    _role_columns('genui_workspace','pre_access_sessions','INSERT',*PRE_ACCESS_COLUMNS),
    _role_columns('genui_workspace','pre_access_sessions','UPDATE','last_seen_at','revoked_at'),
    _role_columns('genui_workspace','sessions','SELECT',*SESSION_COLUMNS,'revoked_at'),
    _role_columns('genui_workspace','sessions','INSERT',*SESSION_COLUMNS),
    _role_columns('genui_workspace','sessions','UPDATE','last_seen_at','revoked_at'),
    {('genui_workspace',table,'DELETE',None) for table in SESSION_TABLES},
    _role_columns('genui_workspace','code_reservations','SELECT',*PENDING_LOOKUP_COLUMNS),
)


# Migration 0010 (ADR-011 pitch, APP-099): member-minted agent tokens (only a
# digest) and their display attributions; genui_workspace column SELECT and
# INSERT, UPDATE(last_used_at, revoked_at) on tokens, no DELETE. 0012
# (APP-113 review) adds expires_at (SELECT and INSERT). 0013 (APP-115 quick
# wins) adds the token scope columns (SELECT and INSERT) and the append-only,
# hash-chained agent_access_log (SELECT and INSERT only: no UPDATE, no DELETE).
AGENT_TABLES = ('agent_access_log','agent_attributions','agent_tokens')
AGENT_TOKEN_COLUMNS = ('organization_id','token_id','token_digest','principal_id','label','membership_epoch_at_mint',
                       'registry_sha256','created_at','expires_at','scope_kind','scope_spaces','scope_apps','scope_actions')
AGENT_ATTRIBUTION_COLUMNS = ('organization_id','attribution_id','token_id','app_id','work_id','record_id','record_version','created_at')
AGENT_ACCESS_COLUMNS = ('organization_id','entry_id','chain_sequence','token_id','principal_id','agent_label','tool','touched_apps',
                        'touched_works','touched_records','touched_folders','touched_files','outcome','reason','space',
                        'resource_label','duration_ms','occurred_at','previous_hash','hash')
AGENT_GRANTS = set().union(
    _role_columns('genui_workspace','agent_tokens','SELECT',*AGENT_TOKEN_COLUMNS,'last_used_at','revoked_at'),
    _role_columns('genui_workspace','agent_tokens','INSERT',*AGENT_TOKEN_COLUMNS),
    _role_columns('genui_workspace','agent_tokens','UPDATE','last_used_at','revoked_at'),
    _role_columns('genui_workspace','agent_attributions','SELECT',*AGENT_ATTRIBUTION_COLUMNS),
    _role_columns('genui_workspace','agent_attributions','INSERT',*AGENT_ATTRIBUTION_COLUMNS),
    _role_columns('genui_workspace','agent_access_log','SELECT',*AGENT_ACCESS_COLUMNS),
    _role_columns('genui_workspace','agent_access_log','INSERT',*AGENT_ACCESS_COLUMNS),
)

# Migration 0011 (ADR-011 pitch, APP-108): folders. genui_workspace column
# SELECT and INSERT on every relation, UPDATE(name) on boxes (rename), no
# DELETE; box_file_sources is read here and written only by the definer.
BOX_TABLES = ('box_briefs','box_derivations','box_explanations','box_file_grants','box_file_sources','box_files','box_shares','boxes')
BOX_COLUMNS = {
    'boxes': ('organization_id','box_id','owner_principal_id','name','kind','app_id','created_at'),
    'box_files': ('organization_id','box_id','file_id','name','kind','media_type','byte_length','content_digest','object_key','uploaded_by','created_at'),
    'box_derivations': ('organization_id','derivation_id','box_id','app_id','work_id','kind','file_ids','file_digests','created_by','created_at'),
    'box_shares': ('organization_id','box_id','principal_id','granted_by','created_at'),
    'box_file_grants': ('organization_id','box_id','file_id','principal_id','app_id','work_id','granted_by','created_at'),
    'box_explanations': ('organization_id','explanation_id','app_id','work_id','template','created_by','created_at'),
    'box_briefs': ('organization_id','brief_id','box_id','body','created_by','created_at'),
}
BOX_GRANTS = set().union(
    *(_role_columns('genui_workspace',table,'SELECT',*columns) for table,columns in BOX_COLUMNS.items()),
    *(_role_columns('genui_workspace',table,'INSERT',*columns) for table,columns in BOX_COLUMNS.items()),
    _role_columns('genui_workspace','box_file_sources','SELECT','organization_id','box_id','file_id','content_digest','source_id','revision_id',
                  'normalized_digest','row_count','registered_by','created_at'),
    _role_columns('genui_workspace','boxes','UPDATE','name'),
)

# Migration 0014 (ADR-011 pitch, APP-121): naming. genui_workspace column
# SELECT and INSERT and the UPDATE of the label columns; no DELETE.
LABEL_TABLES = ('app_labels','member_labels')
LABEL_COLUMNS = {
    'app_labels': ('organization_id','app_id','card','name','description','address','kept','updated_by','updated_at'),
    'member_labels': ('organization_id','principal_id','app_id','card','name','aliases','updated_at'),
}
LABEL_GRANTS = set().union(
    *(_role_columns('genui_workspace',table,'SELECT',*columns) for table,columns in LABEL_COLUMNS.items()),
    *(_role_columns('genui_workspace',table,'INSERT',*columns) for table,columns in LABEL_COLUMNS.items()),
    _role_columns('genui_workspace','app_labels','UPDATE','name','description','address','kept','updated_by','updated_at'),
    _role_columns('genui_workspace','member_labels','UPDATE','name','aliases','updated_at'),
)

# Migration 0007 (ADR-007 D7.3/D7.7/D7.8/D7.9, ADR-009 D9.8; APP-013): the record
# command's relations and genui_data's column-limited writes. query_receipts
# carries no application grant (APP-023 cites receipts).
COMMAND_TABLES = ('organization_sequences','command_claims','command_outcomes','query_receipts')
CLAIM_COLUMNS = ('organization_id','principal_id','app_id','work_id','command_id','idempotency_key','payload_digest','claimed_at')
OUTCOME_COLUMNS = ('organization_id','app_id','work_id','principal_id','command_id','idempotency_key','payload_digest','state','record_id',
                   'record_version','work_version','commit_sequence','audit_event_id','committed_at','retry_expires_at')
REVISION_COLUMNS = ('organization_id','app_id','work_id','collection_id','record_id','version','creator_id','updater_id','commit_sequence',
                    'committed_at','canonical_values')
RECORD_DEPENDENCY_COLUMNS = ('organization_id','app_id','work_id','collection_id','record_id','record_version','input_id','source_id',
                             'source_revision_id','source_digest')
COMMAND_GRANTS = set().union(
    _role_columns('genui_data','organization_sequences','SELECT','organization_id','next_commit_sequence'),
    _role_columns('genui_data','organization_sequences','UPDATE','next_commit_sequence'),
    _role_columns('genui_data','command_claims','SELECT',*CLAIM_COLUMNS),
    _role_columns('genui_data','command_claims','INSERT',*CLAIM_COLUMNS),
    _role_columns('genui_data','command_outcomes','SELECT',*OUTCOME_COLUMNS),
    _role_columns('genui_data','command_outcomes','INSERT',*OUTCOME_COLUMNS),
    _role_columns('genui_data','internal_records','INSERT','organization_id','app_id','work_id','collection_id','record_id','current_version'),
    _role_columns('genui_data','internal_records','UPDATE','current_version'),
    _role_columns('genui_data','internal_record_revisions','INSERT',*REVISION_COLUMNS),
    _role_columns('genui_data','record_source_dependencies','INSERT',*RECORD_DEPENDENCY_COLUMNS),
    _role_columns('genui_data','audit_events','INSERT',*AUDIT_EVENT_COLUMNS),
    _role_columns('genui_data','audit_heads','SELECT','organization_id','writer_role','chain_sequence','head_hash'),
    _role_columns('genui_data','audit_heads','UPDATE','chain_sequence','head_hash'),
)
# Migration 0009 (ADR-011 pitch, APP-100): the Slack link table, genui_workspace's
# column SELECT and INSERT and the UPDATE of the columns a re-link or unlink rewrites.
SLACK_TABLES = ('slack_links',)
SLACK_COLUMNS = ('organization_id','link_id','principal_id','team_id','slack_user_id','membership_epoch_at_link','linked_at')
SLACK_GRANTS = set().union(
    _role_columns('genui_workspace','slack_links','SELECT',*SLACK_COLUMNS,'revoked_at'),
    _role_columns('genui_workspace','slack_links','INSERT',*SLACK_COLUMNS),
    _role_columns('genui_workspace','slack_links','UPDATE','principal_id','membership_epoch_at_link','linked_at','revoked_at'),
)
# 0007's RESTRICTIVE row bindings beyond the audit chains: the data role claims,
# commits and reads outcomes only as the scoped principal, and records itself as
# a revision's updater.
COMMAND_POLICIES = {('command_claims','command_principal','INSERT'),('command_claims','command_principal_read','SELECT'),
                    ('command_outcomes','command_principal','INSERT'),('command_outcomes','command_principal_read','SELECT'),
                    ('internal_record_revisions','record_updater','INSERT')}


def sha(value):
    return 'sha256:'+hashlib.sha256(value.encode()).hexdigest()


def genesis(organization, writer_role):
    """D7.8 genesis head hash, recomputed independently of the database."""
    return 'sha256:'+hashlib.sha256(b'forma:audit-genesis:v1\x00'+organization.encode()+b'\x00'+writer_role.encode()).hexdigest()


def verify_chain(c, organization, writer_role):
    """D7.8 verification: recompute every hash in chain_sequence order; the tail equals the head."""
    head=c.execute('SELECT chain_sequence,head_hash FROM genui.audit_heads WHERE organization_id=%s AND writer_role=%s',(organization,writer_role)).fetchone()
    events=c.execute("""SELECT organization_id::text,event_id::text,writer_role,chain_sequence,kind,actor_principal_id::text,app_id::text,
      work_id::text,record_id::text,code_id::text,outcome_digest::text,occurred_at,previous_hash,hash FROM genui.audit_events
      WHERE organization_id=%s AND writer_role=%s ORDER BY chain_sequence""",(organization,writer_role)).fetchall()
    previous,sequence=genesis(organization,writer_role),1
    for row in events:
        names=('organization_id','event_id','writer_role','chain_sequence','kind','actor_principal_id','app_id','work_id',
               'record_id','code_id','outcome_digest','occurred_at','previous_hash')
        body=dict(zip(names,row[:13]))
        assert body['occurred_at'].microsecond==0 and row[3]==sequence+1 and row[12]==previous,(organization,writer_role,row[3])
        body['occurred_at']=body['occurred_at'].astimezone(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
        computed='sha256:'+hashlib.sha256(b'forma:audit-event:v1\x00'+previous.encode()+canonical(body)).hexdigest()
        assert computed==row[13],('audit hash mismatch',organization,writer_role,row[3])
        previous,sequence=computed,row[3]
    assert head==(sequence,previous),('audit head differs from chain tail',organization,writer_role)
    return [row[4] for row in events]


def admission_checks(pg, passed, denied):
    """ADR-007 Consequences for migration 0003 on the actual local database.

    Fixture rows are written as the operator channel would (genui_migration
    acting as genui_owner under explicit organization scope); every admission
    behavior runs as genui_workspace through the six definer functions.
    """
    org0,org1=ORGANIZATIONS
    def org_scope(c, org):
        c.execute("SELECT set_config('genui.organization_id',%s,true),set_config('genui.principal_id','',true),set_config('genui.membership_epoch','',true)",(org,))
    def operator(c, org):
        c.execute('SET LOCAL ROLE genui_owner'); org_scope(c,org)
    def owner_sql(org, statement, params=()):
        with pg.connect() as c:
            operator(c,org); return c.execute(statement,params).rowcount
    def call(org, statement, params, role='genui_workspace', member=None):
        with pg.connect(role) as c:
            if member is None: org_scope(c,org)
            else: scope(c,org,member[0],member[1])
            return c.execute(statement,params).fetchall()
    RESERVE='SELECT reservation_id::text,expires_at,outcome FROM genui_private.reserve_code(%s,%s,%s)'
    PENDING='SELECT pending_admission_id::text,expires_at,outcome FROM genui_private.record_pending_admission(%s,%s,%s,%s,%s)'
    ADMIT='SELECT principal_id::text,membership_epoch,entitlement_id::text,redemption_id::text,outcome FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)'
    ADVANCE='SELECT genui_private.advance_member_epoch(%s,%s,%s)'
    def reserve(org, digest, context=None):
        return call(org,RESERVE,(org,digest,context or sha('context-'+uuid.uuid4().hex)))
    def outcome(rows): return [row[-1] for row in rows]
    with pg.connect('postgres') as c:
        identity={org:c.execute('SELECT issuer,provider_tenant_id FROM genui.provider_tenant_bindings WHERE organization_id=%s',(org,)).fetchone() for org in ORGANIZATIONS}
        source0=c.execute('SELECT source_id::text FROM genui.sources WHERE organization_id=%s ORDER BY source_id LIMIT 1',(org0,)).fetchone()[0]
        app1,work1=c.execute('SELECT app_id::text,work_id::text FROM genui.saved_work WHERE organization_id=%s ORDER BY app_id,work_id LIMIT 1',(org1,)).fetchone()
        seeded_subject=c.execute('SELECT provider_subject FROM genui.principals WHERE organization_id=%s AND principal_id=%s',(org0,P)).fetchone()[0]
        organizations_before=c.execute('SELECT organization_id::text,xmin::text,ctid::text,status FROM genui.organizations ORDER BY organization_id').fetchall()
    def person(org, label): return (*identity[org],'usr_admission_'+label)

    # --- Structure: relations, RLS, policies, indexes (D7.2, D7.3, D7.5, D7.8) ---
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",(list(ADMISSION_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in sorted(ADMISSION_TABLES)],rows
        policy="(organization_id = (NULLIF(current_setting('genui.organization_id'::text, true), ''::text))::uuid)"
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND policyname='tenant_scope' AND tablename = ANY(%s) ORDER BY tablename""",(list(ADMISSION_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in sorted(ADMISSION_TABLES)],rows
        # 0008 work_shares: owner-held, RLS, FORCE, the exact policy; only genui_admit's INSERT.
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",(list(SHARING_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in SHARING_TABLES],rows
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename""",(list(SHARING_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in SHARING_TABLES],rows
        shares=c.execute("""SELECT grantee,privilege_type,count(*) FROM information_schema.column_privileges
          WHERE table_schema='genui' AND table_name='work_shares' AND grantee<>'genui_owner' GROUP BY 1,2 ORDER BY 1,2""").fetchall()
        assert shares==[('genui_admit','INSERT',9)],shares
        assert c.execute("""SELECT count(*) FROM information_schema.role_table_grants WHERE table_schema='genui' AND table_name='work_shares'
          AND grantee<>'genui_owner'""").fetchone()==(0,)
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner),
          (SELECT count(*) FROM pg_policy p WHERE p.polrelid=c.oid) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
          WHERE n.nspname='genui_private' AND c.relkind='r' ORDER BY c.relname""").fetchall()
        assert rows==[('code_index',False,False,'genui_owner',0),('handle_index',False,False,'genui_owner',0)],rows
    passed('0008 work_shares is owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; only genui_admit holds a privilege on it (INSERT, nine columns), no application role any',
           tables=list(SHARING_TABLES))
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",(list(SLACK_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in SLACK_TABLES],rows
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename""",(list(SLACK_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in SLACK_TABLES],rows
        links=set(c.execute("""SELECT grantee,privilege_type,column_name FROM information_schema.column_privileges
          WHERE table_schema='genui' AND table_name='slack_links' AND grantee<>'genui_owner'""").fetchall())
        assert links=={(r,p,col) for (r,_,p,col) in SLACK_GRANTS},links
        assert c.execute("""SELECT count(*) FROM information_schema.role_table_grants WHERE table_schema='genui' AND table_name='slack_links'
          AND grantee<>'genui_owner'""").fetchone()==(0,)
    passed('0009 slack_links is owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; only genui_workspace holds column SELECT, INSERT and the four re-link/unlink UPDATE columns, no table-level grant',
           tables=list(SLACK_TABLES))
    passed('0003 relations are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; the two genui_private indexes carry no RLS',
           tables=list(ADMISSION_TABLES))

    # --- Exact (role, relation, privilege, column) grant set (D7.3 + D7.1) ---
    acl_roles=('genui_lock','genui_admit')+APP_ROLES
    with pg.connect('postgres') as c:
        actual=set(c.execute("""SELECT pg_get_userbyid(a.grantee),c.relname,a.privilege_type,NULL::text FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace CROSS JOIN LATERAL aclexplode(c.relacl) a
          WHERE n.nspname IN ('genui','genui_private','genui_meta') AND pg_get_userbyid(a.grantee) = ANY(%s)
          UNION ALL SELECT pg_get_userbyid(a.grantee),c.relname,a.privilege_type,d.attname::text FROM pg_attribute d
          JOIN pg_class c ON c.oid=d.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace CROSS JOIN LATERAL aclexplode(d.attacl) a
          WHERE n.nspname IN ('genui','genui_private','genui_meta') AND pg_get_userbyid(a.grantee) = ANY(%s)""",(list(acl_roles),list(acl_roles))).fetchall())
        previous=json.loads((MIGRATIONS/'catalog-v2.json').read_text())
        expected={(r[2],r[1],r[3],None) for r in previous['table_acl'] if r[2] in acl_roles}
        expected|={(r[3],r[1],r[4],r[2]) for r in previous['column_acl'] if r[3] in acl_roles}
        expected|=ADMIT_GRANTS|ENTITLEMENT_READS|DEFINITION_WORK_GRANTS|SESSION_GRANTS|COMMAND_GRANTS|SLACK_GRANTS|AGENT_GRANTS|BOX_GRANTS|LABEL_GRANTS
        assert actual==expected,{'unexpected':sorted(actual-expected,key=str),'missing':sorted(expected-actual,key=str)}
        # Application roles reach 0003's relations only through 0004's entitlement
        # reads, 0005's and 0007's own-chain audit append (D7.8) and 0006's narrow
        # pending lookup (no code_id, no context digest); never codes or indexes.
        assert {t for t in actual if t[0] in APP_ROLES and t[1] in ADMISSION_TABLES+('code_index','handle_index')}==ENTITLEMENT_READS|{
            t for t in DEFINITION_WORK_GRANTS|COMMAND_GRANTS if t[1] in ('audit_events','audit_heads')}|{
            t for t in SESSION_GRANTS if t[1]=='code_reservations'}
        schema=set(c.execute("""SELECT n.nspname,a.privilege_type FROM pg_namespace n CROSS JOIN LATERAL aclexplode(n.nspacl) a
          WHERE n.nspname IN ('genui','genui_private','genui_meta') AND pg_get_userbyid(a.grantee)='genui_admit'""").fetchall())
        assert schema=={('genui','USAGE'),('genui_private','USAGE')},schema
        executes=set(c.execute("""SELECT p.proname,pg_get_userbyid(a.grantee) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
          CROSS JOIN LATERAL aclexplode(coalesce(p.proacl,acldefault('f',p.proowner))) a
          WHERE n.nspname IN ('genui','genui_private','genui_meta') AND a.privilege_type='EXECUTE' AND a.grantee<>p.proowner""").fetchall())
        assert executes=={('lock_member_authority',r) for r in ('genui_workspace','genui_control','genui_data')}|{('lock_work_revision','genui_data')}|{
            (name,'genui_workspace') for name in ADMIT_FUNCTIONS},executes
    passed('catalog (role, relation, privilege, column) grant set equals 0002 reads plus D7.1 genui_admit grants plus 0004 entitlement column reads plus 0005 definition/work/audit columns plus 0006 session columns plus 0007 command columns (D7.3); EXECUTE on the six functions is genui_workspace only',
           admit_grant_count=len(ADMIT_GRANTS),definition_work_grant_count=len(DEFINITION_WORK_GRANTS),session_grant_count=len(SESSION_GRANTS),
           command_grant_count=len(COMMAND_GRANTS))

    # --- genui_admit attributes, memberships and function properties (D7.1, A3, A4) ---
    with pg.connect('postgres') as c:
        assert c.execute("""SELECT rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls,rolcanlogin,rolinherit
          FROM pg_roles WHERE rolname='genui_admit'""").fetchone()==(False,)*7
        assert c.execute("SELECT count(*) FROM pg_auth_members WHERE member='genui_admit'::regrole").fetchone()==(0,)
        incoming=c.execute("""SELECT m.rolname,a.admin_option,a.inherit_option,a.set_option FROM pg_auth_members a JOIN pg_roles m ON m.oid=a.member
          WHERE a.roleid='genui_admit'::regrole ORDER BY a.admin_option""").fetchall()
        assert incoming==[('genui_migration',False,False,True),('genui_migration',True,False,False)],incoming
        assert c.execute("""SELECT count(*) FROM pg_roles r WHERE r.rolname<>'genui_admit' AND pg_has_role('genui_admit',r.oid,'MEMBER')""").fetchone()==(0,)
        assert c.execute("SELECT has_schema_privilege('genui_admit','genui_private','CREATE'),has_schema_privilege('genui_admit','genui','CREATE')").fetchone()==(False,False)
        assert c.execute("SELECT has_database_privilege('genui_admit','genui','CONNECT')").fetchone()==(False,)
        functions=c.execute("""SELECT p.proname,pg_get_function_identity_arguments(p.oid),pg_get_function_result(p.oid),p.prosecdef,p.proisstrict,
          p.provolatile::text,p.proconfig,p.prosrc FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
          WHERE n.nspname='genui_private' AND pg_get_userbyid(p.proowner)='genui_admit' ORDER BY p.proname""").fetchall()
        assert {r[0]:(r[1],r[2]) for r in functions}==ADMIT_FUNCTIONS,functions
        assert all(r[3:7]==(True,True,'v',['search_path=pg_catalog, pg_temp']) for r in functions),functions
        # A4: organizations.status is lock-enabling only; no body writes the organization.
        writes=[r[0] for r in functions if re.search(r'\b(update|insert\s+into|delete\s+from)\s+genui\.organizations\b',r[7],re.I)]
        assert writes==[] and all(re.search(r'genui\.organizations\s+o\s+where\s+o\.organization_id\s*=\s*p_org\s+for\s+share',r[7],re.I)
                                  for r in functions if r[0] not in ('resolve_code','resolve_handle')),writes
    passed('genui_admit is NOLOGIN NOINHERIT with no memberships, only the operator SET grant, no CREATE; eight exact STRICT VOLATILE definers (six of D7.1, the 0008 share_work and the 0011 register_box_source) with fixed search_path and no organizations write',
           incoming=[list(r) for r in incoming])

    # --- RESTRICTIVE audit bindings: INSERT holders equal policy targets (D7.8) ---
    with pg.connect('postgres') as c:
        holders=lambda table,privilege: {r[0] for r in c.execute("""SELECT DISTINCT pg_get_userbyid(a.grantee) FROM pg_class c
          CROSS JOIN LATERAL aclexplode(coalesce(c.relacl,acldefault('r',c.relowner))) a WHERE c.oid=%s::regclass AND a.privilege_type=%s
          UNION SELECT DISTINCT pg_get_userbyid(a.grantee) FROM pg_attribute d CROSS JOIN LATERAL aclexplode(d.attacl) a
          WHERE d.attrelid=%s::regclass AND a.privilege_type=%s""",('genui.'+table,privilege,'genui.'+table,privilege)).fetchall()}
        policies=c.execute("""SELECT tablename,policyname,permissive,roles::text[],cmd,qual,with_check FROM pg_policies
          WHERE schemaname='genui' AND tablename IN ('audit_events','audit_heads') AND policyname<>'tenant_scope' ORDER BY tablename,policyname""").fetchall()
        for role,(chain,kinds) in CHAIN_KINDS.items():
            name='admission_chain' if chain=='admission' else 'operator_chain'
            check="((writer_role = '%s'::text) AND (kind = ANY (ARRAY[%s])))"%(chain,', '.join("'%s'::text"%k for k in kinds))
            assert ('audit_events',name,'RESTRICTIVE',[role],'INSERT',None,check) in policies,(role,policies)
            assert ('audit_heads',name,'RESTRICTIVE',[role],'UPDATE',"(writer_role = '%s'::text)"%chain,None) in policies,(role,policies)
        for role,(chain,kinds) in APPLICATION_CHAIN_KINDS.items():
            name=chain+'_chain'
            check="((writer_role = '%s'::text) AND (kind = ANY (ARRAY[%s])))"%(chain,', '.join("'%s'::text"%k for k in kinds))
            assert ('audit_events',name,'RESTRICTIVE',[role],'INSERT',None,check) in policies,(role,policies)
            assert ('audit_heads',name,'RESTRICTIVE',[role],'UPDATE',"(writer_role = '%s'::text)"%chain,None) in policies,(role,policies)
            # D7.3 "audit_heads (own chain)": the head read is bound to the chain too.
            assert ('audit_heads',name+'_read','RESTRICTIVE',[role],'SELECT',"(writer_role = '%s'::text)"%chain,None) in policies,(role,policies)
        assert len(policies)==13,policies
        targets=lambda table,cmd: {r[3][0] for r in policies if r[0]==table and r[4]==cmd}
        writers={'genui_admit','genui_owner','genui_workspace','genui_control','genui_data'}
        assert holders('audit_events','INSERT')==targets('audit_events','INSERT')==writers
        assert holders('audit_heads','UPDATE')==targets('audit_heads','UPDATE')==writers
        assert targets('audit_heads','SELECT')=={'genui_workspace','genui_control','genui_data'}
        # Insert-only for every role (D7.8), the owner included.
        assert holders('audit_events','UPDATE')==holders('audit_events','DELETE')==holders('audit_events','TRUNCATE')==set()
    passed('every INSERT holder on audit_events (genui_admit, genui_owner, since 0005 genui_workspace and genui_control, since 0007 genui_data) has its RESTRICTIVE chain/kind policy and its own-chain head UPDATE policy; the three application roles also read only their own head',
           policies=[list(p[:5]) for p in policies])
    # D7.8 (as amended, 0003 review m4 / APP-073 review M2): every chain hash is genui.digest.
    with pg.connect('postgres') as c:
        typed=c.execute("""SELECT c.relname,a.attname,format_type(a.atttypid,a.atttypmod) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND (c.relname,a.attname) IN
          (('audit_events','previous_hash'),('audit_events','hash'),('audit_heads','head_hash'),('audit_events','outcome_digest')) ORDER BY 1,2""").fetchall()
        assert typed==[('audit_events','hash','genui.digest'),('audit_events','outcome_digest','genui.digest'),
                       ('audit_events','previous_hash','genui.digest'),('audit_heads','head_hash','genui.digest')],typed
    passed('audit_events.previous_hash, audit_events.hash and audit_heads.head_hash are typed genui.digest (D7.8)',columns=[list(r) for r in typed])

    # --- Operator fixture (as the D7.19 operator channel would write it) ---
    with pg.connect() as c:
        for org in ORGANIZATIONS:
            operator(c,org)
            for chain in ('admission','operator'):
                c.execute('INSERT INTO genui.audit_heads VALUES (%s,%s,1,%s)',(org,chain,genesis(org,chain)))
        operator(c,org0)
        c.execute("INSERT INTO genui_private.handle_index VALUES (%s,%s,NULL),(%s,%s,now()-interval '1 hour'),(%s,%s,now()+interval '1 hour'),(%s,%s,NULL)",
                  (sha('handle-live-0'),org0,sha('handle-retired-0'),org0,sha('handle-rotating-0'),org0,sha('handle-live-1'),org1))
    # APP-073 review M2: malformed chain hashes are refused by the column type even
    # for the operator's own chain (the head row exists and its policy admits it).
    for label,value in (('a non-digest','not-a-digest'),('upper-case hex','sha256:'+'A'*64),('a short digest','sha256:'+'0'*63)):
        denied('operator head write refuses '+label,'genui_migration',
               "UPDATE genui.audit_heads SET head_hash=%s WHERE organization_id=%s AND writer_role='operator'",(value,org0),expected='23514')
        denied('operator event write refuses '+label+' previous_hash','genui_migration',
               """INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,occurred_at,previous_hash,hash)
               VALUES (%s,%s,'operator',2,'code.issued',now(),%s,%s)""",(org0,str(uuid.uuid4()),value,sha('m2-hash')),expected='23514')
        denied('operator event write refuses '+label+' hash','genui_migration',
               """INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,occurred_at,previous_hash,hash)
               VALUES (%s,%s,'operator',2,'code.issued',now(),%s,%s)""",(org0,str(uuid.uuid4()),genesis(org0,'operator'),value),expected='23514')
    codes={}
    def code(org, label, max_redemptions=1, status='active', redeemed=0, issued='now()', expires="now()+interval '14 days'", templates=None):
        digest=sha('code-'+label); code_id=str(uuid.uuid4())
        with pg.connect() as c:
            operator(c,org)
            c.execute('INSERT INTO genui_private.code_index VALUES (%s,%s)',(digest,org))
            c.execute(f"""INSERT INTO genui.access_codes(organization_id,code_id,code_digest,kind,status,destination_organization_id,max_redemptions,
              redeemed_count,issued_at,expires_at,trial_days,issued_by,revoked_at) VALUES (%s,%s,%s,'trial',%s,%s,%s,%s,{issued},{expires},30,'qualification-operator',
              CASE WHEN %s='revoked' THEN now() END)""",(org,code_id,digest,status,org,max_redemptions,redeemed,status))
            for permission,app,work,source in (templates if templates is not None else
                    ([('discover',APP,None,None),('run',APP,None,None),('record_read',APP,WORK,None),('source_read',None,None,source0)] if org==org0
                     else [('discover',app1,None,None),('record_read',app1,work1,None)])):
                c.execute('INSERT INTO genui.code_grant_templates VALUES (%s,%s,%s,%s,%s,%s)',(org,code_id,permission,app,work,source))
        codes[label]=(code_id,digest)
        return code_id,digest
    def code_state(org, label):
        with pg.connect('postgres') as c:
            return c.execute('SELECT redeemed_count,status FROM genui.access_codes WHERE organization_id=%s AND code_id=%s',(org,codes[label][0])).fetchone()
    def row_counts(org):
        with pg.connect('postgres') as c:
            return {t:c.execute(sql.SQL('SELECT count(*) FROM genui.{} WHERE organization_id=%s').format(sql.Identifier(t)),(org,)).fetchone()[0]
                    for t in ('principals','memberships','app_permissions','work_permissions','source_permissions','entitlements',
                              'code_reservations','code_redemptions','audit_events')}
    # D7.17 and template constraints refuse malformed issuance even from the operator.
    with pg.connect() as c:
        operator(c,org0)
        c.execute('INSERT INTO genui_private.code_index VALUES (%s,%s)',(sha('code-constraint-probe'),org0))
    for label,statement,params in (
        ('max_redemptions above 10',"INSERT INTO genui.access_codes VALUES (%s,%s,%s,'trial','active',%s,11,0,now(),now()+interval '1 day',30,'op',NULL)",None),
        ('expiry beyond 30 days',"INSERT INTO genui.access_codes VALUES (%s,%s,%s,'trial','active',%s,1,0,now(),now()+interval '31 days',30,'op',NULL)",None),
        ('foreign destination',"INSERT INTO genui.access_codes VALUES (%s,%s,%s,'trial','active','"+org1+"',1,0,now(),now()+interval '1 day',30,'op',NULL)",'dest'),
        ('trial other than 30 days',"INSERT INTO genui.access_codes VALUES (%s,%s,%s,'trial','active',%s,1,0,now(),now()+interval '1 day',7,'op',NULL)",None),
    ):
        args=(org0,str(uuid.uuid4()),sha('code-constraint-probe')) if params else (org0,str(uuid.uuid4()),sha('code-constraint-probe'),org0)
        denied('operator issuance refuses '+label,'genui_migration',statement,args,expected='23514')
    code(org0,'template-probe')
    for permission,app,work,source in (('admin_policy',None,None,None),('review_publish',APP,None,None),('record_read',APP,None,None),('source_read',APP,None,source0)):
        denied('grant template refuses '+permission+(' without its exact resource shape' if permission in ('record_read','source_read') else ''),'genui_migration',
               'INSERT INTO genui.code_grant_templates VALUES (%s,%s,%s,%s,%s,%s)',(org0,codes['template-probe'][0],permission,app,work,source),expected='23514')

    # --- code_index/handle_index reachable only through their functions (D7.2) ---
    for role in APP_ROLES:
        for table in ('genui_private.code_index','genui_private.handle_index'):
            denied(role+' cannot read '+table+' directly',role,'SELECT organization_id FROM '+table)
        for table in ADMISSION_TABLES:
            if table=='entitlements' and role!='genui_worker':
                # 0004 grants the precheck columns only (D7.3); every other column stays unreadable.
                denied(role+' cannot read entitlements.kind',role,'SELECT kind FROM genui.entitlements')
                denied(role+' cannot read entitlements.revoked_at',role,'SELECT revoked_at FROM genui.entitlements')
                continue
            if table=='audit_heads' and role in APPLICATION_CHAIN_KINDS:
                # 0005 (D7.3): workspace and control read their own chain's head
                # only; definition_work_checks proves it once those heads exist.
                with pg.connect(role) as c:
                    scope(c,org0)
                    assert c.execute('SELECT count(*) FROM genui.audit_heads').fetchone()==(0,)
                    assert c.execute("UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s",(org0,)).rowcount==0
                    c.rollback()
                passed(role+' sees and advances no admission or operator head (0005 own-chain binding)')
                continue
            if table=='code_reservations' and role=='genui_workspace':
                # 0006 (APP-009's pending lookup, an open D7.3 question): the
                # identity, kind, state and expiry columns only; never the code
                # id or the pre-login context digest.
                denied(role+' cannot read code_reservations.code_id',role,'SELECT code_id FROM genui.code_reservations')
                denied(role+' cannot read code_reservations.context_digest',role,'SELECT context_digest FROM genui.code_reservations')
                continue
            denied(role+' cannot read '+table+' directly',role,'SELECT organization_id FROM genui.'+table)
        denied(role+' cannot insert an access code directly',role,
               "INSERT INTO genui.access_codes(organization_id,code_id) VALUES (%s,%s)",(org0,str(uuid.uuid4())))
        denied(role+' cannot insert a reservation directly',role,
               "INSERT INTO genui.code_reservations(organization_id,reservation_id) VALUES (%s,%s)",(org0,str(uuid.uuid4())))
        denied(role+' cannot insert an entitlement directly',role,
               "INSERT INTO genui.entitlements(organization_id,principal_id,entitlement_id) VALUES (%s,%s,%s)",(org0,P,str(uuid.uuid4())))
        denied(role+' cannot insert an audit event directly',role,
               "INSERT INTO genui.audit_events(organization_id,event_id) VALUES (%s,%s)",(org0,str(uuid.uuid4())))
        if role not in APPLICATION_CHAIN_KINDS:
            denied(role+' cannot advance an audit head directly',role,
                   "UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s",(org0,))
        denied(role+' cannot UPDATE membership epoch directly',role,
               "UPDATE genui.memberships SET membership_epoch=membership_epoch WHERE organization_id=%s",(org0,))
        denied(role+' cannot SET ROLE genui_admit',role,'SET ROLE genui_admit')
        if role!='genui_workspace':
            for name,(arguments,_) in ADMIT_FUNCTIONS.items():
                # Non-NULL arguments: a STRICT call on NULL constants folds away before the EXECUTE check.
                values=','.join("'%s'::%s"%(str(uuid.uuid4()) if kind=='uuid' else 'true' if kind=='boolean' else '1' if kind=='bigint' else 'x',kind) for kind in (a.split()[1] for a in arguments.split(', ')))
                denied(role+' cannot execute '+name,role,'SELECT genui_private.%s(%s)'%(name,values))

    # --- resolve_code / resolve_handle before any scope ---
    code(org1,'org1-main')
    code(org0,'main')
    with pg.connect('genui_workspace') as c:
        assert c.execute("SELECT current_setting('genui.organization_id',true)").fetchone() in (('',),(None,))
        resolved=lambda f,d: c.execute(f'SELECT genui_private.{f}(%s)::text',(d,)).fetchone()[0]
        assert resolved('resolve_code',codes['main'][1])==org0 and resolved('resolve_code',codes['org1-main'][1])==org1
        assert resolved('resolve_code',sha('code-unknown')) is None and resolved('resolve_code','not-a-digest') is None
        assert resolved('resolve_handle',sha('handle-live-0'))==org0 and resolved('resolve_handle',sha('handle-live-1'))==org1
        assert resolved('resolve_handle',sha('handle-rotating-0'))==org0
        assert resolved('resolve_handle',sha('handle-retired-0')) is None and resolved('resolve_handle',sha('handle-unknown')) is None
    passed('resolve_code/resolve_handle before scope: known digests map to their organization; unknown, malformed and retired return NULL')

    # --- reserve_code outcomes (D7.1, D7.17) ---
    code(org0,'expired',issued="now()-interval '20 days'",expires="now()-interval '1 day'")
    code(org0,'revoked',status='revoked')
    code(org0,'exhausted',status='exhausted',redeemed=1)
    code(org0,'many',max_redemptions=10)
    assert outcome(reserve(org0,codes['expired'][1]))==['expired']
    assert outcome(reserve(org0,codes['revoked'][1]))==['revoked']
    assert outcome(reserve(org0,codes['exhausted'][1]))==['exhausted']
    assert reserve(org0,sha('code-unknown'))==[] and reserve(org0,codes['org1-main'][1])==[]
    assert call(org1,RESERVE,(org0,codes['main'][1],sha('ctx')))==[] and call(org0,RESERVE,(org0,codes['main'][1],'malformed'))==[]
    reserved=[reserve(org0,codes['many'][1]) for _ in range(10)]
    assert all(outcome(r)==['reserved'] for r in reserved)
    assert outcome(reserve(org0,codes['many'][1]))==['too_many_reservations']
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT kind,state,consumed_at IS NULL,expires_at>now()+interval '9 minutes' AND expires_at<=now()+interval '10 minutes',
          issuer IS NULL FROM genui.code_reservations WHERE organization_id=%s AND code_id=%s""",(org0,codes['many'][0])).fetchall()
        assert rows==[('presignup','live',True,True,True)]*10,rows
    assert code_state(org0,'many')==(0,'active')
    owner_sql(org0,"UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE organization_id=%s AND reservation_id=%s",(org0,reserved[0][0][0]))
    assert outcome(reserve(org0,codes['many'][1]))==['reserved']
    passed('reserve_code: reserved, expired, revoked, exhausted, too_many_reservations at 10 live (expired ones not counted); unknown, foreign, unscoped and malformed refuse with no row; presignup never counts')

    # --- record_pending_admission outcomes and D7.17 arithmetic ---
    a=person(org0,'a'); b=person(org0,'b')
    r_a=reserve(org0,codes['main'][1])[0][0]; r_b=reserve(org0,codes['main'][1])[0][0]
    before=row_counts(org0)
    p_a=call(org0,PENDING,(org0,r_a,*a))
    assert outcome(p_a)==['pending']
    p_a=p_a[0][0]
    after=row_counts(org0)
    assert {k:after[k]-before[k] for k in after}=={**{k:0 for k in after},'code_reservations':1,'audit_events':1}
    assert outcome(call(org0,PENDING,(org0,r_a,*b)))==['reservation_expired']
    assert outcome(call(org0,PENDING,(org0,str(uuid.uuid4()),*b)))==['reservation_expired']
    # Single-use code: one live pending admission leaves remaining = 0.
    assert outcome(call(org0,PENDING,(org0,r_b,*b)))==['exhausted']
    assert outcome(reserve(org0,codes['main'][1]))==['exhausted']
    assert code_state(org0,'main')==(0,'active')
    with pg.connect('postgres') as c:
        assert c.execute("""SELECT kind,state,consumed_at IS NOT NULL FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s""",(org0,r_a)).fetchone()==('presignup','consumed',True)
        assert c.execute("""SELECT kind,state,issuer,provider_tenant_id,provider_subject,expires_at>now()+interval '23 hours' AND expires_at<=now()+interval '24 hours'
          FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s""",(org0,p_a)).fetchone()==('pending','live',*a,True)
    code(org0,'member-check',max_redemptions=2)
    r_m=reserve(org0,codes['member-check'][1])[0][0]
    before=row_counts(org0)
    assert outcome(call(org0,PENDING,(org0,r_m,identity[org0][0],identity[org0][1],seeded_subject)))==['already_member']
    assert row_counts(org0)==before
    with pg.connect('postgres') as c:
        assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,r_m)).fetchone()==('live',)
    code(org0,'late-expiry',issued="now()-interval '2 days'"); code(org0,'late-revoke')
    r_x=reserve(org0,codes['late-expiry'][1])[0][0]; r_y=reserve(org0,codes['late-revoke'][1])[0][0]; r_z=reserve(org0,codes['member-check'][1])[0][0]
    owner_sql(org0,"UPDATE genui.access_codes SET expires_at=now()-interval '1 second' WHERE organization_id=%s AND code_id=%s",(org0,codes['late-expiry'][0]))
    owner_sql(org0,"UPDATE genui.access_codes SET status='revoked',revoked_at=now() WHERE organization_id=%s AND code_id=%s",(org0,codes['late-revoke'][0]))
    owner_sql(org0,"UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE organization_id=%s AND reservation_id=%s",(org0,r_z))
    assert outcome(call(org0,PENDING,(org0,r_x,*person(org0,'x'))))==['expired']
    assert outcome(call(org0,PENDING,(org0,r_y,*person(org0,'y'))))==['revoked']
    assert outcome(call(org0,PENDING,(org0,r_z,*person(org0,'z'))))==['reservation_expired']
    assert call(org1,PENDING,(org0,r_m,*person(org0,'w')))==[]
    # Review m3: malformed identity input is a storage error and returns no row, never 23514/23503.
    before=row_counts(org0)
    for bad in ((identity[org0][0],identity[org0][1],'s'*257),(identity[org0][0],identity[org0][1],''),
                ('',identity[org0][1],'usr_m3'),(identity[org0][0],'t'*257,'usr_m3')):
        assert call(org0,PENDING,(org0,r_m,*bad))==[]
    assert row_counts(org0)==before
    # APP-073 review m3(a): one identity holds at most one live pending admission per
    # code; a repeated callback consumes its new presignup and answers the live one.
    code(org0,'repeat',max_redemptions=3)
    repeat=person(org0,'repeat')
    r_r1=reserve(org0,codes['repeat'][1])[0][0]; r_r2=reserve(org0,codes['repeat'][1])[0][0]
    first=call(org0,PENDING,(org0,r_r1,*repeat))
    assert outcome(first)==['pending']
    before=row_counts(org0)
    assert call(org0,PENDING,(org0,r_r2,*repeat))==first
    assert row_counts(org0)==before
    with pg.connect('postgres') as c:
        assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,r_r2)).fetchone()==('consumed',)
        assert c.execute("""SELECT count(*) FROM genui.code_reservations WHERE organization_id=%s AND code_id=%s AND kind='pending' AND state='live'""",
                         (org0,codes['repeat'][0])).fetchone()==(1,)
    passed('record_pending_admission: pending (24 h, identity-bound, reservation consumed, one audit event), reservation_expired, expired, revoked, exhausted by D7.17 arithmetic, already_member writes nothing; a repeated pending admission of one identity on one code returns the live one with no new row or event; foreign scope refuses')

    # --- admit_redemption outcomes ---
    payload=sha('payload-a')
    before=row_counts(org0)
    admitted=call(org0,ADMIT,(org0,p_a,*a,payload))
    assert [r[1:2]+r[4:] for r in admitted]==[(1,'admitted')]
    principal_a,_,entitlement_a,redemption_a,_=admitted[0]
    after=row_counts(org0)
    assert {k:after[k]-before[k] for k in after}=={'principals':1,'memberships':1,'app_permissions':2,'work_permissions':1,'source_permissions':1,
        'entitlements':1,'code_reservations':0,'code_redemptions':1,'audit_events':1}
    # Single-use code with exactly one pending admission is admitted, then exhausted.
    assert code_state(org0,'main')==(1,'exhausted')
    with pg.connect('postgres') as c:
        assert c.execute('SELECT issuer,provider_tenant_id,provider_subject FROM genui.principals WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchone()==a
        assert c.execute('SELECT status,membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchone()==('active',1)
        assert sorted(c.execute('SELECT app_id::text,permission FROM genui.app_permissions WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchall())==[(APP,'discover'),(APP,'run')]
        assert c.execute('SELECT app_id::text,work_id::text,permission FROM genui.work_permissions WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchall()==[(APP,WORK,'record_read')]
        assert c.execute('SELECT source_id::text,permission FROM genui.source_permissions WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchall()==[(source0,'source_read')]
        assert c.execute('SELECT count(*) FROM genui.organization_permissions WHERE organization_id=%s AND principal_id=%s',(org0,principal_a)).fetchone()==(0,)
        assert c.execute("""SELECT entitlement_id::text,status,kind,expires_at-starts_at,revoked_at FROM genui.entitlements
          WHERE organization_id=%s AND principal_id=%s""",(org0,principal_a)).fetchone()[:4]==(entitlement_a,'active','trial',
            c.execute("SELECT interval '30 days'").fetchone()[0])
        red=c.execute("""SELECT d.redemption_id::text,d.code_id::text,d.pending_admission_id::text,d.payload_digest::text,e.kind,e.actor_principal_id::text,e.outcome_digest::text
          FROM genui.code_redemptions d JOIN genui.audit_events e ON e.organization_id=d.organization_id AND e.event_id=d.audit_event_id
          WHERE d.organization_id=%s AND d.principal_id=%s""",(org0,principal_a)).fetchall()
        assert red==[(redemption_a,codes['main'][0],p_a,payload,'admission.redeemed',principal_a,payload)],red
        assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,p_a)).fetchone()==('consumed',)
    before=row_counts(org0)
    assert call(org0,ADMIT,(org0,p_a,*a,payload))==[(principal_a,1,entitlement_a,redemption_a,'replayed')]
    assert outcome(call(org0,ADMIT,(org0,p_a,*a,sha('payload-changed'))))==['mismatch']
    assert outcome(call(org0,ADMIT,(org0,p_a,*b,payload)))==['mismatch']
    assert outcome(call(org0,ADMIT,(org0,str(uuid.uuid4()),*a,payload)))==['mismatch']
    assert call(org1,ADMIT,(org1,p_a,*a,payload))==[(None,None,None,None,'mismatch')]
    assert call(org1,ADMIT,(org0,p_a,*a,payload))==[] and call(org0,ADMIT,(org0,p_a,*a,'malformed'))==[]
    assert call(org0,ADMIT,(org0,p_a,a[0],a[1],'s'*257,payload))==[] and call(org0,ADMIT,(org0,p_a,'',a[1],a[2],payload))==[]
    assert row_counts(org0)==before and code_state(org0,'main')==(1,'exhausted')
    passed('admit_redemption admitted: principal, epoch-1 membership, verbatim template grants, 30-day entitlement, redemption and one audit event in one call; single-use code with one pending admission admitted then exhausted; replay returns the stored result; changed payload, other identity, unknown and foreign pending refuse with no write')

    def pending_for(org, label, who):
        r=reserve(org,codes[label][1])[0][0]
        rows=call(org,PENDING,(org,r,*who)); assert outcome(rows)==['pending'],rows
        return rows[0][0]
    code(org0,'two',max_redemptions=2)
    c_1=person(org0,'c1'); c_2=person(org0,'c2')
    pc_1=pending_for(org0,'two',c_1); pc_2=pending_for(org0,'two',c_2)
    assert outcome(call(org0,ADMIT,(org0,pc_1,*c_1,sha('p-c1'))))==['admitted'] and code_state(org0,'two')==(1,'active')
    assert outcome(call(org0,ADMIT,(org0,pc_2,*c_2,sha('p-c2'))))==['admitted'] and code_state(org0,'two')==(2,'exhausted')
    code(org0,'member-first'); code(org0,'member-second')
    d=person(org0,'d')
    pd_1=pending_for(org0,'member-first',d); pd_2=pending_for(org0,'member-second',d)
    assert outcome(call(org0,ADMIT,(org0,pd_1,*d,sha('p-d'))))==['admitted']
    before=row_counts(org0)
    assert outcome(call(org0,ADMIT,(org0,pd_2,*d,sha('p-d'))))==['already_member']
    assert row_counts(org0)==before and code_state(org0,'member-second')==(0,'active')
    for label in ('admit-expired','admit-revoked','admit-code-expired','admit-exhausted'):
        code(org0,label,issued="now()-interval '2 days'")
    e=person(org0,'e'); f=person(org0,'f'); g=person(org0,'g'); h=person(org0,'h')
    pe=pending_for(org0,'admit-expired',e); pf=pending_for(org0,'admit-revoked',f)
    pg_=pending_for(org0,'admit-code-expired',g); ph=pending_for(org0,'admit-exhausted',h)
    owner_sql(org0,"UPDATE genui.code_reservations SET expires_at=now()-interval '1 second' WHERE organization_id=%s AND reservation_id=%s",(org0,pe))
    owner_sql(org0,"UPDATE genui.access_codes SET status='revoked',revoked_at=now() WHERE organization_id=%s AND code_id=%s",(org0,codes['admit-revoked'][0]))
    owner_sql(org0,"UPDATE genui.access_codes SET expires_at=now()-interval '1 second' WHERE organization_id=%s AND code_id=%s",(org0,codes['admit-code-expired'][0]))
    # Operator-forced exhaustion (not reachable through the functions) proves the recheck at commit.
    owner_sql(org0,"UPDATE genui.access_codes SET redeemed_count=1,status='exhausted' WHERE organization_id=%s AND code_id=%s",(org0,codes['admit-exhausted'][0]))
    before=row_counts(org0)
    assert outcome(call(org0,ADMIT,(org0,pe,*e,sha('p-e'))))==['expired']
    after=row_counts(org0)
    assert {k:after[k]-before[k] for k in after}=={**{k:0 for k in after},'audit_events':1}
    with pg.connect('postgres') as c:
        assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,pe)).fetchone()==('denied_for_reconciliation',)
    assert outcome(call(org0,ADMIT,(org0,pe,*e,sha('p-e'))))==['expired']
    # Review M1 (ADR-006 Flow 2): a code-side refusal also denies the pending
    # admission for reconciliation with one audit event; a retry answers expired.
    for pending_id,who,label,expected,code_label in ((pf,f,'p-f','revoked','admit-revoked'),(pg_,g,'p-g','expired','admit-code-expired'),
                                                     (ph,h,'p-h','exhausted','admit-exhausted')):
        before=row_counts(org0); state=code_state(org0,code_label)
        assert outcome(call(org0,ADMIT,(org0,pending_id,*who,sha(label))))==[expected]
        after=row_counts(org0)
        assert {k:after[k]-before[k] for k in after}=={**{k:0 for k in after},'audit_events':1},(label,after)
        with pg.connect('postgres') as c:
            assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,pending_id)).fetchone()==('denied_for_reconciliation',)
            assert c.execute("""SELECT kind,actor_principal_id,code_id::text,outcome_digest FROM genui.audit_events WHERE organization_id=%s
              AND writer_role='admission' ORDER BY chain_sequence DESC LIMIT 1""",(org0,)).fetchone()==('admission.denied_for_reconciliation',None,codes[code_label][0],None)
        assert outcome(call(org0,ADMIT,(org0,pending_id,*who,sha(label))))==['expired']
        assert row_counts(org0)==after and code_state(org0,code_label)==state
    passed('admit_redemption: exhausted status at max (2 of 2), already_member consumes nothing; an expired pending admission and one whose code was revoked, expired or (operator-forced) exhausted are each marked denied_for_reconciliation with one audit event and no grant, slot or count change; retries answer expired')

    # --- Concurrency: observed lock waits (pg_stat_activity), never timing guesses ---
    def blocked(first, second, holder_role='genui_workspace'):
        """first(conn) runs in an open transaction; second(conn) must wait on its lock until first commits."""
        with pg.connect(holder_role) as one, pg.connect('genui_workspace') as two, pg.connect('postgres',autocommit=True) as observer:
            held=first(one); out=[]; errors=[]
            def run():
                try: out.append(second(two)); two.commit()
                except BaseException as error: errors.append(repr(error))
            thread=threading.Thread(target=run); thread.start(); deadline=time.monotonic()+3
            while time.monotonic()<deadline:
                if observer.execute('SELECT wait_event_type FROM pg_stat_activity WHERE pid=%s',(two.info.backend_pid,)).fetchone()==('Lock',): break
                if not thread.is_alive(): raise AssertionError('second transaction did not block: '+repr(errors or out))
                time.sleep(.01)
            else: raise AssertionError('lock-wait observation deadline')
            one.commit(); thread.join(4); assert not thread.is_alive() and not errors,errors
            return held,out[0]
    def in_scope(org, statement, params, member=None):
        def run(c):
            if member is None: org_scope(c,org)
            else: scope(c,org,member[0],member[1])
            return c.execute(statement,params).fetchall()
        return run
    # (1) Two identities race record_pending_admission for the last slot of a single-use code.
    code(org0,'race-pending')
    i=person(org0,'i'); j=person(org0,'j')
    r_i=reserve(org0,codes['race-pending'][1])[0][0]; r_j=reserve(org0,codes['race-pending'][1])[0][0]
    held,waited=blocked(in_scope(org0,PENDING,(org0,r_i,*i)),in_scope(org0,PENDING,(org0,r_j,*j)))
    assert (outcome(held),outcome(waited))==(['pending'],['exhausted'])
    # (2) Last slot: a redemption and a new pending admission race in both orders.
    for order in ('admit-first','pending-first'):
        label='race-slot-'+order
        code(org0,label,max_redemptions=2)
        k=person(org0,'k-'+order); l=person(org0,'l-'+order); m=person(org0,'m-'+order)
        pk=pending_for(org0,label,k)
        assert outcome(call(org0,ADMIT,(org0,pk,*k,sha('p-k'))))==['admitted']
        # Both presignup reservations are taken while one slot remains.
        r_l=reserve(org0,codes[label][1])[0][0]; r_m=reserve(org0,codes[label][1])[0][0]
        pl=call(org0,PENDING,(org0,r_l,*l)); assert outcome(pl)==['pending']; pl=pl[0][0]
        assert outcome(reserve(org0,codes[label][1]))==['exhausted']  # remaining = 2 - 1 - 1 live pending
        admit=in_scope(org0,ADMIT,(org0,pl,*l,sha('p-l')))
        pend=in_scope(org0,PENDING,(org0,r_m,*m))
        if order=='admit-first':
            held,waited=blocked(admit,pend); assert (outcome(held),outcome(waited))==(['admitted'],['exhausted'])
        else:
            held,waited=blocked(pend,admit); assert (outcome(held),outcome(waited))==(['exhausted'],['admitted'])
        assert code_state(org0,label)==(2,'exhausted')
    # (3) Two concurrent identical redemptions of one pending admission: one admits, one replays.
    code(org0,'race-admit')
    n=person(org0,'n'); pn=pending_for(org0,'race-admit',n)
    before=row_counts(org0)
    held,waited=blocked(in_scope(org0,ADMIT,(org0,pn,*n,sha('p-n'))),in_scope(org0,ADMIT,(org0,pn,*n,sha('p-n'))))
    assert outcome(held)==['admitted'] and waited==[held[0][:4]+('replayed',)]
    after=row_counts(org0)
    assert after['code_redemptions']-before['code_redemptions']==1 and after['principals']-before['principals']==1
    assert code_state(org0,'race-admit')==(1,'exhausted')
    # (4) Review M2: one identity admitted concurrently through two codes; the
    # loser waits on the principals unique key and answers already_member.
    code(org0,'race-two-codes-a'); code(org0,'race-two-codes-b')
    t=person(org0,'t')
    pt_a=pending_for(org0,'race-two-codes-a',t); pt_b=pending_for(org0,'race-two-codes-b',t)
    before=row_counts(org0)
    held,waited=blocked(in_scope(org0,ADMIT,(org0,pt_a,*t,sha('p-t'))),in_scope(org0,ADMIT,(org0,pt_b,*t,sha('p-t'))))
    assert (outcome(held),outcome(waited))==(['admitted'],['already_member'])
    after=row_counts(org0)
    assert after['principals']-before['principals']==1 and after['code_redemptions']-before['code_redemptions']==1
    assert code_state(org0,'race-two-codes-b')==(0,'active')
    with pg.connect('postgres') as c:
        assert c.execute('SELECT state FROM genui.code_reservations WHERE organization_id=%s AND reservation_id=%s',(org0,pt_b)).fetchone()==('live',)
    passed('concurrent last-slot races admit exactly one: two pending admissions (one pending, one exhausted), redemption versus new pending admission in both orders, two identical redemptions (admitted, replayed) with one slot consumed, and one identity through two codes (admitted, already_member with no write)')

    # --- advance_member_epoch (D7.1, D7.9 session lock order) ---
    principal_c1=call(org0,'SELECT principal_id::text FROM genui_private.admit_redemption(%s,%s,%s,%s,%s,%s)',(org0,pc_1,*c_1,sha('p-c1')))[0][0]
    def epoch(principal):
        with pg.connect('postgres') as c:
            return c.execute('SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(org0,principal)).fetchone()[0]
    assert call(org0,ADVANCE,(org0,principal_a,'session.sign_out'),member=(principal_a,1))==[(2,)] and epoch(principal_a)==2
    before=row_counts(org0)
    assert call(org0,ADVANCE,(org0,principal_c1,'session.sign_out'),member=(principal_a,2))==[(None,)]
    assert call(org0,ADVANCE,(org0,principal_a,'session.sign_out'),member=(principal_a,1))==[(None,)]
    for reason in ('membership.revoked','session.signed_in','admission.redeemed'):
        assert call(org0,ADVANCE,(org0,principal_a,reason),member=(principal_a,2))==[(None,)]
    assert call(org1,ADVANCE,(org0,principal_a,'session.sign_out'),member=(principal_a,2))==[(None,)]
    assert row_counts(org0)==before and epoch(principal_a)==2 and epoch(principal_c1)==1
    assert call(org0,ADVANCE,(org0,principal_a,'session.sign_out_all'),member=(principal_a,2))==[(3,)]
    # The operator's revocation order holds the member row: the sign-out waits, then refuses the moved epoch.
    def revoke(c):
        operator(c,org0)
        c.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE',(org0,))
        c.execute('SELECT principal_id FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR UPDATE',(org0,principal_a))
        c.execute('UPDATE genui.memberships SET membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s',(org0,principal_a))
    _,waited=blocked(revoke,in_scope(org0,ADVANCE,(org0,principal_a,'session.sign_out'),member=(principal_a,3)),holder_role='genui_migration')
    assert waited==[(None,)] and epoch(principal_a)==4
    passed('advance_member_epoch: own epoch advances with one audit event per sign-out reason; another principal, stale epoch, foreign scope and any other reason refuse with no write; it waits on the member row and refuses after a concurrent epoch move')

    # --- Org 1 admission and audit chain verification (D7.8) ---
    o=person(org1,'o'); r_o=reserve(org1,codes['org1-main'][1])[0][0]
    po=call(org1,PENDING,(org1,r_o,*o))[0][0]
    assert outcome(call(org1,ADMIT,(org1,po,*o,sha('p-o'))))==['admitted']
    with pg.connect('postgres') as c:
        chains={org:verify_chain(c,org,'admission') for org in ORGANIZATIONS}
        assert verify_chain(c,org0,'operator')==[] and verify_chain(c,org1,'operator')==[]
        # Positive control: a tampered stored kind no longer verifies.
        assert c.execute("""UPDATE genui.audit_events SET kind=CASE kind WHEN 'admission.pending' THEN 'admission.redeemed' ELSE 'admission.pending' END
          WHERE organization_id=%s AND writer_role='admission' AND chain_sequence=3""",(org0,)).rowcount==1
        try: verify_chain(c,org0,'admission')
        except AssertionError: pass
        else: raise AssertionError('tampered audit chain verified')
        c.rollback()
    assert chains[org1]==['admission.pending','admission.redeemed']
    assert chains[org0].count('admission.redeemed')>=8 and chains[org0].count('session.sign_out')==1 and chains[org0].count('session.sign_out_all')==1
    assert chains[org0].count('admission.denied_for_reconciliation')==4
    # Review m5: code_redemptions.audit_event_id has no FK (D7.8), so the set
    # check covers it: every redemption names its own admission.redeemed event.
    with pg.connect('postgres') as c:
        assert c.execute("""SELECT count(*) FROM genui.code_redemptions d WHERE NOT EXISTS (SELECT 1 FROM genui.audit_events e
          WHERE e.organization_id=d.organization_id AND e.event_id=d.audit_event_id AND e.writer_role='admission' AND e.kind='admission.redeemed'
          AND e.code_id=d.code_id AND e.actor_principal_id=d.principal_id AND e.outcome_digest=d.payload_digest)""").fetchone()==(0,)
        assert c.execute('SELECT count(*) FROM genui.code_redemptions').fetchone()[0]>=10
    passed('audit chains verify per organization: every hash recomputed in chain_sequence order from the genesis head, tail equals head; a tampered event fails verification; every redemption resolves to its own admission.redeemed event',
           admission_events={org:len(kinds) for org,kinds in chains.items()})
    # Mismatched writer_role/kind INSERTs are refused by the RESTRICTIVE policies.
    def event(org, writer, kind, sequence):
        return ("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,occurred_at,previous_hash,hash)
          VALUES (%s,%s,%s,%s,%s,now(),%s,%s)""",(org,str(uuid.uuid4()),writer,sequence,kind,sha('prev-'+uuid.uuid4().hex),sha('h')))
    for writer,kind in (('workspace','session.signed_in'),('operator','code.issued'),('admission','code.issued'),('admission','work.saved')):
        statement,params=event(org0,writer,kind,10**6)
        denied('genui_admit INSERT refuses writer_role '+writer+' kind '+kind,'genui_migration',statement,params,set_role='genui_admit')
    for writer,kind in (('admission','admission.redeemed'),('workspace','session.signed_in'),('operator','admission.pending')):
        statement,params=event(org0,writer,kind,10**6)
        denied('operator INSERT refuses writer_role '+writer+' kind '+kind,'genui_migration',statement,params)
    for role,writer,kind in (('genui_admit','admission','session.sign_out'),('genui_owner','operator','code.issued')):
        with pg.connect() as c:
            c.execute(sql.SQL('SET LOCAL ROLE {}').format(sql.Identifier(role))); org_scope(c,org0)
            statement,params=event(org0,writer,kind,10**6); c.execute(statement,params)
            # Own-chain head only: the other chain's head row is invisible to UPDATE.
            other='operator' if writer=='admission' else 'admission'
            assert c.execute('UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s AND writer_role=%s',(org0,other)).rowcount==0
            assert c.execute('UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s AND writer_role=%s',(org0,writer)).rowcount==1
            c.rollback()
    for statement in ('UPDATE genui.audit_events SET kind=kind','DELETE FROM genui.audit_events'):
        denied('genui_admit cannot '+statement.split()[0]+' audit events','genui_migration',statement,set_role='genui_admit')
    for statement in ('UPDATE genui.audit_events SET kind=kind','DELETE FROM genui.audit_events','TRUNCATE genui.audit_events'):
        denied('operator cannot '+statement.split()[0]+' audit events','genui_migration',statement)
    passed('RESTRICTIVE chain bindings: mismatched writer_role or kind refuses for genui_admit and the operator, matching rows are accepted, head UPDATE reaches only the own chain, audit events are never updated or deleted')

    # --- A4 negative test: no function call wrote genui.organizations ---
    with pg.connect('postgres') as c:
        assert c.execute('SELECT organization_id::text,xmin::text,ctid::text,status FROM genui.organizations ORDER BY organization_id').fetchall()==organizations_before
    passed('no admission function call wrote genui.organizations: tuple xmin/ctid/status unchanged across every call (FOR SHARE locks only)')


def definition_work_checks(pg, passed, denied):
    """ADR-007 D7.3/D7.8 for migration 0005 (APP-010) on the actual local database.

    The storage-module transactions themselves (precheck, fence, admission,
    publication, saved work, restart) are the Rust definition campaign's; this
    proves the grant and chain-binding layer they rely on, as each role.
    """
    org0,org1=ORGANIZATIONS
    with pg.connect('postgres') as c:
        policies=c.execute("SELECT organization_id::text,policy_version,kind FROM genui.publication_policies WHERE policy_id=%s ORDER BY 1",
                           (OWNER_PUBLISH_POLICY,)).fetchall()
        assert policies==[(org,1,'owner_publish') for org in ORGANIZATIONS],policies
    passed('the operator seed (not a migration) gives each fixture organization its owner_publish policy row (plan Task 6)',policy_id=OWNER_PUBLISH_POLICY)
    # Genesis heads for the two application chains, as the operator backfill writes them (D7.7, D7.19).
    with pg.connect() as c:
        for org in ORGANIZATIONS:
            c.execute('SET LOCAL ROLE genui_owner')
            c.execute("SELECT set_config('genui.organization_id',%s,true)",(org,))
            for chain in ('workspace','control'):
                c.execute('INSERT INTO genui.audit_heads VALUES (%s,%s,1,%s) ON CONFLICT DO NOTHING',(org,chain,genesis(org,chain)))
            c.execute('RESET ROLE')
    def event(org, writer, kind):
        return ("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,occurred_at,previous_hash,hash)
          VALUES (%s,%s,%s,%s,%s,now(),%s,%s)""",(org,str(uuid.uuid4()),writer,10**6,kind,sha('prev-'+uuid.uuid4().hex),sha('h-'+uuid.uuid4().hex)))
    revision=str(uuid.uuid4())
    writes={
        'genui_workspace':[
            ("INSERT INTO genui.definition_revisions(organization_id,app_id,revision_id,schema_version,digest,canonical_content) VALUES (%s,%s,%s,1,%s,%s)",
             (org0,APP,revision,sha('draft'),b'{}')),
            ("INSERT INTO genui.definition_inputs(organization_id,app_id,revision_id,input_id,kind,contract_digest,canonical_contract) VALUES (%s,%s,%s,'events','source',%s,%s)",
             (org0,APP,revision,sha('contract'),b'{}')),
            ("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES (%s,%s,%s,%s,1,'grant probe',%s)",
             (org0,APP,str(uuid.uuid4()),PUB,P)),
            event(org0,'workspace','definition.drafted'),
            ("UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s AND writer_role='workspace'",(org0,)),
        ],
        'genui_control':[
            event(org0,'control','publication.published'),
            ("UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s AND writer_role='control'",(org0,)),
        ],
    }
    for role,statements in writes.items():
        chain=APPLICATION_CHAIN_KINDS[role][0]
        with pg.connect(role) as c:
            scope(c,org0)
            for statement,params in statements:
                assert c.execute(statement,params).rowcount==1,(role,statement)
            # D7.3 own chain: the role reads and locks only its own head.
            assert c.execute('SELECT writer_role FROM genui.audit_heads').fetchall()==[(chain,)]
            assert c.execute('SELECT writer_role FROM genui.audit_heads FOR UPDATE').fetchall()==[(chain,)]
            for other in ('workspace','control','data','admission','operator'):
                if other!=chain:
                    assert c.execute('UPDATE genui.audit_heads SET chain_sequence=chain_sequence WHERE organization_id=%s AND writer_role=%s',(org0,other)).rowcount==0
            c.rollback()
    passed('0005 grants: genui_workspace inserts drafts, its own saved work and workspace-chain events, genui_control inserts control-chain events; each reads, locks and advances only its own head (rolled back)')
    # ADR-007 amendment 2026-10-29 (review M1, M2): saved_work.created_by and
    # creator_revoked_at with the composite foreign key to memberships and the
    # workspace's RESTRICTIVE creator check; publications.publication_sequence
    # unique per (organization_id, app_id).
    with pg.connect('postgres') as c:
        columns=c.execute("""SELECT c.relname::text,a.attname::text,format_type(a.atttypid,a.atttypmod),a.attnotnull FROM pg_attribute a
          JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui'
          AND (c.relname::text,a.attname::text) IN (('saved_work','created_by'),('saved_work','creator_revoked_at'),('publications','publication_sequence'))
          ORDER BY 1,2""").fetchall()
        assert columns==[('publications','publication_sequence','genui.safe_counter',True),('saved_work','created_by','uuid',False),
                         ('saved_work','creator_revoked_at','timestamp with time zone',False)],columns
        keys=c.execute("""SELECT conrelid::regclass::text,contype::text,pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid IN
          ('genui.saved_work'::regclass,'genui.publications'::regclass) AND (pg_get_constraintdef(oid) LIKE '%created_by%'
          OR pg_get_constraintdef(oid) LIKE '%publication_sequence%') ORDER BY 1""").fetchall()
        assert keys==[('genui.publications','u','UNIQUE (organization_id, app_id, publication_sequence)'),
                      ('genui.saved_work','f','FOREIGN KEY (organization_id, created_by) REFERENCES genui.memberships(organization_id, principal_id)')],keys
        creator=c.execute("""SELECT policyname::text,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename='saved_work' AND policyname<>'tenant_scope'""").fetchall()
        assert creator==[('saved_work_creator','RESTRICTIVE',['genui_workspace'],'INSERT',None,
                          "(created_by = (NULLIF(current_setting('genui.principal_id'::text, true), ''::text))::uuid)")],creator
    passed('0005 creator authority and publication order (ADR-007 amendment): saved_work.created_by/creator_revoked_at with the composite memberships key and the workspace RESTRICTIVE creator check; publications.publication_sequence NOT NULL and unique per app')
    pub_row=("INSERT INTO genui.publications(organization_id,app_id,publication_id,definition_revision_id,definition_digest,policy_id,policy_version,policy_kind,author_id,approver_id,approver_epoch,renderer_version,operator_version,admission_digest,published_at,canonical_envelope) VALUES (%s,%s,%s,%s,%s,%s,1,'owner_publish',%s,%s,1,1,1,%s,now(),%s)",
             (org0,APP,str(uuid.uuid4()),'00000000-0000-4000-8000-000000000067',sha('d'),OWNER_PUBLISH_POLICY,P,P,sha('d'),b'{}'))
    draft_row=("INSERT INTO genui.definition_revisions(organization_id,app_id,revision_id,schema_version,digest,canonical_content) VALUES (%s,%s,%s,1,%s,%s)",
               (org0,APP,str(uuid.uuid4()),sha('draft'),b'{}'))
    work_row=("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context) VALUES (%s,%s,%s,%s,1,'x')",
              (org0,APP,str(uuid.uuid4()),PUB))
    for name,role,(statement,params) in (
            ('workspace cannot write publications','genui_workspace',pub_row),
            ('workspace cannot write publication inputs','genui_workspace',("INSERT INTO genui.publication_inputs(organization_id,app_id,publication_id,definition_revision_id,input_id,kind,contract_digest) VALUES (%s,%s,%s,%s,'events','source',%s)",(org0,APP,PUB,'00000000-0000-4000-8000-000000000067',sha('c')))),
            ('workspace cannot write publication policies','genui_workspace',("INSERT INTO genui.publication_policies(organization_id,policy_id,policy_version,kind) VALUES (%s,%s,1,'owner_publish')",(org0,str(uuid.uuid4())))),
            # Review m2: policy rows and app shells are operator writes only.
            ('control cannot write publication policies','genui_control',("INSERT INTO genui.publication_policies(organization_id,policy_id,policy_version,kind) VALUES (%s,%s,1,'independent_review')",(org0,str(uuid.uuid4())))),
            ('workspace cannot create an app shell','genui_workspace',("INSERT INTO genui.apps(organization_id,app_id,space_id,owner_principal_id) VALUES (%s,%s,'00000000-0000-4000-8000-000000000384',%s)",(org0,str(uuid.uuid4()),P))),
            # ADR-007 amendment: a saved work names its creator, and only as the scoped principal.
            ('workspace cannot save work for another creator','genui_workspace',("INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES (%s,%s,%s,%s,1,'x','00000000-0000-4000-8000-0000000000ca')",(org0,APP,str(uuid.uuid4()),PUB))),
            ('workspace cannot save work without a creator','genui_workspace',work_row),
            ('workspace cannot revoke creator authority','genui_workspace',("UPDATE genui.saved_work SET creator_revoked_at=now()",())),
            ('control cannot rewrite a publication sequence','genui_control',("UPDATE genui.publications SET publication_sequence=publication_sequence",())),
            ('control cannot write saved work','genui_control',work_row),
            ('control cannot write definition revisions','genui_control',draft_row),
            ('control cannot write work bindings','genui_control',("INSERT INTO genui.work_bindings(organization_id,app_id,work_id,publication_id,input_id,kind,contract_digest) VALUES (%s,%s,%s,%s,'x','source',%s)",(org0,APP,WORK,PUB,sha('c')))),
            ('data cannot write definition revisions','genui_data',draft_row),
            ('data cannot write publications','genui_data',pub_row),
            ('data cannot write saved work','genui_data',work_row),
            ('worker cannot write saved work','genui_worker',work_row),
            ('workspace cannot write permission rows','genui_workspace',("INSERT INTO genui.app_permissions(organization_id,principal_id,app_id,permission) VALUES (%s,%s,%s,'review_publish')",(org0,P,APP))),
            ('control cannot write permission rows','genui_control',("INSERT INTO genui.work_permissions(organization_id,principal_id,app_id,work_id,permission) VALUES (%s,%s,%s,%s,'export')",(org0,P,APP,WORK))),
            ('workspace cannot write the source ledger','genui_workspace',("INSERT INTO genui.sources(organization_id,source_id) VALUES (%s,%s)",(org0,str(uuid.uuid4())))),
            ('workspace cannot write another organization (FORCE RLS)','genui_workspace',(draft_row[0],(org1,)+draft_row[1][1:])),
            ('workspace cannot append to the control chain','genui_workspace',event(org0,'control','publication.published')),
            ('workspace cannot append a control kind','genui_workspace',event(org0,'workspace','publication.published')),
            ('workspace cannot append an admission kind','genui_workspace',event(org0,'workspace','admission.redeemed')),
            ('control cannot append to the workspace chain','genui_control',event(org0,'workspace','work.saved')),
            ('control cannot append a workspace kind','genui_control',event(org0,'control','definition.drafted')),
            ('data cannot append to the workspace chain (0007 binds it to the data chain)','genui_data',event(org0,'workspace','work.saved')),
            ('data cannot append a workspace kind to its own chain','genui_data',event(org0,'data','work.saved')),
            ('data cannot UPDATE audit events','genui_data',('UPDATE genui.audit_events SET kind=kind',())),
            ('workspace cannot UPDATE audit events','genui_workspace',('UPDATE genui.audit_events SET kind=kind',())),
            ('control cannot DELETE audit events','genui_control',('DELETE FROM genui.audit_events',())),
            ('workspace cannot rewrite a draft','genui_workspace',('UPDATE genui.definition_revisions SET digest=digest',())),
            ('control cannot rewrite a publication','genui_control',('UPDATE genui.publications SET admission_digest=admission_digest',())),
            ('workspace cannot delete saved work','genui_workspace',('DELETE FROM genui.saved_work',()))):
        denied('0005 '+name,role,statement,params)
    passed('0005 role separation: workspace never publishes, control never saves work, data/worker write neither, no role writes permission rows, rewrites or deletes drafts, publications, saved work or audit events, or appends to another chain or kind')


def session_checks(pg, passed, denied):
    """ADR-006 and ADR-007 D7.3/D7.5 for migration 0006 (APP-009) on the actual local database.

    The storage-module session transactions (flow, pre-access, admission,
    minting under the member fence, sign-out, expiry, sweep) are the Rust
    session campaign's; this proves the relations, grants and CHECKs they rely
    on, as each role, every probe rolled back.
    """
    org0,org1=ORGANIZATIONS
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",
                       (list(SESSION_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in sorted(SESSION_TABLES)],rows
        policy="(organization_id = (NULLIF(current_setting('genui.organization_id'::text, true), ''::text))::uuid)"
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename,policyname""",(list(SESSION_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in sorted(SESSION_TABLES)],rows
        # Cookie values only as digests: no column of the three holds a raw value.
        digests=c.execute("""SELECT c.relname,a.attname,format_type(a.atttypid,a.atttypmod) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND a.attname LIKE '%%digest' ORDER BY 1,2""",
                          (list(SESSION_TABLES),)).fetchall()
        assert digests==[('pre_access_sessions','cookie_digest','genui.digest'),('prelogin_flows','cookie_digest','genui.digest'),
                         ('prelogin_flows','state_digest','genui.digest'),('sessions','cookie_digest','genui.digest')],digests
        assert c.execute("""SELECT count(*) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
          WHERE n.nspname='genui' AND c.relname = ANY(%s) AND a.attnum>0 AND (a.attname LIKE '%%token' OR a.attname LIKE '%%nonce%%'
          OR a.attname LIKE '%%cookie' OR a.attname IN ('state','code_verifier','refresh_token','access_token'))""",(list(SESSION_TABLES),)).fetchone()==(0,)
        binding=c.execute('SELECT issuer,provider_tenant_id FROM genui.provider_tenant_bindings WHERE organization_id=%s ORDER BY issuer LIMIT 1',
                          (org0,)).fetchone()
    passed('0006 relations are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; cookies and state only as genui.digest, no token, verifier or nonce column',
           tables=list(SESSION_TABLES))
    # 0009 (APP-099): the agent relations, as the session relations: owner-held,
    # ENABLE+FORCE RLS, the exact tenant_scope policy, the token only as a digest.
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",
                       (list(AGENT_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in AGENT_TABLES],rows
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename,policyname""",(list(AGENT_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in AGENT_TABLES],rows
        digests=c.execute("""SELECT a.attname,format_type(a.atttypid,a.atttypmod) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname='agent_tokens' AND a.attnum>0 AND a.attname LIKE 'token%%'
          ORDER BY 1""").fetchall()
        assert digests==[('token_digest','genui.digest'),('token_id','uuid')],digests
    passed('0009/0013 agent_tokens, agent_attributions and agent_access_log are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; the token only as genui.digest',
           tables=list(AGENT_TABLES))
    # 0011 (APP-108): the folder relations, likewise owner-held under FORCE RLS.
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",
                       (list(BOX_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in sorted(BOX_TABLES)],rows
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename,policyname""",(list(BOX_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in sorted(BOX_TABLES)],rows
    passed('0011 folder relations (boxes, files, registrations, derivations, shares, file grants, explanations, briefs) are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy',
           tables=list(BOX_TABLES))
    # 0014 (APP-121): the naming relations, likewise owner-held under FORCE RLS.
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",
                       (list(LABEL_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in sorted(LABEL_TABLES)],rows
        rows=c.execute("""SELECT tablename,policyname,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) ORDER BY tablename,policyname""",(list(LABEL_TABLES),)).fetchall()
        principal="(principal_id = (NULLIF(current_setting('genui.principal_id'::text, true), ''::text))::uuid)"
        assert rows==[('app_labels','tenant_scope','PERMISSIVE',['public'],'ALL',policy,policy),
                      ('member_labels','member_labels_principal','RESTRICTIVE',['genui_workspace'],'ALL',principal,principal),
                      ('member_labels','tenant_scope','PERMISSIVE',['public'],'ALL',policy,policy)],rows
    passed('0014 naming relations (organization labels, personal labels) are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; the workspace reads and writes only the scoped principal\'s personal labels (RESTRICTIVE member_labels_principal)',
           tables=list(LABEL_TABLES))
    flow=("""INSERT INTO genui.prelogin_flows(organization_id,flow_id,cookie_digest,state_digest,kind,code_verifier_ciphertext,client_address_class,
      created_at,expires_at) VALUES (%s,%s,%s,%s,'signin',%s,'v4:192.0.2.1',now(),now()+interval '10 minutes')""",
          (org0,str(uuid.uuid4()),sha('cookie-'+uuid.uuid4().hex),sha('state'),b'\x01'*40))
    pre_id=str(uuid.uuid4())
    pre=("""INSERT INTO genui.pre_access_sessions(organization_id,session_id,cookie_digest,issuer,provider_tenant_id,provider_subject,email_verified,
      flow_kind,refresh_token_ref,created_at,last_seen_at,expires_at) VALUES (%s,%s,%s,%s,%s,'usr_schema',false,'signin',%s,now(),now(),now()+interval '30 minutes')""",
         (org0,pre_id,sha('pre-'+uuid.uuid4().hex),binding[0],binding[1],str(uuid.uuid4())))
    with pg.connect('genui_workspace') as c:
        scope(c,org0)
        for statement,params in (flow,pre):
            assert c.execute(statement,params).rowcount==1
        assert c.execute('UPDATE genui.pre_access_sessions SET last_seen_at=now() WHERE session_id=%s',(pre_id,)).rowcount==1
        assert c.execute('SELECT session_id::text FROM genui.pre_access_sessions WHERE session_id=%s FOR UPDATE SKIP LOCKED',(pre_id,)).fetchall()==[(pre_id,)]
        assert c.execute('DELETE FROM genui.pre_access_sessions WHERE session_id=%s',(pre_id,)).rowcount==1
        assert c.execute('DELETE FROM genui.prelogin_flows').rowcount==1
        assert c.execute("SELECT count(*) FROM genui.code_reservations WHERE kind='pending' AND state='live'").fetchone()[0]>=0
        c.rollback()
    passed('0006 grants: genui_workspace inserts, reads, touches, locks and deletes its own pre-login and pre-access rows (rolled back)')
    for name,role,(statement,params),expected in (
            ('workspace cannot rewrite a pre-login flow','genui_workspace',('UPDATE genui.prelogin_flows SET kind=kind',()),'42501'),
            ('workspace cannot rewrite a session principal','genui_workspace',('UPDATE genui.sessions SET principal_id=principal_id',()),'42501'),
            ('workspace cannot rewrite a pre-access identity','genui_workspace',('UPDATE genui.pre_access_sessions SET email_verified=true',()),'42501'),
            ('workspace cannot read a code id from reservations','genui_workspace',('SELECT code_id FROM genui.code_reservations',()),'42501'),
            ('workspace cannot read a reservation context digest','genui_workspace',('SELECT context_digest FROM genui.code_reservations',()),'42501'),
            ('workspace cannot write a reservation','genui_workspace',("UPDATE genui.code_reservations SET state='consumed'",()),'42501'),
            ('workspace cannot write another organization (FORCE RLS)','genui_workspace',(flow[0],(org1,)+flow[1][1:]),'42501'),
            ('a pre-login flow lasts at most 10 minutes','genui_workspace',(flow[0].replace("interval '10 minutes'","interval '11 minutes'"),flow[1]),'23514'),
            ('a pre-access session lasts at most 30 minutes','genui_workspace',(pre[0].replace("interval '30 minutes'","interval '31 minutes'"),pre[1]),'23514'),
            ('a sign-in flow carries no reservation','genui_workspace',(flow[0].replace("'signin'","'signup'"),flow[1]),'23514'),
            ('control cannot read sessions','genui_control',('SELECT session_id FROM genui.sessions',()),'42501'),
            ('data cannot read pre-access sessions','genui_data',('SELECT session_id FROM genui.pre_access_sessions',()),'42501'),
            ('worker cannot read pre-login flows','genui_worker',('SELECT flow_id FROM genui.prelogin_flows',()),'42501'),
            ('control cannot write sessions','genui_control',flow,'42501'),
            ('data cannot delete sessions','genui_data',('DELETE FROM genui.sessions',()),'42501')):
        denied('0006 '+name,role,statement,params,expected=expected)
    with pg.connect('genui_workspace') as c:
        scope(c,org0)
        assert c.execute("""SELECT count(*) FROM genui.sessions s WHERE s.absolute_expires_at>s.created_at+interval '12 hours'""").fetchone()==(0,)
    passed('0006 role separation and bounds: only genui_workspace touches session rows, only last_seen_at/revoked_at are updatable, the pending lookup reads no code id or context digest, and the 10-minute, 30-minute and 12-hour CHECKs hold')


def command_checks(pg, passed, denied):
    """ADR-007 D7.3/D7.7/D7.8/D7.9 and ADR-009 D9.8 for migration 0007 (APP-013) on the actual local database.

    The storage-module command transaction (claim, fences, sequence, record,
    audit, outcome in D7.9 order) and the route are the Rust command campaign's;
    this proves the relations, grants, RESTRICTIVE bindings and CHECKs they rely
    on, as each role, every probe rolled back.
    """
    org0,org1=ORGANIZATIONS
    author='00000000-0000-4000-8000-0000000000ca'
    with pg.connect('postgres') as c:
        rows=c.execute("""SELECT c.relname,c.relrowsecurity,c.relforcerowsecurity,pg_get_userbyid(c.relowner) FROM pg_class c
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND c.relkind='r' ORDER BY c.relname""",
                       (list(COMMAND_TABLES),)).fetchall()
        assert rows==[(t,True,True,'genui_owner') for t in sorted(COMMAND_TABLES)],rows
        policy="(organization_id = (NULLIF(current_setting('genui.organization_id'::text, true), ''::text))::uuid)"
        rows=c.execute("""SELECT tablename,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename = ANY(%s) AND policyname='tenant_scope' ORDER BY tablename""",(list(COMMAND_TABLES),)).fetchall()
        assert rows==[(t,'PERMISSIVE',['public'],'ALL',policy,policy) for t in sorted(COMMAND_TABLES)],rows
        principal="(principal_id = (NULLIF(current_setting('genui.principal_id'::text, true), ''::text))::uuid)"
        bindings=c.execute("""SELECT tablename,policyname,permissive,roles::text[],cmd,qual,with_check FROM pg_policies WHERE schemaname='genui'
          AND tablename IN ('command_claims','command_outcomes','internal_record_revisions') AND policyname<>'tenant_scope' ORDER BY tablename,policyname""").fetchall()
        assert bindings==[('command_claims','command_principal','RESTRICTIVE',['genui_data'],'INSERT',None,principal),
                          ('command_claims','command_principal_read','RESTRICTIVE',['genui_data'],'SELECT',principal,None),
                          ('command_outcomes','command_principal','RESTRICTIVE',['genui_data'],'INSERT',None,principal),
                          ('command_outcomes','command_principal_read','RESTRICTIVE',['genui_data'],'SELECT',principal,None),
                          ('internal_record_revisions','record_updater','RESTRICTIVE',['genui_data'],'INSERT',None,
                           "(updater_id = (NULLIF(current_setting('genui.principal_id'::text, true), ''::text))::uuid)")],bindings
        typed=c.execute("""SELECT c.relname,a.attname,format_type(a.atttypid,a.atttypmod) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname = ANY(%s) AND a.attnum>0 AND NOT a.attisdropped
          AND (a.attname LIKE '%%digest' OR a.attname LIKE '%%sequence' OR a.attname LIKE '%%version' OR a.attname='command_id') ORDER BY 1,2""",
                       (list(COMMAND_TABLES),)).fetchall()
        assert typed==[('command_claims','command_id','genui.local_id'),('command_claims','payload_digest','genui.digest'),
                       ('command_outcomes','command_id','genui.local_id'),('command_outcomes','commit_sequence','genui.safe_counter'),
                       ('command_outcomes','payload_digest','genui.digest'),('command_outcomes','record_version','genui.safe_counter'),
                       ('command_outcomes','work_version','genui.safe_counter'),('organization_sequences','next_commit_sequence','genui.safe_counter'),
                       ('query_receipts','digest','genui.digest')],typed
        # D7.7: one commit sequence names one revision per organization; D9.8: one receipt digest per organization.
        uniques=set(c.execute("""SELECT c.relname,pg_get_constraintdef(k.oid) FROM pg_constraint k JOIN pg_class c ON c.oid=k.conrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND k.contype='u' AND c.relname IN ('command_outcomes','query_receipts')""").fetchall())
        assert uniques=={('command_outcomes','UNIQUE (organization_id, commit_sequence)'),('command_outcomes','UNIQUE (organization_id, audit_event_id)'),
                         ('query_receipts','UNIQUE (organization_id, digest)')},uniques
        # D7.8: no foreign key from command_outcomes to the insert-only audit_events.
        assert c.execute("""SELECT count(*) FROM pg_constraint k WHERE k.contype='f' AND k.conrelid='genui.command_outcomes'::regclass
          AND k.confrelid='genui.audit_events'::regclass""").fetchone()==(0,)
        assert c.execute("""SELECT count(*) FROM pg_constraint k WHERE k.contype='f' AND k.conrelid='genui.command_outcomes'::regclass
          AND k.confrelid='genui.command_claims'::regclass""").fetchone()==(1,)
        # ADR-002 scope organization/principal/work/command/key, with the work's app (saved_work's key is
        # organization/app/work; APP-013 review m6): the claim's and the outcome's keys, and the outcome's reference.
        scope_key='(organization_id, principal_id, app_id, work_id, command_id, idempotency_key)'
        keys=set(c.execute("""SELECT c.relname,k.contype,pg_get_constraintdef(k.oid) FROM pg_constraint k JOIN pg_class c ON c.oid=k.conrelid
          JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui' AND c.relname IN ('command_claims','command_outcomes')
          AND (k.contype='p' OR k.confrelid='genui.command_claims'::regclass)""").fetchall())
        assert keys=={('command_claims','p','PRIMARY KEY '+scope_key),('command_outcomes','p','PRIMARY KEY '+scope_key),
                      ('command_outcomes','f','FOREIGN KEY '+scope_key+' REFERENCES genui.command_claims'+scope_key)},keys
        work,app,publication,collection=c.execute("""SELECT w.work_id::text,w.app_id::text,w.publication_id::text,r.collection_id::text FROM genui.saved_work w
          JOIN genui.record_collections r USING(organization_id,app_id,work_id) WHERE w.organization_id=%s LIMIT 1""",(org0,)).fetchone()
    passed('0007 relations are owner-held with ENABLE+FORCE RLS and the exact tenant_scope policy; the data role is bound to the scoped principal on claims, outcomes and revisions; ids, digests, sequences and versions are domain-typed; one commit sequence per outcome; the outcome references its claim and has no foreign key into the insert-only audit log',
           tables=list(COMMAND_TABLES))
    # The operator-only sequence row (D7.7/D7.19), as the owner, so the data role probes have a row to lock.
    with pg.connect() as c:
        c.execute('SET LOCAL ROLE genui_owner')
        c.execute("SELECT set_config('genui.organization_id',%s,true)",(org0,))
        c.execute("INSERT INTO genui.organization_sequences VALUES (%s,2) ON CONFLICT DO NOTHING",(org0,))
        c.commit()
    key='0123456789abcdef0123456789abcdef'
    claim=("""INSERT INTO genui.command_claims(organization_id,principal_id,app_id,work_id,command_id,idempotency_key,payload_digest,claimed_at)
      VALUES (%s,%s,%s,%s,'record_decision',%s,%s,now())""",(org0,author,app,work,key,sha('payload')))
    outcome="""INSERT INTO genui.command_outcomes(organization_id,app_id,work_id,principal_id,command_id,idempotency_key,payload_digest,state,record_id,
      record_version,work_version,commit_sequence,audit_event_id,committed_at,retry_expires_at)
      VALUES (%s,%s,%s,%s,'record_decision',%s,%s,'committed',%s,1,1,2,%s,now(),now()+interval 'HORIZON')"""
    record=str(uuid.uuid4())
    with pg.connect('genui_data') as c:
        scope(c,org0,author)
        assert c.execute('SELECT next_commit_sequence FROM genui.organization_sequences WHERE organization_id=%s FOR UPDATE',(org0,)).fetchall()==[(2,)]
        assert c.execute('UPDATE genui.organization_sequences SET next_commit_sequence=3 WHERE organization_id=%s AND next_commit_sequence=2',(org0,)).rowcount==1
        assert c.execute(*claim).rowcount==1
        assert c.execute('SELECT count(*) FROM genui.command_claims').fetchone()==(1,)
        assert c.execute(outcome.replace('HORIZON','24 hours'),(org0,app,work,author,key,sha('payload'),record,str(uuid.uuid4()))).rowcount==1
        assert c.execute('SELECT count(*) FROM genui.command_outcomes WHERE idempotency_key=%s',(key,)).fetchone()==(1,)
        c.rollback()
        # A claim of another principal is row-invisible under this principal's scope.
        scope(c,org0,P)
        assert c.execute('SELECT count(*) FROM genui.command_claims').fetchone()==(0,)
        c.rollback()
    passed('0007 grants: genui_data locks and advances its organization sequence, claims under its own principal, reads the claim and commits an outcome referencing it (rolled back); another principal sees no claim')
    for name,role,(statement,params),expected in (
            ('an outcome needs its claim','genui_data',(outcome.replace('HORIZON','24 hours'),(org0,app,work,P,'ffffffffffffffffffffffffffffffff',sha('payload'),record,str(uuid.uuid4()))),'23503'),
            ('data cannot claim as another principal (scope P, row author)','genui_data',(claim[0],(org0,author,app,work,key,sha('payload'))),'42501'),
            ('data cannot update an outcome','genui_data',('UPDATE genui.command_outcomes SET record_version=record_version',()),'42501'),
            ('data cannot delete a claim','genui_data',('DELETE FROM genui.command_claims',()),'42501'),
            ('data cannot update a revision','genui_data',('UPDATE genui.internal_record_revisions SET canonical_values=canonical_values',()),'42501'),
            ('data cannot create a sequence row (D7.19)','genui_data',("INSERT INTO genui.organization_sequences VALUES (%s,1)",(org1,)),'42501'),
            ('data cannot touch receipts yet','genui_data',('SELECT receipt_id FROM genui.query_receipts',()),'42501'),
            ("data cannot append another chain",'genui_data',("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,
              record_id,code_id,outcome_digest,occurred_at,previous_hash,hash) VALUES (%s,gen_random_uuid(),'workspace',2,'command.committed',%s,NULL,NULL,NULL,NULL,NULL,now(),%s,%s)""",
              (org0,author,sha('p'),sha('h'))),'42501'),
            ("data cannot append another kind",'genui_data',("""INSERT INTO genui.audit_events(organization_id,event_id,writer_role,chain_sequence,kind,actor_principal_id,app_id,work_id,
              record_id,code_id,outcome_digest,occurred_at,previous_hash,hash) VALUES (%s,gen_random_uuid(),'data',2,'work.saved',%s,NULL,NULL,NULL,NULL,NULL,now(),%s,%s)""",
              (org0,author,sha('p'),sha('h'))),'42501'),
            ('a malformed key is refused','genui_data',(claim[0],(org0,P,app,work,'ABCDEF0123456789abcdef0123456789',sha('payload'))),'23514'),
            ('workspace cannot read outcomes','genui_workspace',('SELECT record_id FROM genui.command_outcomes',()),'42501'),
            ('control cannot read claims','genui_control',('SELECT command_id FROM genui.command_claims',()),'42501'),
            ('worker cannot read the sequence','genui_worker',('SELECT next_commit_sequence FROM genui.organization_sequences',()),'42501'),
            ('workspace cannot write the sequence','genui_workspace',('UPDATE genui.organization_sequences SET next_commit_sequence=next_commit_sequence',()),'42501')):
        denied('0007 '+name,role,statement,params,expected=expected)
    with pg.connect('genui_data') as c:
        scope(c,org0,author)
        c.execute(*claim)
        try:
            c.execute(outcome.replace('HORIZON','23 hours'),(org0,app,work,author,key,sha('payload'),record,str(uuid.uuid4())))
        except psycopg.errors.CheckViolation: pass
        else: raise AssertionError('retry horizon other than 24 hours accepted')
        c.rollback()
    passed('0007 role separation and bounds: only genui_data touches the command relations and only as the scoped principal, with no UPDATE or DELETE on claims, outcomes or revisions, no sequence creation and no other chain or kind; the key shape and the 24-hour retry horizon CHECKs hold')


def command(args, check=True):
    p = subprocess.run(args, capture_output=True, text=True, timeout=60)
    if check and p.returncode: raise RuntimeError('local Docker operation failed: '+p.stderr[-1200:])
    return p


class LocalPostgres:
    def __init__(self):
        self.name = 'genui-app008-schema-' + uuid.uuid4().hex[:12]
        self.port = None
        self.removed = False

    def __enter__(self):
        try:
            command(['docker','run','--detach','--rm','--name',self.name,'--publish','127.0.0.1::5432',
                     '--memory=512m','--cpus=1','--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m',
                     '--env=POSTGRES_HOST_AUTH_METHOD=trust',IMAGE])
            address = command(['docker','port',self.name,'5432/tcp']).stdout.strip()
            assert address.startswith('127.0.0.1:') and '\n' not in address
            self.port = int(address.split(':')[1])
            for _ in range(60):
                if command(['docker','exec',self.name,'pg_isready','--host=127.0.0.1','--username=postgres'], False).returncode == 0: break
                time.sleep(.25)
            else: raise RuntimeError('local PostgreSQL startup deadline')
            with self.connect('postgres','postgres',autocommit=True) as c:
                assert c.execute('SHOW server_version').fetchone()[0].split()[0] == '16.13'
                c.execute('CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS')
                c.execute('CREATE ROLE rds_iam NOLOGIN')
                c.execute('GRANT rds_iam TO genui_migration WITH ADMIN TRUE')
                c.execute('CREATE DATABASE genui OWNER genui_migration')
            return self
        except BaseException:
            self.__exit__(None,None,None)
            raise

    def __exit__(self,*_):
        self.removed = command(['docker','rm','--force','--volumes',self.name],False).returncode == 0

    def connect(self, role='genui_migration', database='genui', autocommit=False):
        return psycopg.connect(host='127.0.0.1',port=self.port,dbname=database,user=role,sslmode='disable',
                              connect_timeout=3,autocommit=autocommit,
                              options='-c statement_timeout=5000 -c lock_timeout=3000 -c idle_in_transaction_session_timeout=10000')

    def digest(self):
        raw = command(['docker','exec',self.name,'pg_dump','--username=postgres','--dbname=genui','--no-owner','--no-comments']).stdout
        raw = '\n'.join(x for x in raw.splitlines() if not x.startswith(('\\restrict ','\\unrestrict ')))
        return hashlib.sha256(raw.encode()).hexdigest()


def scope(c, org=ORGANIZATIONS[0], principal=P, epoch=1):
    c.execute("SELECT set_config('genui.organization_id',%s,true),set_config('genui.principal_id',%s,true),set_config('genui.membership_epoch',%s,true)", (org,principal,str(epoch)))


def qualify():
    report={'schema_version':1,'scope':'APP-008/APP-073/APP-010/APP-009/APP-013 real local PostgreSQL schema/ACL, admission-function, definition/work, session and command grant qualification',
            'managed_acceptance':False,'image':IMAGE,'server_version':'16.13','network':'random loopback-only published port',
            'checks':[],'passed':False,'limits':['Local trust authentication is not RDS IAM/TLS qualification.',
            'Operator-seeded contract shapes are not source import, hosted identity or successful application business writes.',
            'Admission functions are exercised directly as genui_workspace with fixture codes written as the operator channel would; '
            'the storage-module admission path, the operator package, sessions and hosted identity are not exercised.',
            'The 0005 definition/work grants are exercised as raw role statements; the storage-module definition, publication and saved-work transactions are the Rust definition campaign.',
            'The 0006 session grants and CHECKs are exercised as raw role statements; the session transactions and routes are the Rust session campaign (auth::tests::native_session_campaign).',
            'The 0007 command grants, RESTRICTIVE bindings and CHECKs are exercised as raw role statements; the command transaction, its lock order and the route are the Rust command campaign (storage::command_qualification::native_command_campaign).',
            'Native pool cancellation/reuse and actual managed trials remain separate.']}
    def passed(name, **facts): report['checks'].append({'name':name,'passed':True,**facts})
    catalog_query=json.loads((MIGRATIONS/'catalog-query.json').read_text())['sql']
    # Version, relation and pinned-file expectations follow the manifest and its
    # latest reviewed catalog; never a constant that a new migration must edit.
    manifest=runtime.migration_manifest(MIGRATIONS)
    sources=runtime.migration_sources(MIGRATIONS)
    latest=len(sources)
    tenant_tables=sorted(row[1] for row in runtime.catalog_resources(MIGRATIONS)[1][latest]['relations']
                         if row[0]=='genui' and row[2] in ('r','p'))
    def drift_refused(pg, name, statements):
        # These mutations are visible to the actual runner in one transaction.
        # A Python Refused must leave that drift untouched; only the test rolls
        # it back afterwards. Committed reproductions are checked separately.
        with pg.connect() as c:
            for statement in statements: c.execute(statement)
            c.execute('RESET ROLE; SET LOCAL search_path=pg_catalog')
            before=c.execute(catalog_query).fetchall()
            version=c.execute('SELECT * FROM genui_meta.schema_version').fetchall()
            try: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('catalog drift accepted: '+name)
            assert c.execute(catalog_query).fetchall()==before
            assert c.execute('SELECT * FROM genui_meta.schema_version').fetchall()==version
            c.rollback()
        passed(name+' refuses without repairing drift')
    report['committed_drift_containers_removed']=[]
    for label, statement, legacy in (
        ('missing real table', 'SET LOCAL ROLE genui_owner; DROP TABLE genui.organization_permissions', False),
        ('disabled tenant RLS', 'SET LOCAL ROLE genui_owner; ALTER TABLE genui.organizations DISABLE ROW LEVEL SECURITY', False),
        ('extra application UPDATE', 'SET LOCAL ROLE genui_owner; GRANT UPDATE ON genui.publications TO genui_workspace', False),
        ('preexisting history1 with role drift', 'ALTER ROLE genui_workspace INHERIT', True),
    ):
        drift=LocalPostgres()
        try:
            with drift:
                with drift.connect() as c:
                    if legacy:
                        c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
                        runtime.initialize_migration_history(c,runtime.INITIAL_MIGRATION_SHA256)
                    else: runtime.apply_migrations(c,MIGRATIONS)
                    c.execute(statement)
                # Mutation is committed before opening the real migration call.
                before=drift.digest()
                with drift.connect() as c:
                    c.execute('SET LOCAL search_path=pg_catalog')
                    catalog_before=c.execute(catalog_query).fetchall()
                try:
                    with drift.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('committed drift accepted: '+label)
                assert drift.digest()==before
                with drift.connect() as c:
                    c.execute('SET LOCAL search_path=pg_catalog')
                    assert c.execute(catalog_query).fetchall()==catalog_before
                passed('committed '+label+' refuses and preserves exact drifted state')
        finally: report['committed_drift_containers_removed'].append(drift.removed)
    replica=LocalPostgres()
    try:
        with replica:
            with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            for parameter, value in (('application_name','unexpected-all-role-default'),('session_replication_role','replica')):
                with replica.connect('postgres') as c:
                    c.execute(sql.SQL('ALTER DATABASE genui SET {} TO {}').format(sql.Identifier(parameter),sql.Literal(value)))
                before=replica.digest()
                for role in ('genui_migration','genui_workspace'):
                    with replica.connect(role) as c:
                        expected_mode='replica' if parameter=='session_replication_role' else 'origin'
                        assert c.execute('SHOW session_replication_role').fetchone()==(expected_mode,)
                        c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
                        try: runtime.verify_schema_catalog(c,MIGRATIONS)
                        except runtime.Refused: pass
                        else: raise AssertionError('all-role database default accepted: '+parameter)
                        assert c.execute('SHOW session_replication_role').fetchone()==(expected_mode,)
                        assert c.execute(catalog_query).fetchall()==catalog_before
                try:
                    with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('migration accepted committed all-role default: '+parameter)
                assert replica.digest()==before
                with replica.connect('postgres') as c:
                    c.execute(sql.SQL('ALTER DATABASE genui RESET {}').format(sql.Identifier(parameter)))
                passed('committed all-role '+parameter+' default refuses on fresh app/operator connections without repair')
            with replica.connect('postgres') as c:
                c.execute('GRANT SET ON PARAMETER session_replication_role TO genui_workspace')
            before=replica.digest()
            with replica.connect('genui_workspace') as c:
                assert c.execute('SHOW session_replication_role').fetchone()==('origin',)
                assert c.execute("SELECT has_parameter_privilege(current_user,'session_replication_role','SET')").fetchone()==(True,)
                c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
                try: runtime.verify_schema_catalog(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('application FK-bypass parameter authority accepted')
                assert c.execute(catalog_query).fetchall()==catalog_before
                assert c.execute("SELECT has_parameter_privilege(current_user,'session_replication_role','SET')").fetchone()==(True,)
            try:
                with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('migration accepted application FK-bypass parameter grant')
            assert replica.digest()==before
            with replica.connect('postgres') as c:
                c.execute('REVOKE SET ON PARAMETER session_replication_role FROM genui_workspace')
            passed('committed application replica-parameter SET grant refuses while session remains origin without repair')
            # Isolated operator-enabled session mutation, independently of the
            # DB default. The permission is fixture setup, not an application grant.
            with replica.connect('postgres') as c:
                c.execute('GRANT SET ON PARAMETER session_replication_role TO genui_migration')
            with replica.connect() as c:
                c.execute('SET LOCAL session_replication_role=replica')
                for verify in (lambda:runtime.verify_schema_catalog(c,MIGRATIONS),lambda:runtime.apply_migrations(c,MIGRATIONS)):
                    try: verify()
                    except runtime.Refused: pass
                    else: raise AssertionError('replica session accepted')
                    assert c.execute('SHOW session_replication_role').fetchone()==('replica',)
                c.rollback()
            passed('operator-enabled current replica session refuses both verifier and migration without SET repair')
    finally: report['replica_drift_container_removed']=replica.removed
    compatibility=LocalPostgres()
    try:
        with compatibility:
            with compatibility.connect() as c: c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
            def remove_creator_bookkeeping():
                with compatibility.connect('postgres') as c:
                    rows=c.execute("""SELECT parent.rolname,grantor.rolname FROM pg_auth_members a
                      JOIN pg_roles parent ON parent.oid=a.roleid JOIN pg_roles member ON member.oid=a.member
                      JOIN pg_roles grantor ON grantor.oid=a.grantor
                      WHERE member.rolname='genui_migration' AND parent.rolname IN
                      ('genui_owner','genui_lock','genui_admit','genui_workspace','genui_control','genui_data','genui_worker')
                      AND a.admin_option AND NOT a.inherit_option AND NOT a.set_option""").fetchall()
                    assert rows
                    for role,grantor in rows:
                        c.execute(sql.SQL('REVOKE {} FROM genui_migration GRANTED BY {} CASCADE').format(sql.Identifier(role),sql.Identifier(grantor)))
                        # PostgreSQL may cascade the creator's dependent explicit
                        # grant. Retain precisely the separately required operator
                        # owner/helper access under the fixture administrator.
                        if role=='genui_owner': c.execute('GRANT genui_owner TO genui_migration WITH ADMIN FALSE, INHERIT TRUE, SET TRUE')
                        elif role in ('genui_lock','genui_admit'):
                            c.execute(sql.SQL('GRANT {} TO genui_migration WITH ADMIN FALSE, INHERIT FALSE, SET TRUE').format(sql.Identifier(role)))
            remove_creator_bookkeeping()
            with compatibility.connect() as c:
                runtime.verify_schema_catalog(c,MIGRATIONS,1)
                runtime.apply_migrations(c,MIGRATIONS)
            passed('managed-style absence of optional creator-only grants permits exact version1 verification and upgrade')
            remove_creator_bookkeeping()
            with compatibility.connect() as c:
                runtime.verify_schema_catalog(c,MIGRATIONS,latest)
                runtime.apply_migrations(c,MIGRATIONS)
            passed('managed-style absence of optional creator-only grants permits exact version'+str(latest)+' repeat')
            with compatibility.connect('postgres') as c:
                c.execute('REVOKE genui_owner FROM genui_migration CASCADE')
            with compatibility.connect() as c:
                try: runtime.verify_schema_catalog(c,MIGRATIONS,latest)
                except runtime.Refused: pass
                else: raise AssertionError('required explicit migration-owner grant was normalized away')
            passed('required explicit migration-owner access remains exact and cannot be normalized away')
    finally: report['compatibility_container_removed']=compatibility.removed
    shadow=LocalPostgres()
    try:
        with shadow:
            with shadow.connect() as c:
                c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
                c.execute("""CREATE SCHEMA app008_untrusted;
                  CREATE SEQUENCE app008_untrusted.marker;
                  CREATE FUNCTION app008_untrusted.current_database() RETURNS name LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN 'genui'::name; END $$;
                  CREATE FUNCTION app008_untrusted.to_regclass(text) RETURNS regclass LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN pg_catalog.to_regclass($1); END $$;
                  CREATE FUNCTION app008_untrusted.pg_advisory_xact_lock(integer,integer) RETURNS void LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); END $$;
                  CREATE FUNCTION app008_untrusted.version_marker() RETURNS integer LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN 1; END $$""")
            for version in (1,latest):
                with shadow.connect('postgres') as c:
                    c.execute("ALTER DATABASE genui SET search_path=app008_untrusted,pg_catalog")
                with shadow.connect() as c:
                    assert c.execute('SHOW search_path').fetchone()==('app008_untrusted, pg_catalog',)
                    try: runtime.apply_migrations(c,MIGRATIONS)
                    except runtime.Refused: pass
                    else: raise AssertionError('untrusted database search_path accepted')
                    assert c.execute('SELECT is_called FROM app008_untrusted.marker').fetchone()==(False,)
                    assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                with shadow.connect('postgres') as c:
                    assert c.execute("SELECT setconfig FROM pg_db_role_setting WHERE setrole=0 AND setdatabase=(SELECT oid FROM pg_database WHERE datname='genui')").fetchone()==(['search_path=app008_untrusted, pg_catalog'],)
                    c.execute('ALTER DATABASE genui RESET search_path')
                passed('version'+str(version)+' migration refuses unsafe database search_path without executing shadow functions or repairing default')
                if version==1:
                    with shadow.connect() as c:
                        c.execute('DROP TABLE genui_meta.schema_version')
                        c.execute('CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,app008_untrusted.version_marker() AS version')
                        try: runtime.apply_migrations(c,MIGRATIONS)
                        except runtime.Refused: pass
                        else: raise AssertionError('shadow version view accepted before catalog proof')
                        assert c.execute('SELECT is_called FROM app008_untrusted.marker').fetchone()==(False,)
                        c.rollback()
                    passed('legacy version view refuses before reading its marker expression')
                    with shadow.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            with shadow.connect() as c:
                c.execute("""CREATE TEMP SEQUENCE review_marker;
                  CREATE FUNCTION pg_temp.review_marker() RETURNS boolean LANGUAGE plpgsql VOLATILE AS
                  $$ BEGIN PERFORM pg_catalog.nextval('pg_temp.review_marker'); RETURN true; END $$;
                  CREATE TEMP VIEW pg_namespace AS SELECT n.* FROM pg_catalog.pg_namespace n
                    WHERE pg_temp.review_marker()""")
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                # Positive control: omitted pg_temp really does take precedence
                # for catalog relation names and executes the hostile expression.
                c.execute('SET LOCAL search_path=pg_catalog')
                c.execute(catalog_query).fetchall()
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(True,)
                c.execute("SELECT pg_catalog.setval('pg_temp.review_marker',1,false)")
                runtime.verify_schema_catalog(c,MIGRATIONS)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                # Restart from the vulnerable path to prove apply_migrations
                # fixes its own first metadata lookup, not just the verifier's.
                c.execute('SET LOCAL search_path=pg_catalog')
                runtime.apply_migrations(c,MIGRATIONS)
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                c.rollback()
            passed('positive-control temporary catalog view executes only under implicit temp-first path; verifier and repeat leave zero marker')
            with shadow.connect() as c:
                c.execute("""CREATE SEQUENCE app008_untrusted.probe_marker;
                  CREATE FUNCTION app008_untrusted.probe_version_marker() RETURNS integer
                  LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.probe_marker'); RETURN %d; END $$;
                  DROP TABLE genui_meta.schema_version;
                  CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,app008_untrusted.probe_version_marker() AS version;
                  GRANT SELECT ON genui_meta.schema_version TO genui_workspace""" % latest)
            with shadow.connect('genui_workspace') as c:
                # This is a real view expression, reachable by the exact probe
                # role. The read-write control proves the sequence marker fires.
                assert c.execute('SELECT version FROM genui_meta.schema_version WHERE singleton=true').fetchone()==(latest,)
            with shadow.connect() as c:
                assert c.execute('SELECT is_called FROM app008_untrusted.probe_marker').fetchone()==(True,)
                c.execute("SELECT pg_catalog.setval('app008_untrusted.probe_marker',1,false)")
            with shadow.connect('genui_workspace') as c:
                c.execute('SET TRANSACTION READ ONLY')
                try: c.execute('SELECT version FROM genui_meta.schema_version WHERE singleton=true').fetchone()
                except psycopg.Error as error: assert error.sqlstate=='25006'
                else: raise AssertionError('read-only control unexpectedly permitted marker nextval')
                c.rollback()
            with shadow.connect('genui_workspace') as c:
                c.execute('SET TRANSACTION READ ONLY')
                try: runtime.probe_database(c,'genui_workspace')
                except runtime.Refused: pass
                else: raise AssertionError('runtime probe accepted malicious version view')
                assert c.execute('SHOW transaction_read_only').fetchone()==('on',)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
            with shadow.connect() as c:
                assert c.execute('SELECT is_called FROM app008_untrusted.probe_marker').fetchone()==(False,)
            passed('actual read-only workspace probe refuses malicious version view before evaluation; read-write and read-only controls prove reachability',read_only_control_sqlstate='25006')
    finally: report['shadow_path_container_removed']=shadow.removed
    fresh=LocalPostgres()
    try:
        with fresh:
            before=fresh.digest()
            with fresh.connect(autocommit=True) as c:
                try: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('autocommit migration was accepted')
            assert fresh.digest()==before
            passed('autocommit migration refuses before changing fresh database')
            with fresh.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            before=fresh.digest()
            with fresh.connect() as c:
                runtime.apply_migrations(c,MIGRATIONS)
                assert c.execute('SELECT singleton,version FROM genui_meta.schema_version').fetchall()==[(True,latest)]
                assert c.execute('SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version').fetchall()==[(v,h) for v,h,_ in runtime.migration_sources(MIGRATIONS)]
            assert fresh.digest()==before
            passed('fresh checksummed version'+str(latest)+' install and exact repeat')
    finally:
        report['fresh_container_removed']=fresh.removed
    pg=LocalPostgres()
    try:
        with pg:
            # Exact checksummed source can still have a SQL failure. Both fresh
            # install and upgrade must roll back roles, schema and history whole.
            with tempfile.TemporaryDirectory(prefix='genui-app008-fault-') as tmp:
                fault=Path(tmp)
                for file in MIGRATIONS.iterdir():
                    if file.is_file(): shutil.copyfile(file,fault/file.name)
                # Interrupt the last migration, after every earlier one ran.
                faulted=json.loads((fault/'manifest.json').read_text())
                target=fault/faulted['migrations'][-1]['file']
                target.write_text(target.read_text()+"\nDO $$ BEGIN RAISE EXCEPTION 'synthetic migration interruption' USING ERRCODE='P0001'; END $$;\n")
                faulted['migrations'][-1]['sha256']=hashlib.sha256(target.read_bytes()).hexdigest()
                (fault/'manifest.json').write_text(json.dumps(faulted))
                for initial in ('fresh','version1'):
                    before=pg.digest()
                    try:
                        with pg.connect() as c: runtime.apply_migrations(c,fault)
                    except psycopg.Error as e: assert e.sqlstate=='P0001'
                    else: raise AssertionError('interrupted migration passed')
                    assert pg.digest()==before
                    with pg.connect('postgres') as c:
                        assert c.execute("SELECT count(*) FROM pg_roles WHERE rolname='genui_lock'").fetchone()==(0,)
                    passed(initial+' migration interruption rolls schema/history/lock-role back')
                    if initial=='fresh':
                        with pg.connect() as c: c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
            for label, statements in (
                ('legacy metadata extra column',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui_meta.schema_version ADD COLUMN unknown text"]),
                ('legacy metadata wrong constraint',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check"]),
                ('legacy metadata PUBLIC write',["SET LOCAL ROLE genui_owner", "GRANT UPDATE ON genui_meta.schema_version TO PUBLIC"]),
                ('legacy application inheritance drift',["ALTER ROLE genui_workspace INHERIT"]),
                ('legacy default privilege drift',["SET LOCAL ROLE genui_owner", "ALTER DEFAULT PRIVILEGES GRANT UPDATE ON TABLES TO genui_workspace"]),
            ): drift_refused(pg,label,statements)
            for inheritance in (False,True):
                with pg.connect() as c:
                    runtime.initialize_migration_history(c,runtime.INITIAL_MIGRATION_SHA256)
                    if inheritance: c.execute('ALTER ROLE genui_workspace INHERIT')
                    c.execute('SET LOCAL search_path=pg_catalog')
                    before=c.execute(catalog_query).fetchall()
                    try: runtime.apply_migrations(c,MIGRATIONS)
                    except runtime.Refused: pass
                    else: raise AssertionError('preexisting history1 was accepted')
                    assert c.execute(catalog_query).fetchall()==before
                    assert c.execute('SELECT version FROM genui_meta.schema_version').fetchone()==(1,)
                    c.rollback()
                passed('preexisting history1'+(' plus role drift' if inheritance else '')+' refuses without bootstrap bypass')
            with pg.connect() as c: c.execute('CREATE TABLE genui_meta.unexpected_partial_state(n integer)')
            before=pg.digest()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('partial version1 foundation accepted')
            assert pg.digest()==before
            with pg.connect() as c: c.execute('DROP TABLE genui_meta.unexpected_partial_state')
            passed('partial version1 state refuses without inventing migration history')
            with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            passed('transactional verified version1-to-version'+str(latest)+' migration')
            before=pg.digest()
            with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            assert pg.digest()==before
            passed('checksummed repeat has identical schema/data dump')
            with pg.connect() as c: c.execute("UPDATE genui_meta.schema_migrations SET sha256=repeat('0',64) WHERE version=%s",(latest,))
            before=pg.digest()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('altered recorded checksum accepted')
            assert pg.digest()==before
            with pg.connect() as c:
                c.execute('UPDATE genui_meta.schema_migrations SET sha256=%s WHERE version=%s',(sources[-1][1],latest))
            passed('altered recorded checksum refuses without altering database')
            for label, statements in (
                ('missing real table',["SET LOCAL ROLE genui_owner", "DROP TABLE genui.organization_permissions"]),
                ('missing real column',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DROP COLUMN display_name"]),
                ('changed column default',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations ALTER COLUMN status SET DEFAULT 'active'"]),
                ('missing check constraint',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DROP CONSTRAINT organizations_status_check"]),
                ('extra index',["SET LOCAL ROLE genui_owner", "CREATE INDEX unexpected_status_idx ON genui.organizations(status)"]),
                ('disabled RLS',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DISABLE ROW LEVEL SECURITY"]),
                ('disabled FORCE RLS',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations NO FORCE ROW LEVEL SECURITY"]),
                ('permissive tenant policy',["SET LOCAL ROLE genui_owner", "ALTER POLICY tenant_scope ON genui.organizations USING (true)"]),
                ('changed table owner',["ALTER TABLE genui.organizations OWNER TO genui_migration"]),
                ('extra workspace UPDATE',["SET LOCAL ROLE genui_owner", "GRANT UPDATE ON genui.publications TO genui_workspace"]),
                ('extra workspace column UPDATE',["SET LOCAL ROLE genui_owner", "GRANT UPDATE(display_name) ON genui.organizations TO genui_workspace"]),
                ('PUBLIC business read',["SET LOCAL ROLE genui_owner", "GRANT SELECT ON genui.internal_records TO PUBLIC"]),
                ('application inheritance drift',["ALTER ROLE genui_workspace INHERIT"]),
                ('application owner membership',["GRANT genui_owner TO genui_workspace"]),
                ('noncreator operator grant flags',["GRANT genui_workspace TO genui_migration WITH ADMIN FALSE, INHERIT FALSE, SET TRUE"]),
                ('unknown incoming owner member',["CREATE ROLE unexpected_catalog_incoming NOLOGIN", "GRANT genui_owner TO unexpected_catalog_incoming"]),
                ('transitive IAM parent membership',["CREATE ROLE unexpected_catalog_group NOLOGIN", "GRANT unexpected_catalog_group TO rds_iam"]),
                ('unexpected incoming helper membership',["GRANT genui_lock TO genui_data"]),
                ('application database CREATE',["GRANT CREATE ON DATABASE genui TO genui_workspace"]),
                ('helper schema CREATE',["SET LOCAL ROLE genui_owner", "GRANT CREATE ON SCHEMA genui_private TO genui_lock"]),
                ('PUBLIC function execute',["SET LOCAL ROLE genui_lock", "GRANT EXECUTE ON FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) TO PUBLIC"]),
                ('unsafe helper search path',["SET LOCAL ROLE genui_lock", "ALTER FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) SET search_path=public,genui"]),
                ('changed helper body',["SET LOCAL ROLE genui_owner", "GRANT CREATE ON SCHEMA genui_private TO genui_lock", "SET LOCAL ROLE genui_lock", "CREATE OR REPLACE FUNCTION genui_private.lock_member_authority(p_org uuid,p_principal uuid,p_epoch bigint) RETURNS TABLE(organization_active boolean,member_active boolean,membership_epoch bigint) LANGUAGE sql SECURITY DEFINER AS 'SELECT true,true,1::bigint'", "SET LOCAL ROLE genui_owner", "REVOKE CREATE ON SCHEMA genui_private FROM genui_lock"]),
                ('role database setting',["ALTER ROLE genui_workspace IN DATABASE genui SET row_security=off"]),
                ('extra schema object',["SET LOCAL ROLE genui_owner", "CREATE VIEW genui.unexpected_view AS SELECT 1 AS n"]),
            ): drift_refused(pg,label,statements)
            with pg.connect('postgres') as c:
                c.execute('ALTER TABLE genui.work_permissions DISABLE TRIGGER ALL')
            before=pg.digest()
            with pg.connect() as c:
                c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('disabled internal FK triggers accepted')
            assert pg.digest()==before
            with pg.connect() as c:
                c.execute('SET LOCAL search_path=pg_catalog');assert c.execute(catalog_query).fetchall()==catalog_before
            with pg.connect('postgres') as c:
                c.execute('ALTER TABLE genui.work_permissions ENABLE TRIGGER ALL')
            passed('committed disabled internal FK triggers refuse without repair')
            for role in ('genui_workspace','genui_control','genui_data','genui_worker'):
                with pg.connect(role) as c: runtime.verify_schema_catalog(c,MIGRATIONS)
            passed('one canonical complete catalog projection is readable by all four application roles')
            bundles=make_bundles()
            before=pg.digest()
            with pg.connect(autocommit=True) as c:
                try: seed(c,bundles)
                except ValueError: pass
                else: raise AssertionError('autocommit seed was accepted')
            assert pg.digest()==before
            passed('autocommit seed refuses before tenant writes')
            with pg.connect() as c: seed(c,bundles)
            before=pg.digest()
            with pg.connect() as c: seed(c,bundles)
            assert pg.digest()==before
            passed('validated operator seed idempotence',bundle_count=len(bundles),seed_sha256=hashlib.sha256(canonical(bundles)).hexdigest())

            with pg.connect('postgres') as c:
                tables=c.execute("SELECT tablename FROM pg_tables WHERE schemaname='genui' ORDER BY tablename").fetchall()
                assert len(tables)==len(tenant_tables) and sorted(table for (table,) in tables)==tenant_tables
                assert c.execute("""SELECT count(*) FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
                  WHERE n.nspname='genui' AND t.relkind='r' AND
                  (NOT t.relrowsecurity OR NOT t.relforcerowsecurity OR pg_get_userbyid(t.relowner)<>'genui_owner')""").fetchone()==(0,)
                assert c.execute("""SELECT count(*) FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid JOIN pg_namespace n ON n.oid=t.relnamespace
                  JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=i.indkey[0]
                  WHERE n.nspname='genui' AND a.attname<>'organization_id'""").fetchone()==(0,)
                assert c.execute("""SELECT count(*) FROM pg_constraint k JOIN pg_class t ON t.oid=k.conrelid JOIN pg_namespace n ON n.oid=t.relnamespace
                  JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.conkey[1]
                  JOIN pg_attribute b ON b.attrelid=k.confrelid AND b.attnum=k.confkey[1]
                  WHERE n.nspname='genui' AND k.contype='f' AND (a.attname<>'organization_id' OR b.attname<>'organization_id')""").fetchone()==(0,)
                # Every genui table has exactly one permissive tenant_scope policy
                # with both USING and WITH CHECK; the only other policies are the
                # RESTRICTIVE audit chain bindings (ADR-007 D7.8), checked below.
                assert c.execute("""SELECT count(*) FROM pg_tables t WHERE t.schemaname='genui' AND 1<>(SELECT count(*) FROM pg_policies p
                  WHERE p.schemaname='genui' AND p.tablename=t.tablename AND p.policyname='tenant_scope' AND p.permissive='PERMISSIVE'
                  AND p.cmd='ALL' AND p.roles='{public}' AND p.qual IS NOT NULL AND p.with_check IS NOT NULL)""").fetchone()==(0,)
                assert c.execute("""SELECT count(*) FROM pg_policies WHERE schemaname='genui' AND policyname<>'tenant_scope'
                  AND NOT (permissive='RESTRICTIVE' AND tablename IN ('audit_events','audit_heads'))
                  AND NOT (permissive='RESTRICTIVE' AND tablename='saved_work' AND policyname='saved_work_creator')
                  AND NOT (permissive='RESTRICTIVE' AND tablename='member_labels' AND policyname='member_labels_principal')
                  AND NOT (permissive='RESTRICTIVE' AND tablename IN ('command_claims','command_outcomes','internal_record_revisions'))""").fetchone()==(0,)
                # 0007's RESTRICTIVE principal bindings, exactly (checked in detail by command_checks).
                assert set(c.execute("""SELECT tablename,policyname,cmd FROM pg_policies WHERE schemaname='genui' AND permissive='RESTRICTIVE'
                  AND tablename IN ('command_claims','command_outcomes','internal_record_revisions')""").fetchall())==COMMAND_POLICIES
                lock=c.execute("SELECT rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls,rolcanlogin,rolinherit FROM pg_roles WHERE rolname='genui_lock'").fetchone()
                assert lock==(False,)*7
                assert c.execute("SELECT count(*) FROM pg_auth_members WHERE member=(SELECT oid FROM pg_roles WHERE rolname='genui_lock')").fetchone()==(0,)
                assert c.execute("SELECT has_schema_privilege('genui_lock','genui_private','CREATE'),has_schema_privilege('genui_lock','genui','CREATE')").fetchone()==(False,False)
                functions=c.execute("""SELECT p.proname,pg_get_userbyid(p.proowner),p.prosecdef,p.proconfig
                  FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='genui_private' ORDER BY p.proname""").fetchall()
                # The two 0002 fences stay genui_lock's; the six D7.1 admission
                # functions (checked in detail by admission_checks) are genui_admit's.
                assert sorted(r for r in functions if r[1]=='genui_lock')==[(name,'genui_lock',True,['search_path=pg_catalog, genui, pg_temp'])
                    for name in ('lock_member_authority','lock_work_revision')]
                assert sorted(r[0] for r in functions if r[1]!='genui_lock')==sorted(ADMIT_FUNCTIONS)
                assert c.execute("""SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace,
                  LATERAL aclexplode(coalesce(p.proacl,acldefault('f',p.proowner))) a
                  WHERE n.nspname='genui_private' AND a.grantee=0 AND a.privilege_type='EXECUTE'""").fetchone()==(0,)
                assert c.execute("SELECT bool_and(proisstrict) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='genui_private'").fetchone()==(True,)
                assert c.execute("""SELECT table_name,column_name FROM information_schema.column_privileges
                  WHERE grantee='genui_lock' AND privilege_type='UPDATE' ORDER BY table_name,column_name""").fetchall()==[('memberships','membership_epoch'),('organizations','status'),('saved_work','version')]
            passed('all real tenant relations have owner/FORCE-RLS/policies/tenant-leading indexes and FKs',table_count=len(tables))
            passed('lock definer is non-login non-inheriting minimal role with exact three UPDATE columns')

            def denied(name, role, statement, params=(), expected='42501', org=ORGANIZATIONS[0], set_role='genui_owner'):
                try:
                    with pg.connect(role) as c:
                        scope(c,org)
                        # The operator identity acts as genui_owner, or (fixture only)
                        # as genui_admit through its SET-only membership.
                        if role=='genui_migration': c.execute(sql.SQL('SET LOCAL ROLE {}').format(sql.Identifier(set_role)))
                        c.execute(statement,params)
                except psycopg.Error as e:
                    assert e.sqlstate==expected,(name,e.sqlstate)
                    passed(name,sqlstate=e.sqlstate)
                else: raise AssertionError('expected refusal: '+name)

            # 0012 (APP-113 review): a member's slash-command name is unique, case-folded,
            # whichever task names it (the route answers name_taken on the refusal).
            named="INSERT INTO genui.saved_work(organization_id,app_id,work_id,publication_id,version,context,created_by) VALUES (%s,%s,%s,%s,1,%s,%s)"
            with pg.connect() as c:
                scope(c); c.execute('SET LOCAL ROLE genui_owner')
                c.execute(named,(ORGANIZATIONS[0],APP,str(uuid.uuid4()),PUB,'forma-command:/qualify-name\nfirst',P))
                c.execute(named,(ORGANIZATIONS[0],APP,str(uuid.uuid4()),PUB,'forma-command:/qualify-name-two\nfirst',P))
            denied('a member cannot hold the same slash-command name twice','genui_migration',named,
                   (ORGANIZATIONS[0],APP,str(uuid.uuid4()),PUB,'forma-command:/QUALIFY-name\nsecond',P),expected='23505')

            for role in ('genui_workspace','genui_control','genui_data','genui_worker'):
                with pg.connect(role) as c:
                    assert c.execute('SELECT count(*) FROM genui.organizations').fetchone()==(0,)
                    for org in ORGANIZATIONS:
                        scope(c,org)
                        assert c.execute('SELECT organization_id::text FROM genui.organizations').fetchall()==[(org,)]
                        assert c.execute('SELECT count(*) FROM genui.organizations WHERE organization_id=%s',(ORGANIZATIONS[1-ORGANIZATIONS.index(org)],)).fetchone()==(0,)
                passed(role+' actual own-tenant positive and missing/foreign scope denial')
                accessible=[]
                with pg.connect(role) as c:
                    for (table,) in tables:
                        allowed=c.execute('SELECT has_table_privilege(current_user,%s,\'SELECT\')',('genui.'+table,)).fetchone()[0]
                        if not allowed: continue
                        accessible.append(table)
                        for org in ORGANIZATIONS:
                            scope(c,org)
                            assert c.execute(sql.SQL('SELECT DISTINCT organization_id::text FROM genui.{}').format(sql.Identifier(table))).fetchall()==[(org,)]
                        c.execute("SELECT set_config('genui.organization_id','',true)")
                        assert c.execute(sql.SQL('SELECT count(*) FROM genui.{}').format(sql.Identifier(table))).fetchone()==(0,)
                        assert c.execute('SELECT has_table_privilege(current_user,%s,\'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER\')',('genui.'+table,)).fetchone()==(False,)
                passed(role+' RLS on every granted real relation',tables=accessible)
                for target in ('genui_owner','genui_lock','genui_migration'):
                    denied(role+' cannot SET ROLE '+target,role,'SET ROLE '+target)
                for table in ('organizations','memberships','publications','saved_work','internal_records','internal_record_revisions'):
                    denied(role+' no direct UPDATE '+table,role,'UPDATE genui.'+table+' SET organization_id=organization_id')
                for table in ('publications','internal_records','internal_record_revisions'):
                    denied(role+' no direct INSERT '+table,role,'INSERT INTO genui.'+table+' DEFAULT VALUES')
                denied(role+' cannot TRUNCATE publications',role,'TRUNCATE genui.publications')
                denied(role+' cannot bypass RLS',role,'SET LOCAL row_security=off; SELECT * FROM genui.organizations')
                denied(role+' malformed scope fails closed',role,"SELECT set_config('genui.organization_id','malformed',true); SELECT * FROM genui.organizations",expected='22P02')
                if role=='genui_worker':
                    denied('worker cannot execute authority fence',role,'SELECT * FROM genui_private.lock_member_authority(%s,%s,1)',(ORGANIZATIONS[0],P))
                else:
                    with pg.connect(role) as c:
                        scope(c)
                        assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,1)',(ORGANIZATIONS[0],P)).fetchone()==(True,True,1)
                        assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,2)',(ORGANIZATIONS[0],P)).fetchall()==[]
                    passed(role+' permitted bounded member fence, mismatched epoch refuses')
                if role!='genui_data':
                    denied(role+' cannot execute work fence',role,'SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,PUB))
            with pg.connect('genui_data') as c:
                scope(c)
                assert c.execute('SELECT publication_id::text,work_version FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,PUB)).fetchone()==(PUB,1)
                assert c.execute('SELECT count(*) FROM genui.internal_records').fetchone()==(1,)
            passed('data has real record read and bounded work lock, no UPDATE authority')
            with pg.connect('genui_data') as c:
                scope(c)
                member_args=[ORGANIZATIONS[0],P,1]
                work_args=[ORGANIZATIONS[0],APP,WORK,PUB,1]
                for index in range(len(member_args)):
                    args=member_args.copy();args[index]=None
                    assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',args).fetchall()==[]
                for index in range(len(work_args)):
                    args=work_args.copy();args[index]=None
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,%s)',args).fetchall()==[]
                for epoch in (0,9007199254740992):
                    scope(c,epoch=epoch)
                    assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchall()==[]
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',work_args[:4]).fetchall()==[]
                scope(c)
                for setting in ('genui.principal_id','genui.membership_epoch','genui.organization_id'):
                    c.execute('SELECT set_config(%s,\'\',true)',(setting,))
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',work_args[:4]).fetchall()==[]
                    scope(c)
                assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,2)',work_args[:4]).fetchall()==[]
                assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,str(uuid.uuid4()))).fetchall()==[]
            passed('NULL or incomplete scope, unsafe epochs and stale work pin fail closed')
            for setting in ('genui.principal_id','genui.membership_epoch'):
                denied('malformed '+setting+' refuses work fence','genui_data',
                       'SELECT set_config(\''+setting+'\',\'malformed\',true); SELECT * FROM genui_private.lock_work_revision(\''+ORGANIZATIONS[0]+'\',\''+APP+'\',\''+WORK+'\',\''+PUB+'\',1)',expected='22P02')
            with pg.connect() as c:
                c.execute('SET LOCAL ROLE genui_owner');scope(c)
                assert c.execute('SELECT count(*) FROM genui.organizations').fetchone()==(1,)
                assert c.execute('SELECT count(*) FROM genui.organizations WHERE organization_id=%s',(ORGANIZATIONS[1],)).fetchone()==(0,)
            passed('actual NOLOGIN table owner remains subject to FORCE RLS')
            denied('real FK refuses foreign-only app','genui_migration',
                   "INSERT INTO genui.saved_work VALUES (%s,%s,%s,%s,1,'invalid')",
                   (ORGANIZATIONS[0],'00000000-0000-4000-8000-0000000000c9',str(uuid.uuid4()),'00000000-0000-4000-8000-0000000000cc'),expected='23503')
            denied('owner tenant policy refuses cross-organization write','genui_migration',
                   "INSERT INTO genui.spaces VALUES (%s,%s,'invalid')",(ORGANIZATIONS[1],str(uuid.uuid4())),expected='42501')
            denied('unknown permission check rejects even operator seed','genui_migration',
                   "INSERT INTO genui.app_permissions VALUES (%s,%s,%s,'unknown')",(ORGANIZATIONS[0],P,APP),expected='23514')
            denied('record binding refuses NULL revision policy','genui_migration',
                   "UPDATE genui.work_bindings SET revision_policy=NULL WHERE organization_id=%s AND kind='records'",(ORGANIZATIONS[0],),expected='23514')
            denied('source binding refuses changed normalized digest','genui_migration',
                   "UPDATE genui.work_bindings SET source_digest='sha256:'||repeat('f',64) WHERE organization_id=%s AND kind='source'",(ORGANIZATIONS[0],),expected='23503')
            denied('source dependency cannot name a different input binding','genui_migration',
                   "UPDATE genui.work_source_dependencies SET input_id='unbound_input' WHERE organization_id=%s",(ORGANIZATIONS[0],),expected='23503')
            denied('publication refuses wrong definition digest','genui_migration',
                   "UPDATE genui.publications SET definition_digest='sha256:'||repeat('f',64) WHERE organization_id=%s",(ORGANIZATIONS[0],),expected='23503')

            # Barrier is the observed pg_stat_activity lock wait, not a timing guess.
            def race(writer_first, organization=False):
                with pg.connect() as writer, pg.connect('genui_data') as reader, pg.connect('postgres',autocommit=True) as observer:
                    writer.execute('SET LOCAL ROLE genui_owner');scope(writer)
                    epoch=writer.execute('SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(ORGANIZATIONS[0],P)).fetchone()[0]
                    scope(reader,epoch=epoch)
                    errors=[]; result=[]; finished=threading.Event()
                    def revoke():
                        try:
                            if organization:
                                writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR UPDATE',(ORGANIZATIONS[0],))
                                writer.execute("UPDATE genui.organizations SET status='suspended' WHERE organization_id=%s",(ORGANIZATIONS[0],))
                            else:
                                writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE',(ORGANIZATIONS[0],))
                                writer.execute('SELECT principal_id FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR UPDATE',(ORGANIZATIONS[0],P))
                                writer.execute("UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s",(ORGANIZATIONS[0],P))
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    def read():
                        try: result.extend(reader.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchall())
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    if writer_first:
                        revoke(); assert not errors;finished.clear();thread=threading.Thread(target=read);pid=reader.info.backend_pid
                    else:
                        read();assert result==[(True,True,epoch)];finished.clear();thread=threading.Thread(target=revoke);pid=writer.info.backend_pid
                    thread.start()
                    deadline=time.monotonic()+2
                    while time.monotonic()<deadline:
                        state=observer.execute('SELECT wait_event_type FROM pg_stat_activity WHERE pid=%s',(pid,)).fetchone()
                        if state==('Lock',): break
                        if finished.is_set(): raise AssertionError('fence did not block')
                        time.sleep(.01)
                    else: raise AssertionError('lock-wait observation deadline')
                    if writer_first: writer.commit()
                    else: reader.commit()
                    thread.join(4);assert not thread.is_alive() and not errors,errors
                    if writer_first: assert result==([(False,True,epoch)] if organization else [])
                    writer.commit();reader.commit()
                with pg.connect() as c:
                    c.execute('SET LOCAL ROLE genui_owner');scope(c)
                    c.execute("UPDATE genui.organizations SET status='active' WHERE organization_id=%s",(ORGANIZATIONS[0],))
                    c.execute("UPDATE genui.memberships SET status='active',membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s",(ORGANIZATIONS[0],P))
                passed(('organization suspension' if organization else 'membership revocation')+(' writer-first' if writer_first else ' reader-first')+' actual lock ordering')
            for org in (False,True):
                for first in (False,True): race(first,org)
            def work_race(writer_first):
                with pg.connect() as writer, pg.connect('genui_data') as reader, pg.connect('postgres',autocommit=True) as observer:
                    writer.execute('SET LOCAL ROLE genui_owner');scope(writer)
                    epoch=writer.execute('SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(ORGANIZATIONS[0],P)).fetchone()[0]
                    version=writer.execute('SELECT version FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s',(ORGANIZATIONS[0],APP,WORK)).fetchone()[0]
                    scope(reader,epoch=epoch)
                    errors=[];result=[];finished=threading.Event()
                    def advance():
                        try:
                            writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE',(ORGANIZATIONS[0],))
                            writer.execute('SELECT principal_id FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR SHARE',(ORGANIZATIONS[0],P))
                            writer.execute('SELECT work_id FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s FOR UPDATE',(ORGANIZATIONS[0],APP,WORK))
                            writer.execute('UPDATE genui.saved_work SET version=version+1 WHERE organization_id=%s AND app_id=%s AND work_id=%s',(ORGANIZATIONS[0],APP,WORK))
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    def read_work():
                        try:
                            assert reader.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchone()==(True,True,epoch)
                            result.extend(reader.execute('SELECT publication_id::text,work_version FROM genui_private.lock_work_revision(%s,%s,%s,%s,%s)',(ORGANIZATIONS[0],APP,WORK,PUB,version)).fetchall())
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    if writer_first:
                        advance();assert not errors;finished.clear();thread=threading.Thread(target=read_work);pid=reader.info.backend_pid
                    else:
                        read_work();assert result==[(PUB,version)];finished.clear();thread=threading.Thread(target=advance);pid=writer.info.backend_pid
                    thread.start();deadline=time.monotonic()+2
                    while time.monotonic()<deadline:
                        if observer.execute('SELECT wait_event_type FROM pg_stat_activity WHERE pid=%s',(pid,)).fetchone()==('Lock',): break
                        if finished.is_set(): raise AssertionError('work fence did not block')
                        time.sleep(.01)
                    else: raise AssertionError('work lock-wait observation deadline')
                    if writer_first: writer.commit()
                    else: reader.commit()
                    thread.join(4);assert not thread.is_alive() and not errors,errors
                    if writer_first: assert result==[]
                    writer.commit();reader.commit()
                passed('saved-work version advance '+('writer-first' if writer_first else 'reader-first')+' actual lock ordering')
            for first in (False,True): work_race(first)
            before=pg.digest()
            try:
                with pg.connect() as c: seed(c,bundles)
            except ValueError: pass
            else: raise AssertionError('seed reset an existing changed membership epoch')
            assert pg.digest()==before
            passed('operator seed refuses changed existing epoch without overwriting authority')
            admission_checks(pg,passed,denied)
            definition_work_checks(pg,passed,denied)
            session_checks(pg,passed,denied)
            command_checks(pg,passed,denied)
            report['source_sha256']={str(p.relative_to(Path(__file__).resolve().parents[2])):hashlib.sha256(p.read_bytes()).hexdigest()
                                    for p in (Path(runtime.__file__),Path(__file__),Path(__file__).with_name('tenant_seed.py'),MIGRATIONS/'manifest.json',
                                              *(MIGRATIONS/item['file'] for item in manifest['migrations']+manifest['catalog']))}
            report['passed']=True
    finally:
        report['container_removed']=pg.removed
    return report


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path);args=parser.parse_args()
    result=qualify(); rendered=json.dumps(result,indent=2)+'\n'
    if args.output: args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(rendered)
    print(rendered,end='')
