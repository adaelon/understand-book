import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, createWriteStream } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const out = resolve('evals/video/generated/vi15-browser', new Date().toISOString().replaceAll(':','-'));
mkdirSync(out, { recursive: true });
const save = (name, value) => writeFileSync(join(out,name), JSON.stringify(value,null,2));
const workspace = resolve(process.argv[2] || 'evals/video/generated/vi11/workspace');
const link = JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
let requests = [], bound;
const recorder = createServer(async (req,res) => {
  let raw=''; for await (const chunk of req) raw+=chunk;
  const body=JSON.parse(raw); requests.push(body); save(`request-${requests.length}.json`,body);
  const call=(name,args)=>({role:'assistant',content:null,tool_calls:[{id:`call-${requests.length}`,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
  let message;
  if(requests.length===1) message=call('tool.search',{task:'video.review_audio 原音频 听核 讲者措辞',required_capabilities:['source_read'],scope:'passage',operation:'read_source',effect_mode:'read_only',max_results:4});
  else if(requests.length===2) message=call('video.review_audio',{location:{video:bound.frame.video,start_ms:578000,end_ms:586000},question:'讲者说的是 underflow 还是 overflow？'});
  else if(requests.length===3) {
    const reviews=body.messages.filter(m=>m.role==='tool').map(m=>{try{const v=JSON.parse(m.content);return v.model_body??v;}catch{return null;}});
    const result=reviews.find(r=>r?.review?.review_result_ref);assert(result,JSON.stringify(reviews));
    assert.match(result.review.reviewed_text,/underflow/i);assert.match(result.review.reviewed_text,/overflow/i);
    save('review.json',result);message=call('source.present',{target:result.target});
  }
  else {
    const receipt=body.messages.filter(m=>m.role==='tool').map(m=>{try{const v=JSON.parse(m.content);return v.model_body??v;}catch{return null;}}).find(r=>r?.source_ref_id);
    if(!receipt) { res.writeHead(500);res.end('Missing source receipt; inspect saved request');return; }
    message={role:'assistant',content:`RECORDING ONLY: Whisper 原音频复核返回 underflow 和 overflow；其余措辞保留机器候选属性。 [[source:${receipt.source_ref_id}]]`};
  }
  res.writeHead(200,{'Content-Type':'application/json'}); res.end(JSON.stringify({choices:[{message}],usage:{total_tokens:10}}));
});
await new Promise(r=>recorder.listen(0,'127.0.0.1',r));
const env={...process.env,UNDERSTAND_BOOK_ADDR:'127.0.0.1:8794',UNDERSTAND_BOOK_MEMORY_DIR:join(out,'memory'),
  UNDERSTAND_BOOK_PRIVATE_DIR:join(out,'private'),OPENCODE_API_KEY:'vi7-local-recorder',
  OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi15-recorder',UNDERSTAND_BOOK_PROVIDER:'native',VIDEO_WHISPER_PYTHON:resolve('.worktree-local/whisper-venv/Scripts/python.exe'),VIDEO_WHISPER_MODEL_DIR:resolve('.worktree-local/whisper-models/small.en')};
const children=[];
function start(exe,args,name,cwd=process.cwd()) {
  const child=spawn(exe,args,{env,cwd,windowsHide:true,stdio:['ignore','pipe','pipe']});
  const log=createWriteStream(join(out,`${name}.log`)); child.stdout.pipe(log); child.stderr.pipe(log); children.push(child); return child;
}
let browser, page;
try {
  start(resolve(process.env.VI15_SERVER_EXE || join(process.env.CARGO_TARGET_DIR || 'target','debug/server.exe')),[workspace],'server');
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
  await page.waitForFunction(()=>!document.querySelector('button.send-agent')?.disabled);
  assert.equal(await page.getByRole('button',{name:'发送',exact:true}).isDisabled(),false);
  const admission=page.waitForResponse(r=>r.url().endsWith('/agent/runs')&&r.request().method()==='POST');
  await page.getByRole('button',{name:'发送',exact:true}).click();
  const accepted=await (await admission).json();
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
  assert(requests.every(r=>!r.messages.some(m=>Array.isArray(m.content)&&m.content.some(p=>p.type==='input_audio'))));
  assert.equal((await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft,null);
  const saved=run.final_view.outcome.answer_view.sources[0]; assert(saved);
  const source=await api('/agent/source.resolve',{turn_id:accepted.turn_id,source_ref_id:saved.source_ref_id});
  save('source.json',source);assert.equal(source.stale,false);assert.deepEqual(source.media.publication,bound.publication);
  if (process.argv[2]) assert(source.media.translations.length > 0, 'VI10 original source must carry admitted Chinese alongside English');
  assert.equal(source.media.target.kind,'audio');assert.equal(source.media.audio_reviews.length,1);assert(source.media.audio_reviews[0].uncertainties.length>0);
  const main=page.locator('.video-reader video');
  await main.evaluate(v=>{v.pause();v.currentTime=507;v.playbackRate=1.5;});
  await page.waitForFunction(()=>!document.querySelector('.video-reader video')?.seeking);
  await page.locator('.agent-source-button').last().click();
  const preview=page.getByLabel('媒体来源预览'); await preview.waitFor();
  await preview.getByLabel('原音频复核结果').waitFor();
  const side=preview.locator('audio').first();
  await side.evaluate(v=>{v.muted=true;return v.play();});
  await page.waitForTimeout(300);assert(await side.evaluate(v=>v.currentTime>=578 && v.currentTime<586));
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.screenshot({path:join(out,'answer-source-preview.png'),fullPage:true});
  await page.keyboard.press('Escape');
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.locator('.agent-source-button').last().click();await preview.waitFor();
  await page.getByRole('button',{name:'进入原视频位置'}).click();
  assert(Math.abs(await main.evaluate(v=>v.currentTime)-578)<.01);
  assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.reload();await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await page.locator('.agent-source-button').last().click();await page.getByLabel('媒体来源预览').waitFor();
  assert((await page.getByLabel('媒体来源预览').innerText()).includes('09:38'));
  assert((await page.getByLabel('原音频复核结果').innerText()).includes('underflow'));
  assert.deepEqual(errors,[]);
  const result={date:new Date().toISOString(),browser:browser.version(),publication:bound.publication,bound_frame:bound.frame,
    audio_location:source.media.target.location,audio_review:source.media.audio_reviews[0],focus_does_not_pause:true,manual_play_and_edits_preserve_binding:true,reload_restores_draft:true,
    accepted_turn:accepted.turn_id,history_preserves_binding:true,source_preview_preserves_main_player:true,explicit_entry_seeks_original:true,reloaded_history_source:true,requests:requests.length,actual_audio_provider:'local faster-whisper small.en CPU int8',
    model:'local HTTP recorder; no semantic inference',source_translation_count:source.media.translations.length,output:out};
  save('verification.json',result);console.log(JSON.stringify(result,null,2));
} catch (error) {
  if(page) { await page.screenshot({path:join(out,'failure.png'),fullPage:true}).catch(()=>{}); save('failure.json',{error:String(error),text:(await page.locator('body').innerText()).slice(0,5000),buttons:await page.locator('button').evaluateAll(bs=>bs.map(b=>({text:b.innerText,label:b.getAttribute('aria-label'),title:b.title})))}); }
  throw error;
} finally {
  await browser?.close(); for(const child of children) child.kill(); await new Promise(r=>recorder.close(r));
}
