import assert from 'node:assert/strict';
import { test } from 'node:test';
import { originalPage } from '../src/canvas/box-api.ts';
const page = () => ({format:'xlsx',sheets:['First','Other'],sheet_index:1,row_count:3,column_count:2,offset:0,page_size:50,rows:[['',''],['  head  ','001.2300'],['<script>text</script>','-1.25e2']],sha256:`sha256:${'a'.repeat(64)}`});
test('original pages preserve lexical values, empty coordinates and sheet identity',()=>{
 const value=page(); assert.deepEqual(originalPage(value),value);
 const long='x'.repeat(2000); value.rows[1]![0]=long; assert.equal(originalPage(value)?.rows[1]?.[0],long);
});
test('a truncated, malformed, mismatched or out-of-bounds page is refused, never silently shortened',()=>{
 for(const delta of [{rows:[['lost']]},{column_count:33},{row_count:100001},{offset:-1},{offset:1},{sheet_index:2},{sheets:['a'.repeat(8193)]},{sha256:'bad'},{rows:[['',''],['a','b'],['c',5]]}]) assert.equal(originalPage({...page(),...delta}),null);
 assert.equal(originalPage({...page(),format:'csv'}),null);
 assert.ok(originalPage({...page(),format:'csv',sheets:[],sheet_index:0}));
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
