// ADM9 isolated Rust/SQLite candidate; no external Provider calls.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const base=process.env.ADM9_URL ?? 'https://localhost:18443';
const output=process.env.ADM9_EVIDENCE ?? 'tmp/adm9-candidate';
await fs.mkdir(output,{recursive:true});
const browser=await chromium.launch({headless:true});
const context=await browser.newContext({ignoreHTTPSErrors:true,viewport:{width:1440,height:1000}});
const page=await context.newPage();
page.setDefaultTimeout(15000);
const errors=[],checks=[];
page.on('pageerror',e=>errors.push(String(e)));
const passed=label=>{checks.push(label);console.log(label);};
const money=(v,d=6)=>{const n=BigInt(v),scale=10n**BigInt(d);return `${n/scale}.${(n%scale).toString().padStart(d,'0').replace(/0+$/,'').padEnd(2,'0')}`;};
async function api(path) {const r=await page.request.get(base+path);assert.equal(r.status(),200,path);return r.json();}
async function login(user) {
  await page.getByLabel('邮箱或账号',{exact:true}).fill(user);
  await page.getByLabel('密码',{exact:true}).fill('fixture-only-password');
  await page.getByRole('button',{name:'登录',exact:true}).click();
}
async function filter(from,to,user='') {
  await page.getByLabel('开始日期',{exact:true}).fill(from);
  await page.getByLabel('结束日期',{exact:true}).fill(to);
  if(await page.getByLabel('账号（留空为全部）').count()) await page.getByLabel('账号（留空为全部）').fill(user);
  await page.getByRole('button',{name:'查询期间',exact:true}).click();
  await page.getByRole('region',{name:new RegExp(`所选期间 · ${from} 至 ${to}${user ? ` · ${user}` : ''}$`)}).waitFor();
}
try {
  await page.goto(base+'/admin/usage');
  await login('B');
  await page.getByRole('heading',{name:'全站用量',exact:true}).waitFor();
  await page.reload();
  const result=await api('/api/admin/usage');
  const today=result.today_date;
  assert.equal(result.summary.request_count,43);
  assert.equal(result.summary.unknown_cost_requests,3);
  assert.equal(result.summary.pending_requests,2);
  assert.equal(result.summary.active_requests,1);
  assert.equal(result.summary.active_users,1);
  assert.equal(result.summary.returning_users,1);
  assert.equal(result.summary.completed_runs,0);
  const summary=page.getByRole('region',{name:new RegExp('^所选期间')});
  await summary.waitFor();
  for (const [label,value] of [['已确认模型成本（元）',money(result.summary.confirmed_cost_micro_cny)],['账号扣减（元）',money(result.summary.account_debit_micro_cny)],['待核算占用（元）',money(result.summary.pending_micro_cny)],['收款净额（元）',money(result.summary.receipt_net_fen,2)]]) {
    assert.equal(await summary.locator('dt').filter({hasText:label}).locator('..').locator('dd').innerText(),value);
  }
  assert.match(await summary.innerText(),/3 条成本尚未确认/);
  passed('真实登录与深链接刷新；分列成本、扣减、预留、免扣未知、收款和回访与 API 一致');
  const tasks=page.locator('section').filter({has:page.getByRole('heading',{name:'任务费用 · 按已确认成本排序',exact:true})}).last();
  await tasks.getByRole('button',{name:'下一页',exact:true}).click();
  await tasks.getByText('共 21 条 · 第 2 页',{exact:true}).waitFor();
  assert.equal(await tasks.locator('tbody tr').count(),1);
  await tasks.getByRole('button',{name:'上一页',exact:true}).click();
  await tasks.getByText('共 21 条 · 第 1 页',{exact:true}).waitFor();
  await tasks.locator('tr').filter({hasText:'r00'}).getByRole('button',{name:'调用明细'}).click();
  const details=page.getByRole('region',{name:'任务调用明细'});
  await details.getByText('共 23 条 · 第 1 页',{exact:true}).waitFor();
  await details.getByRole('button',{name:'下一页',exact:true}).click();
  await details.getByText('共 23 条 · 第 2 页',{exact:true}).waitFor();
  assert.equal(await details.locator('tbody tr').count(),3);
  const all=[];
  for(let offset=0;offset<43;offset+=20) all.push(...(await api(`/api/admin/charges?from=${today}&to=${today}&limit=20&offset=${offset}`)).items);
  assert.equal(all.reduce((n,c)=>n+(c.provider_cost_micro_cny??0),0),result.summary.confirmed_cost_micro_cny);
  assert.equal(all.reduce((n,c)=>n+(c.account_debit_micro_cny??0),0),result.summary.account_debit_micro_cny);
  passed('21 项任务与 23 条单任务调用均可分页；跨页逐次费用合计与服务汇总一致');
  await filter(today,today,'B');
  assert.equal(await summary.locator('dt').filter({hasText:'调用记录'}).locator('..').locator('dd').innerText(),'0');
  const yesterday=new Date(Date.parse(`${today}T00:00:00Z`)-86400000).toISOString().slice(0,10);
  await filter(yesterday,yesterday,'A');
  assert.equal(await summary.locator('dt').filter({hasText:'活跃 / 回访账号'}).locator('..').locator('dd').innerText(),'1 / 0');
  await filter(today,today,'A');
  passed('账号隔离筛选、空期间与香港跨日回访展示正确');
  await page.screenshot({path:`${output}/usage-desktop.png`,fullPage:true});
  await page.setViewportSize({width:390,height:844});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth),true);
  assert.ok((await tasks.locator('tbody tr').first().boundingBox()).height < 90, 'task identifier must not wrap one character per line');
  await page.screenshot({path:`${output}/usage-mobile.png`,fullPage:true});
  passed('390px 手机布局可操作，页面无横向溢出');
  await page.goto(base+'/admin/users/A');
  await page.getByRole('heading',{name:'使用与回访',exact:true}).waitFor();
  assert.match(await page.getByRole('heading',{name:'使用与回访',exact:true}).locator('..').innerText(),/2 \/ 1/);
  await page.getByRole('heading',{name:'账号用量汇总',exact:true}).waitFor();
  await page.goto(base+'/admin/users');
  const row=page.locator('tr').filter({has:page.getByRole('link',{name:'A',exact:true})});
  assert.match(await row.innerText(),/阅读：/);
  assert.doesNotMatch(await row.innerText(),/暂无记录/);
  passed('账号列表最近阅读/提问与详情活跃、回访、单人汇总接通');
  await page.getByRole('button',{name:'退出登录',exact:true}).click();
  await login('A');
  await page.getByRole('heading',{name:'无管理权限',exact:true}).waitFor();
  assert.equal((await page.request.get(base+'/api/admin/usage')).status(),403);
  assert.equal(await page.getByRole('region',{name:/所选期间/}).count(),0);
  passed('切换普通账号清除用量页面，管理汇总接口返回 403');
  assert.deepEqual(errors,[]);
  await fs.writeFile(`${output}/result.json`,JSON.stringify({checks,errors},null,2));
} catch(e) {await page.screenshot({path:`${output}/failure.png`,fullPage:true});throw e;}
finally {await browser.close();}
