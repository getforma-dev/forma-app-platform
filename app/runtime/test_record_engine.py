import copy
import datetime
import importlib.util
import io
import json
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('record_engine',Path(__file__).with_name('record_engine.py'))
engine=importlib.util.module_from_spec(spec);spec.loader.exec_module(engine)

ALEX='00000000-0000-4000-8000-0000000000a1'
def record(n,amount,day,restaurant='Cafe A'):
    return {'id':f'00000000-0000-4000-8000-{n:012d}','version':1,'sequence':n,'creator':ALEX,'values':[
        {'field_id':'amount','value':{'type':'money','minor_units':str(amount),'currency':'USD','minor_unit_exponent':2}},
        {'field_id':'date','value':{'type':'date','value':day}},
        {'field_id':'restaurant','value':{'type':'text','value':restaurant}},
    ]}
def request():
    common={'version':1,'input_id':'meals','schema_id':'meal'}
    def view(id,kind,**extra):return {'id':id,'title':id,'kind':kind,'renderer_version':1,'records':dict(common,**extra)}
    money={'op':'sum','field':'amount'}
    components=[view('list','records_list',columns=['restaurant','date','amount'],limit=1),view('spend','record_summary',measure=money),view('count','record_summary',measure={'op':'count'}),view('month','record_trend',measure=money,date_field='date'),view('top','record_ranking',measure=money,group_field='restaurant',limit=1),view('budget','record_budget',measure=money,budget={'origin':'preset','minor_units':'10000','currency':'USD','minor_unit_exponent':2})]
    return {'kind':'record_components_v1','deadline_ms':1800,'parameters':{'filter_epoch':3,'filters':[{'id':'period','start':'2026-11-01','end':'2026-12-01'}]},'payload':{'creators':[{'id':ALEX,'label':'Alex Rivera'}],'components':components,'filters':[{'id':'period','input_id':'meals','field':'date','kind':'date_range','targets':[v['id'] for v in components]},{'id':'restaurant','input_id':'meals','field':'restaurant','kind':'text_contains','targets':['list']}],'datasets':[{'input_id':'meals','schema_id':'meal','fields':[{'id':'amount','value_type':{'type':'money','currency':'USD','minor_unit_exponent':2},'nullable':False},{'id':'date','value_type':{'type':'date'},'nullable':False},{'id':'restaurant','value_type':{'type':'text'},'nullable':False}],'rows':[record(5,9999,'2026-12-01'),record(4,2500,'2026-11-04','Cafe B'),record(3,750,'2026-11-03'),record(2,1250,'2026-11-02'),record(1,9999,'2026-10-31')]}]}}

