import assert from 'node:assert/strict';
import {test} from 'node:test';
import {build} from 'esbuild';
import {fileURLToPath} from 'node:url';
// Actual app-view/presentation controllers; only canvas transport and authorized
// group loading are local spies. No backend call or duplicated form renderer.
// Move-aware DOM spy, with no HTML parser or executable markup sink.
class N {
  parentElement: E | null = null;
  constructor(readonly ownerDocument: D) {}
  get isConnected(): boolean { return !!this.parentElement?.isConnected; }
  remove() { const p = this.parentElement; if (p) p.children = p.children.filter(n => n !== this); this.parentElement = null; }
  replaceWith(n: N) { const p = this.parentElement; if (!p) return; const i = p.children.indexOf(this); n.remove(); p.children.splice(i, 1, n); n.parentElement = p; this.parentElement = null; }
}
class E extends N {
  children: N[] = []; dataset: Record<string,string> = {}; attributes: Record<string,string> = {};
  textContent = ''; className = ''; hidden = false; open = false; disabled = false; type = ''; value = ''; tabIndex = 0;
  listeners = new Map<string, ((e: any) => void)[]>();
  style: Record<string,any> = {setProperty(k:string,v:string){this[k]=v;},removeProperty(k:string){delete this[k];}};
  inert=false;scrollTop=0;scrollLeft=0;clientWidth=1000;clientHeight=800;offsetWidth=400;offsetHeight=400;
  classList = {add:(...names:string[])=>{this.className=[...new Set([...this.className.split(/\s+/),...names])].filter(Boolean).join(' ');},remove:(...names:string[])=>{this.className=this.className.split(/\s+/).filter(n=>!names.includes(n)).join(' ');},contains:(name:string)=>this.className.split(/\s+/).includes(name),toggle:(name:string,on?:boolean)=>{const keep=on??!this.classList.contains(name);if(keep)this.classList.add(name);else this.classList.remove(name);return keep;}};
  constructor(doc: D, readonly tag: string) { super(doc); }
  override get isConnected(): boolean { return this === this.ownerDocument.root || super.isConnected; }
  get childElementCount() { return this.children.filter(n => n instanceof E).length; }
  setAttribute(k: string,v: string) { this.attributes[k]=v;if(k==='class')this.className=v; if(k.startsWith('data-'))this.dataset[data(k)]=v; }
  getAttribute(k:string){return k.startsWith('data-')?this.dataset[data(k)]??null:this.attributes[k]??null;}
  removeAttribute(k:string){delete this.attributes[k];if(k.startsWith('data-'))delete this.dataset[data(k)];}
  get firstElementChild(){return this.children.find(n=>n instanceof E)??null;}
  hasAttribute(k: string) { return k in this.attributes; }
  append(...nodes: (N|string)[]) { for(const n of nodes)if(typeof n==='string')this.textContent+=n;else{n.remove();this.children.push(n);n.parentElement=this;} }
  replaceChildren(...nodes:N[]){for(const n of [...this.children])n.remove();this.textContent='';this.append(...nodes);}
  contains(n:N):boolean{return n===this||this.children.some(c=>c instanceof E&&c.contains(n));}
  matches(s:string):boolean{
    if(s.startsWith('#'))return this.getAttribute('id')===s.slice(1);
    if(s.includes(':not(:disabled)')&&this.disabled)return false;
    if(s.includes(':not([type="hidden"])')&&this.type==='hidden')return false;
    s=s.replace(/:not\([^)]*\)/g,'');const tag=s.match(/^[a-z]+/)?.[0];if(tag&&this.tag!==tag)return false;
    if(s.startsWith('.'))return this.className.split(/\s+/).includes(s.slice(1));
    return [...s.matchAll(/\[([^=\]]+)(?:="([^"]*)")?\]/g)].every(([,k,v])=>{if(k==='open')return this.open;if(k==='hidden')return this.hidden;const a=k!.startsWith('data-')?this.dataset[data(k!)]:this.attributes[k!];return v===undefined?a!==undefined:a===v;});
  }
  querySelectorAll(s:string):E[]{const selectors=s.split(',').map(s=>s.trim()),out:E[]=[];const walk=(p:E)=>{for(const c of p.children)if(c instanceof E){if(selectors.some(s=>s.startsWith(':scope > ')?p===this&&c.matches(s.slice(9)):c.matches(s)))out.push(c);walk(c);}};walk(this);return out;}
  querySelector(s:string):E|null{return this.querySelectorAll(s)[0]??null;}
  closest(s:string):E|null{return s.split(',').some(v=>this.matches(v.trim()))?this:this.parentElement?.closest(s)??null;}
  addEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,[...(this.listeners.get(t)??[]),f]);}
  removeEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,(this.listeners.get(t)??[]).filter(v=>v!==f));}
  emit(t:string,target:E=this,extra:Record<string,unknown>={}){const event={target,defaultPrevented:false,...extra,preventDefault(){this.defaultPrevented=true;},stopPropagation(){}};for(const f of [...(this.listeners.get(t)??[])])f(event);return event;}
  getBoundingClientRect(){return{left:0,top:0,width:this.offsetWidth,height:this.offsetHeight};}
  getClientRects(){return this.isConnected&&!this.closest('[hidden]')?[{}]:[];}
  focus(){this.ownerDocument.activeElement=this;}scrollIntoView(){}showModal(){this.open=true;}close(){this.open=false;this.emit('close');}
}
const data=(s:string)=>s.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase());
class D{
 root=new E(this,'main');activeElement:E|null=null;listeners=new Map<string,((e:any)=>void)[]>();
 createElement(t:string){return new E(this,t);}createElementNS(_ns:string,t:string){return new E(this,t);}createComment(_t:string){return new N(this);}
 querySelector(s:string){return this.root.querySelector(s);}getElementById(){return null;}
 addEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,[...(this.listeners.get(t)??[]),f]);}
 removeEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,(this.listeners.get(t)??[]).filter(v=>v!==f));}
 emit(t:string,detail:any){for(const f of this.listeners.get(t)??[])f({detail});}
}

