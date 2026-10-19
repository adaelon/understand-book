import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, createWriteStream } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const out = resolve('evals/video/generated/vi6', new Date().toISOString().replaceAll(':','-'));
mkdirSync(out, { recursive: true });
const save = (name, value) => writeFileSync(join(out,name), JSON.stringify(value,null,2));
const workspace = resolve('evals/video/generated/vi2/moved-workspace');
const link = JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
let requests = [], bound;
const recorder = createServer(async (req,res) => {
  let raw=''; for await (const chunk of req) raw+=chunk;
  const body=JSON.parse(raw); requests.push(body); save(`request-${requests.length}.json`,body);
  const call=(name,args)=>({role:'assistant',content:null,tool_calls:[{id:`call-${requests.length}`,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
  let message;
  if(requests.length===1) message=call('tool.search',{task:'video.read_frames 视频画面取帧',required_capabilities:['source_read'],scope:'passage',operation:'read_source',effect_mode:'read_only',max_results:4});
  else if(requests.length===2) message=call('video.read_frames',{location:{video:bound.frame.video,start_ms:bound.frame.actual_pts_ms,end_ms:bound.frame.actual_pts_ms+1000},sampling:[bound.frame.actual_pts_ms]});
  else message={role:'assistant',content:'RECORDING ONLY: frozen video frame received; no model inference.'};
  res.writeHead(200,{'Content-Type':'application/json'}); res.end(JSON.stringify({choices:[{message}],usage:{total_tokens:10}}));
});
await new Promise(r=>recorder.listen(0,'127.0.0.1',r));
const env={...process.env,UNDERSTAND_BOOK_ADDR:'127.0.0.1:8794',UNDERSTAND_BOOK_MEMORY_DIR:join(out,'memory'),
  UNDERSTAND_BOOK_PRIVATE_DIR:join(out,'private'),OPENCODE_API_KEY:'vi6-local-recorder',
  OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi6-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
const children=[];
function start(exe,args,name,cwd=process.cwd()) {
  const child=spawn(exe,args,{env,cwd,windowsHide:true,stdio:['ignore','pipe','pipe']});
  const log=createWriteStream(join(out,`${name}.log`)); child.stdout.pipe(log); child.stderr.pipe(log); children.push(child); return child;
}
let browser, page;
try {
  start(resolve('target/debug/server.exe'),[workspace],'server');
  start(process.execPath,[join(dirname(require.resolve('vite/package.json')),'bin/vite.js'),'--port','5176','--strictPort'],'web',resolve('packages/web'));
  for(const url of ['http://127.0.0.1:8794/desktop/status','http://localhost:5176']) {
    let ready=false;
    for(let n=0;n<120;n++) { try {if((await fetch(url)).ok){ready=true;break;}}catch{} await new Promise(r=>setTimeout(r,250)); }
    assert(ready,`Host unavailable: ${url}`);
  }
  browser=await chromium.launch({channel:'msedge',headless:true});
  page=await browser.newPage({baseURL:'http://localhost:5176',viewport:{width:1440,height:1000}});
  const errors=[];page.on('pageerror',e=>{errors.push(e.message);save('page-errors.json',errors);console.log('Browser error:',e.message);});
  await page.goto('http://localhost:5176');
  await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  const input=page.getByRole('textbox',{name:'围绕当前阅读内容提问'});
  await input.waitFor({state:'attached'});
  if (!await input.isVisible()) await page.getByRole('button',{name:'问答',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('video')?.readyState>=2);
  await page.locator('video').evaluate(v=>{v.muted=true;return v.play();});
  await page.waitForFunction(()=>!document.querySelector('textarea[data-workspace-input="agent"]')?.disabled);
  await input.focus();
  assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
  await page.locator('video').evaluate(v=>v.pause());
  await page.locator('video').evaluate(v=>{v.currentTime=492;v.muted=true;});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return !v.seeking&&v.readyState>=2;});
  await input.focus(); assert(await page.locator('video').evaluate(v=>v.paused));
  // Composition generates the same input event used by the native textarea.
  await input.dispatchEvent('compositionstart');
  await input.fill('解释这行代码');
  await input.dispatchEvent('compositionend');
  await page.getByLabel('问题绑定位置').waitFor();
  const windowId=await page.evaluate(()=>sessionStorage.getItem('video-question-window'));
  const api=async(path,body)=>{
    const response=await page.request.post(`/api${path}`,{data:body});const value=await response.json();
    assert(response.ok(),`${path}: ${JSON.stringify(value)}`); return value;
  };
  let draft;
  for(let n=0;n<40;n++) {draft=(await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft;if(draft)break;await page.waitForTimeout(50);}
  assert(draft);bound=draft.context;assert(Math.abs(bound.frame.actual_pts_ms-492000)<100);
  assert.deepEqual(bound.publication,link.reference);
  await page.locator('video').evaluate(v=>{v.currentTime=507;return v.play();});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return !v.seeking&&!v.paused;});
  await input.focus(); assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
  await input.fill('解释这行代码（再次编辑和粘贴）');
  assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
  await page.waitForTimeout(150);
  const before=(await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft;
  assert.deepEqual(before.context,bound);
  await page.screenshot({path:join(out,'bound-0812-playing-0827.png'),fullPage:true});
  await page.reload(); await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await input.waitFor(); await page.waitForFunction(()=>document.querySelector('textarea[data-workspace-input="agent"]')?.value.includes('再次编辑'));
  assert((await page.getByLabel('问题绑定位置').innerText()).includes('08:12'));
  assert.equal(await page.getByRole('button',{name:'发送',exact:true}).isDisabled(), false);
  // Keep the VI6 admission check; VI7 additionally covers sending through the page and source preview.
  const accepted=await api('/agent/runs',{message:before.message,video_window_id:windowId,video_draft_id:bound.draft_id});
  let run;
  for(let n=0;n<240;n++) {
    run=await (await page.request.get(`/api/agent/runs/${accepted.turn_id}`)).json();
    if(run.persistence_state==='saved'&&['completed','failed','cancelled'].includes(run.execution_state))break;
    await page.waitForTimeout(250);
  }
  save('run.json',run);assert.equal(run.execution_state,'completed',JSON.stringify(run));
  assert.deepEqual(run.final_view.video_question,bound);
  const text=requests[0].messages.map(m=>typeof m.content==='string'?m.content:'').join('\n');
  assert(text.includes(String(bound.frame.actual_pts_ms)));assert(text.includes(bound.publication.publication_id));
  const imageRequests=requests.filter(r=>r.messages.some(m=>Array.isArray(m.content)&&m.content.some(p=>p.type==='image_url')));
  assert(imageRequests.length>0);
  const imageParts=imageRequests[0].messages.flatMap(m=>Array.isArray(m.content)?m.content:[]);
  const metadata=imageParts.filter(p=>p.type==='text').map(p=>{try{return JSON.parse(p.text);}catch{return null;}}).find(p=>p?.purpose==='video_source');
  assert(metadata);assert(Math.abs(metadata.read.frame.actual_pts_ms-bound.frame.actual_pts_ms)<1);
  assert.deepEqual(metadata.publication,bound.publication);
  assert.equal((await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft,null);
  assert.deepEqual(errors,[]);
  const result={date:new Date().toISOString(),browser:browser.version(),publication:bound.publication,bound_frame:bound.frame,
    decoded_frame:metadata.read.frame,focus_does_not_pause:true,manual_play_and_edits_preserve_binding:true,reload_restores_draft:true,
    accepted_turn:accepted.turn_id,history_preserves_binding:true,requests:requests.length,images:imageParts.filter(p=>p.type==='image_url').length,
    model:'local HTTP recorder; no semantic inference',output:out};
  save('verification.json',result);console.log(JSON.stringify(result,null,2));
} catch (error) {
  if(page) { await page.screenshot({path:join(out,'failure.png'),fullPage:true}).catch(()=>{}); save('failure.json',{error:String(error),text:(await page.locator('body').innerText()).slice(0,5000),buttons:await page.locator('button').evaluateAll(bs=>bs.map(b=>({text:b.innerText,label:b.getAttribute('aria-label'),title:b.title})))}); }
  throw error;
} finally {
  await browser?.close(); for(const child of children) child.kill(); await new Promise(r=>recorder.close(r));
}
