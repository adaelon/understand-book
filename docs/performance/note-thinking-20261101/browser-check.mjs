import { chromium, expect } from '../../../packages/web/node_modules/@playwright/test/index.mjs';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const output=path.dirname(fileURLToPath(import.meta.url));
const fixture=JSON.parse(fs.readFileSync(path.join(output,'browser-fixture.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const calls=[],errors=[],missing=[];
async function setup(width,height,mode='ready'){
  const page=await browser.newPage({viewport:{width,height},deviceScaleFactor:1,locale:'zh-CN'});
  let corrected=false;
  page.on('pageerror',error=>errors.push(String(error)));
  await page.route('**/__thinking_fixture',route=>route.fulfill({json:fixture}));
  await page.route('**/api/**',async route=>{
    const url=new URL(route.request().url()),action=url.pathname.split('/').at(-1),body=route.request().postDataJSON()??{};
    calls.push({path:url.pathname,body});
    const thinking=structuredClone(corrected?fixture.corrected:fixture.thinking);
    if(mode==='empty')thinking.current_understanding=[];
    if(mode==='pending')thinking.update_status='pending';
    if(mode==='failed'){thinking.update_status='failed';thinking.updates=[{job_ref:'failed-job',workspace_id:'scope-a',status:'failed'}];}
    let result;
    if(url.pathname.includes('/me/notes/')){
      if(action==='list')result={notes:[fixture.note],next_cursor:null};
      if(action==='read')result=body.revision?{...fixture.note,value:fixture.history.revisions.find(r=>r.revision===body.revision)}:fixture.note;
      if(action==='history')result=fixture.history;
      if(action==='links')result={links:[]};
      if(action==='attachments')result={items:[]};
      if(action==='board_locations')result={items:[]};
      if(action==='thinking')result=thinking;
      if(action==='thinking_evidence')result=fixture.evidence;
      if(action==='thinking_correct'){corrected=true;result={evidence_ref:fixture.corrected.current_understanding[0].evidence_ref};}
      if(action==='thinking_start')result=fixture.action;
      if(action==='thinking_retry'){mode='pending';result={status:'pending'};}
    }
    if(url.pathname.endsWith('/tutor/understanding'))result={rows:[],interpretations:[],current_understanding:thinking.current_understanding,next:null,evidence_watermark:1,projection_watermark:1};
    if(url.pathname.endsWith('/tutor/evidence'))result=fixture.evidence;
    if(result===undefined){missing.push(url.pathname);return route.fulfill({status:404,json:{message:'Unknown acceptance fixture route'}});}
    return route.fulfill({json:result});
  });
  return page;
}
try{
  const desktop=await setup(1366,960);
  await desktop.goto('http://127.0.0.1:5182/understanding-preview.html');
  await expect(desktop.getByText('仍把停止移动看成行程计时结束')).toBeVisible();
  await desktop.screenshot({path:path.join(output,'desktop-current.png')});
  await desktop.getByRole('button',{name:'展开想法窗口',exact:true}).click();
  await desktop.getByRole('button',{name:'思考过程',exact:true}).click();
  await expect(desktop.getByRole('heading',{name:'思考路径',exact:true})).toBeVisible();
  await desktop.screenshot({path:path.join(output,'desktop-path.png')});
  await desktop.getByRole('button',{name:'这条想法 · 第 1 版',exact:true}).click();
  await expect(desktop.getByRole('heading',{name:'第 1 版 · 只读',exact:true})).toBeVisible();
  await desktop.getByRole('button',{name:'对照当前版',exact:true}).click();
  await expect(desktop.locator('.note-diff')).toContainText(fixture.history.revisions.find(r=>r.revision===1).record.content);
  await desktop.screenshot({path:path.join(output,'desktop-versions.png')});
  await desktop.getByRole('button',{name:'纠正判断',exact:true}).click();
  await desktop.getByRole('textbox',{name:'补充你的实际情况'}).fill('我是在求证，还没有确定停车不算。请保留为待确认。');
  await desktop.getByRole('button',{name:'保存补充',exact:true}).click();
  await expect(desktop.getByText('原判断已撤回，依据用户补充重新确认')).toBeVisible();
  await desktop.locator('.thought-detail').evaluate(el=>el.scrollTop=0);
  await desktop.screenshot({path:path.join(output,'desktop-corrected.png')});
  await desktop.getByRole('button',{name:'当前表述',exact:true}).click();
  await expect(desktop.getByRole('button',{name:'当前表述',exact:true})).toHaveAttribute('aria-pressed','true');
  await expect(desktop.locator('.thought-prose')).toHaveText(fixture.note.value.record.content);
  const mobile=await setup(360,800);
  await mobile.goto('http://127.0.0.1:5182/understanding-preview.html');
  await mobile.getByRole('button',{name:'展开想法窗口',exact:true}).click();
  await mobile.getByRole('button',{name:'思考过程',exact:true}).click();
  await expect(mobile.getByRole('heading',{name:'思考路径',exact:true})).toBeAttached();
  await mobile.screenshot({path:path.join(output,'mobile-path.png')});
  await mobile.locator('.thinking-path-heading').scrollIntoViewIfNeeded();
  await mobile.screenshot({path:path.join(output,'mobile-timeline.png')});
  const overflow=await mobile.evaluate(()=>document.documentElement.scrollWidth>innerWidth);
  if(overflow)throw new Error('Mobile page has horizontal overflow');
  await mobile.getByRole('button',{name:'练一练',exact:true}).click();
  const pending=await mobile.evaluate(async()=>{const m=await import('/src/note-thinking.ts');return m.pendingUnderstandingAction.value;});
  expect(pending).toMatchObject({...fixture.action,mode:'practice'});
  for(const mode of ['empty','pending','failed']){
    const page=await setup(900,850,mode);await page.goto('http://127.0.0.1:5182/understanding-preview.html');
    await expect(page.locator('.note-thinking')).toBeVisible();
    await expect(page.getByText(mode==='empty'?'还没有相关理解观察。':mode==='pending'?'新表达正在整理，先保留上一次判断。':'新表达已保留，理解说明尚未整理完成。',{exact:false})).toBeVisible();
    if(mode==='failed'){await page.getByRole('button',{name:'重试整理'}).click();await expect(page.getByText('新表达正在整理，先保留上一次判断。')).toBeVisible();}
    await page.screenshot({path:path.join(output,`${mode}.png`)});await page.close();
  }
  const overview=await setup(1000,900);await overview.goto('http://127.0.0.1:5182/understanding-preview.html?overview');
  await expect(overview.getByText('仍把停止移动看成行程计时结束')).toBeVisible();await overview.screenshot({path:path.join(output,'overview.png')});
  await overview.getByRole('button',{name:'想法 1 · 第 1 版',exact:true}).click();
  await expect(overview.getByRole('heading',{name:'第 1 版 · 只读',exact:true})).toBeAttached();
  if(errors.length||missing.length)throw new Error(JSON.stringify({errors,missing}));
  fs.writeFileSync(path.join(output,'browser-report.json'),JSON.stringify({status:'passed',viewport:[1366,960,360,800],checks:['current body unchanged','exact version + diff','click-to-submit correction','original scope continuation','empty/pending/retry','overview reverse note navigation','no page errors','no horizontal overflow'],calls,errors,missing},null,2));
}finally{await browser.close();}
