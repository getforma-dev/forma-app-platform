import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import {build} from 'esbuild';

const APP = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
const WORK = '11111111-2222-4333-8444-555555555555';
const OTHER = '99999999-2222-4333-8444-555555555555';
const RECORD = '22222222-2222-4333-8444-555555555555';
const COLLECTION = '33333333-2222-4333-8444-555555555555';
const FILE = '44444444-2222-4333-8444-555555555555';
const BOX = '55555555-2222-4333-8444-555555555555';
const CARD = '66666666-2222-4333-8444-555555555555';
const HASH = `sha256:${'a'.repeat(64)}`;
const owner = {app:APP, work:WORK, title:'Example app'};
const reference = {ref:'r1', app_id:APP, work_id:WORK, record_id:RECORD, address:'example-app', label:'An example row'};
const receipt = {reference:'r1',kind:'record',app_id:APP,work_id:WORK,record_id:RECORD,collection_id:COLLECTION,
  publication_id:OTHER,definition_digest:HASH,version:1,work_version:1,commit_sequence:1,committed_at:'2026-11-02T20:00:00Z'};
const row = {ref:'f1r1',file_id:FILE,file:'example.csv',sha256:HASH,sheet:null,row:1,label:'An example row'};
const recordSnapshot = () => ({kind:'records',question:'What is here?',reply:{text:'A saved record answer.',references:[reference],receipts:[receipt]},
  scope:{kind:'app',address:'Example app',works:[{app_id:APP,work_id:WORK,address:'example-app'}]}});
const fileSnapshot = () => ({kind:'file',question:'What is in the file?',reply:{text:'A saved file answer.',references:[row]},
  source:{app:APP,work:WORK,boxId:BOX,fileId:FILE,fileName:'example.csv',revisionId:OTHER}});
const saved = (overrides: Record<string, unknown> = {}) => ({card_id:CARD,kind:'answer',app_id:APP,work_id:WORK,source:'',address:'example-app',title:'What is here?',
  spec:{question:'What is here?',text:'A saved record answer.',scope:{kind:'app',address:'example-app'},references:[reference],receipts:[receipt]},
  created_at:'2026-11-02T20:00:00Z',mine:true,everyone:false,can_share:false,can_manage:false,...overrides});
const savedFile = () => saved({spec:{question:'What is in the file?',text:'A saved file answer.',scope:{kind:'app',address:'example-app'},references:[],receipts:[],
  file_source:{box_id:BOX,file_id:FILE,name:'example.csv',kind:'csv',sha256:HASH},file_references:[row]}});
const response = (data: unknown, status=200) => ({ok:status>=200&&status<300,status,json:async()=>data});
const flush = async () => {for(let i=0;i<24;i++) await Promise.resolve();};
let serial=0;

class Element {
  attrs=new Map<string,string>();children:(Element|string)[]=[];parent:Element|null=null;
  hidden=false;disabled=false;attached=true;
  listeners=new Map<string,(()=>void)[]>();
  constructor(readonly tag:string,readonly ownerDocument:any) {}
  get isConnected():boolean{return this.attached&&(!this.parent||this.parent.isConnected);}
  set textContent(value:string){this.replaceChildren(value);}
  get textContent():string{return this.children.map(child=>typeof child==='string'?child:child.textContent).join('');}
  setAttribute(name:string,value:string){this.attrs.set(name,value);}
  append(...children:(Element|string)[]){for(const child of children){if(child instanceof Element)child.parent=this;this.children.push(child);}}
  replaceChildren(...children:(Element|string)[]){for(const child of this.children)if(child instanceof Element)child.parent=null;this.children=[];this.append(...children);}
  addEventListener(name:string,handler:()=>void){this.listeners.set(name,[...(this.listeners.get(name)??[]),handler]);}
  click(){if(!this.disabled)for(const handler of this.listeners.get('click')??[])handler();}
  matches(selector:string){
    if(selector.startsWith('.'))return(this.attrs.get('class')??'').split(' ').includes(selector.slice(1));
    const attr=/^\[([^=]+)="([^"]*)"\]$/.exec(selector);if(attr)return this.attrs.get(attr[1]!)===attr[2];
    return this.tag===selector;
  }
  querySelectorAll(selector:string):Element[]{return this.children.flatMap(child=>child instanceof Element?[...(child.matches(selector)?[child]:[]),...child.querySelectorAll(selector)]:[]);}
  querySelector(selector:string):Element|null{return this.querySelectorAll(selector)[0]??null;}
  button(label:string){const found=this.querySelectorAll('button').find(node=>node.textContent===label);assert.ok(found,`button ${label} exists`);return found;}
}

