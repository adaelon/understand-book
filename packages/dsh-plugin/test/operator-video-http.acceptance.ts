/** Controlled semantic executor, real video pipeline, installed Engine and Rust HTTP/cost ledger. */
import assert from 'node:assert/strict';
import {copyFileSync,existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
import {createServer,request as httpRequest} from 'node:http';
import type {AddressInfo} from 'node:net';
import {createOperatorBuildHost} from '../src/operator-build-host.ts';
import {harness} from './harness.ts';

const fixture=await(await fetch('http://127.0.0.1:4191/fixture')).json();
// Fetch forbids this fixture's port, so use the same local HTTP adapter as B1 acceptance.
const fetcher:typeof fetch=async(input,init)=>new Promise<Response>((resolve,reject)=>{
  const req=httpRequest(String(input),{method:init?.method,headers:Object.fromEntries(new Headers(init?.headers).entries()),signal:init?.signal??undefined},res=>{
    const chunks:Buffer[]=[];res.on('data',data=>chunks.push(Buffer.from(data)));res.on('error',reject);
    res.on('end',()=>resolve(new Response(Buffer.concat(chunks).toString(),{status:res.statusCode,headers:{'Content-Type':'application/json'}})));
  });req.on('error',reject);req.end(init?.body===undefined?undefined:String(init.body));
});
// The compiled/source Engine has its own fetch implementation. Forward its cost calls on a permitted port.
const proxy=createServer((req,res)=>{
  const upstream=httpRequest(`http://127.0.0.1:4190${req.url}`,{method:req.method,headers:{...req.headers,host:'127.0.0.1:4190'}},response=>{
    res.writeHead(response.statusCode!,response.headers);response.pipe(res);
  });upstream.on('error',error=>{res.writeHead(502);res.end(error.message);});req.pipe(upstream);
});
await new Promise<void>(resolve=>proxy.listen(0,'127.0.0.1',resolve));
const connection={base_url:`http://127.0.0.1:${(proxy.address() as AddressInfo).port}`,origin:fixture.origin,cookie:fixture.admin_cookie,csrf_token:fixture.admin_csrf};
async function api(route:string,body?:unknown){
  const r=await fetcher(`${connection.base_url}/api${route}`,{method:body===undefined?'GET':'POST',headers:{Cookie:connection.cookie,Origin:connection.origin,'X-CSRF-Token':connection.csrf_token,'Content-Type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
  const data=await r.json();assert.ok(r.ok,`${r.status} ${route} ${JSON.stringify(data)}`);return data;
}
const taskId=`video-http-${randomUUID()}`,root=path.join(fixture.root,taskId),source=path.join(root,'tensor.mp4');
mkdirSync(root,{recursive:true});
const input=process.env.UNDERSTAND_BOOK_VIDEO_ACCEPTANCE_SOURCE ?? 'D:/codex-build/understand-book-vi18/installation-smoke/workspace/media/original.mp4';
assert.ok(existsSync(input),'provide the short MP4 in UNDERSTAND_BOOK_VIDEO_ACCEPTANCE_SOURCE');copyFileSync(input,source);
const executable=process.env.UNDERSTAND_BOOK_OPERATOR_VIDEO_ENGINE ?? process.execPath;
const config={executable,...(process.env.UNDERSTAND_BOOK_OPERATOR_VIDEO_ENGINE?{}:{prefixArgs:['--import','tsx',fileURLToPath(new URL('engine-source-cli.ts',import.meta.url))]}),
  driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:24};
const codex=process.env.UNDERSTAND_BOOK_VIDEO_TEST_CODEX!;assert.ok(codex&&existsSync(codex),'controlled Codex executable is required');
const video={codexExecutable:codex,publisherExecutable:path.resolve('target/debug/publish_book.exe'),publicationServiceRoot:path.join(root,'publication-staging'),
  pythonExecutable:process.env.VIDEO_WHISPER_PYTHON ?? 'E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-venv/Scripts/python.exe',
  whisperModelDir:process.env.VIDEO_WHISPER_MODEL_DIR ?? 'E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-models/small.en'};
const h=await harness();
const endpoint=`/admin/operator-builds/${taskId}`,progress:any[]=[];let pauseOnce=true;
const operatorFetch:typeof fetch=async(url,init)=>{
  const res=await fetcher(url,init);
  if(String(url).endsWith('/progress')&&res.ok){
    const body=JSON.parse(String(init?.body)),p=body.observation?.progress;
    if(p){progress.push(p);console.log(JSON.stringify({stage:p.stage,completed:p.completed,total:p.total,status:p.status}));}
    if(pauseOnce&&p?.stage==='translation'&&p.total>0&&p.completed===p.total){
      pauseOnce=false;const task=await api(endpoint);await api(`${endpoint}/pause`,{operation_id:randomUUID(),expected_revision:task.revision});
      return Response.json(await api(endpoint));
    }
  }
  return res;
};
const options={connection,rootSessionId:h.parent.id,video};
let host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',options,operatorFetch);
try{
  const created=await api('/admin/operator-builds',{operation_id:randomUUID(),task_id:taskId,name:'视频一键预构建真实链路验收',max_micro_cny:100,reserved_per_execution_micro_cny:100,
    visible_to_users:fixture.visible_to_users,materials:[{material_id:'video',label:'Tensor短讲',material_kind:'video',target_input:source,root_dir:root,build_tutor:false,pass2:'disabled',max_parallel:1}]});
  await api(`${endpoint}/start`,{operation_id:randomUUID(),expected_revision:created.operator_build.revision});
  await host.processNext();let task=await api(endpoint);assert.equal(task.state,'interrupted',JSON.stringify(task));assert.equal(pauseOnce,false,'pause occurred after an accepted translation');
  const workspace=task.materials[0].workspace_dir,invocation=task.materials[0].invocation_ref;
  const chargesBefore=await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`);assert.equal(chargesBefore.total,1);
  const transcript=readFileSync(path.join(workspace,'transcript_map.json'),'utf8'),translation=readFileSync(path.join(workspace,'video_translation_progress.json'),'utf8');
  await host.dispose();host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',options,operatorFetch);
  await api(`${endpoint}/continue`,{operation_id:randomUUID(),expected_revision:task.revision});
  await host.processNext();task=await api(endpoint);assert.equal(task.state,'interrupted',JSON.stringify(task));assert.match(task.error,/external_build_cost_http_409/u);
  assert.equal((await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`)).total,1,'exhausted budget does not start another Codex executor');
  await host.dispose();host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',options,operatorFetch);
  await api(`${endpoint}/continue`,{operation_id:randomUUID(),expected_revision:task.revision,max_micro_cny:10000});
  await host.processNext();task=await api(endpoint);assert.equal(task.state,'ready',JSON.stringify(task));assert.equal(task.materials[0].invocation_ref,invocation);
  assert.equal(readFileSync(path.join(workspace,'transcript_map.json'),'utf8'),transcript,'accepted transcript reused');
  assert.equal(readFileSync(path.join(workspace,'video_translation_progress.json'),'utf8'),translation,'accepted translation reused');
  assert.equal(existsSync(path.join(workspace,'teaching_map.json')),false);
  const caps=['text','video','translation','visual_index','graph','discourse'];assert.deepEqual(task.materials[0].required_capabilities,caps);
  const reference=task.materials[0].published_book_ref;assert.ok(reference?.publication_id);
  const published=await api(`${endpoint}/publish`,{operation_id:randomUUID(),release:{release_id:`video-course-${randomUUID()}`,expected_version:0,visible_to_users:fixture.visible_to_users,
    manifest:{name:'视频预构建验收',publisher:'验收课程组',public_goal:'理解张量的基本概念',applicability:'深度学习入门',prerequisites:'基础编程',
      materials:[{published_book_ref:reference,required_capabilities:caps}],topic_links:[],suggested_route:[]}}});
  assert.equal(published.workspace_release.readiness_at_publication.status,'ready');
  const budget=await api(`/admin/build-tasks/${taskId}`),charges=await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`);
  assert.equal(charges.total,4,'exactly translation, visual, graph, discourse executions');assert.equal(budget.unknown_count,4);assert.equal(budget.debited_micro_cny,0);
  const records=readdirSync(path.join(root,'registry/external-build-cost',taskId)).map(f=>JSON.parse(readFileSync(path.join(root,'registry/external-build-cost',taskId,f),'utf8')));
  assert.equal(records.filter(r=>r.receipt?.status==='partial').length,4);assert.equal(h.model.requests.length,0,'DSH root does not process media');
  assert.equal(await host.processNext(),false);
  const result={status:'passed',task_id:taskId,source,root,workspace,invocation,reference,release_id:published.workspace_release.release_id,
    actual_executions:charges.total,cost_budget:budget,progress,checks:{one_click:true,pause_after_accepted_translation:true,restart_reuses_accepted:true,
      exhausted_budget_blocks_before_spawn:true,default_no_tutor:true,platform_receipts:true,shared_release_ready:true}};
  mkdirSync('target/video-operator-20261018',{recursive:true});writeFileSync('target/video-operator-20261018/http-acceptance-result.json',JSON.stringify(result,null,2));
  console.log(JSON.stringify({status:result.status,task_id:taskId,root,actual_executions:charges.total}));
}finally{await host.dispose();await h.dispose();await new Promise<void>(resolve=>proxy.close(()=>resolve()));}
