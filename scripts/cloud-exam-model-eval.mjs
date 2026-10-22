// Actual isolated multi-user Reader runs. The host contains only original
// fixture content and explicitly synthetic admission rates, never sale prices.
import fs from 'node:fs';
import path from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { startProviderRecorder, measuredUsage, usageByPurpose } from '../evals/semantic/provider-recorder.mjs';
import { cloudExamChecks } from './cloud-exam-checks.mjs';
import { isDeliveryReview, reviewDeliveryValidation, billingEvidence } from './cloud-exam-review-evidence.mjs';

const args=process.argv.slice(2);
const option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const binary=option('--test-bin',process.env.CLOUD_EXAM_TEST_BIN);
if(!binary)throw new Error('Supply the already compiled server test executable with --test-bin.');
if(fs.existsSync('.env'))process.loadEnvFile('.env');
const {OPENCODE_BASE_URL:base,OPENCODE_API_KEY:key,FLUID_LLM_MODEL:model}=process.env;
if(!base||!key||!model)throw new Error('The existing real provider configuration is required.');
const output=path.resolve(option('--out','docs/performance/cloud-learning-20261020/costs/live'));
if(fs.existsSync(path.join(output,'run.json')))throw new Error('Choose a fresh output directory; saved runs are not overwritten.');
fs.mkdirSync(output,{recursive:true});
for(const material of ['lesson.txt','video-transcript.txt']){
  fs.copyFileSync(path.join('docs/examples/cloud-course',material),path.join(output,material));
}
const save=(name,value)=>fs.writeFileSync(path.join(output,name),JSON.stringify(value,null,2)+'\n');
const detailedUsage=records=>({
  ...measuredUsage(records),
  prompt_tokens:records.reduce((n,r)=>n+(r.usage?.prompt_tokens??0),0),
  completion_tokens:records.reduce((n,r)=>n+(r.usage?.completion_tokens??0),0),
  cached_prompt_tokens:records.reduce((n,r)=>n+(r.usage?.prompt_cache_hit_tokens??r.usage?.prompt_tokens_details?.cached_tokens??0),0),
});
const only=option('--only','').split(',').filter(Boolean);
const report={protocol:'cloud-exam-live.v1',started_at:new Date().toISOString(),model,provider_origin:new URL(base).origin,
  source_head:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8',windowsHide:true}).trim(),
  scope:'Actual cloud Reader admission, frozen exam context, model tools, source resolution and account usage; original two-second fixture, not a selected academic course.',
  monetary_cost:{status:'unknown',reason:'No attributable current supplier rate was configured. Host ledger values use an admission-only test rate and are not provider costs or product prices.'},
  cases:[],status:'running'};
save('run.json',report);
// Windows locks a running executable. A private copy lets the shared build
// continue linking the next server snapshot while this sample is in flight.
const temporaryRoot=path.resolve('target/cloud-exam-tmp');
fs.mkdirSync(temporaryRoot,{recursive:true});
const executableDirectory=fs.mkdtempSync(path.join(temporaryRoot,'run-'));
const executable=path.join(executableDirectory,path.basename(binary));
fs.copyFileSync(path.resolve(binary),executable);
report.test_executable={source:path.resolve(binary),built_at:fs.statSync(binary).mtime.toISOString()};
const recorder=await startProviderRecorder(base,{tokenLimit:800000});
const child=spawn(executable,['cloud_learning_browser_host','--ignored','--nocapture','--test-threads=1'],{
  windowsHide:true,stdio:['ignore','pipe','pipe'],env:{...process.env,CLOUD_EXAM_REAL_PROVIDER:'1',
    TEMP:executableDirectory,TMP:executableDirectory,OPENCODE_BASE_URL:recorder.url}});
