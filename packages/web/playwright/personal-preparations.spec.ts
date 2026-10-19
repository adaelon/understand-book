import {test,expect} from '@playwright/test';
import {mkdirSync,writeFileSync} from 'node:fs';
import path from 'node:path';
// Reader polling can still be in flight when the test closes its page.
// Only teardown ignores unfinished forwarding; active-test errors still fail the run.
test.afterEach(async({context})=>{await context.unrouteAll({behavior:'ignoreErrors'});});
test('personal upload, percent confirmation, original workspace continuation and mobile layout',async({page,context})=>{
  test.setTimeout(120000);page.setDefaultTimeout(15000);
  const fixture=await(await page.request.get('http://127.0.0.1:4191/fixture')).json();
  const writes:any[]=[],errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await context.route(/\/api\/|\/books\//,async route=>{const req=route.request(),url=new URL(req.url());if(req.method()==='POST')writes.push({path:url.pathname,body:req.postDataJSON()});const response=await route.fetch({url:`http://127.0.0.1:4190${url.pathname}${url.search}`,headers:{...await req.allHeaders(),host:'127.0.0.1:4190',origin:fixture.origin,cookie:fixture.reader_cookie}});await route.fulfill({response});});
  const api=async(route:string,body?:any,admin=false)=>{const r=await page.request.fetch(`http://127.0.0.1:4190/api${route}`,{method:body===undefined?'GET':'POST',headers:{origin:fixture.origin,cookie:admin?fixture.admin_cookie:fixture.reader_cookie,'X-CSRF-Token':admin?fixture.admin_csrf:fixture.reader_csrf},...(body===undefined?{}:{data:body})});expect(r.ok(),`${route}: ${await r.text()}`).toBeTruthy();return r.json();};
  const suffix=Date.now(),goal=`个人资料准备 ${suffix}`,otherGoal=`切换后工作区 ${suffix}`;
  await page.goto('/');await page.getByLabel('这次想学会什么？').fill(goal);await page.getByRole('button',{name:'创建工作区',exact:true}).click();
  const detail=page.getByRole('region',{name:'当前工作区',exact:true});await expect(detail).toContainText(goal);
  const panel=detail.getByRole('region',{name:'个人资料准备',exact:true});
  const content=Buffer.from('# Transactions\n\n'+ 'Transactions group operations into one atomic change.\n'.repeat(1500));
  await panel.getByLabel('上传个人资料').setInputFiles({name:'transactions.md',mimeType:'text/markdown',buffer:content});await expect(panel).toContainText('等待准备');
  const creation=writes.find(v=>v.body?.action==='create'&&v.body?.upload_id);expect(creation).toBeTruthy();
  const uploadWrites=writes.filter(v=>v.body?.action==='upload');expect(uploadWrites.length).toBeGreaterThan(1);
  const workspaceId=creation.path.split('/')[3],id=creation.body.task_id,endpoint=`/admin/operator-builds/${id}`;
  await panel.getByRole('button',{name:'生成准备方案',exact:true}).click();
  await api('/admin/operator-builds/next',{host_session_id:`ui-${suffix}`},true);
  const source=await api(`/admin/books`,undefined,true);const book=source.books.find((b:any)=>b.published_book_ref.book_id===fixture.first.book_id);
  // Controlled host progress tests the live UI; semantic/native execution is covered by personal-http.acceptance.ts.
  const directory=`${fixture.root}/library/published/${fixture.first.book_id}/${fixture.first.publication_id}`;
  await api(`${endpoint}/materials/source/progress`,{host_session_id:`ui-${suffix}`,state:'planned',workspace_dir:directory,plan:{build_plan_path:`${directory}/plan.json`,workspace_dir:directory,plan:{private_artifacts:[],estimate:{input_tokens:{lower:20,upper:100},output_tokens:{lower:10,upper:50},unknown_stages:['structure']}},model_runtime:{provider:'probe',model:'scripted',max_output_tokens:100,context_window_tokens:1000},review_markdown:'原文与基础理解'}},true);
  await api(`${endpoint}/finish`,{host_session_id:`ui-${suffix}`,state:'awaiting_confirmation'},true);
  await expect(panel).toContainText('请确认额度上限');await expect(panel).toContainText('有效至');await expect(panel).toContainText('已能估算部分');
  await panel.getByLabel('transactions.md额度上限').fill('1.25');await panel.getByRole('button',{name:'确认上限并开始',exact:true}).click();await expect(panel).toContainText('等待开始');
  const start=writes.find(v=>v.body?.action==='start'&&v.body.task_id===id);expect(start.body.max_milli_percent).toBe(1250);expect(start.body.period_id).toBe(fixture.period_id);
  await panel.getByRole('button',{name:'暂停准备',exact:true}).click();await expect(panel).toContainText('已暂停');
  await page.getByLabel('这次想学会什么？').fill(otherGoal);await page.getByRole('button',{name:'创建工作区',exact:true}).click();await expect(detail).toContainText(otherGoal);await expect(detail).not.toContainText('transactions.md');
  await page.locator('.learning-space-card').filter({hasText:goal}).getByRole('button',{name:'管理工作区',exact:true}).click();await expect(panel).toContainText('已暂停');
  await panel.getByRole('button',{name:'确认当前额度并继续',exact:true}).click();await expect(panel).toContainText('等待开始');
  await api('/admin/operator-builds/next',{host_session_id:`ui-${suffix}`},true);
  await api(`${endpoint}/materials/source/progress`,{host_session_id:`ui-${suffix}`,state:'complete',workspace_dir:directory,invocation_ref:'controlled-ui-preparation'},true);
  await api(`${endpoint}/materials/source/import`,{host_session_id:`ui-${suffix}`,workspace_dir:directory,invocation_ref:'controlled-ui-preparation'},true);
  await api(`${endpoint}/finish`,{host_session_id:`ui-${suffix}`,state:'ready'},true);
  await expect(panel).toContainText('已加入原工作区');
  const origin=await api(`/learning-workspaces/${workspaceId}`);expect(origin.members).toHaveLength(1);expect(origin.goal.text).toBe(goal);
  const other=(await api('/learning-workspaces')).workspaces.find((w:any)=>w.goal.text===otherGoal);expect(other.members).toHaveLength(0);
  const output=path.resolve('../../docs/performance/b2-v1-20261018');mkdirSync(output,{recursive:true});
  await page.setViewportSize({width:1440,height:1000});await panel.screenshot({path:path.join(output,'personal-desktop.png')});
  await page.setViewportSize({width:390,height:844});await panel.scrollIntoViewIfNeeded();expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);await page.screenshot({path:path.join(output,'personal-mobile.png'),fullPage:true});
  await panel.getByRole('button',{name:'继续学习',exact:true}).click();await expect(page.locator('.network-library')).not.toBeVisible();
  expect(errors).toEqual([]);expect(book).toBeTruthy();writeFileSync(path.join(output,'browser.json'),JSON.stringify({scenario:'personal-preparation',upload_chunks:uploadWrites.length,bytes:content.length,confirmed_milli_percent:start.body.max_milli_percent,workspace_preserved:true,mobile_overflow:false,page_errors:errors,semantic_progress:'controlled-host; actual execution covered separately'},null,2));
});
