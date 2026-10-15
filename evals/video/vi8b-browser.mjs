import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, createWriteStream } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const out = resolve('evals/video/generated/vi8b', new Date().toISOString().replaceAll(':','-'));
mkdirSync(out, { recursive: true });
const save = (name, value) => writeFileSync(join(out,name), JSON.stringify(value,null,2));
const workspace = resolve('evals/video/generated/vi2/moved-workspace');
const link = JSON.parse(readFileSync(join(workspace,'video_publication.json'),'utf8'));
let requests = [], bound, observed = [];
const batches = [
  { start_ms: 555123, end_ms: 615123, sampling: [555123,585000,600000] },
  { start_ms: 615123, end_ms: 651000, sampling: [615123,630000,650000] },
];
const recorder = createServer(async (req,res) => {
  try {
    let raw=''; for await (const chunk of req) raw+=chunk;
    const body=JSON.parse(raw); requests.push(body); save(`request-${requests.length}.json`,body);
    const parts=body.messages.flatMap(m=>Array.isArray(m.content)?m.content:[]);
    for(let i=0;i<parts.length;i++) {
      let metadata; try {metadata=JSON.parse(parts[i].text);} catch {continue;}
      if(metadata?.purpose!=='video_source') continue;
      const image=parts[i+1]; assert.equal(image.type,'image_url');
      const png=Buffer.from(image.image_url.url.split(',')[1],'base64');
      assert.equal(png.readUInt32BE(16),1920); assert.equal(png.readUInt32BE(20),1080);
      observed.push({ ...metadata, png:png.toString('base64') });
      writeFileSync(join(out,`model-frame-${observed.length}.png`),png);
    }
    const call=(name,args)=>({role:'assistant',content:null,tool_calls:[{id:`call-${requests.length}`,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
    let message;
    if(requests.length===1) message=call('tool.search',{task:'video.read_frames 视频画面取帧',required_capabilities:['source_read'],scope:'passage',operation:'read_source',effect_mode:'read_only',max_results:4});
    else if(requests.length===2 || requests.length===3) {
      const {sampling,...range}=batches[requests.length-2];
      message=call('video.read_frames',{location:{video:bound.frame.video,...range},sampling});
    } else if(requests.length===4) {
      assert.equal(observed.length,6);
      message=call('tool.search',{task:'source.present 已观察画面来源',required_capabilities:['source_presentation'],max_results:1});
    } else if(requests.length===5) {
      message=call('source.present',{target:{kind:'visual',publication:bound.publication,frames:[observed[0].read.frame,observed.at(-1).read.frame],regions:[]}});
    } else {
      const receipt=body.messages.filter(m=>m.role==='tool').map(m=>{try{const v=JSON.parse(m.content);return v.model_body??v;}catch{return null;}}).find(r=>r?.source_ref_id);
      assert(receipt,'Missing source receipt');
      message={role:'assistant',content:`RECORDING ONLY: ordered process samples; source retains two supporting frames. [[source:${receipt.source_ref_id}]]`};
    }
    res.writeHead(200,{'Content-Type':'application/json'}); res.end(JSON.stringify({choices:[{message}],usage:{total_tokens:10}}));
  } catch(error) {save('recorder-error.json',{error:String(error)});res.writeHead(500);res.end(String(error));}
});
await new Promise(r=>recorder.listen(0,'127.0.0.1',r));
const env={...process.env,UNDERSTAND_BOOK_ADDR:'127.0.0.1:8794',UNDERSTAND_BOOK_MEMORY_DIR:join(out,'memory'),
  UNDERSTAND_BOOK_PRIVATE_DIR:join(out,'private'),OPENCODE_API_KEY:'vi8b-local-recorder',
  OPENCODE_BASE_URL:`http://127.0.0.1:${recorder.address().port}`,FLUID_LLM_MODEL:'vi8b-recorder',UNDERSTAND_BOOK_PROVIDER:'native'};
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
  const editRange=async(start,end)=> {
    await page.getByRole('button',{name:/^(选择时间范围|编辑时间范围)$/}).click();
    await page.getByRole('spinbutton',{name:'起点（秒）'}).fill(String(start));
    await page.getByRole('spinbutton',{name:'终点（秒）'}).fill(String(end));
    await page.getByRole('button',{name:'保存时间范围',exact:true}).click();
    return await waitDraft(d=>d?.context.range?.start_ms===start*1000 && d.context.range.end_ms===end*1000);
  };
  // A range is independent content; cancelling the sole selection releases it.
  let draft=await editRange(555.123,651);
  assert.equal(draft.message,''); assert.equal(draft.context.region,null);
  await page.getByRole('button',{name:'取消时间范围',exact:true}).click(); await waitDraft(d=>d===null);
  draft=await select(); const frame=draft.context.frame, region=draft.context.region;
  draft=await editRange(550,651);
  assert.deepEqual(draft.context.region,region);
  draft=await editRange(555.123,651);
  assert.deepEqual(draft.context.frame,frame);
  await page.getByRole('button',{name:'取消时间范围',exact:true}).click();
  draft=await waitDraft(d=>d && !d.context.range); assert.deepEqual(draft.context.region,region);
  draft=await editRange(555.123,651);
  await page.getByRole('button',{name:'取消选区',exact:true}).click();
  draft=await waitDraft(d=>d?.context.range && !d.context.region);
  draft=await select(); bound=draft.context;
  assert.deepEqual(bound.publication,link.reference);
  await input.fill('解释 09:15 到 10:51 的 dtype 示例变化，区分前后画面和未读取的过程。');
  await waitDraft(d=>d?.message.includes('dtype'));
  await main.evaluate(v=>{v.currentTime=700;v.muted=true;return v.play();});
  await page.waitForFunction(()=>{const v=document.querySelector('.video-reader video');return !v.seeking&&!v.paused;});
  await input.fill('解释 09:15 到 10:51 的 dtype 示例变化，保留引用帧之间的未读间隙。');
  draft=await waitDraft(d=>d?.message.includes('未读间隙'));
  assert.deepEqual(draft.context,bound);
  assert.equal(await main.evaluate(v=>v.paused),false);
  await page.screenshot({path:join(out,'bound-range-playing-later.png'),fullPage:true});
  await page.reload(); await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await input.waitFor(); await page.waitForFunction(()=>document.querySelector('textarea[data-workspace-input="agent"]')?.value.includes('未读间隙'));
  assert.deepEqual((await getDraft()).context,bound);
  assert((await page.getByLabel('问题绑定位置').innerText()).includes('时间范围 09:15–10:51'));
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
  assert.equal(observed.length,6);
  const times=observed.map(o=>o.read.frame.actual_pts_ms);
  assert(times.every((t,i)=>i===0||t>times[i-1]));
  assert(times.every(t=>t>=bound.range.start_ms && t<bound.range.end_ms));
  assert(observed.every(o=>!o.region && o.continuous_coverage===false));
  assert(observed.every(o=>o.publication.publication_id===bound.publication.publication_id));
  for (const request of requests.slice(2,4)) {
    const projections=request.messages.filter(m=>m.role==='tool').flatMap(m=>{try{const v=JSON.parse(m.content);return [v.model_body??v];}catch{return [];}}).filter(v=>v?.requested_range);
    assert(projections.length>0); assert(projections.at(-1).transcript.length>0);
    assert.equal(projections.at(-1).continuous_coverage,false);
  }
  assert.equal((await getDraft()),null);
  const saved=run.final_view.outcome.answer_view.sources[0]; assert(saved);
  const source=await api('/agent/source.resolve',{turn_id:accepted.turn_id,source_ref_id:saved.source_ref_id});
  save('source.json',source); assert.equal(source.stale,false); assert.deepEqual(source.media.publication,bound.publication);
  assert.deepEqual(source.media.target.frames,[observed[0].read.frame,observed.at(-1).read.frame]);
  assert.equal(source.frames.length,2,'citation should not expand to the whole requested range or all six read frames');
  for(const image of source.frames) {
    const input=observed.find(o=>o.read.frame.actual_pts_ms===image.frame.actual_pts_ms);
    assert.equal(image.url.split(',')[1],input.png,'preview must replay actual input pixels');
  }
  await main.evaluate(v=>{v.pause();v.currentTime=507;v.playbackRate=1.5;});
  await page.waitForFunction(()=>!document.querySelector('.video-reader video')?.seeking);
  await page.locator('.agent-source-button').last().click();
  const preview=page.getByLabel('媒体来源预览'); await preview.waitFor();
  await preview.locator('img').first().waitFor();
  assert((await preview.getByLabel('未连续观察的间隙').innerText()).includes('09:15–10:50'));
  const side=preview.locator('video').first();
  await side.evaluate(v=>{v.muted=true;return v.play();});
  await page.waitForTimeout(300);
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.screenshot({path:join(out,'answer-source-preview.png'),fullPage:true});
  await page.keyboard.press('Escape');
  assert.equal(await main.evaluate(v=>v.currentTime),507);assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.locator('.agent-source-button').last().click();await preview.waitFor();
  await page.getByRole('button',{name:'进入原视频位置'}).click();
  assert(Math.abs(await main.evaluate(v=>v.currentTime)-times[0]/1000)<.01);
  assert.equal(await main.evaluate(v=>v.playbackRate),1.5);
  await page.reload();await page.getByRole('button',{name:'打开视频阅读与提问'}).click();
  await page.locator('.agent-source-button').last().click();await page.getByLabel('媒体来源预览').waitFor();
  assert((await page.getByLabel('媒体来源预览').innerText()).includes('10:50'));
  assert.deepEqual(errors,[]);
  const result={date:new Date().toISOString(),browser:browser.version(),publication:bound.publication,material:bound.material,
    bound_frame:bound.frame,bound_region:bound.region,bound_range:bound.range,batches,actual_pts_ms:times,
    referenced_frames:source.media.target.frames,range_only_cancel_releases:true,region_and_range_independent:true,
    manual_play_and_edits_preserve_binding:true,reload_restores_draft:true,accepted_turn:accepted.turn_id,
    history_preserves_binding:true,source_preview_preserves_main_player:true,explicit_entry_seeks_original:true,
    reloaded_history_source:true,visible_unsampled_gaps:true,requests:requests.length,images:observed.length,
    model:'local HTTP recorder; no semantic inference',output:out};
  save('verification.json',result);console.log(JSON.stringify(result,null,2));
} catch (error) {
  if(page) { await page.screenshot({path:join(out,'failure.png'),fullPage:true}).catch(()=>{}); save('failure.json',{error:String(error),text:(await page.locator('body').innerText()).slice(0,5000),buttons:await page.locator('button').evaluateAll(bs=>bs.map(b=>({text:b.innerText,label:b.getAttribute('aria-label'),title:b.title})))}); }
  throw error;
} finally {
  await browser?.close(); for(const child of children) child.kill(); await new Promise(r=>recorder.close(r));
}
