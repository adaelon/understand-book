// Run the generated page's own controls/code builder; rustc decides its claims.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import os from 'node:os';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
const root = path.dirname(fileURLToPath(import.meta.url));
const variant = process.argv[2] || 'sol/author/output.json';
const output = JSON.parse(fs.readFileSync(path.join(root, variant), 'utf8'));
const nodes = new Map();
const node = id => {
  if (!nodes.has(id)) nodes.set(id, {value:'', textContent:'', handlers:{}, addEventListener(kind, fn){this.handlers[kind]=fn;}});
  return nodes.get(id);
};
let reader, restorer;
const page = vm.createContext({document:{getElementById:node},window:{presentation:{
  initialState:output.write.initial_state,commitState(){},registerStateReader(fn){reader=fn;},registerStateRestorer(fn){restorer=fn;},
}}});
const script = output.write.html.match(/<script>([\s\S]*?)<\/script>/)[1];
vm.runInContext(script, page);
const change = (id, value) => {node(id).value=String(value);node(id).handlers.change({target:node(id)});};
const observe = () => node('leftObserve').handlers.click();
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'tb4-content-'));
const results=[];
for (const copy of [true,false]) for (const mutable of [false,true]) {
  change('leftCopy', copy);change('leftMutable',mutable);
  assert.equal(node('leftResult').textContent,'结果未查看。');
  observe();
  const source=node('leftCode').textContent;
  const filename=path.join(temp,`case_${copy}_${mutable}.rs`);
  const exe=filename.replace(/\.rs$/,'.exe');fs.writeFileSync(filename,source);
  const compiled=spawnSync('rustup',['run','1.95.0','rustc','--edition=2021','--error-format=json',filename,'-o',exe],{encoding:'utf8'});
  const diagnostics=(compiled.stderr||'').trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));
  const codes=diagnostics.map(d=>d.code?.code).filter(Boolean);
  const run=compiled.status===0?spawnSync(exe,[],{encoding:'utf8'}):null;
  const shown=node('leftResult').textContent;
  results.push({copy,mutable,source,compile_exit:compiled.status,diagnostic_codes:codes,stdout:run?.stdout??null,page_result:shown});
  assert.equal(compiled.status,mutable?0:1);
  if (mutable) {assert.equal(run.status,0);assert.equal(run.stdout.replaceAll('\r',''),'b = Foo(8)\na = Foo(8)\n');assert(shown.includes('b = Foo(8)\na = Foo(8)'));}
  else {assert(codes.includes('E0594'));assert(shown.includes('E0594'));}
}
// This is the published state-restorer contract, not a reimplementation of it.
const saved={values:{page:{leftCopy:false,leftMutable:true,leftRevealed:true,rightCopy:false,rightMutable:true,rightRevealed:true}},visible_step:'observed-right'};
restorer(saved);
const actual=JSON.parse(JSON.stringify(reader()));
const restored=Object.keys(saved.values.page).every(key=>actual.values[key]===saved.values.page[key]);
const report={artifact:variant,rust:'1.95.0',cases:results,state_restore:{passed:restored,expected:saved,actual}};
const file=variant==='sol/author/output.json'?'content-original.json':'content-final.json';
fs.writeFileSync(path.join(root,file),JSON.stringify(report,null,2)+'\n');
assert(restored,'Generated page must restore values.page, including conditions and revealed support');
console.log(`${results.length} generated Rust cases and page-state restoration passed`);