/** Real saved-card transport/parsers and Ask receipt validator; only DOM/canvas boundaries are stubbed. */
async function fixture(){
  const entry=fileURLToPath(new URL('../src/canvas/app-saved-answers.ts',import.meta.url));
  const cardEntry=fileURLToPath(new URL('../src/canvas/cards.ts',import.meta.url));
  const source=await readFile(entry,'utf8');
  const askSource=await readFile(new URL('../src/canvas/ask.ts',import.meta.url),'utf8');
  const recordFunctions=askSource.slice(askSource.indexOf('export function recordSet('),askSource.indexOf('\nfunction answerActions('));
  const placements:any[]=[],removed:string[]=[],previews:any[]=[];
  const utilities=new Map<string,Element>();
  const document_={querySelector:(selector:string)=>utilities.get(/^\[data-utility="([^\"]+)"\]$/.exec(selector)?.[1]??'')??null};
  const el=(_document:unknown,tag:string,attrs:Record<string,string>={},...children:any[])=>{
    const node=new Element(tag,document_);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);node.append(...children);return node;
  };
  const host=el(document_,'aside');host.hidden=true;
  const mocks={
    './render':{el},
    './board':{removeUtilityItem:(node:Element)=>{const id=node.attrs.get('data-utility')!;removed.push(id);utilities.delete(id);return true;}},
    './shape':{addressOfWork:()=> 'example-app',previewNewCard:async(spec:unknown)=>{previews.push(spec);return true;}},
    './ask':{placePinnedAnswer:(card:unknown)=>{placements.push(card);return true;}},
  };
  const key=`__savedAnswers${serial++}`;(globalThis as any)[key]=mocks;
  const exports=new Map<string,string[]>(Object.entries(mocks).map(([path,names])=>[path,Object.keys(names)]));
  const result=await build({entryPoints:[entry],bundle:true,format:'esm',platform:'node',write:false,plugins:[{name:'saved-answer-boundaries',setup(builder){
    builder.onLoad({filter:/app-saved-answers\.ts$/},()=>({contents:source+'\nexport {storeCard as testStoreCard} from "./cards";',loader:'ts'}));
    builder.onResolve({filter:/.*/},args=>{
      if(args.importer===cardEntry&&args.path==='./talk-view')return{path:'card-view',namespace:'saved-spy'};
      if(args.importer===entry&&exports.has(args.path))return{path:args.path,namespace:'saved-spy'};
      return undefined;
    });
    builder.onLoad({filter:/.*/,namespace:'saved-spy'},args=>{
      if(args.path==='card-view')return{contents:'export const cardView = () => null;',loader:'js'};
      let contents=exports.get(args.path)!.map(name=>`export const ${name} = (...args) => globalThis[${JSON.stringify(key)}][${JSON.stringify(args.path)}][${JSON.stringify(name)}](...args);`).join('\n');
      if(args.path==='./ask')contents+='\nconst MAX_RECORD_SET = 200; const isUuid = value => /^[0-9a-f-]{36}$/.test(value); const addressOfWork = () => "example-app";\n'+recordFunctions;
      return{contents,loader:'ts'};
    });
  }}]});
  const api=await import('data:text/javascript;base64,'+Buffer.from(result.outputFiles[0]!.text).toString('base64'));
  const oldFetch=globalThis.fetch;
  const requests:{url:string;method:string;body:any}[]=[];
  let respond=async(request:typeof requests[number]):Promise<any>=>request.method==='GET'?response({cards:[]}):response({card:saved({card_id:request.body.card_id})});
  globalThis.fetch=(async(url:string,options:RequestInit)=>{const request={url,method:String(options.method),body:options.body?JSON.parse(String(options.body)):null};requests.push(request);return respond(request);}) as typeof fetch;
  const controller=api.mountAppSavedAnswers(host,owner);
  return{api,host,controller,document_,requests,placements,removed,previews,utilities,
    respond:(fn:typeof respond)=>{respond=fn;},
    actions:(snapshot:unknown,onSaved?:(card:unknown)=>void)=>api.renderAppAnswerActions(document_,snapshot,{...owner,onSaved}) as Element,
    utility:(id:string)=>{const node=el(document_,'article',{'data-utility':`utility-pin-${id}`});utilities.set(`utility-pin-${id}`,node);return node;},
    cleanup:()=>{controller.dispose();globalThis.fetch=oldFetch;delete(globalThis as any)[key];}};
}

