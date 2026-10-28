"""Deterministic, synthetic OOXML fixtures; no office application or real data."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED
from io import BytesIO
import json

ROOT = Path(__file__).resolve().parent / 'fixtures'
ROOT.mkdir(exist_ok=True)
NS = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
RELS = 'http://schemas.openxmlformats.org/package/2006/relationships'
DOC = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'

def workbook(changes=None, extras=None):
    files = {
        '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>',
        '_rels/.rels': f'<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{DOC}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        'xl/workbook.xml': f'<workbook xmlns="{NS}" xmlns:r="{DOC}"><sheets><sheet name="Inventory" sheetId="1" r:id="rId1"/><sheet name="Other" sheetId="2" r:id="rId2"/></sheets></workbook>',
        'xl/_rels/workbook.xml.rels': f'<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{DOC}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="{DOC}/worksheet" Target="worksheets/sheet2.xml"/></Relationships>',
        'xl/sharedStrings.xml': f'<sst xmlns="{NS}" count="2" uniqueCount="2"><si><t>α &amp; team</t></si><si><t>Other sheet only</t></si></sst>',
        'xl/worksheets/sheet1.xml': f'<worksheet xmlns="{NS}"><dimension ref="A1:C2"/><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>9007199254740993</v></c><c r="C1" t="b"><v>1</v></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>literal =SUM(A1)</t></is></c><c r="B2"><v>123.45</v></c><c r="C2"><v>-1.25e2</v></c></row></sheetData></worksheet>',
        'xl/worksheets/sheet2.xml': f'<worksheet xmlns="{NS}"><dimension ref="A1:B1"/><sheetData><row r="1"><c r="A1" t="s"><v>1</v></c><c r="B1"><v>22</v></c></row></sheetData></worksheet>',
    }
    for name, transform in (changes or {}).items(): files[name]=transform(files[name])
    files.update(extras or {})
    output=BytesIO()
    with ZipFile(output,'w') as z:
        for name,value in sorted(files.items()):
            entry=ZipInfo(name,date_time=(2026,10,28,0,0,0));entry.compress_type=ZIP_DEFLATED
            z.writestr(entry,value.encode() if isinstance(value,str) else value)
    return output.getvalue()

cases = {
    'valid.xlsx':workbook(),
    'zero-exponents.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('123.45','0e3').replace('-1.25e2','-0e3')}),
    'duplicate-values.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('<v>123.45</v>','<v>123.45</v><v>999</v>')}),
    'actual-strings.xlsx':workbook({'xl/sharedStrings.xml':lambda s:s.replace('</sst>','<si><t>extra beyond declaration</t></si></sst>')}),
    'duplicate-cell.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('r="C2"','r="B2"')}),
    'deep.xml.xlsx':workbook(extras={'xl/deep.xml':'<x>'*33+'</x>'*33}),
    'formula.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('<v>123.45</v>','<f>SUM(B1)</f><v>123.45</v>')}),
    'unselected-formula.xlsx':workbook({'xl/worksheets/sheet2.xml':lambda s:s.replace('<v>22</v>','<f>NOW()</f><v>22</v>')}),
    'content-type-macro.xlsx':workbook({'[Content_Types].xml':lambda s:s.replace('spreadsheetml.sheet.main+xml','spreadsheetml.macroEnabled.main+xml')}),
    'macro.xlsx':workbook(extras={'xl/vbaProject.bin':b'not executable; refusal control'}),
    'external.xlsx':workbook({'xl/_rels/workbook.xml.rels':lambda s:s.replace('Target="worksheets/sheet1.xml"','Target="https://example.invalid/evidence" TargetMode="External"')}),
    'declared-strings.xlsx':workbook({'xl/sharedStrings.xml':lambda s:s.replace('uniqueCount="2"','uniqueCount="4294967295"')}),
    'dimension.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('A1:C2','A1:XFD1048576')}),
    'actual-coordinate.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('r="C2"','r="AG2"')}),
    'doctype.xlsx':workbook({'xl/sharedStrings.xml':lambda s:'<!DOCTYPE sst [<!ENTITY x "never">]>'+s}),
    'bad-numeric.xlsx':workbook({'xl/worksheets/sheet1.xml':lambda s:s.replace('123.45','1e-2147483648')}),
}
for name,value in cases.items(): (ROOT/name).write_bytes(value)
(ROOT/'literal.csv').write_bytes('name,note,large\r\nα,"two\nlines",9007199254740993\r\n"=SUM(A1)","quoted ""text""",42\r\n'.encode())
for fmt in ('csv','xlsx'):
    value={'profile':'demo','format':fmt}
    if fmt=='xlsx':value['sheet']='Inventory'
    (ROOT/(fmt+'-config.json')).write_text(json.dumps(value,indent=2)+'\n')
print(f'Created {len(cases)} synthetic XLSX fixtures and literal CSV/configuration fixtures')