let log='';
child.stdout.on('data',b=>{log+=b.toString();});
child.stderr.on('data',b=>{log+=b.toString();});
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
let fixture;
function readerHeaders() {
  return { 'Content-Type':'application/json', Origin:fixture.origin, Cookie:fixture.reader_cookie,
    'X-CSRF-Token':fixture.reader_csrf };
}
async function api(route,body,admin=false){
  const response=await fetch(`http://127.0.0.1:4200/api${route}`,{method:body===undefined?'GET':'POST',
    headers:{'Content-Type':'application/json',Origin:fixture.origin,Cookie:admin?fixture.admin_cookie:fixture.reader_cookie,
      'X-CSRF-Token':admin?fixture.admin_csrf:fixture.reader_csrf},
    ...(body===undefined?{}:{body:JSON.stringify(body)}),signal:AbortSignal.timeout(30000)});
  const value=await response.json();
  if(!response.ok)throw new Error(`${route}: HTTP ${response.status} ${value.error_code??''} ${value.message??''}`);
  return value;
}
function observeRun(turnId) {
  const events=[], controller=new AbortController();
  const observation={events};
  const done=(async()=>{
    try {
      const response=await fetch(`http://127.0.0.1:4200/api/agent/runs/${turnId}/events`,
        {headers:readerHeaders(),signal:controller.signal});
      observation.http_status=response.status;
      if(!response.ok)throw new Error(`Run event observation returned HTTP ${response.status}`);
      const decoder=new TextDecoder();let pending='';
      for await(const chunk of response.body) {
        pending+=decoder.decode(chunk,{stream:true}).replace(/\r\n/g,'\n');
        let boundary;
        while((boundary=pending.indexOf('\n\n'))>=0) {
          const frame=pending.slice(0,boundary);pending=pending.slice(boundary+2);
          const data=frame.split('\n').filter(line=>line.startsWith('data:')).map(line=>line.slice(5).trimStart()).join('\n');
          if(data)events.push({observed_at:new Date().toISOString(),event:JSON.parse(data)});
        }
      }
    }catch(error){if(!controller.signal.aborted)observation.error=error.message;}
  })();
  return {observation,stop:async()=>{controller.abort();await done;}};
}
const date=days=>new Date(Date.now()+480*60000+days*86400000).toISOString().slice(0,10);
let learning;
const settings=(days,syllabus)=>({exam_date:date(days),syllabus,daily_minutes:30,utc_offset_minutes:480,entry_mode:'question'});
async function setSprint(days,syllabus){
  learning=await api(`/learning-workspaces/${learning.learning_workspace_id}`);
  const result=await api(`/learning-workspaces/${learning.learning_workspace_id}/exam-sprint`,{
    expected_goal_revision:learning.goal.revision,goal_text:'期末冲刺：能独立指出一条陈述对应的材料版本与视频时间片段',sprint:settings(days,syllabus)});
  learning=result.workspace;
  return result.sprint;
}
async function runCase(id,message,sprint){
  if(only.length&&!only.includes(id))return;
  const before=recorder.records.length,started=performance.now();
  let row={id,message,sprint,status:'running',usage_attribution:'Requests started while this case was running; background calls may overlap adjacent cases.'};
  report.cases.push(row);save('run.json',report);
  let observation;
  try{
    let scene=await api('/workspaces',{published_book_ref:fixture.text,attachment_id:`eval-${id}`,learning_workspace_id:learning.learning_workspace_id});
    const stamp=()=>({attachment_id:`eval-${id}`,generation:scene.generation,expected_revision:scene.revision});
    scene=await api(`/workspaces/${scene.workspace_id}/chat/new`,stamp());
    const accepted=await api(`/workspaces/${scene.workspace_id}/agent/runs`,{
      ...stamp(),client_request_id:crypto.randomUUID(),session_id:scene.selected_chat,published_book_ref:fixture.text,message});
    row.turn_id=accepted.turn_id;
    row.workspace_id=scene.workspace_id;
    row.learning_workspace_id=learning.learning_workspace_id;
    observation=observeRun(accepted.turn_id);
    const deadline=Date.now()+8*60*1000;
    let current;
    do{
      await pause(600);
      current=await api(`/agent/runs/${accepted.turn_id}`);
      if(current.turn?.status!=='pending_assistant')break;
    }while(Date.now()<deadline);
    if(current.turn?.status==='pending_assistant'){
      await api(`/agent/runs/${accepted.turn_id}/cancel`,{});throw new Error('Bounded eight-minute task deadline reached.');
    }
    row.turn=current.turn;
    row.status=current.turn?.status;
    row.sources=[];
    for(const source of current.turn?.outcome?.answer_view?.sources??[]){
      try{
        scene=await api(`/workspaces/${scene.workspace_id}`);
        const resolved=await api(`/workspaces/${scene.workspace_id}/agent/source.resolve`,{
          ...stamp(),turn_id:accepted.turn_id,source_ref_id:source.source_ref_id});
        row.sources.push({source_ref_id:source.source_ref_id,resolved});
      }catch(error){row.sources.push({source_ref_id:source.source_ref_id,error:error.message});}
    }
    row.answer=current.turn?.outcome?.answer??null;
    fs.writeFileSync(path.join(output,`${id}-answer.md`),row.answer??JSON.stringify(current.turn?.error??current,null,2));
  }catch(error){row.status='failed';row.error=error.message;}
  if(observation){await observation.stop();row.stream_observation=observation.observation;}
  row.elapsed_ms=performance.now()-started;
  row.provider_records=recorder.records.slice(before).map(r=>r.ordinal);
  row.usage=detailedUsage(recorder.records.slice(before));
  row.validation=cloudExamChecks(row,recorder.records.slice(before),fixture.video);
  row.delivery_review_validation=reviewDeliveryValidation(row,recorder.records.slice(before));
  save(`${id}.json`,row);save('provider.json',recorder.records);save('run.json',report);
  console.log(JSON.stringify({id,status:row.status,elapsed_ms:row.elapsed_ms,usage:row.usage,error:row.error}));
}
try{
  const deadline=Date.now()+90000;
  while(!log.includes('CLOUD_LEARNING_BROWSER_READY')){
    if(child.exitCode!==null)throw new Error(`Fixture host exited (${child.exitCode}); see host.log.`);
    if(Date.now()>deadline)throw new Error('Fixture host startup timed out.');
    await pause(250);
  }
  fixture=await(await fetch('http://127.0.0.1:4201/fixture')).json();
  await api('/admin/users/A/cloud-plan-purchases',{operation_id:'cloud-exam-purchase',product_id:'browser-basic-day3',
    product_revision:1,paid_at:Math.floor(Date.now()/1000),channel:'isolated-model-test',external_ref:'cloud-exam-original',
    note:'No actual payment; synthetic allowance admission only'},true);
  const result=await api('/workspace-releases/cloud-original-fixture/versions/1/use',{operation_id:'cloud-exam-use',
    goal:{text:'期末冲刺：能独立指出一条陈述对应的材料版本与视频时间片段',constraints:[]},
    exam_sprint:settings(3,'原文陈述、资料版本、视频时间片段')});
  learning=result.workspace;
  report.materials={text:fixture.text,video:fixture.video,release_id:fixture.release_id};
  let sprint=(await api(`/learning-workspaces/${learning.learning_workspace_id}/exam-sprint`)).sprint;
  await runCase('three-days','根据工作区已经确认的考试日期和范围，给我今天30分钟的复习安排。不要先做诊断，不要制作交互页面；说明先练什么以及为什么。用中文简洁回答。',sprint);
  sprint=await setSprint(1,'只考视频时间片段定位；不考资料版本管理');
  await runCase('changed-date-scope','我刚修改了工作区的考试日期和范围。请按当前设置重新安排今天30分钟，只安排考试范围内的内容。不要先做诊断，不要制作交互页面。',sprint);
  await runCase('direct-question','我现在不会这道题，请直接讲这题，不要先给我诊断或先修测验：样例视频有两秒，我的陈述只涉及第一秒，为什么不能把整段视频当作精确证据窗口？请直接用这道题解释。不要制作交互页面。',sprint);
  sprint=await setSprint(1,'原文证据窗口与视频时间片段定位');
  await runCase('text-video-sources','请实际查这个工作区的原创讲义和样例视频字幕，用中文回答：讲义为什么要求保留原始来源引用？视频字幕说动态图案持续多久，应回看哪个时间窗口？分别给能跳回讲义原文和视频时段的来源引用。只依据文字和字幕，不需要分析画面，也不要制作交互页面。',sprint);
  report.admin_usage=await api('/admin/charges?user_id=A&limit=100&offset=0',undefined,true);
  report.initial_charge_states=report.admin_usage.items.map(row=>({call_id:row.call_id,purpose:row.purpose,state:row.state}));
  // A completed answer can start the existing account-level memory review.
  // Observe its settlement before stopping this isolated host; these are reads,
  // and do not start another model run or retry a failed call.
  const settlementDeadline=Date.now()+60000;
  while(report.admin_usage.items.some(row=>row.state==='reserved'||row.state==='sent')&&Date.now()<settlementDeadline){
    await pause(600);
    report.admin_usage=await api('/admin/charges?user_id=A&limit=100&offset=0',undefined,true);
  }
  report.account_usage=await api('/account/usage?limit=100&offset=0');
  report.storage=await api('/account/storage');
  report.execution_status=report.cases.every(c=>c.status==='completed')?'completed':'failed';
  report.status=report.cases.every(c=>c.validation.all_passed&&c.delivery_review_validation.all_passed)?'awaiting_semantic_review':'failed_checks';
}catch(error){report.status='failed';report.error=error.message;}
finally{
  try{await fetch('http://127.0.0.1:4201/stop');}catch{}
  if(child.exitCode===null)await Promise.race([new Promise(resolve=>child.once('exit',resolve)),pause(12000)]);
  if(child.exitCode===null)child.kill();
  await recorder.stop(10000);
  report.finished_at=new Date().toISOString();
  for(const row of report.cases){
    row.usage=detailedUsage(recorder.records.filter(r=>row.provider_records?.includes(r.ordinal)));
    save(`${row.id}.json`,row);
  }
  report.usage=detailedUsage(recorder.records);
  report.usage_by_purpose={...usageByPurpose(recorder.records.filter(r=>!isDeliveryReview(r))),
    tutor_delivery_review:detailedUsage(recorder.records.filter(isDeliveryReview))};
  report.billing_validation=billingEvidence(report,recorder.records);
  if(!report.billing_validation.all_passed&&report.status!=='failed')report.status='failed_checks';
  report.records_outside_case_windows=recorder.records.filter(r=>!report.cases.some(c=>c.provider_records?.includes(r.ordinal))).map(r=>r.ordinal);
  report.provider_elapsed_ms=recorder.records.reduce((n,r)=>n+(r.elapsed_ms??0),0);
  report.budget_rejections=recorder.budgetRejections;
  try{
    // This exact directory was created by this run below the workspace target.
    // Check its resolved parent before recursively removing Windows fixtures.
    if(path.dirname(path.resolve(executableDirectory))!==temporaryRoot)throw new Error('Unexpected task temporary path');
    fs.rmSync(executableDirectory,{recursive:true,force:true,maxRetries:3,retryDelay:100});
  }catch{report.temporary_directory_retained=executableDirectory;}
  report.executable_copy_removed=!fs.existsSync(executable);
  save('provider.json',recorder.records);save('run.json',report);
  fs.writeFileSync(path.join(output,'host.log'),log);
  console.log(JSON.stringify({status:report.status,cases:report.cases.length,usage:report.usage,provider_elapsed_ms:report.provider_elapsed_ms}));
  if(report.status==='failed'||report.status==='failed_checks')process.exitCode=1;
}
