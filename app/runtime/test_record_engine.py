import copy
import datetime
import importlib.util
import io
import json
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('record_engine',Path(__file__).with_name('record_engine.py'))
engine=importlib.util.module_from_spec(spec);spec.loader.exec_module(engine)

def record(n,amount,day,restaurant='Cafe A'):
    return {'id':f'00000000-0000-4000-8000-{n:012d}','version':1,'sequence':n,'values':[
        {'field_id':'amount','value':{'type':'money','minor_units':str(amount),'currency':'USD','minor_unit_exponent':2}},
        {'field_id':'date','value':{'type':'date','value':day}},
        {'field_id':'restaurant','value':{'type':'text','value':restaurant}},
    ]}
def request():
    common={'version':1,'input_id':'meals','schema_id':'meal'}
    def view(id,kind,**extra):return {'id':id,'title':id,'kind':kind,'renderer_version':1,'records':dict(common,**extra)}
    money={'op':'sum','field':'amount'}
    components=[view('list','records_list',columns=['restaurant','date','amount'],limit=1),view('spend','record_summary',measure=money),view('count','record_summary',measure={'op':'count'}),view('month','record_trend',measure=money,date_field='date'),view('top','record_ranking',measure=money,group_field='restaurant',limit=1),view('budget','record_budget',measure=money,budget={'origin':'preset','minor_units':'10000','currency':'USD','minor_unit_exponent':2})]
    return {'kind':'record_components_v1','deadline_ms':1800,'parameters':{'filter_epoch':3,'filters':[{'id':'period','start':'2026-11-01','end':'2026-12-01'}]},'payload':{'components':components,'filters':[{'id':'period','input_id':'meals','field':'date','kind':'date_range','targets':[v['id'] for v in components]},{'id':'restaurant','input_id':'meals','field':'restaurant','kind':'text_contains','targets':['list']}],'datasets':[{'input_id':'meals','schema_id':'meal','fields':[{'id':'amount','value_type':{'type':'money','currency':'USD','minor_unit_exponent':2},'nullable':False},{'id':'date','value_type':{'type':'date'},'nullable':False},{'id':'restaurant','value_type':{'type':'text'},'nullable':False}],'rows':[record(5,9999,'2026-12-01'),record(4,2500,'2026-11-04','Cafe B'),record(3,750,'2026-11-03'),record(2,1250,'2026-11-02'),record(1,9999,'2026-10-31')]}]}}

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

if __name__=='__main__':unittest.main()
