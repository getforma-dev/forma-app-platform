"""Operator-only APP-008 contract fixtures; never a signup or application route.

Requires the existing contract validator environment. Source digests and admission
results remain explicitly synthetic; APP-011 owns actual source-byte packaging.
"""
import copy
import json
from pathlib import Path
import sys

CONTRACTS = Path(__file__).resolve().parents[2] / "contracts"
sys.path.insert(0, str(CONTRACTS))
from validate import canonical, digest, validate_bundle  # noqa: E402

ORGANIZATIONS = ("00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002")
SPACE = "00000000-0000-4000-8000-000000000384"
ISSUER = "app008-fixture-only"


def make_bundles():
    result = []
    for name, organization in (("northstar", ORGANIZATIONS[0]), ("northstar", ORGANIZATIONS[1]), ("spend-review", ORGANIZATIONS[1])):
        # PH01 seeding is edition impl-2026-10-28.1 data (operator-seeded revisions; ADR-009 D9.3), so it reads
        # that edition's retained fixtures, not the current edition's.
        bundle = copy.deepcopy(json.loads((CONTRACTS / "fixtures" / "impl-2026-10-28.1" / (name + ".json")).read_text()))
        old = bundle["context"]["organization_id"]
        def replace(value):
            if isinstance(value, dict): return {k: replace(v) for k, v in value.items()}
            if isinstance(value, list): return [replace(v) for v in value]
            return organization if value == old else value
        bundle = replace(bundle)
        receipt = bundle["receipt"]
        receipt["digest"] = digest("query-receipt", {k: v for k, v in receipt.items() if k != "digest"})
        validate_bundle(bundle)
        result.append(bundle)
    return result


