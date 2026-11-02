import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyPresentationTheme, parsePresentation, presentationFor, invalidatePresentation } from '../src/presentation/presentation.ts';
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
  classList = { add: (...names: string[]) => { this.className += ` ${names.join(' ')}`; } };
  constructor(doc: D, readonly tag: string) { super(doc); }
  override get isConnected(): boolean { return this === this.ownerDocument.root || super.isConnected; }
  get childElementCount() { return this.children.filter(n => n instanceof E).length; }
  setAttribute(k: string,v: string) { this.attributes[k]=v; if(k.startsWith('data-'))this.dataset[data(k)]=v; }
  hasAttribute(k: string) { return k in this.attributes; }
  append(...nodes: (N|string)[]) { for(const n of nodes)if(typeof n==='string')this.textContent+=n;else{n.remove();this.children.push(n);n.parentElement=this;} }
  replaceChildren(...nodes:N[]){for(const n of [...this.children])n.remove();this.textContent='';this.append(...nodes);}
  contains(n:N):boolean{return n===this||this.children.some(c=>c instanceof E&&c.contains(n));}
  matches(s:string):boolean{
    if(s.includes(':not(:disabled)')&&this.disabled)return false;
    if(s.includes(':not([type="hidden"])')&&this.type==='hidden')return false;
    s=s.replace(/:not\([^)]*\)/g,'');const tag=s.match(/^[a-z]+/)?.[0];if(tag&&this.tag!==tag)return false;
    if(s.startsWith('.'))return this.className.split(/\s+/).includes(s.slice(1));
    return [...s.matchAll(/\[([^=\]]+)(?:="([^"]*)")?\]/g)].every(([,k,v])=>{if(k==='open')return this.open;if(k==='hidden')return this.hidden;const a=k!.startsWith('data-')?this.dataset[data(k!)]:this.attributes[k!];return v===undefined?a!==undefined:a===v;});
  }
  querySelectorAll(s:string):E[]{const selectors=s.split(',').map(s=>s.trim()),out:E[]=[];const walk=(p:E)=>{for(const c of p.children)if(c instanceof E){if(selectors.some(s=>s.startsWith(':scope > ')?p===this&&c.matches(s.slice(9)):c.matches(s)))out.push(c);walk(c);}};walk(this);return out;}
  querySelector(s:string):E|null{return this.querySelectorAll(s)[0]??null;}
  closest(s:string):E|null{return this.matches(s)?this:this.parentElement?.closest(s)??null;}
  addEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,[...(this.listeners.get(t)??[]),f]);}
  removeEventListener(t:string,f:(e:any)=>void){this.listeners.set(t,(this.listeners.get(t)??[]).filter(v=>v!==f));}
  emit(t:string,target:E=this,extra:Record<string,unknown>={}){const event={target,defaultPrevented:false,...extra,preventDefault(){this.defaultPrevented=true;},stopPropagation(){}};for(const f of [...(this.listeners.get(t)??[])])f(event);return event;}
  getClientRects(){return this.isConnected&&!this.closest('[hidden]')?[{}]:[];}
  focus(){this.ownerDocument.activeElement=this;}scrollIntoView(){}showModal(){this.open=true;}close(){this.open=false;this.emit('close');}
}
const data=(s:string)=>s.slice(5).replace(/-([a-z])/g,(_,c)=>c.toUpperCase());
class D{root=new E(this,'main');activeElement:E|null=null;createElement(t:string){return new E(this,t);}createComment(_t:string){return new N(this);}querySelector(s:string){return this.root.querySelector(s);}}
const source={version:1,description:'<script>inert text</script>',sections:[{id:'main',title:'Overview',components:['summary']}],actions:[{id:'create',label:'Log something',behavior:'dialog',target:'form'},{id:'overview',label:'Overview',behavior:'navigate',target:'summary'}],theme:{mode:'dark',accent:'green',density:'compact',font:'mono'}};
function fixture(){const d=new D(),root=d.root;root.dataset.presentation=JSON.stringify(source);const header=d.createElement('div');header.dataset.appPresentationHeader='';const parts=d.createElement('div');parts.dataset.documentParts='';const summary=d.createElement('article');summary.dataset.component='summary';const form=d.createElement('article');form.dataset.component='form';const input=d.createElement('input');input.value='Unsent draft';const key=d.createElement('input');key.type='hidden';key.value='original-key';form.append(input,key);parts.append(summary,form);root.append(header,parts);return{d,root,parts,form,input,key};}
function dom(run:()=>void){const prior=Object.getOwnPropertyDescriptor(globalThis,'HTMLElement');Object.defineProperty(globalThis,'HTMLElement',{configurable:true,value:E});try{run();}finally{if(prior)Object.defineProperty(globalThis,'HTMLElement',prior);else Reflect.deleteProperty(globalThis,'HTMLElement');}}
test('closed presentation rejects executable style/URLs/unknown behavior and duplicate components',()=>{
 assert.deepEqual(parsePresentation(JSON.stringify(source)),source);
 for(const bad of [{...source,css:'body{}'},{...source,version:2},{actions:source.actions},{...source,theme:{accent:'url(x)'}},{...source,theme:null},{...source,actions:[{id:'a',label:'Run',behavior:'javascript',target:'form'}]},{...source,actions:[{id:'a',label:'Run',behavior:'navigate',target:'https://foreign.test'}]},{...source,sections:[{id:'a',components:['form','form']}]}])assert.deepEqual(parsePresentation(bad),{});
});
test('dialog moves one form, preserves draft/key and Escape restores focus',()=>dom(()=>{
 const{root,form,input,key}=fixture(),c=presentationFor(root as unknown as HTMLElement),button=root.querySelector('[data-app-action="create"]')!;root.emit('click',button);const dialog=root.querySelector('dialog')!;
 assert.ok(dialog.open);assert.equal(dialog.querySelector('[data-component="form"]'),form);assert.equal(root.querySelectorAll('[data-component="form"]').length,1);assert.equal(input.value,'Unsent draft');assert.equal(key.value,'original-key');assert.equal(input.ownerDocument.activeElement,input);assert.equal(root.dataset.appFont,'mono');
 dialog.emit('cancel');assert.equal(root.querySelector('dialog'),null);assert.ok(form.closest('[data-app-dialog-pool]')?.hidden);assert.equal(input.ownerDocument.activeElement,button);assert.equal(form.querySelector('input'),input);c.dispose();
}));
test('refresh keeps open dialog and preserved form; lease masking clears moved protected content',()=>dom(()=>{
 const{root,parts,form,input}=fixture(),c=presentationFor(root as unknown as HTMLElement);root.emit('click',root.querySelector('[data-app-action="create"]')!);const dialog=root.querySelector('dialog')!;c.beforeRefresh();assert.ok(parts.contains(form));
 const next=root.ownerDocument.createElement('div');next.dataset.documentParts='';const summary=root.ownerDocument.createElement('article');summary.dataset.component='summary';next.append(summary,form);parts.replaceWith(next);c.afterRefresh(JSON.stringify(source));assert.equal(root.querySelector('dialog'),dialog);assert.ok(dialog.contains(form));assert.equal(root.ownerDocument.activeElement,input);
 invalidatePresentation(root as unknown as HTMLElement);next.remove();assert.equal(root.querySelector('dialog'),null);assert.equal(root.querySelector('[data-app-action]'),null);assert.equal(root.querySelector('input'),null);
}));
test('removed action closes dialog; missing target never rebinds or gains a button',()=>dom(()=>{
 const{root}=fixture(),c=presentationFor(root as unknown as HTMLElement);root.emit('click',root.querySelector('[data-app-action="create"]')!);c.beforeRefresh();c.afterRefresh(JSON.stringify({...source,actions:[{id:'create',label:'Other',behavior:'dialog',target:'missing'}]}));assert.equal(root.querySelector('dialog'),null);assert.equal(root.querySelector('[data-app-action]'),null);
}));

