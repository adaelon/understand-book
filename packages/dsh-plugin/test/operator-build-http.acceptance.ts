import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync,readdirSync,readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
import {request as httpRequest} from 'node:http';
import {createOperatorBuildHost} from '../src/operator-build-host.ts';
import {operatorHarness} from './operator-build-fixture.ts';

const fixture=await (await fetch('http://127.0.0.1:4191/fixture')).json();
const startedAt=Date.now();
// Port 4190 is reserved by WHATWG fetch; use the same raw local HTTP transport as Q3 acceptance.
const nativeFetch=globalThis.fetch;
globalThis.fetch=async(input,init)=>new Promise<Response>((resolve,reject)=>{
  const request=httpRequest(String(input),{method:init?.method,headers:Object.fromEntries(new Headers(init?.headers).entries()),signal:init?.signal??undefined},response=>{
    const chunks:Buffer[]=[];response.on('data',chunk=>chunks.push(Buffer.from(chunk)));response.on('error',reject);
    response.on('end',()=>resolve(new Response(Buffer.concat(chunks).toString('utf8'),{status:response.statusCode,headers:{'Content-Type':'application/json'}})));
  });request.on('error',reject);request.end(init?.body===undefined?undefined:String(init.body));
});
const connection={base_url:'http://127.0.0.1:4190',origin:'https://127.0.0.1:4190',cookie:fixture.admin_cookie,csrf_token:fixture.admin_csrf};
async function api(route:string,body?:unknown){
  const r=await fetch(`${connection.base_url}/api${route}`,{method:body===undefined?'GET':'POST',headers:{Cookie:connection.cookie,Origin:connection.origin,'X-CSRF-Token':connection.csrf_token,'Content-Type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});
  const value=await r.json();assert.ok(r.ok,`${r.status} ${route} ${JSON.stringify(value)}`);return value;
}
const resumeTask=process.env.UNDERSTAND_BOOK_OPERATOR_ACCEPTANCE_TASK;
const taskId=resumeTask??`b1-http-${randomUUID()}`;
const endpoint=`/admin/operator-builds/${taskId}`;
let task=resumeTask?await api(endpoint):undefined;
const root=task?.materials[1].root_dir??path.join(fixture.root,taskId);mkdirSync(root,{recursive:true});const source=path.join(root,'book.md');
if(!resumeTask)writeFileSync(source,'# Transactions\n\nTransactions group operations into one atomic change.\n');
const audience=fixture.visible_to_users??[fixture.reader_user_id,fixture.admin_user_id];assert.ok(audience.every((v:unknown)=>typeof v==='string'));
const h=await operatorHarness({assertAdmitted:()=>{if(resumeTask)assert.fail('completed Engine units must not call the model during import recovery');},onFirstCommit:async()=>{const task=await api(endpoint);await api(`${endpoint}/pause`,{operation_id:randomUUID(),expected_revision:task.revision});}});
const config={executable:process.env.UNDERSTAND_BOOK_TEST_NODE??process.execPath,prefixArgs:['--import','tsx',fileURLToPath(new URL('engine-source-cli.ts',import.meta.url))],driverRoot:path.join(root,'registry'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:24};
const host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',{connection,rootSessionId:h.parent.id});
try{
  let invocation:string;
  let executionsBeforeResume=0;
  if(!resumeTask){
  const created=await api('/admin/operator-builds',{operation_id:randomUUID(),task_id:taskId,name:'B1真实HTTP双资料构建',max_micro_cny:1000000,reserved_per_execution_micro_cny:1000,visible_to_users:audience,materials:[
    {material_id:'existing',label:'复用已有资料',published_book_ref:fixture.first},
    {material_id:'missing',label:'准备缺失资料',target_input:source,root_dir:root,pass2:'disabled',max_parallel:3},
  ]});
  await api(`${endpoint}/prepare`,{operation_id:randomUUID(),expected_revision:created.operator_build.revision});
  await host.processNext();task=await api(endpoint);assert.equal(task.state,'awaiting_confirmation',JSON.stringify(task));assert.equal(h.model.requests.length,0);
  await api(`${endpoint}/start`,{operation_id:randomUUID(),expected_revision:task.revision});
  await host.processNext();task=await api(endpoint);assert.equal(task.state,'interrupted',JSON.stringify(task));invocation=task.materials[1].invocation_ref;assert.ok(invocation);
  const pendingBudget=await api(`/admin/build-tasks/${taskId}`);assert.ok(pendingBudget.unknown_count>0);assert.equal(pendingBudget.debited_micro_cny,0);
  await api(`${endpoint}/continue`,{operation_id:randomUUID(),expected_revision:task.revision,max_micro_cny:2000000});
  }else{
    invocation=task.materials[1].invocation_ref;assert.equal(task.materials[1].observation.step.action.kind,'DONE');
    executionsBeforeResume=(await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`)).total;
    if(task.state!=='ready')await api(`${endpoint}/continue`,{operation_id:randomUUID(),expected_revision:task.revision});
  }
  if(task.state!=='ready')await host.processNext();task=await api(endpoint);assert.equal(task.state,'ready',JSON.stringify(task));assert.equal(task.materials[1].invocation_ref,invocation!);assert.equal(h.pass1Generations(),resumeTask?0:1);
  assert.deepEqual(task.materials[0].published_book_ref,fixture.first);const generated=task.materials[1].published_book_ref;assert.ok(generated?.publication_id);
  const releaseId=task.release?.release_id??`course-${randomUUID()}`;const refs=[fixture.first,generated];
  const previous=task.release?await api(`/admin/workspace-releases/${releaseId}/versions/${task.release.version}`):undefined;
  if(previous)assert.equal(previous.availability.status,'ready',JSON.stringify(previous.availability));
  const publication=await api(`${endpoint}/publish`,{operation_id:randomUUID(),release:{release_id:releaseId,expected_version:task.release?.version??0,visible_to_users:audience,manifest:{name:'B1双资料课程',publisher:'验收课程组',public_goal:'理解事务基础并完成课程复习',applicability:'数据库课程学习',prerequisites:'基础编程',materials:refs.map(published_book_ref=>({published_book_ref,required_capabilities:['text','tutor']})),topic_links:[{title:'共同概念',description:'结合两份资料理解事务',materials:refs}],suggested_route:[]}}});
  assert.equal(publication.workspace_release.readiness_at_publication.status,'ready',JSON.stringify(publication.workspace_release));
  if(previous)assert.deepEqual((await api(`/admin/workspace-releases/${releaseId}/versions/${previous.version}`)).readiness_at_publication,previous.readiness_at_publication);
  const budget=await api(`/admin/build-tasks/${taskId}`);assert.equal(budget.debited_micro_cny,0);assert.ok(budget.unknown_count>0);assert.equal(budget.max_micro_cny,2000000);
  const charges=await api(`/admin/charges?cost_owner=platform&parent_task_ref=${taskId}&limit=100&offset=0`);assert.equal(charges.total,budget.unknown_count);
  if(resumeTask){assert.equal(charges.total,executionsBeforeResume);assert.equal(h.model.requests.length,0);}
  const records=readdirSync(path.join(root,'registry','external-build-cost',taskId)).map(file=>JSON.parse(readFileSync(path.join(root,'registry','external-build-cost',taskId,file),'utf8')));
  const pass1Candidates=readdirSync(path.join(task.materials[1].workspace_dir,'.build','automatic-build','v2','tasks','pass1'),{recursive:true}).filter(file=>String(file).endsWith(`${path.sep}candidate.json`)).length;
  assert.equal(pass1Candidates,1);
  const result={task_id:taskId,release_id:releaseId,version:publication.workspace_release.version,root,invocation:invocation!,materials:refs,pass1_candidate_count:pass1Candidates,elapsed_ms:Date.now()-startedAt,
    ...(resumeTask?{recovery_model_requests:h.model.requests.length}:{}),model_requests:records.reduce((sum,record)=>sum+(record.receipt?.evidence.model_calls_observed??0),0),actual_executions:charges.total,budget,status:'passed'};
  writeFileSync(path.resolve('target/b1-http-acceptance-result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{
  writeFileSync(path.resolve('target/b1-http-acceptance-progress.json'),JSON.stringify({task_id:taskId,root,pass1_generations:h.pass1Generations(),model_requests:h.model.requests.length,resume:!!resumeTask},null,2));
  await host.dispose();await h.dispose();globalThis.fetch=nativeFetch;
}
