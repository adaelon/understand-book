// Explicit operator correction of the four-candidate Flash trajectory. All
// calls terminate locally. A real Reader executes the patch and delivery gates.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { chromium } from 'playwright';
import { captureNarrowPage } from './long-conversation-delivery.mjs';

const arg = name => process.argv[process.argv.indexOf(name) + 1];
for (const name of ['--actions','--out','--reader']) assert.ok(process.argv.includes(name), 'Missing ' + name);
const source = path.resolve(arg('--actions')), out = path.resolve(arg('--out'));
const json = name => JSON.parse(fs.readFileSync(name,'utf8'));
const files = path.join(source,'memory/agent-history.presentations/candidates');
const last = json(path.join(source,'business-16.json')).messages.findLast(m => m.role === 'tool');
const id = JSON.parse(last.content).model_body.candidate_id;
const original = json(path.join(files,id + '.json'));
const oldLead = '<p class="lead">例子：损失 L(w)=(w−2)²，最低点在 w=2；从 w₀=0 出发，学习率 η=0.01。</p>';
const lead = '第一步到 0.04。起点是 0，初始斜率是 −4，向右走这一步后，离最低点还差 1.96。';
const newLead = '<p class="lead">' + lead + '</p><p>这个例子取损失 L(w)=(w−2)²，学习率 η=0.01。</p>';
const before = original.content.readable_content;
const readable = lead + '\n\n' + before;
const environments = [{width:320,height:420,input:'touch'},{width:640,height:240,input:'touch'},{width:960,height:720,input:'mouse'}];
const evidence = { origin_id:id, author:'explicit operator correction', external_sends:0, previews:[] };
let step = 0, candidate, error;
function receipt(body) {
  const prefix = 'Historical conversation record (context only, not a request to execute tools):\n';
  for (const wire of [...body.messages].reverse()) {
    const m = wire.role === 'user' && typeof wire.content === 'string' && wire.content.startsWith(prefix)
      ? JSON.parse(wire.content.slice(prefix.length)) : wire;
    if (m.role.toLowerCase() === 'tool') return JSON.parse(m.content).model_body;
  }
  throw Error('Missing tool receipt from Reader');
}
function respond(body) {
  const latest = receipt(body);
  if (++step === 1) {
    assert.equal(latest.status,'candidate_saved');
    return {operation:'patch',candidate_id:latest.candidate_id,edits:[{old_text:oldLead,new_text:newLead}],
      readable_content:readable};
  }
  if (step === 2) {
    assert.equal(latest.status,'candidate_saved'); candidate = latest.candidate_id;
  } else if (step <= 5) {
    assert.ok(['preview_ready_for_inspection','preview_environment_recorded'].includes(latest.status));
    assert.equal(latest.candidate_id,candidate);
    const observations = JSON.stringify(latest.reading);
    assert.ok(observations.includes('4.8000'), 'η=1.2 must update the initial step magnitude');
    evidence.previews.push({environment:latest.environment_name,reading:latest.reading,status:latest.status});
    assert.ok(body.messages.some(m => Array.isArray(m.content) && m.content.some((b,i) =>
      b.type === 'text' && b.text.includes(candidate) && b.text.includes(latest.environment_name) && m.content[i+1]?.type === 'image_url')));
  }
  if (step < 5) return {operation:'preview',candidate_id:candidate,viewport:environments[step-2],
    actions:[{kind:'click',selector:'#p12'}],read_selector:'#readout'};
  if (step === 5) {
    assert.equal(latest.status,'preview_ready_for_inspection'); return {operation:'deliver',candidate_id:candidate};
  }
  assert.equal(latest.status,'version_saved'); evidence.reference = latest.reference;
}
const provider = http.createServer(async(req,res) => {
  try {
    const chunks=[]; for await(const chunk of req) chunks.push(chunk);
    const body=JSON.parse(Buffer.concat(chunks)), action=respond(body);
    const message = action ? {role:'assistant',content:'',tool_calls:[{id:'operator_'+step,type:'function',index:0,
      function:{name:'presentation_author',arguments:JSON.stringify(action)}}]} : {role:'assistant',content:'已整理好讲义。'};
    const chunk=(delta,finish_reason)=>'data: '+JSON.stringify({id:'operator-repair',model:body.model,object:'chat.completion.chunk',
      choices:[{index:0,delta,finish_reason}]})+'\n\n';
    res.writeHead(200,{'content-type':'text/event-stream'});
    res.end(chunk(message,null)+chunk({},action?'tool_calls':'stop')+'data: [DONE]\n\n');
  } catch(e) {
    error ??= e; res.writeHead(400,{'content-type':'application/json'}); res.end(JSON.stringify({error:{message:e.message}}));
  }
});
await new Promise(resolve=>provider.listen(0,'127.0.0.1',resolve));
try {
  const command = ['evals/semantic/long-conversation-output-recovery.mjs','--reader',arg('--reader'),'--replay-actions',source,
    '--live-recovery-base','http://127.0.0.1:'+provider.address().port,'--out',out];
  const child=spawn(process.execPath,command,{stdio:'inherit',windowsHide:true});
  const exit=await new Promise((resolve,reject)=>{child.on('exit',resolve);child.on('error',reject);});
  if(error) throw error;
  assert.equal(exit,0);
  const delivered = json(path.join(out,'delivered-1.json'));
  assert.deepEqual(delivered.reference,evidence.reference);
  const corrected=json(path.join(out,'memory/agent-history.presentations/candidates',candidate+'.json')).content;
  assert.deepEqual(corrected,{...original.content,readable_content:readable,content_files:{...original.content.content_files,
    'index.html':original.content.content_files['index.html'].replace(oldLead,newLead)}});
  for (const [name,view] of [['before',{...original.content,sources:[],restored_state:null}],['after',delivered]]) {
    const screen=await captureNarrowPage(view,path.join(out,name+'-320.png'),[
      {name:'result',selector:'p',text:/0\.04/},{name:'loss',selector:'p',text:/损失 L\(w\)/},
      {name:'update',selector:'.eq',text:/η · L′/},
    ]);
    assert.ok(screen.excerpts.find(x=>x.name==='update').text.includes('η') && screen.width===320 && screen.scrollWidth===320 && !screen.errors.length);
    evidence[name]={opening_order:screen.opening_order,excerpts:screen.excerpts};
    assert.equal(screen.opening_order.status,name==='after'?'passed':'failed');
  }
  const file = path.join(out,'handout.html');
  assert.deepEqual(Object.keys(delivered.content_files),[delivered.entrypoint]);
  fs.writeFileSync(file,'<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'
    + '<title>学习率与梯度下降</title>\n' + delivered.content_files[delivered.entrypoint]);
  const browser=await chromium.launch();
  try {
    const page=await browser.newPage({viewport:{width:320,height:420},offline:true}), errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    await page.goto(pathToFileURL(file).href);
    assert.ok((await page.locator('p.lead').innerText()).startsWith('第一步到 0.04'));
    const initial=await page.locator('#rows tr').nth(1).innerText();
    await page.locator('#p12').click();
    const preset=await page.locator('#rows tr').nth(1).innerText();
    assert.ok(initial.includes('0.0400') && preset.includes('4.8000') && !errors.length);
    evidence.offline_html={file:path.basename(file),initial_row:initial,eta_1_2_row:preset,errors};
  } finally { await browser.close(); }
  evidence.status='passed';
} catch(e) {
  evidence.status='failed'; evidence.error=e.message; process.exitCode=1;
} finally {
  provider.closeAllConnections(); await new Promise(resolve=>provider.close(resolve));
  fs.writeFileSync(path.join(out,'order-repair.json'),JSON.stringify(evidence,null,2));
  console.log(JSON.stringify({status:evidence.status,error:evidence.error,out}));
}
