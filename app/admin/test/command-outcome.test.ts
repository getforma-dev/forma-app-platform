import assert from 'node:assert/strict';
import { test } from 'node:test';
import { postCommand } from '../src/canvas/commands.ts';
const path = '/v1/work/app/work/commands/create';
const outcome = {kind:'command_outcome',state:'committed',record_version:1,work_version:2,commit_sequence:17,audit_event_id:'audit'};
test('ambiguous server/proxy failures recover the already committed outcome by the original key', async () => {
 const prior=globalThis.fetch;
 try { for(const status of [500,502,503,504]) {
  const calls:{url:string;method:string;key:string|null}[]=[];
  globalThis.fetch=async(url,options)=>{calls.push({url:String(url),method:options?.method??'GET',key:new Headers(options?.headers).get('idempotency-key')});return calls.length===1?new Response('<html>gateway interruption</html>',{status}):Response.json(outcome);};
  assert.deepEqual(await postCommand(path,{value:'once'},'original-key'),{kind:'committed',recordVersion:'1',workVersion:'2',commit:'17',audit:'audit'});
  assert.deepEqual(calls,[{url:path,method:'POST',key:'original-key'},{url:`${path}/outcomes/original-key`,method:'GET',key:null}]);
 }}finally{globalThis.fetch=prior;}
});
test('unavailable outcome lookup remains uncertain, without claiming nothing changed or minting another command',async()=>{
 const prior=globalThis.fetch;const calls:string[]=[];
 try {globalThis.fetch=async url=>{calls.push(String(url));return new Response('',{status:calls.length===1?504:404});};
 const result=await postCommand(path,{value:'once'},'same-key');assert.equal(result.kind,'unconfirmed');assert.match('message'in result?result.message:'',/same form/);assert.equal(calls.length,2);
 }finally{globalThis.fetch=prior;}
});
test('an in-flight retry freezes both payload bytes and idempotency identity',async()=>{
 const prior=globalThis.fetch,priorTimer=globalThis.setTimeout;const bodies:string[]=[];const keys:(string|null)[]=[];const body={value:'original'};
 try {
 globalThis.setTimeout=((fn:()=>void)=>{fn();return 1;}) as unknown as typeof setTimeout;
 globalThis.fetch=async(_url,options)=>{bodies.push(String(options?.body));keys.push(new Headers(options?.headers).get('idempotency-key'));if(bodies.length===1){body.value='edited while waiting';return Response.json({code:'in_flight'},{status:409,headers:{'retry-after':'1'}});}return Response.json(outcome);};
 assert.equal((await postCommand(path,body,'fixed-key')).kind,'committed');assert.deepEqual(bodies,['{"value":"original"}','{"value":"original"}']);assert.deepEqual(keys,['fixed-key','fixed-key']);
 }finally{globalThis.fetch=prior;globalThis.setTimeout=priorTimer;}
});
test('explicit route refusals preserve their definitive classification',async()=>{
 const prior=globalThis.fetch;
 try{for(const [status,kind]of [[400,'refused'],[404,'refused'],[409,'conflict'],[412,'stale']] as const){let calls=0;globalThis.fetch=async()=>{calls++;return Response.json({code:'refused'},{status});};assert.equal((await postCommand(path,{},'key')).kind,kind);assert.equal(calls,1);}}finally{globalThis.fetch=prior;}
});

test('an app data bound is a definitive refusal with clear limits, without retry or outcome lookup',async()=>{
 const prior=globalThis.fetch;let calls=0;
 try{
  globalThis.fetch=async()=>{calls++;return Response.json({code:'app_data_limit'},{status:422});};
  const result=await postCommand(path,{},'same-key',{refused:'A generic refusal'});
  assert.equal(result.kind,'refused');assert.match('message'in result?result.message:'',/100,000 rows or 64 MiB/);
  assert.match('message'in result?result.message:'',/Nothing was saved. Original files are unchanged/);assert.equal(calls,1);
 }finally{globalThis.fetch=prior;}
});
