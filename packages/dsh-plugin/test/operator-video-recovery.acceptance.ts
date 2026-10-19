/** Replay a completed video using the final compiled Engine; no accepted semantic call may repeat. */
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {createServer,request as httpRequest} from 'node:http';
import type {AddressInfo} from 'node:net';
import {videoOperatorCommand} from '../src/engine-client.ts';

const file=path.resolve('target/video-operator-20261018/http-acceptance-result.json'),result=JSON.parse(readFileSync(file,'utf8'));
const fixture=await(await fetch('http://127.0.0.1:4191/fixture')).json();
const proxy=createServer((req,res)=>{
  const upstream=httpRequest(`http://127.0.0.1:4190${req.url}`,{method:req.method,headers:{...req.headers,host:'127.0.0.1:4190'}},reply=>{res.writeHead(reply.statusCode!,reply.headers);reply.pipe(res);});
  upstream.on('error',error=>{res.writeHead(502);res.end(error.message);});req.pipe(upstream);
});
await new Promise<void>(resolve=>proxy.listen(0,'127.0.0.1',resolve));
const connection={base_url:`http://127.0.0.1:${(proxy.address() as AddressInfo).port}`,origin:fixture.origin,cookie:fixture.admin_cookie,csrf_token:fixture.admin_csrf};
const get=async(route:string)=>{const r=await fetch(connection.base_url+'/api'+route,{headers:{Cookie:connection.cookie}});assert.ok(r.ok);return r.json();};
const task=await get('/admin/operator-builds/'+result.task_id),before=await get('/admin/build-tasks/'+result.task_id);
const immutable=['source.txt','transcript_map.json','video_translation_progress.json','video_enrichments.json'].map(name=>[name,readFileSync(path.join(result.workspace,name),'utf8')]);
const progress:unknown[]=[];
try{
  const replay=await videoOperatorCommand({executable:path.resolve('target/video-operator-20261018/bin/understand-book-build-x86_64-pc-windows-msvc.exe'),driverRoot:path.join(result.root,'registry')},{
    input_path:result.source,workspace_dir:result.workspace,codex_executable:path.resolve('target/video-operator-20261018/bin/controlled-video-codex.exe'),
    publisher_executable:path.resolve('target/debug/publish_book.exe'),publication_service_root:path.join(result.root,'publication-staging'),
    external_cost:{connection,registry_root:path.join(result.root,'registry'),binding:{version:'external_build_cost.v1',cost_owner:'platform',task_ref:result.task_id,
      max_micro_cny:task.max_micro_cny,reserved_per_execution_micro_cny:task.reserved_per_execution_micro_cny,confirmation:task.confirmation}},
  },async(value)=>{progress.push(value);},new AbortController().signal);
  assert.equal(replay.progress.status,'completed');
  for(const[name,bytes]of immutable)assert.equal(readFileSync(path.join(result.workspace,name),'utf8'),bytes,name);
  assert.deepEqual(await get('/admin/build-tasks/'+result.task_id),before);
  result.checks.final_compiled_replay_no_extra_calls=true;result.final_compiled_replay={progress_updates:progress.length,additional_executions:0};
  writeFileSync(file,JSON.stringify(result,null,2));console.log(JSON.stringify(result.final_compiled_replay));
}finally{await new Promise<void>(resolve=>proxy.close(()=>resolve()));}