const APP='aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee', WORK='11111111-2222-4333-8444-555555555555';
let moduleNumber=0;
async function fixture(options:{defer?:boolean;standalone?:boolean}={}) {
 const d=new D(), parked=d.createElement('section');d.root.append(parked);
 const make=(id:string,kind:string)=>{const e=d.createElement('article');e.dataset.card=id;e.dataset.kind=kind;e.dataset.component=id;e.className='cv-card';parked.append(e);return e;};
 const face=make('face','app'),summary=make('summary','summary'),form=make('form','form');
 const input=d.createElement('input');input.value='Unsent draft';const key=d.createElement('input');key.type='hidden';key.value='same-command-key';form.append(input,key);
 const receipt=d.createElement('details');receipt.textContent='Retained receipt bytes';summary.append(receipt);
 const definition={version:1,description:'Track real requests and their progress.',sections:[{id:'main',components:['summary']}],actions:[{id:'create',label:'Add record',behavior:'dialog',target:'form'}],theme:{mode:'dark',accent:'green'}};
 const group={app:APP,work:WORK,title:'One app',cards:[face,summary,form],origins:[],canShare:true,canEditPresentation:true,presentation:JSON.stringify(definition),partLayout:JSON.stringify([{id:'summary',row:0,column:0,width:12},{id:'form',row:1,column:0,width:12}])};
 const groups=new Map([[WORK,group]]);
 const states=new Map<E,any>();let mounts=0,removed=0,loads=0,remembered=0,release:()=>void=()=>{};
 const gate=options.defer?new Promise<void>(resolve=>release=resolve):Promise.resolve();
 const canvas={mountItem(item:E,at:any){mounts++;states.set(item,{...at,width:400,height:300});d.root.append(item);},removeItem(item:E){removed++;states.delete(item);item.remove();},resizeItem(item:E,width:number,height:number){Object.assign(states.get(item),{width,height});item.offsetWidth=width;item.offsetHeight=height;},getItemState(item:E){return {...states.get(item)};},moveItemTo(item:E,x:number,y:number){Object.assign(states.get(item),{x,y});},getCamera(){return{zoom:1};},centerItem(){},focusItem(){}};
 const board={canvas:()=>canvas,group:(work:string)=>groups.get(work)??null,findCard:(id:string)=>{for(const group of groups.values()){const card=group.cards.find(card=>card.dataset.card===id);if(card)return{group,card,item:null};}return null;},lift(){},show:(id:string)=>group.cards.find(card=>card.dataset.card===id)??null,park:(card:E)=>parked.append(card),pin(){},changed(){},member:()=> '0123456789abcdef',flash(){},announce(){}};
 const store=new Map<string,string>();
 Object.assign(globalThis,{HTMLElement:E,document:d,matchMedia:()=>({matches:false}),localStorage:{getItem:(k:string)=>store.get(k)??null,setItem:(k:string,v:string)=>store.set(k,v)},__view:{board,ensure:async(_app:string,work:string)=>{loads++;await gate;return groups.get(work)??null;},remember:()=>remembered++,standalone:()=>!!options.standalone,removeWork:()=>0,hooks:null}});
 const stubs:Record<string,string>={
  './board':`const f=globalThis.__view;export const stackBoard=f.board;export const ensureAppGroup=f.ensure;export const rememberAppPlacement=f.remember;export const isStandaloneWork=f.standalone;export const removeWork=f.removeWork;export const refreshGroupPresentation=async()=>true;export const runGroup=()=>{};export const onLayoutSaved=()=>{};export const setUniteHooks=h=>f.hooks=h;`,
  './explain':`export const toggleAppSummary=()=>{};export const disposeAppSummary=()=>{};`,
  './library':`export const findApps=()=>[];`, './slash':`export const registerCommands=()=>{};export const registerCompleter=()=>{};`,
  './stacks':`export const releaseCards=()=>[];`, '../sharing/dialog':`export const dismissSharing=()=>false;export const openSharing=()=>{};`,
  './render':`export const originKind=()=>'';export function el(d,t,a={},...children){const n=d.createElement(t);for(const[k,v]of Object.entries(a))n.setAttribute(k,v);n.append(...children.filter(c=>c!==null&&c!==undefined&&c!==false));return n;}`,
  '../canvas-vendor/index':`export function createCanvasItem(o){const item=o.content.ownerDocument.createElement('article');item.dataset.instanceId=o.instanceId;item.className='widget-instance';item.append(o.content);return item;}`,
 };
 const result=await build({entryPoints:[fileURLToPath(new URL('../src/canvas/unite.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'node',banner:{js:`// fixture ${++moduleNumber}`},plugins:[{name:'app-view-test-boundaries',setup(b){b.onResolve({filter:/.*/},args=>stubs[args.path]?{path:args.path,namespace:'fixture'}:null);b.onLoad({filter:/.*/,namespace:'fixture'},args=>({contents:stubs[args.path]!,loader:'js'}));}}]});
 const api=await import('data:text/javascript;base64,'+Buffer.from(result.outputFiles[0]!.text).toString('base64'));
 api.startUnite(d);
 return{api,d,group,groups,form,input,key,receipt,face,parked,store,release,canvas,setStandalone:(value:boolean)=>{options.standalone=value;},mounts:()=>mounts,removed:()=>removed,loads:()=>loads,remembered:()=>remembered};
}

test('Card → Stack → Open retains one outer item and exact component/form/receipt nodes',async()=>{
 const f=await fixture();await f.api.setAppView(APP,WORK,'face');const item=f.api.appViewForWork(WORK).item;
 assert.equal(f.mounts(),1);assert.ok(item.querySelector('[data-app-view-parking]').contains(f.face));
 assert.equal(item.querySelector('.cv-app-view-picker').querySelector('[data-app-view-mode="face"]').textContent,'Card');
 assert.equal(item.querySelector('.cv-app-face-summary').textContent,'Track real requests and their progress.');
 const link=item.querySelector('a');assert.equal(link.getAttribute('target'),'_blank');assert.equal(link.getAttribute('rel'),'noopener');
 for(const mode of ['stack','open','face','open']){await f.api.setAppView(APP,WORK,mode);assert.equal(f.api.appViewForWork(WORK).item,item);assert.equal(f.input.value,'Unsent draft');assert.equal(f.key.value,'same-command-key');assert.ok(item.contains(f.receipt));}
 assert.equal(f.mounts(),1);assert.equal(f.removed(),0);assert.equal(f.group.cards.filter((card:E)=>item.contains(card)).length,3);
 const before=f.canvas.getItemState(item);await f.api.setAppView(APP,WORK,'open');assert.deepEqual(f.canvas.getItemState(item),before);
});

test('Summary open and close preserve custom Open dimensions, one item and drafts across all app views',async()=>{
 const f=await fixture(), storageKey='forma.canvas.united.v1:0123456789abcdef';
 f.store.set(storageKey,JSON.stringify([{app:APP,work:WORK,x:95,y:130,previous:{},mode:'open',width:820,height:550}]));
 await f.api.restoreAppViews();
 const item=f.api.appViewForWork(WORK).item, root=item.querySelector('.cv-unite'), host=item.querySelector('[data-app-companion]');
 const original=f.canvas.getItemState(item);
 const summary=(open:boolean)=>{host.hidden=!open;f.d.emit('forma:app-companion',{app:APP,work:WORK,open});};
 const stored=()=>JSON.parse(f.store.get(storageKey)!)[0];
 assert.deepEqual(original,{x:95,y:130,width:820,height:550});
 summary(true);
 assert.equal(f.canvas.getItemState(item).width,1000,'the companion is clamped to the available viewport');
 assert.equal(root.classList.contains('has-companion'),true);
 assert.equal(item.querySelector('[data-app-summary]').getAttribute('aria-expanded'),'true');
 for(const mode of ['face','stack','open']){
  await f.api.setAppView(APP,WORK,mode,{focus:false});
  assert.equal(f.api.appViewForWork(WORK).item,item);
  assert.equal(item.querySelector('[data-app-companion]'),host);assert.equal(host.hidden,false);
  assert.equal(f.form.querySelector('input'),f.input);assert.equal(f.input.value,'Unsent draft');
  assert.equal(f.key.value,'same-command-key');assert.ok(item.contains(f.receipt));
  assert.equal(stored().mode,mode);assert.equal(stored().width,820);assert.equal(stored().height,550);
 }
 summary(false);
 assert.deepEqual(f.canvas.getItemState(item),original,'closing restores the window size instead of persisting companion geometry');
 assert.equal(root.classList.contains('has-companion'),false);
 assert.equal(item.querySelector('[data-app-summary]').getAttribute('aria-expanded'),'false');
 summary(true);summary(false);
 assert.deepEqual(f.canvas.getItemState(item),original,'repeated opening never accumulates extra width');
 assert.equal(f.mounts(),1);assert.equal(f.removed(),0);
});

test('resizing a clamped Summary window starts at its rendered width without a dead drag range',async()=>{
 const f=await fixture();await f.api.restoreAppViews();await f.api.setAppView(APP,WORK,'open');
 const item=f.api.appViewForWork(WORK).item, host=item.querySelector('[data-app-companion]');
 host.hidden=false;f.d.emit('forma:app-companion',{app:APP,work:WORK,open:true});
 assert.equal(f.canvas.getItemState(item).width,1000);
 const event=(target:E,x:number)=>({target,pointerId:1,button:0,clientX:x,clientY:550,preventDefault(){},stopPropagation(){}});
 const handle=item.querySelector('[data-unite-resize]');
 for(const listener of [...(f.d.listeners.get('pointerdown')??[])])listener(event(handle,664));
 for(const listener of [...(f.d.listeners.get('pointermove')??[])])listener(event(handle,614));
 assert.equal(f.canvas.getItemState(item).width,950,'a 50px drag immediately changes the visible window by 50px');
 for(const listener of [...(f.d.listeners.get('pointerup')??[])])listener(event(handle,614));
 host.hidden=true;f.d.emit('forma:app-companion',{app:APP,work:WORK,open:false});
 assert.equal(f.canvas.getItemState(item).width,614,'closing retains the explicit resize, excluding companion width');
 assert.equal(JSON.parse(f.store.get('forma.canvas.united.v1:0123456789abcdef')!)[0].width,614);
 assert.equal(f.input.value,'Unsent draft');assert.equal(f.mounts(),1);
});

test('changing mode while a component dialog is open returns its original form before parking',async()=>{
 const f=await fixture();await f.api.setAppView(APP,WORK,'open');const item=f.api.appViewForWork(WORK).item,root=item.querySelector('.cv-unite');
 root.emit('click',root.querySelector('[data-app-action="create"]'));assert.ok(root.querySelector('dialog')?.open);assert.ok(root.querySelector('dialog')?.contains(f.form));
 await f.api.setAppView(APP,WORK,'stack');assert.equal(root.querySelector('dialog'),null);assert.ok(item.contains(f.form));
 await f.api.setAppView(APP,WORK,'open');assert.equal(f.form.querySelector('input'),f.input);assert.equal(f.key.value,'same-command-key');
 assert.equal(root.querySelectorAll('[data-card="form"]').length,1);
});

test('overlapping mode requests serialize one placement and a removal cancels pending acquisition',async()=>{
 const f=await fixture({defer:true});const a=f.api.setAppView(APP,WORK,'face'),b=f.api.setAppView(APP,WORK,'stack'),c=f.api.setAppView(APP,WORK,'open');
 await new Promise(resolve=>setTimeout(resolve,0));assert.equal(f.mounts(),0);f.release();await Promise.all([a,b,c]);assert.equal(f.mounts(),1);assert.equal(f.api.appViewForWork(WORK).mode,'open');
 const g=await fixture({defer:true});const pending=g.api.setAppView(APP,WORK,'face');await new Promise(resolve=>setTimeout(resolve,0));g.api.removeAppView(WORK);g.release();
 assert.match(await pending,/cancelled/);assert.equal(g.mounts(),0);assert.equal(g.remembered(),0);
});

test('a dismissed file conversion cannot place an app after acquisition resolves',async()=>{
 const f=await fixture({defer:true});let current=true;
 const pending=f.api.setAppView(APP,WORK,'face',{isCurrent:()=>current});
 await new Promise(resolve=>setTimeout(resolve,0));current=false;f.release();
 assert.match(await pending,/cancelled/);assert.equal(f.mounts(),0);assert.equal(f.remembered(),0);
 assert.equal(f.api.appViewForWork(WORK),null);
});

test('atomic independent card suppresses alternate modes and all app split entry points refuse',async()=>{
 const f=await fixture({standalone:true});await f.api.setAppView(APP,WORK,'face');const view=f.api.appViewForWork(WORK);
 assert.equal(view.mode,'open');assert.equal(view.item.querySelector('.cv-app-view-picker').hidden,true);
 assert.match(f.api.breakApart(WORK),/stay together/);assert.match(f.api.splitFromItem(view.item),/stay together/);assert.equal(f.mounts(),1);
 assert.equal(view.item.querySelector('[data-unite-split]'),null);assert.equal(view.item.querySelector('[data-stack-spread]'),null);
 f.api.removeAppView(WORK);assert.equal(f.api.appViewForWork(WORK),null);assert.equal(f.removed(),1);assert.ok(f.parked.contains(f.form));assert.equal(f.key.value,'same-command-key');
});

test('removal before initial layout restoration cannot resurrect an old united placement',async()=>{
 const f=await fixture(), member='0123456789abcdef';
 f.store.set(`forma.canvas.united.v1:${member}`,JSON.stringify([{app:APP,work:WORK,x:90,y:100,previous:{},mode:'open'}]));
 await f.api.setAppView(APP,WORK,'face');f.api.removeAppView(WORK);
 await f.api.restoreAppViews();
 assert.equal(f.api.appViewForWork(WORK),null);assert.equal(f.mounts(),1);
 assert.deepEqual(JSON.parse(f.store.get(`forma.canvas.united.v1:${member}`)!),[]);
 await f.api.setAppView(APP,WORK,'stack');assert.equal(f.api.appViewForWork(WORK).mode,'stack');assert.equal(f.mounts(),2,'a later explicit Add is still allowed');
});

test('changing an existing Open app to independent Card updates labels and selector on the same item',async()=>{
 const f=await fixture();await f.api.setAppView(APP,WORK,'open');const item=f.api.appViewForWork(WORK).item;
 f.setStandalone(true);await f.api.setAppView(APP,WORK,'open');
 assert.equal(item.querySelector('.cv-unite').dataset.placementKind,'card');
 assert.equal(item.querySelector('.cv-app-view-picker').hidden,true);
 assert.equal(item.querySelector('[data-unite-ask]').getAttribute('aria-label'),'Ask about this card');
 assert.equal(item.querySelector('[data-edit-presentation]').getAttribute('aria-label'),'Card settings');
 f.setStandalone(false);await f.api.setAppView(APP,WORK,'open');
 assert.equal(item.querySelector('.cv-app-view-picker').hidden,false);assert.equal(item.querySelector('[data-edit-presentation]').getAttribute('aria-label'),'App settings');assert.equal(f.mounts(),1);
});

test('hidden components leave Stack and Focus sequences but retain the same draft for later restoration',async()=>{
 const f=await fixture();await f.api.setAppView(APP,WORK,'stack');const item=f.api.appViewForWork(WORK).item;
 f.api.shuffleAppView(item,2);assert.equal(f.api.selectedAppCard(item),f.form);
 f.store.set(`forma.app.hidden.v1:${APP}/${WORK}`,JSON.stringify(['summary','form']));f.d.emit('forma:app-view-preferences',{app:APP,work:WORK});
 assert.equal(f.api.selectedAppCard(item),f.face);assert.deepEqual(f.api.frameCardsOf('form'),['face']);
 assert.ok(item.querySelector('[data-app-view-parking]').contains(f.form));assert.equal(f.input.value,'Unsent draft');
 await f.api.setAppView(APP,WORK,'open');assert.equal(item.querySelector('[data-app-action="create"]'),null);
 f.store.delete(`forma.app.hidden.v1:${APP}/${WORK}`);f.d.emit('forma:app-view-preferences',{app:APP,work:WORK});
 assert.ok(item.querySelector('[data-app-action="create"]'));assert.equal(f.key.value,'same-command-key');assert.equal(f.mounts(),1);
});


test('Arrange moves whole frames into rows and persists positions without changing modes, Open sizes or drafts',async()=>{
 const f=await fixture();await f.api.restoreAppViews();
 const otherWork='22222222-2222-4333-8444-555555555555';
 const otherCards=f.group.cards.map((card:E)=>{const copy=f.d.createElement('article');copy.dataset={...card.dataset,card:'other-'+card.dataset.card};copy.className='cv-card';f.parked.append(copy);return copy;});
 f.groups.set(otherWork,{...f.group,work:otherWork,cards:otherCards});
 await f.api.setAppView(APP,WORK,'open',{at:{x:900,y:1400}});
 await f.api.setAppView(APP,otherWork,'stack',{at:{x:-600,y:-600}});
 const first=f.api.appViewForWork(WORK).item,second=f.api.appViewForWork(otherWork).item;
 const size=f.canvas.getItemState(first),stackSize=f.canvas.getItemState(second);
 assert.equal(f.api.arrangeAppViews(),2);
 assert.equal(f.api.appViewForWork(WORK).mode,'open');assert.equal(f.api.appViewForWork(otherWork).mode,'stack');
 const a=f.canvas.getItemState(first),b=f.canvas.getItemState(second);
 assert.deepEqual(a,{...size,x:64,y:64});assert.deepEqual(b,{...stackSize,x:64,y:64+first.offsetHeight+48});
 assert.equal(f.api.appViewForWork(WORK).item,first);assert.ok(first.contains(f.form));assert.equal(f.input.value,'Unsent draft');assert.equal(f.key.value,'same-command-key');
 const rows=JSON.parse(f.store.get('forma.canvas.united.v1:0123456789abcdef')!);
 assert.equal(rows.length,2);assert.deepEqual(rows.map((row:any)=>[row.mode,row.x,row.y]),[['open',a.x,a.y],['stack',b.x,b.y]]);
 assert.equal(rows[0].width,size.width);assert.equal(rows[0].height,size.height);
 const before=JSON.stringify(rows);f.api.arrangeAppViews();assert.equal(f.store.get('forma.canvas.united.v1:0123456789abcdef'),before);
});

test('Face without an admitted description has a useful inert summary',async()=>{
 const f=await fixture();f.group.presentation='{}';await f.api.setAppView(APP,WORK,'face');
 assert.equal(f.api.appViewForWork(WORK).item.querySelector('.cv-app-face-summary').textContent,'Open your workspace to explore the data and use its tools.');
});


test('queued generic placement preserves the mode chosen by earlier explicit requests',async()=>{
 const f=await fixture({defer:true});
 const explicit=f.api.setAppView(APP,WORK,'open');
 await new Promise(resolve=>setTimeout(resolve,0));
 const lateLive=f.api.setAppView(APP,WORK,'face',{preserveMode:true});
 f.release();await Promise.all([explicit,lateLive]);
 const item=f.api.appViewForWork(WORK).item;
 assert.equal(f.api.appViewForWork(WORK).mode,'open');assert.equal(f.mounts(),1);
 const stack=f.api.setAppView(APP,WORK,'stack');
 const generic=f.api.setAppView(APP,WORK,'face',{preserveMode:true});
 await Promise.all([stack,generic]);assert.equal(f.api.appViewForWork(WORK).mode,'stack');
 assert.equal(f.api.appViewForWork(WORK).item,item);assert.equal(f.input.value,'Unsent draft');assert.equal(f.key.value,'same-command-key');
 await f.api.setAppView(APP,WORK,'face');assert.equal(f.api.appViewForWork(WORK).mode,'face','an explicit Library mode still wins');
 f.setStandalone(true);await f.api.setAppView(APP,WORK,'face',{preserveMode:true});
 assert.equal(f.api.appViewForWork(WORK).mode,'open','independent Cards remain atomic Open');
 const cold=await fixture();await cold.api.setAppView(APP,WORK,'face',{preserveMode:true});
 assert.equal(cold.api.appViewForWork(WORK).mode,'face','a first generic placement uses its requested default');
});

test('Focus starts a parked component read only after the original node and dialog are visible', async () => {
  const d = new D(), parked = d.createElement('section');
  parked.hidden = true; d.root.append(parked);
  const cards = ['first', 'second'].map(id => {const card = d.createElement('article'); card.dataset.card = id; card.dataset.kind = 'summary'; parked.append(card); return card;});
  const draft = d.createElement('input'); draft.value = 'Unsent value'; cards[0]!.append(draft);
  const doc = d as any;
  doc.body = d.root;
  doc.createElementNS = (_namespace: string, tag: string) => d.createElement(tag);
  doc.getElementById = (id: string) => d.root.querySelectorAll('[id]').find(node => node.getAttribute('id') === id) ?? null;
  const reads: string[] = [];
  const group = {app: APP, work: WORK, title: 'Whole app', cards};
  const el = (_document: D, tag: string, attrs: Record<string,string>, ...children: (N|string)[]) => {
    const node = d.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {node.setAttribute(key, value); if (key === 'hidden') node.hidden = true;}
    node.append(...children); return node;
  };
  const board = {
    findCard: (id: string) => {const card = cards.find(card => card.dataset.card === id); return card ? {card, group} : null;},
    show: (id: string) => {
      const card = cards.find(card => card.dataset.card === id)!;
      const dialog = doc.getElementById('cv-focus');
      assert.equal(dialog.hidden, false, 'the Focus dialog must be revealed before its governed read');
      assert.equal(card.parentElement?.hasAttribute('data-focus-stage'), true, 'the original component must already be in Focus');
      assert.equal(card.closest('[hidden], [inert]'), null, 'a parked component cannot count as shown');
      reads.push(id); return card;
    },
    park: (card: E) => parked.append(card), workOf: () => WORK, group: () => group, announce: () => undefined,
  };
  const key = `__focusRead${moduleNumber++}`;
  (globalThis as any)[key] = {el, board};
  const entry = fileURLToPath(new URL('../src/canvas/focus.ts', import.meta.url));
  const stubs: Record<string,string> = {
    '../canvas-vendor/index': 'export const eventOriginatesInTextEntry = () => false;',
    './board': `export const stackBoard = globalThis[${JSON.stringify(key)}].board; export const isOnBoard = () => true; export const onBoardSettled = () => {}; export const paneKind = kind => kind;`,
    './render': `export const el = globalThis[${JSON.stringify(key)}].el;`,
    './stacks': 'export const stackCardsOf = () => null;',
    './unite': 'export const frameCardsOf = () => ["first", "second"]; export const appViewOf = () => null; export const isUniteItem = () => false; export const selectedAppCard = () => null; export const keepScroll = () => () => {};',
    '../presentation/presentation': 'export const appComponentVisible = () => true;',
  };
  try {
    const built = await build({entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', write: false, plugins: [{name: 'focus-boundaries', setup(builder) {
      builder.onResolve({filter: /.*/}, args => args.importer === entry && args.path in stubs ? {path: args.path, namespace: 'focus-spy'} : undefined);
      builder.onLoad({filter: /.*/, namespace: 'focus-spy'}, args => ({contents: stubs[args.path], loader: 'js'}));
    }}]});
    const api = await import('data:text/javascript;base64,' + Buffer.from(built.outputFiles[0]!.text).toString('base64'));
    api.startFocus(doc);
    assert.equal(api.openFocus('first'), true);
    assert.deepEqual(reads, ['first']); assert.equal(cards[0]!.querySelector('input'), draft); assert.equal(draft.value, 'Unsent value');
    assert.equal(api.openFocus('second'), true);
    assert.deepEqual(reads, ['first', 'second']); assert.equal(cards[0]!.parentElement, parked);
  } finally {delete (globalThis as any)[key];}
});