test('Save files a verified record snapshot once; Make it a card remains an explicit preview',async()=>{
  const f=await fixture();try{
    const notified:any[]=[];const actions=f.actions(recordSnapshot(),card=>notified.push(card));
    let resolve!:(value:unknown)=>void;f.respond(()=>new Promise(done=>{resolve=done;}));
    actions.button('Save answer').click();actions.button('Save answer').click();await flush();
    assert.equal(f.requests.length,1);const request=f.requests[0]!.body;
    assert.equal(request.app_id,APP);assert.equal(request.work_id,WORK);assert.equal(request.scope.address,'example-app');
    assert.deepEqual(request.references,[reference]);assert.deepEqual(request.receipts,[receipt]);
    resolve(response({card:saved({card_id:request.card_id})}));await flush();
    assert.equal(actions.button('Saved').disabled,true);assert.match(actions.textContent,/Filed in this app/);assert.equal(notified.length,1);
    assert.deepEqual(f.placements,[],'saving never places the answer');
    actions.button('Make it a card').click();await flush();assert.equal(f.previews.length,1);
    assert.deepEqual(f.previews[0].view,{records:[RECORD]});assert.equal(f.requests.length,1,'the preview does not save another snapshot');
  }finally{f.cleanup();}
});

test('uncertain saves retry the same ID without creating duplicate snapshots',async()=>{
  const f=await fixture();try{
    const actions=f.actions(recordSnapshot());f.respond(async()=>response({code:'unavailable'},503));
    actions.button('Save answer').click();await flush();assert.equal(actions.button('Save answer').disabled,false);
    f.respond(async request=>response({card:saved({card_id:request.body.card_id})}));actions.button('Save answer').click();await flush();
    assert.equal(f.requests.length,2);assert.equal(f.requests[0]!.body.card_id,f.requests[1]!.body.card_id);assert.deepEqual(f.placements,[]);
  }finally{f.cleanup();}
});

test('file snapshots send the bound file and row receipts without offering a record-card preview',async()=>{
  const f=await fixture();try{
    const actions=f.actions(fileSnapshot());assert.equal(actions.querySelector('.cv-app-answer-make'),null);
    f.respond(async request=>response({card:{...savedFile(),card_id:request.body.card_id}}));
    actions.button('Save answer').click();await flush();
    const request=f.requests[0]!.body;assert.deepEqual(request.file_source,{box_id:BOX,file_id:FILE});assert.deepEqual(request.file_references,[row]);
    assert.deepEqual(request.references,[]);assert.deepEqual(request.receipts,[]);assert.equal(actions.button('Saved').disabled,true);assert.deepEqual(f.placements,[]);
    const wrong=fileSnapshot();wrong.reply.references[0]={...row,file_id:OTHER};
    assert.equal(f.actions(wrong).button('Save answer').disabled,true);
    const other=recordSnapshot();other.scope.works[0]!.work_id=OTHER;
    assert.equal(f.actions(other).button('Save answer').disabled,true);
  }finally{f.cleanup();}
});

test('the drawer filters ownership and receipt scope, and explicitly places the freshly authorized snapshot',async()=>{
  const f=await fixture();try{
    const corrupt=saved({card_id:OTHER,spec:{...saved().spec,references:[{...reference,app_id:OTHER}]}});
    const initial=saved();f.respond(async()=>response({cards:[initial,saved({card_id:FILE,work_id:OTHER}),saved({card_id:BOX,kind:'card'}),corrupt]}));
    assert.equal(f.requests.length,0);f.controller.show();await flush();
    assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,1);assert.match(f.host.textContent,/2026-11-02 20:00 UTC/);assert.match(f.host.textContent,/App records/);
    assert.equal(f.host.hidden,true,'shared companion visibility belongs to its coordinator');assert.deepEqual(f.placements,[]);
    const fresh=saved({spec:{...initial.spec,text:'A refreshed authorized snapshot.'}});f.respond(async()=>response({cards:[fresh]}));
    f.host.button('Place on canvas').click();await flush();
    assert.equal(f.requests.length,2);assert.equal(f.placements.length,1);assert.equal(f.placements[0].spec.text,'A refreshed authorized snapshot.');
  }finally{f.cleanup();}
});