test('presentation text bounds count Unicode characters like admission', () => {
  const text = {version:1,description:'😀'.repeat(1000)};
  assert.deepEqual(parsePresentation(text), text);
  assert.deepEqual(parsePresentation({...text,description:'😀'.repeat(1001)}), {});
});

test('default renderer tokens allow local Shape but a published design clears that preview', () => dom(() => {
 const {root,input,key}=fixture();
 applyPresentationTheme(root as unknown as HTMLElement, '{}');
 assert.equal(root.dataset.appPresentation,'default');
 root.dataset.theme='harbor-dark';root.dataset.themeDensity='compact';root.dataset.themePreview='true';
 applyPresentationTheme(root as unknown as HTMLElement, JSON.stringify(source));
 assert.equal(root.dataset.appPresentation,'authored');assert.equal(root.dataset.appMode,'dark');assert.equal(root.dataset.appAccent,'green');
 assert.equal(root.dataset.theme,undefined);assert.equal(root.dataset.themeDensity,undefined);assert.equal(root.dataset.themePreview,undefined);
 assert.equal(input.value,'Unsent draft');assert.equal(key.value,'original-key');
}));
test('all finite themes keep readable text, action labels, focus and input boundaries', () => {
  const css = readFileSync(new URL('../src/styles/presentation.css', import.meta.url), 'utf8');
  const lum = (hex:string) => {const c=hex.slice(1).match(/../g)!.map(v=>parseInt(v,16)/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);return c[0]!*.2126+c[1]!*.7152+c[2]!*.0722;};
  const ratio=(a:string,b:string)=>{const x=lum(a),y=lum(b);return(Math.max(x,y)+.05)/(Math.min(x,y)+.05);};
  for(const [mode,bg,panel,ink,muted,onAccent,control] of [['light','#ffffff','#f8fafc','#162033','#49576b','#ffffff','#64748b'],['dark','#10171d','#19232c','#f1f5f9','#b4c2d0','#102a31','#94a3b8']]){
    assert.ok(ratio(ink!,panel!)>=4.5);assert.ok(ratio(muted!,panel!)>=4.5);assert.ok(css.includes(`--app-control-line:${control}`));assert.ok(ratio(control!,bg!)>=3);
    const rules=[...css.matchAll(/(\[data-app-mode=dark\])?\[data-app-accent=[a-z]+\] \{ --app-accent:(#[a-f0-9]{6})/g)].filter(m=>(mode==='dark')===!!m[1]);assert.equal(rules.length,8);
    for(const rule of rules){assert.ok(ratio(rule[2]!,onAccent!)>=4.5,`${mode} label ${rule[2]}`);assert.ok(ratio(rule[2]!,bg!)>=3,`${mode} focus ${rule[2]}`);}
  }
});

test('Tab and Shift+Tab wrap visible controls without leaving the component dialog',()=>dom(()=>{
 const {root,input}=fixture();presentationFor(root as unknown as HTMLElement);root.emit('click',root.querySelector('[data-app-action="create"]')!);
 const dialog=root.querySelector('dialog')!,close=dialog.querySelector('.app-dialog-close')!;
 input.focus();assert.equal(dialog.emit('keydown',input,{key:'Tab',shiftKey:false}).defaultPrevented,true);assert.equal(root.ownerDocument.activeElement,close);
 assert.equal(dialog.emit('keydown',close,{key:'Tab',shiftKey:true}).defaultPrevented,true);assert.equal(root.ownerDocument.activeElement,input);
}));
