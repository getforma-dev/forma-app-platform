import { build } from '@getforma/build';
import { readFileSync, writeFileSync, mkdirSync, rmSync } from 'node:fs';
import assert from 'node:assert/strict';
const source = readFileSync('src/home/app.tsx','utf8');
const cases = [
  {name:'enum-condition', source:source.replace('() => card.isSummary',"() => card.kind === 'summary'"), expected:['BUILD-610']},
  {name:'four-component-depth', source:`import { h, mount } from '@getforma/core';
function Fourth() { return <strong>four levels</strong>; }
function Third() { return <div><Fourth /></div>; }
function Second() { return <section><Third /></section>; }
function First() { return <article><Second /></article>; }
function Page() { return <main><First /></main>; }
mount(() => Page(), '#app');`, expected:['BUILD-403','BUILD-610']},
  {name:'reassigned-declared-local',source:source.replace('const [requested, setRequested] = createSignal(props.epoch);','let requested = props.epoch;').replace('requested() + 1; setRequested(current);','++requested;').replace('data.epoch !== requested()','data.epoch !== requested'),expected:['BUILD-610']},
  {name:'negative-literal-attribute',source:source.replace('tabIndex={card.tabIndex}','tabIndex={-1}'),expected:['BUILD-207']},
];
const results=[];
try {
  for(const test of cases) {
    const folder=`.cache/refusals/${test.name}`; mkdirSync(folder,{recursive:true}); const file=`${folder}/app.tsx`; writeFileSync(file,test.source);
    let diagnostics=[]; let refused=false;
    try { await build({app:'negative-control',entryPoints:[{entry:file,outfile:'home.js'}],routes:{'/':{js:['home'],css:[],canvas:true}},outputDir:`${folder}/dist`,ssr:true,ssrEntryPoints:{home:file},strict:true}); }
    catch(error) { refused=true; diagnostics=error.diagnostics ?? []; if(!diagnostics.length) throw error; }
    const codes=[...new Set(diagnostics.map(d=>d.code))];
    results.push({name:test.name,refused,codes,expected:test.expected,diagnostics,source:test.source});
    assert.equal(refused,true,`${test.name} unexpectedly admitted`); assert.ok(codes.some(code=>test.expected.includes(code)),`${test.name} wrong refusal`);
  }
} finally { writeFileSync('../../../evidence/APP-003/refusals.json',JSON.stringify({results,passed:results.length===cases.length&&results.every(r=>r.refused&&r.codes.some(c=>r.expected.includes(c)))},null,2)+'\n'); rmSync('.cache/refusals',{recursive:true,force:true}); }