test('lost access prevents placement and removes the previously placed utility, without deleting saved data',async()=>{
  const f=await fixture();try{
    f.respond(async()=>response({cards:[saved()]}));f.controller.show();await flush();f.utility(CARD);
    f.respond(async()=>response({cards:[]}));f.host.button('Place on canvas').click();await flush();
    assert.deepEqual(f.placements,[]);assert.deepEqual(f.removed,[`utility-pin-${CARD}`]);assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,0);
    assert.match(f.host.textContent,/no longer available/);assert.ok(f.requests.every(request=>request.method==='GET'));
  }finally{f.cleanup();}
});

test('Put all away removes only this app’s answer placements and keeps durable snapshots',async()=>{
  const f=await fixture();try{
    f.utility(CARD);const other=f.utility(OTHER);
    f.respond(async()=>response({cards:[saved(),saved({card_id:OTHER,app_id:OTHER})]}));f.controller.show();await flush();
    f.host.button('Put all away').click();await flush();
    assert.deepEqual(f.removed,[`utility-pin-${CARD}`]);assert.equal(f.utilities.get(`utility-pin-${OTHER}`),other);
    assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,1);assert.equal(f.requests.length,1);assert.match(f.host.textContent,/still saved here/);
  }finally{f.cleanup();}
});

test('deletion is explicit, failures retain the snapshot, and success removes only its placement',async()=>{
  const f=await fixture();try{
    f.respond(async()=>response({cards:[savedFile()]}));f.controller.show();await flush();f.utility(CARD);
    assert.match(f.host.textContent,/example.csv/);assert.match(f.host.textContent,/row 1/);
    f.respond(async()=>response({code:'unavailable'},503));f.host.button('Delete saved answer').click();await flush();
    assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,1);assert.deepEqual(f.removed,[]);
    f.respond(async request=>request.method==='GET'?response({cards:[]}):response({removed:true}));
    f.host.button('Delete saved answer').click();await flush();
    assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,0);assert.deepEqual(f.removed,[`utility-pin-${CARD}`]);
    assert.deepEqual(f.requests.filter(request=>request.method==='POST').map(request=>request.url),[`/v1/cards/${CARD}/remove`,`/v1/cards/${CARD}/remove`]);
  }finally{f.cleanup();}
});

test('hidden or disposed drawers ignore late inventory and placement responses and stop listening',async()=>{
  const f=await fixture();try{
    let resolve!:(value:unknown)=>void;f.respond(()=>new Promise(done=>{resolve=done;}));
    f.controller.show();await flush();f.controller.hide();resolve(response({cards:[saved()]}));await flush();assert.equal(f.host.querySelectorAll('.cv-app-saved-entry').length,0);
    f.respond(async()=>response({cards:[saved()]}));f.controller.show();await flush();
    f.respond(()=>new Promise(done=>{resolve=done;}));f.host.button('Place on canvas').click();await flush();f.controller.hide();
    resolve(response({cards:[saved()]}));await flush();assert.deepEqual(f.placements,[]);
    f.controller.dispose();const before=f.requests.length;
    f.respond(async()=>response({card:saved()}));await f.api.testStoreCard({card_id:CARD});await flush();
    assert.equal(f.requests.length,before+1,'disposed drawer makes no inventory read for a store notification');assert.equal(f.host.children.length,0);
  }finally{f.cleanup();}
});

test('every saved source is reachable in bounded pages without placing an answer',async()=>{
  const f=await fixture();try{
    const file=savedFile();file.spec.file_references=Array.from({length:51},(_,index)=>({...row,row:index+1,ref:`f1r${index+1}`}));
    f.respond(async()=>response({cards:[file]}));f.controller.show();await flush();
    assert.match(f.host.textContent,/Sources \(51\)/);assert.match(f.host.textContent,/1–50 of 51/);
    assert.equal(f.host.querySelectorAll('pre').length,50);assert.doesNotMatch(f.host.textContent,/row 51/);
    f.host.button('Next sources').click();assert.match(f.host.textContent,/row 51/);assert.match(f.host.textContent,/51–51 of 51/);
    assert.equal(f.host.querySelectorAll('pre').length,1);assert.equal(f.host.button('Next sources').disabled,true);
    f.host.button('Previous sources').click();assert.equal(f.host.querySelectorAll('pre').length,50);assert.deepEqual(f.placements,[]);assert.equal(f.requests.length,1);
  }finally{f.cleanup();}
});
