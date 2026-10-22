import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdtempSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { musicOperatorCommand } from '../src/engine-client.ts';

const engine=path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_ENGINE??'target/music-operator-20261022/engine/understand-book-build-x86_64-pc-windows-msvc.exe');
const source=path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_CURATION??'evals/music/schubert-curation.json');
const root=mkdtempSync(path.join(tmpdir(),'ub-music-installed-')),installed=path.join(root,'engine');
cpSync(path.dirname(engine),installed,{recursive:true});
const requests:string[]=[],server=createServer((req,res)=>{
  requests.push(req.url!);res.setHeader('Content-Type','application/json');
  if(req.url==='/api/admin/build-tasks/music-engine')res.end(JSON.stringify({cost_owner:'platform',max_micro_cny:100}));
  else{res.writeHead(500);res.end(JSON.stringify({error_code:'unexpected_model_charge'}));}
});
await new Promise<void>(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${(server.address() as AddressInfo).port}`;
const config={executable:path.join(installed,path.basename(engine)),driverRoot:path.join(root,'registry')};
const input={curation_path:source,workspace_dir:path.join(root,'workspace'),required_capabilities:['music','music_score','music_events'],
  external_cost:{binding:{version:'external_build_cost.v1',cost_owner:'platform',task_ref:'music-engine',max_micro_cny:100,reserved_per_execution_micro_cny:100,confirmation:'curated-fixture'},
    connection:{base_url:base,cookie:'fixture',origin:base,csrf_token:'fixture'},registry_root:config.driverRoot}};
const progress:any[]=[],signal=new AbortController().signal;
const library=path.join(installed,'resources/music/verovio/verovio-module.mjs'),original=readFileSync(library);
try{
  writeFileSync(library,'throw new Error("replaceable_verovio_probe");\n');
  await assert.rejects(musicOperatorCommand(config,input,async p=>{progress.push(p);},signal),/replaceable_verovio_probe/);
  writeFileSync(library,original);
  await assert.rejects(musicOperatorCommand(config,{...input,required_capabilities:[...input.required_capabilities,'music_note_alignment']},async p=>{progress.push(p);},signal),/music_note_alignment/);
  const partialManifest=readFileSync(path.join(input.workspace_dir,'music_source_manifest.json'),'utf8');
  const result=await musicOperatorCommand(config,input,async p=>{progress.push(p);},signal);
  assert.equal(result.processing.external_model_calls,0);assert.equal(result.processing.provider_cost_micro_cny,0);
  assert.deepEqual(result.available_capabilities,['music','music_score','music_events']);
  const manifest=JSON.parse(readFileSync(path.join(input.workspace_dir,'music_source_manifest.json'),'utf8'));
  assert.equal(JSON.stringify(manifest),JSON.stringify(JSON.parse(partialManifest)),'capability retry preserves already accepted music facts');
  assert.equal(manifest.recording.kind,'performance');assert.ok(manifest.pages.length>0&&manifest.events.length>0);
  assert.equal(existsSync(path.join(input.workspace_dir,'transcript_map.json')),false);
  assert.ok(readFileSync(path.join(input.workspace_dir,'music/NOTICE.txt'),'utf8').includes('SIL OPEN FONT LICENSE'));
  assert.ok(progress.some(p=>p.status==='failed')&&progress.some(p=>p.status==='completed'));
  assert.ok(requests.every(url=>url==='/api/admin/build-tasks/music-engine'));
  const evidence={status:'passed',source,installed_engine:config.executable,workspace:input.workspace_dir,
    renderer:JSON.parse(readFileSync(path.join(input.workspace_dir,'music/build-report.json'),'utf8')).renderer,
    library_replacement_observed:true,resume_after_renderer_failure:true,missing_capability_stays_incomplete:true,accepted_manifest_preserved:true,pages:manifest.pages.length,events:manifest.events.length,
    capabilities:result.available_capabilities,external_model_calls:0,cost_requests:requests,progress};
  mkdirSync('docs/performance/music-operator-20261022',{recursive:true});
  writeFileSync('docs/performance/music-operator-20261022/installed-engine-verification.json',JSON.stringify(evidence,null,2));
  console.log(JSON.stringify({status:'passed',root,capabilities:result.available_capabilities}));
}finally{writeFileSync(library,original);await new Promise<void>(resolve=>server.close(()=>resolve()));}
