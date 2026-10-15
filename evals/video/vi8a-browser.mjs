import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, createWriteStream } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const out = resolve('evals/video/generated/vi8a', new Date().toISOString().replaceAll(':','-'));
mkdirSync(out, { recursive: true });
const save = (name, value) => writeFileSync(join(out,name), JSON.stringify(value,null,2));
const workspace = resolve('evals/video/generated/vi2/moved-workspace');
const link = JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
let requests = [], bound, observedRegion;
const recorder = createServer(async (req,res) => {
  let raw=''; for await (const chunk of req) raw+=chunk;
  const body=JSON.parse(raw); requests.push(body); save(`request-${requests.length}.json`,body);
  const call=(name,args)=>({role:'assistant',content:null,tool_calls:[{id:`call-${requests.length}`,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
  let message;
  if(requests.length===1) message=call('tool.search',{task:'video.read_frames 视频画面取帧',required_capabilities:['source_read'],scope:'passage',operation:'read_source',effect_mode:'read_only',max_results:4});
  else if(requests.length===2) message=call('video.read_frames',{location:{video:bound.frame.video,start_ms:bound.frame.actual_pts_ms,end_ms:bound.frame.actual_pts_ms+1000},sampling:[bound.frame.actual_pts_ms],region:bound.region.rect_normalized});
  else if(requests.length===3) {
    const metadata=body.messages.flatMap(m=>Array.isArray(m.content)?m.content:[]).filter(p=>p.type==='text')
      .map(p=>{try{return JSON.parse(p.text);}catch{return null;}}).find(p=>p?.purpose==='video_source');
    assert(metadata?.region);
    observedRegion=metadata.region;
    // The first image-bearing response establishes the observation. Source presentation
    // becomes available on the next sampling, after that response has succeeded.
    message=call('tool.search',{task:'source.present 已观察画面来源',required_capabilities:['source_presentation'],max_results:1});
  }
  else if(requests.length===4) message=call('source.present',{target:{kind:'visual',publication:bound.publication,frames:[],regions:[observedRegion]}});
  else {
    const receipt=body.messages.filter(m=>m.role==='tool').map(m=>{try{const v=JSON.parse(m.content);return v.model_body??v;}catch{return null;}}).find(r=>r?.source_ref_id);
    if(!receipt) { res.writeHead(500);res.end('Missing source receipt; inspect saved request');return; }
    message={role:'assistant',content:`RECORDING ONLY: saved visual source. [[source:${receipt.source_ref_id}]]`};
  }
  res.writeHead(200,{'Content-Type':'application/json'}); res.end(JSON.stringify({choices:[{message}],usage:{total_tokens:10}}));
});
await new Promise(r=>recorder.listen(0,'127.0.0.1',r));
const env={...process.env,UNDERSTAND_BOOK_ADDR:'127.0.0.1:8794',UNDERSTAND_BOOK_MEMORY_DIR:join(out,'memory'),
  UNDERSTAND_BOOK_PRIVATE_DIR:join(out,'private'),OPENCODE_API_KEY:'vi8a-local-recorder',
  OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi8a-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
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
  await page.locator('video').evaluate(v=>{v.currentTime=650;v.muted=true;});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return !v.seeking&&v.readyState>=2;});
  const windowId=await page.evaluate(()=>sessionStorage.getItem('video-question-window'));
  const api=async(path,body)=>{
    const response=await page.request.post(`/api${path}`,{data:body});const value=await response.json();
    assert(response.ok(),`${path}: ${JSON.stringify(value)}`); return value;
  };
  const getDraft=async()=> (await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft;
  const waitDraft=async predicate=> {
    for(let n=0;n<100;n++) {const d=await getDraft();if(predicate(d))return d;await page.waitForTimeout(50);}
    throw new Error('Draft did not reach expected state');
  };
  const main=page.locator('.video-reader video');
  const select=async()=> {
    await page.getByRole('button',{name:/^(框选画面|重新框选)$/}).click();
    const layer=page.getByLabel('画面框选区域');await layer.scrollIntoViewIfNeeded();
    const box=await layer.boundingBox();assert(box);
    await page.mouse.move(box.x+box.width/16,box.y+box.height*4/9);
    await page.mouse.down();
    await page.mouse.move(box.x+box.width*31/48,box.y+box.height*7/12,{steps:8});
    await page.mouse.up();
    return await waitDraft(d=>d?.context.region);
  };
  const layouts=[];
  for(const layout of [
    {name:'wide',width:1440,height:1000,playerHeight:''},
    {name:'narrow',width:1024,height:900,playerHeight:''},
    {name:'letterbox',width:1024,height:1000,playerHeight:'560px'},
    {name:'pillarbox',width:1440,height:1000,playerHeight:'260px'},
  ]) {
    await page.setViewportSize({width:layout.width,height:layout.height});
    await main.evaluate((v,height)=>{v.style.height=height;},layout.playerHeight);
    const draft=await select();
    assert.equal(draft.message,'');
    const rect=draft.context.region.rect_normalized;
    // Pointer coordinates are quantized by the browser; compare the actual original pixel range.
    layouts.push({name:layout.name,player:await main.boundingBox(),rect,pixels:[rect[0]*1920,rect[1]*1080,rect[2]*1920,rect[3]*1080]});
    if(layouts.length>1) assert.deepEqual(rect,layouts[0].rect,'layout changed source pixels');
    await page.screenshot({path:join(out,`selection-${layout.name}.png`),fullPage:true});
    await page.getByRole('button',{name:'取消选区',exact:true}).click();
    await waitDraft(d=>d===null);
  }
  await page.setViewportSize({width:1440,height:1000});await main.evaluate(v=>{v.style.height='';});
  let draft=await select();bound=draft.context;assert(Math.abs(bound.frame.actual_pts_ms-650000)<100);
  await input.fill('解释框选代码'); await waitDraft(d=>d?.message==='解释框选代码');
  await page.getByRole('button',{name:'取消选区',exact:true}).click();
  const textOnly=await waitDraft(d=>d && !d.context.region);
  assert.equal(textOnly.context.draft_id,bound.draft_id);assert.deepEqual(textOnly.context.frame,bound.frame);
  draft=await select();bound=draft.context;
  assert.deepEqual(bound.publication,link.reference);
  await page.setViewportSize({width:1024,height:900});
  assert.deepEqual((await getDraft()).context,bound,'resize must preserve the whole binding');
  await page.setViewportSize({width:1440,height:1000});
  await page.locator('video').evaluate(v=>{v.currentTime=507;return v.play();});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return !v.seeking&&!v.paused;});
  await input.focus(); assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
  await input.fill('解释这行代码（再次编辑和粘贴）');
  assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
  await page.waitForTimeout(150);
  const before=(await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft;
  assert.deepEqual(before.context,bound);
  assert.equal(await page.locator('.video-region-box').count(),0,'old region must not cover a different playing frame');
  await page.screenshot({path:join(out,'bound-region-playing-later.png'),fullPage:true});
  assert(await page.getByRole('button',{name:'重新框选',exact:true}).isDisabled());
  await page.getByRole('button',{name:'回到问题画面',exact:true}).click();
  await page.locator('.video-region-box').waitFor();
  assert.deepEqual((await getDraft()).context,bound,'return to frame must preserve binding');
  await page.reload(); await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await input.waitFor(); await page.waitForFunction(()=>document.querySelector('textarea[data-workspace-input="agent"]')?.value.includes('再次编辑'));
  assert((await page.getByLabel('问题绑定位置').innerText()).includes('10:50'));
  assert((await page.getByLabel('问题绑定位置').innerText()).includes('已选区域'));
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
  const imageRequests=requests.filter(r=>r.messages.some(m=>Array.isArray(m.content)&&m.content.some(p=>p.type==='image_url')));
  assert(imageRequests.length>0);
  const imageParts=imageRequests[0].messages.flatMap(m=>Array.isArray(m.content)?m.content:[]);
  const metadata=imageParts.filter(p=>p.type==='text').map(p=>{try{return JSON.parse(p.text);}catch{return null;}}).find(p=>p?.purpose==='video_source');
  assert(metadata);assert(Math.abs(metadata.read.frame.actual_pts_ms-bound.frame.actual_pts_ms)<1);
  assert.deepEqual(metadata.publication,bound.publication);
  assert(metadata.region);
  const png=Buffer.from(imageParts.find(p=>p.type==='image_url').image_url.url.split(',')[1],'base64');
  const imageSize=[png.readUInt32BE(16),png.readUInt32BE(20)];
  assert(imageSize[0]<1920 && imageSize[1]<1080);writeFileSync(join(out,'model-crop.png'),png);
  assert.equal((await api('/reader/video_draft',{window_id:windowId,action:'get'})).draft,null);
  const saved=run.final_view.outcome.answer_view.sources[0]; assert(saved);
  const source=await api('/agent/source.resolve',{turn_id:accepted.turn_id,source_ref_id:saved.source_ref_id});
  save('source.json',source);assert.equal(source.stale,false);assert.deepEqual(source.media.publication,bound.publication);
  assert.deepEqual(source.frames[0].frame,bound.frame);assert(source.frames[0].url.startsWith('data:image/png;base64,'));
  assert.deepEqual(source.frames[0].region,metadata.region);
  assert.equal(source.frames[0].url.split(',')[1],png.toString('base64'),'preview must replay model crop pixels');
  await main.evaluate(v=>{v.pause();v.currentTime=507;v.playbackRate=1.5;});
  await page.waitForFunction(()=>!document.querySelector('.video-reader video')?.seeking);
  await page.locator('.agent-source-button').last().click();
  const preview=page.getByLabel('媒体来源预览'); await preview.waitFor();
  await preview.locator('img').waitFor();
  const side=preview.locator('video').first();
  await side.evaluate(v=>{v.muted=true;return v.play();});
  await page.waitForTimeout(300);
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.screenshot({path:join(out,'answer-source-preview.png'),fullPage:true});
  await page.keyboard.press('Escape');
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.locator('.agent-source-button').last().click();await preview.waitFor();
  await page.getByRole('button',{name:'进入原视频位置'}).click();
  assert(Math.abs(await main.evaluate(v=>v.currentTime)-bound.frame.actual_pts_ms/1000)<.01);
  assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.reload();await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await page.locator('.agent-source-button').last().click();await page.getByLabel('媒体来源预览').waitFor();
  assert((await page.getByLabel('媒体来源预览').innerText()).includes('10:50'));
  assert.deepEqual(errors,[]);
  const result={date:new Date().toISOString(),browser:browser.version(),publication:bound.publication,bound_frame:bound.frame,
    decoded_frame:metadata.read.frame,bound_region:bound.region,decoded_region:metadata.region,image_size:imageSize,layouts,
    selection_only_cancel_releases:true,text_cancel_preserves_binding:true,resize_preserves_binding:true,return_to_bound_frame:true,
    focus_does_not_pause:true,manual_play_and_edits_preserve_binding:true,reload_restores_draft:true,
    accepted_turn:accepted.turn_id,history_preserves_binding:true,source_preview_preserves_main_player:true,explicit_entry_seeks_original:true,reloaded_history_source:true,requests:requests.length,images:imageParts.filter(p=>p.type==='image_url').length,
    model:'local HTTP recorder; no semantic inference',output:out};
  save('verification.json',result);console.log(JSON.stringify(result,null,2));
} catch (error) {
  if(page) { await page.screenshot({path:join(out,'failure.png'),fullPage:true}).catch(()=>{}); save('failure.json',{error:String(error),text:(await page.locator('body').innerText()).slice(0,5000),buttons:await page.locator('button').evaluateAll(bs=>bs.map(b=>({text:b.innerText,label:b.getAttribute('aria-label'),title:b.title})))}); }
  throw error;
} finally {
  await browser?.close(); for(const child of children) child.kill(); await new Promise(r=>recorder.close(r));
}
