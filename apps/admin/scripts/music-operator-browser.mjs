// Actual Admin rendering with controlled HTTP state; this is UI evidence, not an Engine/model evaluation.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const requireWeb = createRequire(path.join(root, 'packages/web/package.json'));
const { chromium } = requireWeb('@playwright/test');
const output = path.join(root, 'docs/performance/music-operator-20261022');
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width:1440,height:1000 } });
const errors=[],writes=[];
page.on('pageerror', error => errors.push(error.message));
let task;
const reply = async (route, json) => route.fulfill({ json });
await page.route('**/api/**', async route => {
  const req=route.request(),url=new URL(req.url()),body=req.method()==='POST'?req.postDataJSON():null;
  if(body)writes.push({path:url.pathname,body});
  if(url.pathname==='/api/auth/me')return reply(route,{user_id:'operator',csrf_token:'fixture',capabilities:{admin:true}});
  if(url.pathname==='/api/admin/books')return reply(route,{books:[],total:0});
  if(url.pathname==='/api/admin/operator-builds'&&body){
    task={...body,state:'draft',revision:0,created_at:1792641600,updated_at:1792641600,pause_requested:false,
      materials:body.materials.map(m=>({...m,state:'pending'}))};
  }else if(url.pathname.endsWith('/start')){
    task={...task,state:'running',revision:1,budget:{max_micro_cny:task.max_micro_cny,provider_cost_micro_cny:0,
      estimated_cost_micro_cny:0,pending_micro_cny:0,reserved_micro_cny:0,remaining_micro_cny:task.max_micro_cny,unknown_count:0},
      materials:task.materials.map(m=>({...m,state:'running',observation:{progress:{stage:'music_score',completed:2,total:4,unit:'tasks',message:'生成可定位的谱面，共 4 页'}}}))};
  }
  if(body)return reply(route,{operation_id:body.operation_id,user_id:'operator',actor:'operator',kind:'operator-builds',created_at:1792641600,ok:true,parameters:body,operator_build:task});
  if(url.pathname==='/api/admin/operator-builds')return reply(route,{items:task?[task]:[],total:task?1:0});
  if(url.pathname.startsWith('/api/admin/operator-builds/'))return reply(route,task);
  if(url.pathname==='/api/admin/charges')return reply(route,{items:[],total:0});
  throw new Error(`Unhandled fixture request ${url.pathname}`);
});
try{
  await page.goto('http://127.0.0.1:4186/admin/operator-builds');
  await page.getByRole('button',{name:'创建准备任务',exact:true}).click();
  const dialog=page.getByRole('dialog');
  await dialog.getByLabel('准备任务名称',{exact:true}).fill('舒伯特 · 音乐瞬间第 3 首');
  await dialog.getByLabel('平台预算上限（元）',{exact:true}).fill('1');
  await dialog.getByLabel('每次执行预计占用（元）',{exact:true}).fill('0.01');
  await dialog.getByLabel('成果授权账号（空格或逗号分隔）',{exact:true}).fill('A B');
  await dialog.getByLabel('资料类型',{exact:true}).selectOption('music');
  await dialog.getByLabel('资料名称',{exact:true}).fill('舒伯特《音乐瞬间》第 3 首：精选演奏与乐谱');
  await dialog.getByLabel('来源文件或工作区位置',{exact:true}).fill('E:/music/schubert/curation.json');
  await dialog.getByLabel('成果根目录',{exact:true}).fill('E:/music/builds');
  await page.screenshot({path:path.join(output,'music-create-desktop.png'),fullPage:true});
  await page.setViewportSize({width:390,height:844});
  assert(await dialog.evaluate(el=>el.scrollWidth<=el.clientWidth+1),'create dialog should fit narrow viewport');
  await page.screenshot({path:path.join(output,'music-create-mobile.png'),fullPage:true});
  await dialog.getByRole('button',{name:'创建并开始构建',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  const progress=page.getByRole('status',{name:'舒伯特《音乐瞬间》第 3 首：精选演奏与乐谱构建进度',exact:true});
  await progress.getByRole('progressbar').waitFor();
  assert((await progress.innerText()).includes('2 / 4 项（50%）'));
  assert.equal(writes.length,2);assert.equal(writes[0].body.materials[0].material_kind,'music');
  assert.deepEqual(writes[0].body.materials[0].required_capabilities,['music','music_score','music_events']);
  await page.getByRole('region',{name:'运营构建任务详情',exact:true}).scrollIntoViewIfNeeded();
  assert(await page.locator('body').evaluate(el=>el.scrollWidth<=window.innerWidth),'progress should fit narrow viewport');
  await page.screenshot({path:path.join(output,'music-progress-mobile.png'),fullPage:true});
  await page.setViewportSize({width:1440,height:1000});
  await page.screenshot({path:path.join(output,'music-progress-desktop.png'),fullPage:true});
  assert.deepEqual(errors,[]);
  writeFileSync(path.join(output,'browser-verification.json'),JSON.stringify({status:'passed',fixture:'controlled HTTP observations; actual Admin renderer',create_requests:1,start_requests:1,capabilities:writes[0].body.materials[0].required_capabilities,progress:{completed:2,total:4},viewports:[1440,390],errors},null,2));
  console.log(JSON.stringify({status:'passed',output}));
}finally{await browser.close();}
