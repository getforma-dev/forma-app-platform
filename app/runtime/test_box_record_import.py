import datetime
import hashlib
import tempfile
import unittest
from pathlib import Path
import duckdb
from box_record_import import export, scalar


class RecordImportTests(unittest.TestCase):
    def test_exact_coefficients_and_nulls(self):
        self.assertEqual(scalar({'type':'decimal','scale':2,'unit':'ratio'},1,False), {'type':'decimal','coefficient':'1','scale':2,'unit':'ratio'})
        self.assertEqual(scalar({'type':'money','currency':'JPY','minor_unit_exponent':0},123,False), {'type':'money','minor_units':'123','currency':'JPY','minor_unit_exponent':0})
        self.assertEqual(scalar({'type':'text'},'',True), {'type':'text','value':''})
        self.assertEqual(scalar({'type':'text'},None,True), {'type':'null'})
        self.assertEqual(scalar({'type':'date'},datetime.date(2026,11,2),False)['value'],'2026-11-02')
        with self.assertRaises(ValueError): scalar({'type':'text'},'x'*2001,False)
        with self.assertRaises(ValueError): scalar({'type':'integer','unit':'count'},None,False)

    def test_full_parquet_without_original_column(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'source.parquet'
            db=duckdb.connect()
            db.execute('COPY (SELECT i::BIGINT AS amount FROM range(2505) r(i)) TO ? (FORMAT PARQUET)',[str(path)])
            request={'path':str(path),'digest':'sha256:'+hashlib.sha256(path.read_bytes()).hexdigest(),'rows':2505,'fields':[{'id':'amount','nullable':False,'value_type':{'type':'decimal','scale':2,'unit':'count'}}]}
            result=export(request)
            self.assertEqual(len(result),2505)
            self.assertEqual(result[-1][0]['value']['coefficient'],'2504')
            request['rows']=2504
            with self.assertRaises(ValueError): export(request)
            request['rows']=100001
            with self.assertRaises(ValueError): export(request)
            request['rows']=2505;request['digest']='sha256:'+'0'*64
            with self.assertRaises(ValueError): export(request)


if __name__=='__main__': unittest.main()
