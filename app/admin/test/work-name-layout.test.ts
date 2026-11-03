import assert from 'node:assert/strict';
import { test } from 'node:test';
import { declaredLayout } from '../src/canvas/page-layout.ts';
import { readWorkName, saveWorkName } from '../src/canvas/work-name.ts';
test('declared layout preserves note, form and query order; collisions flow and extras remain', () => {
  assert.deepEqual(declaredLayout(['app','chart','form','note','records'], JSON.stringify([
    {id:'note',row:0,column:0,width:12}, {id:'form',row:2,column:0,width:6}, {id:'chart',row:2,column:6,width:6},
  ])), [
    {id:'note',row:1,column:0,width:12}, {id:'form',row:2,column:0,width:6}, {id:'chart',row:2,column:6,width:6},
    {id:'app',row:3,column:0,width:12}, {id:'records',row:4,column:0,width:12},
  ]);
  assert.deepEqual(declaredLayout(['a','b'], '[{"id":"a","row":0,"column":0,"width":8},{"id":"b","row":0,"column":6,"width":8}]'), [
    {id:'a',row:1,column:0,width:8},{id:'b',row:2,column:6,width:6},
  ]);
  for(const bad of ['null','[{"id":"a","row":0,"column":"url(x)","width":12}]','[{"id":"a","row":-1,"column":0,"width":12}]']) assert.equal(declaredLayout(['a'],bad),null);
});
test('independent work names use the exact work and version; conflict never falls back to shell labels', async () => {
  const prior = globalThis.fetch, calls: {url:string;body:unknown}[]=[];
  try {
    globalThis.fetch = async (url, init) => { calls.push({url:String(url),body:init?.body ? JSON.parse(String(init.body)) : null}); return new Response(JSON.stringify({name:'First',version:4,can_name:true,scope:'work'})); };
    assert.deepEqual(await readWorkName('shell','first'), {name:'First',version:4,can_name:true,scope:'work'});
    globalThis.fetch = async (url,init) => { calls.push({url:String(url),body:JSON.parse(String(init?.body))});return new Response(JSON.stringify({code:'version_conflict'}),{status:409}); };
    assert.match(String(await saveWorkName('shell','first','New',4)), /draft is kept/);
    assert.deepEqual(calls,[{url:'/v1/work/shell/first/label',body:null},{url:'/v1/work/shell/first/label',body:{name:'New',expected_version:4}}]);
    globalThis.fetch = async () => new Response(JSON.stringify({saved:true,label:{name:'Different',version:5,can_name:true,scope:'work'}}));
    assert.equal(typeof await saveWorkName('shell','first','New',4),'string');
    globalThis.fetch = async () => new Response(JSON.stringify({saved:true,label:{name:'New',version:5,can_name:true,scope:'work'}}));
    assert.deepEqual(await saveWorkName('shell','first','New',4),{name:'New',version:5,can_name:true,scope:'work'});
    globalThis.fetch = async () => new Response(JSON.stringify({saved:false,label:{name:'New',version:4,can_name:true,scope:'work'}}));
    assert.deepEqual(await saveWorkName('shell','first','New',4),{name:'New',version:4,can_name:true,scope:'work'});
    for (const [saved,version] of [[true,6],[false,5],[false,3]]) {
      globalThis.fetch = async () => new Response(JSON.stringify({saved,label:{name:'New',version,can_name:true,scope:'work'}}));
      assert.equal(typeof await saveWorkName('shell','first','New',4),'string');
    }
  } finally { globalThis.fetch=prior; }
});
