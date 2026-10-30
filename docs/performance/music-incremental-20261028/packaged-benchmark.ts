import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { musicOperatorCommand } from '../../../packages/dsh-plugin/src/engine-client.ts';

const root=path.resolve('tmp/music-incremental-20261028/packaged');
if(existsSync(root))throw new Error('Use a fresh packaged benchmark directory');
mkdirSync(root,{recursive:true});
const engines={baseline:path.resolve('tmp/continuous-executor-20261028/release/understand-book-build-x86_64-pc-windows-msvc.exe'),
 candidate:'E:/allwork/Understand Book/operator-builds/20261028-prebuild-reuse/understand-book-build-x86_64-pc-windows-msvc.exe'};
const requests:string[]=[],server=createServer((req,res)=>{
 requests.push(req.url!);res.setHeader('Content-Type','application/json');
 if(req.url==='/api/admin/build-tasks/local-music-benchmark')res.end(JSON.stringify({cost_owner:'platform',max_micro_cny:100}));
 else{res.writeHead(500);res.end(JSON.stringify({error_code:'unexpected_model_charge'}));}
});
await new Promise<void>(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${(server.address() as {port:number}).port}`,samples:any[]=[];
try{
 for(let repeat=0;repeat<2;repeat++)for(const work of ['schubert','beethoven'])for(const variant of ['baseline','candidate'] as const){
  const seed=path.resolve('tmp/music-incremental-20261028/seed',work),workspace=path.join(root,`${work}-${repeat}-${variant}`);
  cpSync(seed,workspace,{recursive:true});
  const config={executable:engines[variant],driverRoot:path.join(root,'registry')};
  const input={curation_path:path.resolve('evals/music',`${work}-curation.json`),workspace_dir:workspace,
   required_capabilities:['music','music_score','music_events'],external_cost:{binding:{version:'external_build_cost.v1',cost_owner:'platform',task_ref:'local-music-benchmark',max_micro_cny:100,reserved_per_execution_micro_cny:100,confirmation:'local-fixture'},
    connection:{base_url:base,cookie:'fixture',csrf_token:'fixture',origin:base},registry_root:config.driverRoot}};
  let accepted:string|undefined;
  for(const phase of ['uncached_rebuild','repeated_rebuild']){
   const begin=performance.now();
   const result:any=await musicOperatorCommand(config,input,async()=>{},new AbortController().signal);
   const elapsed_ms=performance.now()-begin;
   assert.equal(result.processing.external_model_calls,0);
   assert.equal(result.processing.provider_cost_micro_cny,0);
   const manifest=readFileSync(path.join(workspace,'music_source_manifest.json'),'utf8');
   if(accepted)assert.equal(manifest,accepted);else accepted=manifest;
   assert.equal(manifest,readFileSync(path.resolve('tmp/music-incremental-20261028/baseline',work,'music_source_manifest.json'),'utf8'));
   assert.deepEqual(readFileSync(path.join(workspace,'.build/music/calibrations.json')),readFileSync(path.join(seed,'.build/music/calibrations.json')));
   const report=JSON.parse(readFileSync(path.join(workspace,'music/build-report.json'),'utf8'));
   samples.push({repeat,work,variant,phase,elapsed_ms,reused_stages:report.reused_stages??null});
   console.log(JSON.stringify(samples.at(-1)));
  }
  writeFileSync(path.join(root,'results.json'),JSON.stringify({engines,real_model:false,source_model_calls:0,outputs_equal:true,requests,samples},null,2)+'\n');
 }
 assert.ok(requests.every(url=>url==='/api/admin/build-tasks/local-music-benchmark'));
}finally{await new Promise<void>(resolve=>server.close(()=>resolve()));}
