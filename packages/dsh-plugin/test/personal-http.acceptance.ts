/** Real Rust HTTP + compiled Engine + native DSH protocol, with a controlled metered provider. */
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import path from 'node:path';
import {randomUUID} from 'node:crypto';
import {createServer,request as httpRequest} from 'node:http';
import type {AddressInfo} from 'node:net';
import {createOperatorBuildHost} from '../src/operator-build-host.ts';
import {operatorHarness} from './operator-build-fixture.ts';

const fixture=await(await fetch('http://127.0.0.1:4191/fixture')).json();
// Synthetic prices exercise charging only; they are not a production cost sample.
writeFileSync(path.join(fixture.root,'service-rates.json'),JSON.stringify({rates:[{
  version:'personal-http-fixture',service_kind:'video_transcription',starts_at:0,expires_at:4102444800,
  debit_micro_cny_per_second:10,estimated_cost_micro_cny_per_second:2,
}]}));
const proxy=createServer((req,res)=>{const upstream=httpRequest(`http://127.0.0.1:4190${req.url}`,{method:req.method,headers:{...req.headers,host:'127.0.0.1:4190'}},r=>{res.writeHead(r.statusCode!,r.headers);r.pipe(res);});upstream.on('error',e=>{res.writeHead(502);res.end(e.message);});req.pipe(upstream);});
await new Promise<void>(resolve=>proxy.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${(proxy.address() as AddressInfo).port}`;
const connection={base_url:base,origin:fixture.origin,cookie:fixture.admin_cookie,csrf_token:fixture.admin_csrf};
const events:{path:string;body:any}[]=[];
const observedFetch:typeof fetch=async(url,init)=>{events.push({path:new URL(String(url)).pathname,body:init?.body?JSON.parse(String(init.body)):null});return fetch(url,init);};
async function api(route:string,body?:unknown,admin=false){const r=await fetch(`${base}/api${route}`,{method:body===undefined?'GET':'POST',headers:{Cookie:admin?fixture.admin_cookie:fixture.reader_cookie,Origin:fixture.origin,'X-CSRF-Token':admin?fixture.admin_csrf:fixture.reader_csrf,'Content-Type':'application/json'},...(body===undefined?{}:{body:JSON.stringify(body)})});const v=await r.json();assert.ok(r.ok,`${route}: ${JSON.stringify(v)}`);return v;}
const space=await api('/learning-workspaces',{goal:{text:'学习事务与深度学习视频'},members:[fixture.first]});
const other=await api('/learning-workspaces',{goal:{text:'期末考试冲刺'}});
const route=`/learning-workspaces/${space.learning_workspace_id}/preparations`;
async function upload(name:string,data:Buffer){const upload_id=randomUUID();for(let offset=0;offset<data.length;offset+=32768){const chunk=data.subarray(offset,offset+32768);const v=await api(route,{action:'upload',upload_id,filename:name,total:data.length,offset,chunk_base64:chunk.toString('base64')});assert.equal(v.received,offset+chunk.length);}return api(route,{action:'create',operation_id:randomUUID(),task_id:randomUUID(),upload_id});}
const current=async(id:string)=>(await api(route)).items.find((t:any)=>t.task_id===id);
async function act(id:string,action:string,limit=50000){return api(route,{action,operation_id:randomUUID(),task_id:id,expected_revision:(await current(id)).revision,period_id:fixture.period_id,max_milli_percent:limit});}
const book=await upload('transactions.md',Buffer.from('# Transactions\n\nTransactions group operations into one atomic change.\n'));
const h=await operatorHarness({completeUsage:true,onFirstCommit:async()=>{await act(book.task_id,'pause');}});
const config={executable:process.env.UNDERSTAND_BOOK_BUILD_EXECUTABLE??path.resolve('target/b2-v1-20261018/bin/understand-book-build-x86_64-pc-windows-msvc.exe'),driverRoot:path.join(fixture.root,'personal-driver'),maxOutputTokens:4096,safetyMarginTokens:4096,handoffsPerCall:64};
const hostConfig={connection,rootSessionId:h.parent.id,video:{codexExecutable:'must-not-launch-personal-video',publisherExecutable:path.resolve('target/debug/publish_book.exe'),pythonExecutable:'E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-venv/Scripts/python.exe',whisperModelDir:'E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-models/small.en'}};
let host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',hostConfig,observedFetch);
try{
  await act(book.task_id,'prepare');await host.processNext();assert.equal((await current(book.task_id)).state,'awaiting_confirmation');assert.equal(h.model.requests.length,0);
  await act(book.task_id,'start',1);await host.processNext();assert.equal((await current(book.task_id)).state,'interrupted');assert.equal(h.model.requests.length,0,'low budget cannot dispatch a provider call');
  await act(book.task_id,'continue');await host.processNext();assert.equal((await current(book.task_id)).state,'interrupted');assert.ok(h.model.requests.length>0);
  const pass1=h.pass1Generations();await host.dispose();host=createOperatorBuildHost(h.ctx,config,'understand-book-executor',hostConfig,observedFetch);
  await act(book.task_id,'continue');await host.processNext();let built=await current(book.task_id);assert.equal(built.state,'ready',JSON.stringify(built));assert.equal(h.pass1Generations(),pass1,'accepted pass1 is reused after pause');
  const budget=await api(`/admin/build-tasks/${book.task_id}`,undefined,true);
  const charges=await api(`/admin/charges?cost_owner=account&parent_task_ref=${book.task_id}&limit=100&offset=0`,undefined,true);
  assert.equal(charges.total,h.model.requests.length);assert.ok(budget.debited_micro_cny>0);assert.ok(budget.debited_micro_cny<=500000);
  assert.ok(charges.items.every((c:any)=>c.user_id==='A'&&c.learning_workspace_id===space.learning_workspace_id));
  const calls=h.model.requests.length;
  await api(`/learning-workspaces/${space.learning_workspace_id}/goal`,{goal:{text:'只复习事务，并保持原资料'}});
  const reused=await api(route,{action:'create',operation_id:randomUUID(),task_id:randomUUID(),name:'已有成果复用',published_book_ref:built.materials[0].published_book_ref});assert.equal(reused.state,'ready');assert.equal(h.model.requests.length,calls);
  assert.equal((await api(`/learning-workspaces/${other.learning_workspace_id}`)).members.length,0);
  const source=process.env.UNDERSTAND_BOOK_VIDEO_ACCEPTANCE_SOURCE??'D:/codex-build/understand-book-vi18/installation-smoke/workspace/media/original.mp4';
  const video=await upload('tensor.mp4',readFileSync(source));await act(video.task_id,'start');await host.processNext();
  const videoResult=await current(video.task_id);assert.equal(videoResult.state,'ready',JSON.stringify(videoResult));assert.equal(h.model.requests.length,calls,'personal video import/transcript does not invoke the paid CLI');
  const videoBudget=await api(`/admin/build-tasks/${video.task_id}`,undefined,true);assert.ok(videoBudget.debited_micro_cny>0);assert.equal(videoBudget.unknown_count,0);
  const videoCharges=await api(`/admin/charges?cost_owner=account&parent_task_ref=${video.task_id}&limit=100&offset=0`,undefined,true);
  assert.ok(videoCharges.items.length>0);
  assert.ok(videoCharges.items.every((item:any)=>item.entry_kind==='service'&&item.quantity_ms>0&&item.period_id===fixture.period_id));
  const ref=videoResult.materials[0].published_book_ref;
  const transcript=await api(`/books/${encodeURIComponent(ref.book_id)}/publications/${encodeURIComponent(ref.publication_id)}/video_transcript`);
  assert.ok(transcript.map.segments.length>0);
  const output={scenario:'personal-preparation-http',compiled_engine:config.executable,book_task:book.task_id,workspace_id:space.learning_workspace_id,other_workspace_id:other.learning_workspace_id,book:built,video:videoResult,budget,video_budget:videoBudget,video_charges:videoCharges.items,service_rates:'synthetic-test-only',provider_calls:calls,charges:charges.items,pass1_reused:true,transcript_segments:transcript.map.segments.length,paid_provider:false};
  const outputDirectory=process.env.UNDERSTAND_BOOK_ACCEPTANCE_OUTPUT??'target/b2-v1-20261018';
  mkdirSync(outputDirectory,{recursive:true});writeFileSync(path.join(outputDirectory,'http-result.json'),JSON.stringify(output,null,2));
  console.log(JSON.stringify({success:true,provider_calls:calls,debited_micro_cny:budget.debited_micro_cny,pending:budget.unknown_count,video_model_calls:0,video_segments:output.transcript_segments,workspace_id:space.learning_workspace_id}));
}finally{await host.dispose();await h.dispose();await new Promise<void>(resolve=>proxy.close(()=>resolve()));}
