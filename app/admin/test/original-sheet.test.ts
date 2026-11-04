import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fetchOriginalPage, originalPage } from '../src/canvas/box-api.ts';
const page = () => ({format:'xlsx',sheets:['First','Other'],sheet_index:1,row_count:3,column_count:2,offset:0,page_size:50,column_offset:0,column_page_size:32,formula_count:0,missing_formula_results:0,formula_cells:[] as {row:number;column:number;cached:boolean}[],rows:[['',''],['  head  ','001.2300'],['<script>text</script>','-1.25e2']],sha256:`sha256:${'a'.repeat(64)}`});
const widePage = (columnOffset = 0, rowOffset = 0, rowCount = 82) => ({
 ...page(),format:'csv',sheets:[],sheet_index:0,row_count:rowCount,column_count:153,offset:rowOffset,column_offset:columnOffset,
 rows:Array.from({length:Math.min(50,rowCount-rowOffset)},(_,r)=>Array.from({length:Math.min(32,153-columnOffset)},(_,c)=>`r${rowOffset+r+1}c${columnOffset+c+1}`)),
});
test('original pages preserve lexical values, empty coordinates and sheet identity',()=>{
 const value=page(); assert.deepEqual(originalPage(value),value);
 // A long Unicode cell is retained completely, including surrogate pairs.
 const long='🧾'.repeat(65536); value.rows[1]![0]=long; assert.equal(originalPage(value)?.rows[1]?.[0],long);
 value.rows[1]![0]='x'.repeat(65536);assert.ok(originalPage(value));
 value.rows[1]![0]+='x';assert.equal(originalPage(value),null,'the character limit counts code points, not just UTF-16 units');
});
test('a truncated, malformed, mismatched or out-of-bounds page is refused, never silently shortened',()=>{
 for(const delta of [{rows:[['lost']]},{column_count:16385},{row_count:1048577},{row_count:2.5},{offset:-1},{offset:1},{offset:50},{column_offset:1},{column_offset:32},{column_offset:-32},{column_offset:16384},{page_size:51},{column_page_size:33},{sheet_index:2},{sheets:['a'.repeat(8193)]},{sha256:'bad'},{rows:[['',''],['a','b'],['c',5]]},{rows:[['',''],['x'.repeat(131073),'b'],['c','d']]}]) assert.equal(originalPage({...page(),...delta}),null,JSON.stringify(Object.keys(delta)));
 assert.equal(originalPage({...page(),format:'csv'}),null);
 assert.ok(originalPage({...page(),format:'csv',sheets:[],sheet_index:0}));
});

test('wide and tall sheets retain total geometry while each page contains only its requested window',()=>{
 const first=widePage(); const last=widePage(128,50);
 assert.deepEqual(originalPage(first),first);
 assert.equal(originalPage(first)?.rows[0]?.[31],'r1c32');
 assert.deepEqual(originalPage(last),last);
 assert.equal(originalPage(last)?.rows.length,32);
 assert.equal(originalPage(last)?.rows[0]?.length,25);
 assert.equal(originalPage(last)?.rows[31]?.[24],'r82c153');
 assert.equal(originalPage({...last,rows:last.rows.slice(0,31)}),null,'final row cannot disappear');
 assert.equal(originalPage({...last,rows:last.rows.map(row=>row.slice(0,24))}),null,'final column cannot disappear');
 const corner={...widePage(0,1048550,1048576),column_count:16384,column_offset:16352};
 assert.ok(originalPage(corner),'the final Excel row and column window is supported');
});

test('ragged source records are represented by explicit empty coordinates rather than shifted cells',()=>{
 const value=widePage(128,50,52);
 value.rows[0]=Array.from({length:25},()=> '');
 value.rows[1]=['last source value',...Array.from({length:24},()=> '')];
 assert.deepEqual(originalPage(value)?.rows,value.rows);
 assert.equal(originalPage({...value,rows:[[],value.rows[1]]}),null,'a short wire row must be rejected rather than silently padded by the client');
});