def seed(connection, bundles):
    """Caller owns the entire transaction; a changed existing seed refuses whole.

    All bundles validate before the first SQL write. This only seeds the declared
    synthetic contract shape and never provisions a provider or active entitlement.
    """
    from psycopg import sql
    if connection.autocommit:
        raise ValueError("operator seed requires one caller-owned transaction")
    for bundle in bundles:
        validate_bundle(bundle)
        if bundle["context"]["organization_id"] not in ORGANIZATIONS:
            raise ValueError("seed organization refused")
    if connection.execute("SELECT session_user::text,current_user::text,current_database()").fetchone() != ("genui_migration", "genui_migration", "genui"):
        raise ValueError("operator seed identity refused")
    connection.execute("SET LOCAL ROLE genui_owner")

    def put(table, fields, values, key_count):
        # table/field names are fixed call sites below, never fixture values.
        identifiers = sql.SQL(',').join(map(sql.Identifier, fields))
        params = sql.SQL(',').join(sql.Placeholder() for _ in fields)
        connection.execute(sql.SQL("INSERT INTO genui.{} ({}) VALUES ({}) ON CONFLICT DO NOTHING").format(sql.Identifier(table), identifiers, params), values)
        where = sql.SQL(' AND ').join(sql.SQL('{}=%s').format(sql.Identifier(k)) for k in fields[:key_count])
        check = sql.SQL("SELECT ROW({}) IS NOT DISTINCT FROM ROW({}) FROM genui.{} WHERE {}").format(identifiers, params, sql.Identifier(table), where)
        if connection.execute(check, (*values, *values[:key_count])).fetchall() != [(True,)]:
            raise ValueError("existing seed differs; no authority overwritten")

    for b in bundles:
        o = b["context"]["organization_id"]; p = b["context"]["principal_id"]
        d, pub, w = b["definition"], b["publication"], b["work"]
        a, r, publication, work = d["app_id"], d["revision_id"], pub["publication_id"], w["work_id"]
        tenant = "ten_" + o
        connection.execute("SELECT set_config('genui.organization_id',%s,true)", (o,))
        put('organizations', ('organization_id','display_name','status'), (o,'Synthetic organization '+o[-4:],'active'), 1)
        put('provider_tenant_bindings', ('organization_id','issuer','provider_tenant_id','active'), (o,ISSUER,tenant,True), 3)
        actors = {p: ('active',1), pub['approver_id']: ('active',1),
                  '00000000-0000-4000-8000-00000000006e': ('revoked',2),
                  '00000000-0000-4000-8000-00000000006f': ('active',1),
                  '00000000-0000-4000-8000-000000000f00': ('active',1),
                  '00000000-0000-4000-8000-0000000000ca': ('active',1)}
        for principal, (status, epoch) in actors.items():
            put('principals', ('organization_id','principal_id','issuer','provider_tenant_id','provider_subject'), (o,principal,ISSUER,tenant,'usr_'+principal), 2)
            put('memberships', ('organization_id','principal_id','status','membership_epoch'), (o,principal,status,epoch), 2)
        put('organization_permissions', ('organization_id','principal_id','permission'),
            (o,'00000000-0000-4000-8000-000000000f00','admin_policy'), 3)
        put('spaces', ('organization_id','space_id','display_name'), (o,SPACE,'Synthetic fixture space'), 2)
        put('apps', ('organization_id','app_id','space_id','owner_principal_id'), (o,a,SPACE,p), 2)
        put('definition_revisions', ('organization_id','app_id','revision_id','schema_version','digest','canonical_content'), (o,a,r,1,d['digest'],canonical(d['content'])), 3)
        for item in d['content']['inputs']:
            put('definition_inputs', ('organization_id','app_id','revision_id','input_id','kind','contract_digest','canonical_contract'),
                (o,a,r,item['id'],item['kind'],digest('input-contract',item),canonical(item)), 4)
        for item in d['content']['record_schemas']:
            put('definition_record_schemas', ('organization_id','app_id','revision_id','schema_id','schema_version','digest','canonical_schema'),
                (o,a,r,item['id'],item['version'],digest('record-schema',item),canonical(item)), 5)
        policy = pub['policy']
        put('publication_policies', ('organization_id','policy_id','policy_version','kind'), (o,policy['id'],policy['version'],policy['kind']), 3)
        put('publications', ('organization_id','app_id','publication_id','definition_revision_id','definition_digest','policy_id','policy_version','policy_kind','author_id','approver_id','approver_epoch','renderer_version','operator_version','admission_digest','published_at','canonical_envelope'),
            (o,a,publication,r,pub['definition_digest'],policy['id'],policy['version'],policy['kind'],pub['author_id'],pub['approver_id'],pub['approver_epoch'],1,1,pub['admission_digest'],pub['published_at'],canonical(pub)), 3)
        for item in d['content']['inputs']:
            put('publication_inputs', ('organization_id','app_id','publication_id','input_id','definition_revision_id','kind','contract_digest'),
                (o,a,publication,item['id'],r,item['kind'],digest('input-contract',item)), 4)
        for item in d['content']['record_schemas']:
            put('publication_record_schemas', ('organization_id','app_id','publication_id','schema_id','schema_version','definition_revision_id','digest'),
                (o,a,publication,item['id'],item['version'],r,digest('record-schema',item)), 5)
        put('saved_work', ('organization_id','app_id','work_id','publication_id','version','context'), (o,a,work,publication,w['version'],w['context']), 3)
        for item in b['sources']:
            put('sources', ('organization_id','source_id'), (o,item['source_id']), 2)
            put('source_revisions', ('organization_id','source_id','revision_id','content_digest','normalized_digest','contract_digest','deleted_at','canonical_envelope'),
                (o,item['source_id'],item['revision_id'],item['content_digest'],item['normalized_digest'],digest('input-contract',item['input_contract']),item.get('deleted_at'),canonical(item)), 3)
        for item in w['record_collections']:
            put('record_collections', ('organization_id','app_id','work_id','collection_id','publication_id','schema_id','schema_version'),
                (o,a,work,item['id'],publication,item['schema_id'],item['schema_version']), 4)
        for item in w['bindings']:
            put('work_bindings', ('organization_id','app_id','work_id','input_id','publication_id','kind','contract_digest','source_id','source_revision_id','source_digest','collection_id','schema_id','schema_version','revision_policy'),
                (o,a,work,item['input_id'],publication,item['kind'],item['contract_digest'],*(item.get(k) for k in ('source_id','source_revision_id','source_digest','collection_id','schema_id','schema_version','revision_policy'))), 4)
        for item in w['source_dependencies']:
            put('work_source_dependencies', ('organization_id','app_id','work_id','input_id','source_id','source_revision_id','source_digest'),
                (o,a,work,item['input_id'],item['source_id'],item['source_revision_id'],item['source_digest']), 6)
        outcome = b['outcome']; collection = w['record_collections'][0]['id']; record = outcome['record_id']; version = outcome['record_version']
        put('internal_records', ('organization_id','app_id','work_id','collection_id','record_id','current_version'), (o,a,work,collection,record,version), 5)
        put('internal_record_revisions', ('organization_id','app_id','work_id','collection_id','record_id','version','creator_id','updater_id','commit_sequence','committed_at','canonical_values'),
            (o,a,work,collection,record,version,p,p,outcome['commit_sequence'],outcome['committed_at'],canonical(b['command']['values'])), 6)
        for item in w['source_dependencies']:
            put('record_source_dependencies', ('organization_id','app_id','work_id','collection_id','record_id','record_version','input_id','source_id','source_revision_id','source_digest'),
                (o,a,work,collection,record,version,item['input_id'],item['source_id'],item['source_revision_id'],item['source_digest']), 9)
        for permission in ('discover','run','edit_definition'):
            put('app_permissions', ('organization_id','principal_id','app_id','permission'), (o,p,a,permission), 4)
        put('app_permissions', ('organization_id','principal_id','app_id','permission'), (o,pub['approver_id'],a,'review_publish'), 4)
        for permission in ('record_read','record_write','share_work'):
            put('work_permissions', ('organization_id','principal_id','app_id','work_id','permission'), (o,p,a,work,permission), 5)
        for source in b['sources']:
            put('source_permissions', ('organization_id','principal_id','source_id','permission'), (o,p,source['source_id'],'source_read'), 4)
    connection.execute("RESET ROLE")


if __name__ == '__main__':
    print(json.dumps({'scope':'APP-008 synthetic contract shapes; not imported data or live admission', 'bundles':make_bundles()}, indent=2))