class RecordEngine(unittest.TestCase):
    def test_full_filtered_totals_independent_of_page_and_top_n(self):
        result=engine.execute(request());parts={v['id']:v for v in result['components']}
        self.assertEqual(parts['spend']['value'],'4500');self.assertEqual(parts['count']['value'],'3')
        self.assertEqual(parts['list']['population'],3);self.assertEqual(len(parts['list']['rows']),1);self.assertTrue(parts['list']['truncated_page'])
        self.assertEqual(parts['top']['value'],'4500');self.assertEqual(parts['top']['groups'],[{'label':'Cafe B','missing':False,'value':'2500','count':1}]);self.assertTrue(parts['top']['truncated_page'])
        self.assertEqual(parts['month']['groups'],[{'label':'2026-11','missing':False,'value':'4500','count':3}]);self.assertEqual(parts['budget']['budget']['ratio_basis_points'],'4500');self.assertEqual(parts['budget']['budget']['remaining_minor_units'],'5500')
    def test_filter_before_page_and_only_explicit_targets(self):
        r=request();r['payload']['datasets'][0]['rows']=[record(i,1,'2026-11-01','Newest') for i in range(300,0,-1)]+[record(301,77,'2026-11-02','Needle')]
        r['parameters']['filters'].append({'id':'restaurant','value':'needle'})
        parts={v['id']:v for v in engine.execute(r)['components']}
        self.assertEqual(parts['list']['population'],1);self.assertEqual(parts['list']['rows'][0]['id'],record(301,0,'2026-11-01')['id']);self.assertEqual(parts['spend']['value'],'377')
    def test_exact_negative_large_money_null_and_zero_budget(self):
        r=request();r['payload']['datasets'][0]['rows']=[record(1,10**30,'2026-11-01'),record(2,-25,'2026-11-02'),record(3,0,'2026-11-02')]
        r['payload']['datasets'][0]['rows'][2]['values'][0]['value']={'type':'null'}
        r['payload']['components'][-1]['records']['budget']['minor_units']='0'
        parts={v['id']:v for v in engine.execute(r)['components']};self.assertEqual(parts['spend']['value'],str(10**30-25));self.assertIsNone(parts['budget']['budget']['ratio_basis_points'])
    def test_null_group_never_collides_with_literal_label(self):
        r=request();r['payload']['datasets'][0]['rows']=[record(1,100,'2026-11-01','Missing value'),record(2,200,'2026-11-01')]
        r['payload']['datasets'][0]['rows'][1]['values'][2]['value']={'type':'null'}
        r['payload']['components'][4]['records']['limit']=5
        groups=engine.execute(r)['components'][4]['groups']
        self.assertEqual(len(groups),2);self.assertEqual([g['value'] for g in groups],['200','100']);self.assertEqual([g['missing'] for g in groups],[True,False])

    def test_empty_is_real_zero(self):
        r=request();r['payload']['datasets'][0]['rows']=[]
        parts={v['id']:v for v in engine.execute(r)['components']};self.assertEqual(parts['spend']['value'],'0');self.assertEqual(parts['spend']['state'],'empty');self.assertEqual(parts['list']['rows'],[])
    def test_overflow_bound_deadline_unknown_filter_and_invalid_calendar_refuse(self):
        cases=[]
        r=request();r['payload']['datasets'][0]['rows']=[record(i,1,'2026-11-01') for i in range(2001)];cases.append(r)
        r=request();r['payload']['datasets'][0]['rows']=[record(1,10**38-1,'2026-11-01'),record(2,1,'2026-11-01')];cases.append(r)
        r=request();r['parameters']['filters'][0]['id']='foreign';cases.append(r)
        r=request();r['parameters']['filters'][0]['start']='2026-02-30';cases.append(r)
        r=request();r['parameters']['filters'].append(r['parameters']['filters'][0]);cases.append(r)
        for r in cases:
            with self.subTest(r=str(r)[:70]),self.assertRaises(engine.Refused):engine.execute(r)
        with self.assertRaisesRegex(engine.Refused,'deadline'):engine.execute(request(),start=0)
    def test_child_entry_protocol_works_in_isolated_python(self):
        import subprocess,sys
        p=subprocess.run([sys.executable,'-I',str(Path(__file__).with_name('query_engine.py'))],input=json.dumps(request()).encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
        result=json.loads(p.stdout);self.assertEqual(result['outcome'],'record_components');self.assertEqual(result['components'][1]['value'],'4500')


# ADR-020 C1 and C4 (APP-204): the countdown, the ranking by creator and the
# matches measure, computed over the snapshot the parent captured under the
# viewer's own rows. Principal ids enter the payload; only labels leave it.
BEN='00000000-0000-4000-8000-0000000000b1'
ANA='00000000-0000-4000-8000-0000000000b2'
CAL='00000000-0000-4000-8000-0000000000b3'
def item_ref(n):return {'type':'reference','input_id':'items','record_id':f'00000000-0000-4000-8000-{n:012d}'}
def item(n,prompt,state,opened):
    return {'id':f'00000000-0000-4000-8000-{n:012d}','version':1,'sequence':n,'creator':ALEX,'values':[
        {'field_id':'prompt','value':{'type':'text','value':prompt} if prompt is not None else {'type':'null'}},
        {'field_id':'state','value':{'type':'enum','value':state}},
        {'field_id':'opened_at','value':{'type':'timestamp','value':opened} if opened else {'type':'null'}}]}
def answer(n,creator,ref,choice,sequence=None):
    return {'id':f'00000000-0000-4000-8000-{n:012d}','version':1,'sequence':sequence or n,'creator':creator,'values':[
        {'field_id':'item','value':item_ref(ref)},{'field_id':'choice','value':{'type':'enum','value':choice}}]}
ITEM_FIELDS=[{'id':'prompt','value_type':{'type':'text'},'nullable':True},{'id':'state','value_type':{'type':'enum','values':['open','closed']},'nullable':False},{'id':'opened_at','value_type':{'type':'timestamp'},'nullable':True}]
ANSWER_FIELDS=[{'id':'item','value_type':{'type':'reference','target_input':'items'},'nullable':False},{'id':'choice','value_type':{'type':'enum','values':['a','b','c','d']},'nullable':False}]
CLOSED={'field_id':'state','comparison':'eq','value':{'type':'enum','value':'closed'}}
def participation(items=None,responses=None,expected=None,limit=50):
    def view(id,kind,input_id,**extra):return {'id':id,'title':id,'kind':kind,'renderer_version':1,'records':dict({'version':1,'input_id':input_id,'schema_id':input_id},**extra)}
    matches={'op':'matches','key_input':'expected','fields':['item','choice']}
    components=[view('timer','countdown','items',timestamp_field='opened_at',offset_seconds=30,closed_when=CLOSED),
        view('board','record_ranking','responses',group_by='creator',measure=matches,limit=limit),
        view('right','record_summary','responses',measure=matches),
        view('busiest','record_ranking','responses',group_by='creator',measure={'op':'count'})]
    items=[item(1,'First','closed','2026-11-29T15:00:00Z'),item(2,'Second','open','2026-11-29T15:05:00Z')] if items is None else items
    responses=[answer(11,BEN,1,'a'),answer(12,BEN,2,'c'),answer(13,ANA,1,'b'),answer(14,ANA,2,'c'),answer(15,CAL,1,'a')] if responses is None else responses
    expected=[answer(21,ALEX,1,'a'),answer(22,ALEX,2,'c')] if expected is None else expected
    rows=items+responses+expected
    names={ALEX:'Alex Rivera',BEN:'Ben Okafor',ANA:'Ana Lima',CAL:'Member 00000000'}
    creators=[{'id':c,'label':names.get(c,'Member '+c[:8])} for c in sorted({r['creator'] for r in rows})]
    return {'kind':'record_components_v1','deadline_ms':1800,'parameters':{'filter_epoch':0,'filters':[]},'payload':{'creators':creators,'components':components,'filters':[],'datasets':[
        {'input_id':'items','schema_id':'items','fields':ITEM_FIELDS,'rows':items},
        {'input_id':'responses','schema_id':'responses','fields':ANSWER_FIELDS,'rows':responses},
        {'input_id':'expected','schema_id':'expected','fields':ANSWER_FIELDS,'rows':expected}]}}
def parts(request):return {v['id']:v for v in engine.execute(request)['components']}

class Countdown(unittest.TestCase):
    def test_the_current_record_is_the_highest_commit_sequence(self):
        timer=parts(participation())['timer']
        self.assertEqual((timer['state'],timer['population'],timer['value'],timer['label'],timer['closed']),('ready',2,'2026-11-29T15:05:30Z','Second',False))
        self.assertEqual(timer['value_type'],{'type':'timestamp'})
        # Closing (or touching) the older item makes it current: "most recently touched".
        r=participation();r['payload']['datasets'][0]['rows'][0]['sequence']=9
        timer=parts(r)['timer'];self.assertEqual((timer['label'],timer['value'],timer['closed']),('First','2026-11-29T15:00:30Z',True))
        # Row order in the payload never decides; the sequence does.
        r=participation();r['payload']['datasets'][0]['rows'].reverse()
        self.assertEqual(parts(r)['timer']['label'],'Second')
    def test_closed_when_and_a_null_timestamp_close_it(self):
        r=participation(items=[item(2,'Second','closed','2026-11-29T15:05:00Z')])
        timer=parts(r)['timer'];self.assertEqual((timer['value'],timer['closed']),('2026-11-29T15:05:30Z',True))
        timer=parts(participation(items=[item(2,'Second','open',None)]))['timer']
        self.assertEqual((timer['value'],timer['closed'],timer['state']),(None,True,'ready'))
        r=participation();del r['payload']['components'][0]['records']['closed_when']
        r['payload']['datasets'][0]['rows'][1]['values'][1]['value']['value']='closed'
        self.assertFalse(parts(r)['timer']['closed'],'without closed_when only a null target closes it')
    def test_offsets_cross_days_and_negative_offsets_are_exact(self):
        r=participation(items=[item(2,'Late','open','2026-12-31T23:59:50Z')])
        self.assertEqual(parts(r)['timer']['value'],'2027-01-01T00:00:20Z')
        r['payload']['components'][0]['records']['offset_seconds']=-604800
        self.assertEqual(parts(r)['timer']['value'],'2026-12-24T23:59:50Z')
        del r['payload']['components'][0]['records']['offset_seconds']
        self.assertEqual(parts(r)['timer']['value'],'2026-12-31T23:59:50Z')
    def test_empty_shows_nothing_and_the_label_is_the_first_text_bounded(self):
        timer=parts(participation(items=[]))['timer']
        self.assertEqual((timer['state'],timer['population'],timer['value'],timer['label'],timer['closed']),('empty',0,None,'',True))
        timer=parts(participation(items=[item(2,'x'*250,'open','2026-11-29T15:05:00Z')]))['timer'];self.assertEqual(timer['label'],'x'*200)
        timer=parts(participation(items=[item(2,None,'open','2026-11-29T15:05:00Z')]))['timer'];self.assertEqual(timer['label'],'')
        # Only countdowns carry label and closed.
        self.assertNotIn('label',parts(participation())['right']);self.assertNotIn('closed',parts(participation())['board'])
    def test_malformed_countdowns_refuse(self):
        cases=[]
        r=participation();r['payload']['components'][0]['records']['timestamp_field']='prompt';cases.append(r)
        r=participation();r['payload']['components'][0]['records']['offset_seconds']=604801;cases.append(r)
        r=participation();r['payload']['components'][0]['records']['offset_seconds']=1.5;cases.append(r)
        r=participation();r['payload']['components'][0]['records']['closed_when']=dict(CLOSED,comparison='like');cases.append(r)
        r=participation();r['payload']['components'][0]['records']['closed_when']=dict(CLOSED,field_id='unknown');cases.append(r)
        r=participation(items=[item(2,'Second','open','2026-11-29 15:05:00')]);cases.append(r)
        r=participation(items=[item(2,'Second','open','9999-12-31T23:59:59Z')]);cases.append(r)
        r=participation();r['payload']['filters']=[{'id':'prompt_filter','input_id':'items','field':'prompt','kind':'text_contains','targets':['timer']}]
        r['parameters']['filters']=[{'id':'prompt_filter','value':'First'}];cases.append(r)
        r=participation();r['payload']['components'][0]['records']['columns']=['prompt'];cases.append(r)
        for r in cases:
            with self.subTest(r=str(r['payload']['components'][0])[:90]),self.assertRaises(engine.Refused):engine.execute(r)

class RankingByCreator(unittest.TestCase):
    def test_reviewer_snapshot_ranks_every_contributor_by_label(self):
        board=parts(participation())['board']
        self.assertEqual(board['groups'],[{'label':'Ben Okafor','missing':False,'value':'2','count':2},{'label':'Ana Lima','missing':False,'value':'1','count':2},{'label':'Member 00000000','missing':False,'value':'1','count':1}])
        self.assertEqual((board['value'],board['population'],board['truncated_page'],board['value_type']),('4',5,False,{'type':'integer'}))
        self.assertEqual(parts(participation())['busiest']['groups'][0]['value'],'2')
    def test_contributor_snapshot_is_their_own_rows_and_no_key(self):
        # What the parent captures for Ben under own_rows: his responses, no expected row.
        r=participation(responses=[answer(11,BEN,1,'a'),answer(12,BEN,2,'c')],expected=[])
        board=parts(r)['board'];self.assertEqual(board['groups'],[{'label':'Ben Okafor','missing':False,'value':'0','count':2}])
        self.assertEqual(parts(r)['right']['value'],'0')
    def test_ties_follow_labels_then_a_stable_hidden_order(self):
        r=participation(responses=[answer(11,CAL,1,'a'),answer(12,ANA,1,'a'),answer(13,BEN,1,'a')])
        self.assertEqual([g['label'] for g in parts(r)['board']['groups']],['Ana Lima','Ben Okafor','Member 00000000'])
        # Two members who chose the same name stay two groups, in a stable order.
        next(c for c in r['payload']['creators'] if c['id']==ANA)['label']='Ben Okafor'
        groups=parts(r)['board']['groups'];self.assertEqual([g['label'] for g in groups],['Ben Okafor','Ben Okafor','Member 00000000'])
        r2=copy.deepcopy(r);r2['payload']['datasets'][1]['rows'].reverse();self.assertEqual(parts(r2)['board']['groups'],groups)
    def test_fifty_is_the_bound_and_truncation_is_reported(self):
        many=[f'00000000-0000-4000-8000-{900+i:012d}' for i in range(51)]
        r=participation(responses=[answer(100+i,c,1,'a') for i,c in enumerate(many)])
        board=parts(r)['board'];self.assertEqual(len(board['groups']),50);self.assertTrue(board['truncated_page']);self.assertEqual(board['population'],51)
        r=participation(limit=51)
        with self.assertRaises(engine.Refused):engine.execute(r)
    def test_labels_never_ids_leave_the_engine(self):
        text=json.dumps(engine.execute(participation()))
        for principal in (ALEX,BEN,ANA,CAL):self.assertNotIn(principal,text)
    def test_group_by_and_group_field_are_exclusive_and_creators_must_cover_rows(self):
        cases=[]
        r=participation();r['payload']['components'][1]['records']['group_field']='choice';cases.append(r)
        r=participation();del r['payload']['components'][1]['records']['group_by'];cases.append(r)
        r=participation();r['payload']['components'][1]['records']['group_by']='field';cases.append(r)
        r=participation();r['payload']['creators']=[c for c in r['payload']['creators'] if c['id']!=BEN];cases.append(r)
        r=participation();del r['payload']['creators'];cases.append(r)
        r=participation();r['payload']['datasets'][1]['rows'][0]['creator']=None;cases.append(r)
        r=participation();next(c for c in r['payload']['creators'] if c['id']==BEN)['label']='';cases.append(r)
        r=participation();r['payload']['creators'].reverse();cases.append(r)
        r=participation();r['payload']['creators'].append(dict(r['payload']['creators'][-1]));cases.append(r)
        r=participation();r['payload']['creators']={c['id']:c['label'] for c in r['payload']['creators']};cases.append(r)
        for r in cases:
            with self.subTest(r=str(r['payload']['components'][1])[:90]),self.assertRaises(engine.Refused):engine.execute(r)

class Matches(unittest.TestCase):
    def test_typed_equality_on_every_named_field(self):
        # A choice matches only for the same item: the reference is part of the key.
        r=participation(responses=[answer(11,BEN,2,'a'),answer(12,BEN,1,'c')])
        self.assertEqual(parts(r)['right']['value'],'0')
        r=participation();r['payload']['components'][2]['records']['measure']['fields']=['choice']
        self.assertEqual(parts(r)['right']['value'],'4','choice alone: a, c, c, a match a key choice')
        # Null equals null; another input id in the reference is another value.
        r=participation(responses=[answer(11,BEN,1,'a')],expected=[answer(21,ALEX,1,'a')])
        r['payload']['datasets'][1]['rows'][0]['values'][0]['value']={'type':'null'};r['payload']['datasets'][2]['rows'][0]['values'][0]['value']={'type':'null'}
        self.assertEqual(parts(r)['right']['value'],'1')
        r=participation(responses=[answer(11,BEN,1,'a')],expected=[answer(21,ALEX,1,'a')])
        r['payload']['datasets'][2]['rows'][0]['values'][0]['value']['input_id']='other'
        self.assertEqual(parts(r)['right']['value'],'0')
    def test_an_empty_key_scores_zero_and_duplicates_count_once_per_row(self):
        self.assertEqual(parts(participation(expected=[]))['right']['value'],'0')
        r=participation(expected=[answer(21,ALEX,1,'a'),answer(22,ALEX,1,'a')])
        self.assertEqual(parts(r)['right']['value'],'2','Ben and Cal answered item 1 with a')
    def test_malformed_matches_refuse(self):
        cases=[]
        def measure(**m):
            r=participation();r['payload']['components'][2]['records']['measure'].update(m);return r
        cases+=[measure(key_input='responses'),measure(key_input='missing'),measure(fields=[]),measure(fields=['item','item']),measure(fields=['unknown']),measure(fields=['a','b','c','d','e'])]
        r=participation();r['payload']['datasets'][2]['fields']=[ANSWER_FIELDS[0],{'id':'choice','value_type':{'type':'text'},'nullable':False}];cases.append(r)
        r=participation();r['payload']['components'][0]['records']['measure']={'op':'matches','key_input':'expected','fields':['item']};cases.append(r)
        for r in cases:
            with self.subTest(r=str(r['payload']['components'][2])[:90]),self.assertRaises(engine.Refused):engine.execute(r)
    def test_child_protocol_carries_the_new_members(self):
        import subprocess,sys
        p=subprocess.run([sys.executable,'-I',str(Path(__file__).with_name('query_engine.py'))],input=json.dumps(participation()).encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
        result=json.loads(p.stdout);self.assertEqual(result['outcome'],'record_components')
        self.assertEqual(result['components'][0]['value'],'2026-11-29T15:05:30Z');self.assertEqual(result['components'][1]['groups'][0]['label'],'Ben Okafor')


# ADR-020 section 9 (APP-208): the multi-select. A ranking over a multi_enum
# field counts a row once per selected value; the list cell carries the typed
# selection; a set has no order and is never a filter or a key field.
TAGS={'type':'multi_enum','values':['red','green','blue']}
def pick(n,title,tags,creator=ALEX):
    return {'id':f'00000000-0000-4000-8000-{n:012d}','version':1,'sequence':n,'creator':creator,'values':[
        {'field_id':'title','value':{'type':'text','value':title}},
        {'field_id':'tags','value':{'type':'multi_enum','values':tags} if tags is not None else {'type':'null'}}]}
def multi_select(rows=None,components=None,filters=None):
    common={'version':1,'input_id':'choices','schema_id':'choices'}
    def view(id,kind,**extra):return {'id':id,'title':id,'kind':kind,'renderer_version':1,'records':dict(common,**extra)}
    components=components if components is not None else [
        view('list','records_list',columns=['title','tags']),
        view('by_tag','record_ranking',measure={'op':'count'},group_field='tags',limit=10),
        view('count','record_summary',measure={'op':'count'})]
    rows=rows if rows is not None else [pick(1,'one',['red','blue']),pick(2,'two',['blue']),pick(3,'three',['red','green','blue'],BEN),pick(4,'four',None)]
    return {'kind':'record_components_v1','deadline_ms':1800,'parameters':{'filter_epoch':1,'filters':[]},'payload':{
        'creators':[{'id':ALEX,'label':'Alex Rivera'},{'id':BEN,'label':'Ben Okafor'}],'components':components,'filters':filters or [],
        'datasets':[{'input_id':'choices','schema_id':'choices','fields':[{'id':'title','value_type':{'type':'text'},'nullable':False},{'id':'tags','value_type':TAGS,'nullable':True}],'rows':rows}]}}

class MultiSelect(unittest.TestCase):
    def test_a_ranking_counts_a_row_once_per_selected_value(self):
        p=parts(multi_select())
        self.assertEqual(p['by_tag']['groups'],[
            {'label':'blue','missing':False,'value':'3','count':3},
            {'label':'red','missing':False,'value':'2','count':2},
            {'label':'green','missing':False,'value':'1','count':1},
            {'label':'Missing value','missing':True,'value':'1','count':1}])
        # The ranking's population and total, and the count summary, count rows once.
        self.assertEqual((p['by_tag']['population'],p['by_tag']['value'],p['count']['value']),(4,'4','4'))
        self.assertEqual(p['list']['rows'][0]['cells'][1],{'type':'multi_enum','values':['red','blue']})
        self.assertEqual(p['list']['rows'][3]['cells'][1],{'type':'null'})
        self.assertEqual(p['list']['columns'][1]['value_type'],TAGS)
    def test_the_ranking_page_and_an_all_null_input(self):
        r=multi_select();r['payload']['components'][1]['records']['limit']=2
        p=parts(r);self.assertEqual([g['label'] for g in p['by_tag']['groups']],['blue','red']);self.assertTrue(p['by_tag']['truncated_page'])
        p=parts(multi_select(rows=[pick(1,'one',None)]))
        self.assertEqual(p['by_tag']['groups'],[{'label':'Missing value','missing':True,'value':'1','count':1}])
    def test_a_stored_selection_must_be_an_ordered_subset_of_the_declared_values(self):
        for bad in (['blue','red'],['red','red'],['purple'],[]):
            with self.subTest(bad=bad),self.assertRaisesRegex(engine.Refused,'value_refused'):
                engine.execute(multi_select(rows=[pick(1,'one',bad)]))
        r=multi_select(rows=[pick(1,'one',['red'])]);r['payload']['datasets'][0]['rows'][0]['values'][1]['value']={'type':'enum','value':'red'}
        with self.assertRaisesRegex(engine.Refused,'value_refused'):engine.execute(r)
    def test_a_set_is_never_an_order_a_filter_or_a_key(self):
        r=multi_select();r['payload']['components'][0]['records']['order']={'field':'tags','direction':'asc'}
        with self.assertRaisesRegex(engine.Refused,'multi_enum_order_refused'):engine.execute(r)
        # An order refuses before any row is read.
        r['payload']['datasets'][0]['rows']=[]
        with self.assertRaisesRegex(engine.Refused,'multi_enum_order_refused'):engine.execute(r)
        for kind in ('enum','text_contains'):
            r=multi_select(filters=[{'id':'tag','input_id':'choices','field':'tags','kind':kind,'targets':['list']}])
            r['parameters']['filters']=[{'id':'tag','value':'red'}]
            with self.subTest(kind=kind),self.assertRaises(engine.Refused):engine.execute(r)
        r=multi_select();r['payload']['datasets'].append({'input_id':'keys','schema_id':'keys','fields':r['payload']['datasets'][0]['fields'],'rows':[]})
        r['payload']['components'][2]['records']['measure']={'op':'matches','key_input':'keys','fields':['tags']}
        with self.assertRaises(engine.Refused):engine.execute(r)
    def test_a_trend_never_groups_a_set(self):
        r=multi_select();r['payload']['components'][1]={'id':'t','title':'t','kind':'record_trend','renderer_version':1,'records':{'version':1,'input_id':'choices','schema_id':'choices','measure':{'op':'count'},'date_field':'tags'}}
        with self.assertRaises(engine.Refused):engine.execute(r)

if __name__=='__main__':unittest.main()