test('saved formula values and missing caches carry distinct source coordinates',()=>{
 const value={...page(),formula_count:3,missing_formula_results:1,formula_cells:[{row:2,column:2,cached:true},{row:3,column:1,cached:false}]};
 value.rows[2]![0]='';
 assert.deepEqual(originalPage(value),value);
 assert.equal(originalPage(value)?.rows[1]?.[1],'001.2300','cached numeric lexical value is preserved');
 assert.equal(originalPage(value)?.formula_cells[1]?.cached,false,'a missing cache is not an ordinary empty cell');
 for(const delta of [
  {formula_count:-1},{formula_count:1.5},{formula_count:7},{formula_count:4000001},{missing_formula_results:4},
  {formula_cells:[{row:2,column:2,cached:true},{row:2,column:2,cached:false}]},
  {formula_cells:[{row:0,column:1,cached:true}]},{formula_cells:[{row:4,column:1,cached:true}]},
  {formula_cells:[{row:1,column:0,cached:true}]},{formula_cells:[{row:1,column:3,cached:true}]},
  {formula_cells:[{row:1.5,column:1,cached:true}]},{formula_cells:[{row:1,column:1,cached:'yes'}]},
  {formula_cells:[null]},{missing_formula_results:0},{formula_count:1},
 ]) assert.equal(originalPage({...value,...delta}),null,JSON.stringify(delta));
 assert.equal(originalPage({...value,rows:[['',''],['  head  ','001.2300'],['not a cached result','-1.25e2']]}),null,'a missing cache cannot masquerade as a saved value');
 const later={...widePage(128,50),format:'xlsx',sheets:['Wide'],sheet_index:0,formula_count:2,missing_formula_results:1,formula_cells:[{row:51,column:129,cached:true},{row:82,column:153,cached:false}]};
 later.rows[31]![24]='';
 assert.ok(originalPage(later));
 assert.equal(originalPage({...later,formula_cells:[{row:50,column:129,cached:true}]}),null,'previous row window cannot leak in');
 assert.equal(originalPage({...later,formula_cells:[{row:51,column:128,cached:true}]}),null,'previous column window cannot leak in');
 assert.equal(originalPage({...widePage(),formula_count:1}),null,'CSV never claims workbook formulas');
});

test('the page request carries both offsets and refuses a valid response for a different window',async()=>{
 const previous=globalThis.fetch;
 const requests:{url:string;body:unknown}[]=[];
 let answer=widePage(128,50);
 globalThis.fetch=async (url,init)=>{
  requests.push({url:String(url),body:JSON.parse(String(init?.body))});
  return new Response(JSON.stringify(answer),{status:200,headers:{'content-type':'application/json'}});
 };
 try {
  const received=await fetchOriginalPage('box','file',0,50,128);
  assert.deepEqual(received,answer);
  assert.deepEqual(requests,[{url:'/v1/boxes/box/files/file/sheet',body:{sheet_index:0,offset:50,column_offset:128}}]);
  answer=widePage(96,50);
  await assert.rejects(fetchOriginalPage('box','file',0,50,128),/malformed/);
 } finally {globalThis.fetch=previous;}
});

test('column navigation supports Excel names beyond AZ and one-based numeric jumps',async()=>{
 const {spreadsheetColumnName,spreadsheetColumnIndex}=await import('../src/canvas/box-viewer.ts');
 for(const [index,name] of [[0,'A'],[25,'Z'],[26,'AA'],[51,'AZ'],[52,'BA'],[127,'DX'],[152,'EW'],[16383,'XFD']] as const) {
  assert.equal(spreadsheetColumnName(index),name);
  assert.equal(spreadsheetColumnIndex(name),index);
  assert.equal(spreadsheetColumnIndex(String(index+1)),index);
 }
 assert.equal(spreadsheetColumnIndex('  xfd  '),16383);
 for(const invalid of ['', '0', '-1', '1.5', '1e2', 'A0', 'AAAA', 'XFE', 'ZZZ', '16385']) assert.equal(spreadsheetColumnIndex(invalid),null,invalid);
});

test('saving during delayed restoration preserves every unresolved reference, and removal stays explicit',async()=>{
 const {mergeSheetPlacements,sheetPlacementId}=await import('../src/canvas/box-api.ts');
 const boxId='00000000-0000-4000-8000-000000000001';
 const ref=(n:number)=>({boxId,fileId:`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`,x:n,y:n});
 const first=ref(2),delayed=ref(3),added=ref(4),removed=new Set<string>();
 let stored=JSON.stringify([first,delayed]);
 let release!:()=>void;const pending=new Promise<void>(resolve=>{release=resolve;});
 const restore=pending.then(()=>{stored=JSON.stringify(mergeSheetPlacements(JSON.parse(stored),[delayed],removed));});
 // Only the first original reference has resolved when a new upload lands.
 stored=JSON.stringify(mergeSheetPlacements(JSON.parse(stored),[first,added],removed));
 assert.deepEqual(JSON.parse(stored),[first,delayed,added]);
 removed.add(sheetPlacementId(delayed));
 stored=JSON.stringify(mergeSheetPlacements(JSON.parse(stored),[first,added],removed));
 release();await restore;
 assert.deepEqual(JSON.parse(stored),[first,added],'explicit removal survives the delayed metadata response');
 assert.equal(mergeSheetPlacements([{...first,x:Infinity}],[],removed).length,0);
});
