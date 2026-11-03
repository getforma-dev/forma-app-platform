"""Additive closed record-components receipt schema; no old receipt changes."""
def receipt_schema(UUID, DIGEST, ID, INT, POS):
    def obj(properties):
        return {'type':'object','properties':properties,'required':list(properties),'additionalProperties':False}
    def arr(items,maximum):return {'type':'array','items':items,'maxItems':maximum}
    actor={'oneOf':[obj({'kind':{'const':'member'},'principal_id':UUID,'membership_epoch':POS}),obj({'kind':{'const':'guest'},'account_id':UUID,'share_id':UUID,'share_version':POS})]}
    date={'type':'string','pattern':r'^[0-9]{4}-[0-9]{2}-[0-9]{2}(?![\s\S])','format':'date'}
    filters=arr({'oneOf':[obj({'id':ID,'value':{'type':'string','minLength':1,'maxLength':200}}),obj({'id':ID,'start':date,'end':date})]},8)
    revision=obj({'record_id':UUID,'version':POS,'commit_sequence':POS})
    snapshot=arr(obj({'input_id':ID,'collection_id':UUID,'schema_id':ID,'schema_version':POS,'records':arr(revision,2000)}),32)
    budget=obj({'origin':{'const':'preset'},'minor_units':{'type':'string','pattern':r'^(0|[1-9][0-9]{0,37})(?![\s\S])'},'currency':{'const':'USD'},'minor_unit_exponent':{'const':2}})
    components=arr(obj({'view_id':ID,'kind':{'enum':['records_list','record_summary','record_trend','record_ranking','record_budget']},'population':{'type':'integer','minimum':0,'maximum':2000},'truncated_page':{'type':'boolean'},'budget':{'oneOf':[{'type':'null'},budget]}}),32)
    return obj({'kind':{'const':'record_components_receipt'},'schema_version':{'const':1},'organization_id':UUID,'app_id':UUID,'work_id':UUID,'publication_id':UUID,'definition_digest':DIGEST,'work_version':POS,'policy_epoch':POS,'actor':actor,'operator_version':{'const':1},'parameters':obj({'filter_epoch':INT,'filters':filters}),'components':components,'snapshot':snapshot,'snapshot_digest':DIGEST,'result_digest':DIGEST,'partial':{'const':False},'total_scope':{'const':'full_authorized_filtered_collection'},'digest':DIGEST})

def validate_semantics(document):
    components=document['components']
    if len({c['view_id'] for c in components})!=len(components):raise ValueError('duplicate record component')
    if any((c['kind']=='record_budget') != (c['budget'] is not None) for c in components):raise ValueError('record budget provenance mismatch')
    filters=document['parameters']['filters']
    if len({f['id'] for f in filters})!=len(filters):raise ValueError('duplicate record filter')
    if any('start' in f and f['start']>=f['end'] for f in filters):raise ValueError('empty record date range')
    snapshots=document['snapshot']
    if len({s['input_id'] for s in snapshots})!=len(snapshots):raise ValueError('duplicate record input')
    revisions=[(s['input_id'],s['collection_id'],r['record_id']) for s in snapshots for r in s['records']]
    if len(revisions)>2000 or len(set(revisions))!=len(revisions):raise ValueError('record snapshot bound or duplicate')
