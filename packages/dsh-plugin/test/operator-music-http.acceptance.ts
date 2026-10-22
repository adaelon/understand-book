/** Actual curated audio/score, installed Engine, operator queue, HTTP publication and two private workspaces. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { createServer, request as httpRequest } from 'node:http';
import type { AddressInfo } from 'node:net';
import { createOperatorBuildHost } from '../src/operator-build-host.ts';
import { harness } from './harness.ts';

const fixture=await(await fetch('http://127.0.0.1:4191/fixture')).json();
const fetcher:typeof fetch=async(input,init)=>new Promise<Response>((resolve,reject)=>{
  const req=httpRequest(String(input),{method:init?.method,headers:Object.fromEntries(new Headers(init?.headers).entries()),signal:init?.signal??undefined},res=>{
    const chunks:Buffer[]=[];res.on('data',data=>chunks.push(Buffer.from(data)));res.on('error',reject);
    res.on('end',()=>resolve(new Response(Buffer.concat(chunks),{status:res.statusCode,headers:res.headers as HeadersInit})));
  });req.on('error',reject);req.end(init?.body===undefined?undefined:String(init.body));
});
const proxy=createServer((req,res)=>{
  const upstream=httpRequest(`http://127.0.0.1:4190${req.url}`,{method:req.method,headers:{...req.headers,host:'127.0.0.1:4190'}},response=>{
    res.writeHead(response.statusCode!,response.headers);response.pipe(res);
  });upstream.on('error',error=>{res.writeHead(502);res.end(error.message);});req.pipe(upstream);
});
await new Promise<void>(resolve=>proxy.listen(0,'127.0.0.1',resolve));
const connection={base_url:`http://127.0.0.1:${(proxy.address() as AddressInfo).port}`,origin:fixture.origin,cookie:fixture.admin_cookie,csrf_token:fixture.admin_csrf};
async function request(route:string,body?:unknown,cookie=connection.cookie,csrf=connection.csrf_token){
  return fetcher(`${connection.base_url}${route}`,{method:body===undefined?'GET':'POST',headers:{Cookie:cookie,Origin:connection.origin,'X-CSRF-Token':csrf,'Content-Type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
}
async function api(route:string,body?:unknown,cookie=connection.cookie,csrf=connection.csrf_token){
  const response=await request(`/api${route}`,body,cookie,csrf),value=await response.json();
  assert.ok(response.ok,`${response.status} ${route}: ${JSON.stringify(value)}`);return value;
}
const taskId=`music-http-${randomUUID()}`,root=path.join(fixture.root,taskId);
const source=path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_CURATION??'evals/music/schubert-curation.json');
const originalEngine=path.resolve(process.env.UNDERSTAND_BOOK_MUSIC_ENGINE??'target/music-operator-20261022/engine/understand-book-build-x86_64-pc-windows-msvc.exe');
assert.ok(existsSync(source)&&existsSync(originalEngine),'build the music Engine and provide the curated sample');
const installed=path.join(root,'installed-engine');mkdirSync(root,{recursive:true});cpSync(path.dirname(originalEngine),installed,{recursive:true});
const config={executable:path.join(installed,path.basename(originalEngine)),driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:24};
const h=await harness(),endpoint=`/admin/operator-builds/${taskId}`,progress:any[]=[];
let pauseOnce=true;
const operatorFetch:typeof fetch=async(url,init)=>{
  const response=await fetcher(url,init);
  if(String(url).endsWith('/progress')&&response.ok){
    const body=JSON.parse(String(init?.body)),p=body.observation?.progress;
    if(p){progress.push(p);console.log(JSON.stringify(p));}
    if(pauseOnce&&p?.stage==='music_score'&&p.completed===p.total&&p.total>0){
      pauseOnce=false;const task=await api(endpoint);await api(`${endpoint}/pause`,{operation_id:randomUUID(),expected_revision:task.revision});
      return Response.json(await api(endpoint));
    }
  }
  return response;
};
const options={connection,rootSessionId:h.parent.id};
let host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',options,operatorFetch);
try{
  const created=await api('/admin/operator-builds',{operation_id:randomUUID(),task_id:taskId,name:'精选舒伯特音乐运营链路验收',max_micro_cny:100,reserved_per_execution_micro_cny:100,
    visible_to_users:fixture.visible_to_users,materials:[{material_id:'schubert',label:'舒伯特音乐瞬间第3首',material_kind:'music',target_input:source,root_dir:root,build_tutor:false,max_parallel:1}]});
  await api(`${endpoint}/start`,{operation_id:randomUUID(),expected_revision:created.operator_build.revision});
  await host.processNext();let task=await api(endpoint);
  assert.equal(pauseOnce,false,'actual score completion triggered the pause');
  assert.equal(task.state,'interrupted',JSON.stringify(task));
  const workspace=task.materials[0].workspace_dir,invocation=task.materials[0].invocation_ref;
  const originalAudio=readFileSync(path.join(workspace,'music/audio/recording.mp3'));
  await host.dispose();host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',options,operatorFetch);
  await api(`${endpoint}/continue`,{operation_id:randomUUID(),expected_revision:task.revision});
  await host.processNext();task=await api(endpoint);
  assert.equal(task.state,'ready',JSON.stringify(task));assert.equal(task.materials[0].invocation_ref,invocation);
  assert.ok(originalAudio.equals(readFileSync(path.join(workspace,'music/audio/recording.mp3'))));
  assert.equal(existsSync(path.join(workspace,'transcript_map.json')),false,'pure music must not fabricate a speech transcript');
  assert.equal(task.materials[0].observation.processing.external_model_calls,0);
  assert.equal(h.model.requests.length,0,'DSH root did not analyze music');
  const reference=task.materials[0].published_book_ref;assert.ok(reference?.publication_id);
  const catalog=await api('/admin/books?limit=100&offset=0');
  assert.ok(catalog.books.some((book:any)=>book.published_book_ref.publication_id===reference.publication_id&&book.material_kind==='music'));
  const published=await api(`${endpoint}/publish`,{operation_id:randomUUID(),release:{release_id:`music-course-${randomUUID()}`,expected_version:0,visible_to_users:fixture.visible_to_users,
    manifest:{name:'音乐瞬间 · 入门伴听',publisher:'验收课程组',public_goal:'辨认旋律与伴奏并比较片段',applicability:'古典音乐入门',prerequisites:'无需读谱经验',
      materials:[{published_book_ref:reference,required_capabilities:['music','music_score','music_events']}],topic_links:[],suggested_route:[]}}});
  assert.equal(published.workspace_release.readiness_at_publication.status,'ready');
  const releaseId=published.workspace_release.release_id;
  const reader=await api('/auth/me',undefined,fixture.reader_cookie);
  const readerWorkspace=await api(`/workspace-releases/${releaseId}/versions/1/use`,{operation_id:randomUUID(),goal:{text:'先听出主题'}},fixture.reader_cookie,reader.csrf_token);
  const adminWorkspace=await api(`/workspace-releases/${releaseId}/versions/1/use`,{operation_id:randomUUID(),goal:{text:'练习区分伴奏'} });
  assert.notEqual(readerWorkspace.workspace.learning_workspace_id,adminWorkspace.workspace.learning_workspace_id);
  assert.deepEqual(readerWorkspace.workspace.members.map((m:any)=>m.published_book_ref),adminWorkspace.workspace.members.map((m:any)=>m.published_book_ref));
  await api(`/learning-workspaces/${readerWorkspace.workspace.learning_workspace_id}/goal`,{goal:{text:'接着辨认主题再次出现'}},fixture.reader_cookie,reader.csrf_token);
  const readerAfter=await api(`/learning-workspaces/${readerWorkspace.workspace.learning_workspace_id}`,undefined,fixture.reader_cookie);
  const adminAfter=await api(`/learning-workspaces/${adminWorkspace.workspace.learning_workspace_id}`);
  assert.equal(readerAfter.goal.text,'接着辨认主题再次出现');assert.equal(adminAfter.goal.text,'练习区分伴奏');
  assert.deepEqual(readerAfter.members.map((m:any)=>m.published_book_ref),readerWorkspace.workspace.members.map((m:any)=>m.published_book_ref));
  const prefix=`/api/books/${encodeURIComponent(reference.book_id)}/publications/${encodeURIComponent(reference.publication_id)}`;
  const contextResponse=await request(`${prefix}/music_context`,undefined,fixture.reader_cookie);
  assert.ok(contextResponse.ok,await contextResponse.clone().text());
  const context=await contextResponse.json();
  assert.ok(context.manifest.pages.length>0&&context.manifest.events.length>0);
  for(const cookie of [fixture.reader_cookie,fixture.admin_cookie]){
    const audio=await fetcher(`${connection.base_url}${context.audio_url}`,{headers:{Cookie:cookie,Range:'bytes=0-31'}});
    assert.equal(audio.status,206);assert.equal((await audio.arrayBuffer()).byteLength,32);
  }
  const score=await request(context.score_pages[0].url,undefined,fixture.reader_cookie);assert.ok((await score.text()).includes('<svg'));
  const budget=await api(`/admin/build-tasks/${taskId}`),charges=await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`);
  assert.equal(charges.total,0);assert.equal(budget.provider_cost_micro_cny,0);assert.equal(budget.debited_micro_cny,0);
  assert.equal(await host.processNext(),false);
  const result={status:'passed',task_id:taskId,source,root,workspace,installed_engine:config.executable,reference,release_id:releaseId,
    private_workspace_ids:[readerWorkspace.workspace.learning_workspace_id,adminWorkspace.workspace.learning_workspace_id],progress,
    source_kind:context.manifest.recording.kind,coverage:context.manifest.coverage,cost_budget:budget,external_model_calls:0,
    checks:{same_queue:true,pause_resume:true,immutable_audio:true,no_transcript:true,no_model_charges:true,shared_audio_range:true,two_private_workspaces:true,private_goal_update_isolated:true,external_verovio:true}};
  mkdirSync('docs/performance/music-operator-20261022',{recursive:true});
  writeFileSync('docs/performance/music-operator-20261022/http-acceptance-result.json',JSON.stringify(result,null,2));
  console.log(JSON.stringify({status:result.status,task_id:taskId,workspace,reference}));
}finally{await host.dispose();await h.dispose();await new Promise<void>(resolve=>proxy.close(()=>resolve()));}
