import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {chromium} from '@playwright/test';
const accounts=JSON.parse(fs.readFileSync(0,'utf8'));
const out=path.resolve('docs/performance/cloud-learning-20261020/release-20261021');fs.mkdirSync(out,{recursive:true});
const base='https://understandbook.top';
const report={status:'running',origin:base,checked_at:new Date().toISOString(),checks:[],errors:[],httpFailures:[],model_requests:0};
const browser=await chromium.launch({channel:'msedge',headless:true});
function observe(page){
 page.on('pageerror',e=>report.errors.push(e.message));
 page.on('response',r=>{if(r.status()>=400&&!r.url().endsWith('/auth/me'))report.httpFailures.push({url:r.url().split('?')[0],status:r.status()});});
 page.on('request',r=>{if(r.method()==='POST'&&/\/agent\/(?:runs|run)(?:\?|$)/.test(r.url()))report.model_requests++;});
}
async function login(username){
 const context=await browser.newContext({viewport:{width:1440,height:900},reducedMotion:'reduce'});
 const response=await context.request.post(base+'/api/auth/login',{data:{username,password:accounts[username]},headers:{Origin:base}});assert.equal(response.status(),200);
 const page=await context.newPage();observe(page);return {context,page};
}
async function logout(context){
 const me=await context.request.get(base+'/api/auth/me');
 if(me.ok())await context.request.post(base+'/api/auth/logout',{data:{},headers:{Origin:base,'X-CSRF-Token':(await me.json()).csrf_token}});
 await context.close();
}
try{
 const anonymous=await browser.newContext({viewport:{width:390,height:844}});const home=await anonymous.newPage();observe(home);
 await home.goto(base+'/home.html',{waitUntil:'domcontentloaded',timeout:60000});await home.locator('.home-nav-actions').waitFor({timeout:60000});
 assert.equal(await home.locator('.home-nav-actions a[download]').getAttribute('href'),'/downloads/UnderstandBookSetup.exe');
 assert.ok(await home.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1));
 await home.screenshot({path:path.join(out,'homepage-mobile.png'),fullPage:true});
 await anonymous.close();report.checks.push('Mobile homepage and Windows download entry');
 const {context,page}=await login('reader');
 try{
  await page.goto(base+'/',{waitUntil:'domcontentloaded',timeout:60000});await page.waitForURL('**/index.html',{waitUntil:'domcontentloaded',timeout:60000});
  await page.getByRole('heading',{name:'学习工作区',exact:true}).waitFor({timeout:60000});
  report.checks.push('Authenticated root opens cloud Reader');
  await page.getByRole('button',{name:'使用额度',exact:true}).click();
  await page.getByRole('button',{name:'套餐与权益 · 月付 / 冲刺',exact:true}).click();
  await page.getByRole('heading',{name:'云端学习套餐',exact:true}).waitFor();
  await page.getByText('该时长的套餐尚未开放购买。',{exact:true}).waitFor();
  await page.getByRole('heading',{name:'个人云端空间',exact:true}).waitFor();
  assert.equal(await page.locator('.cloud-plans [role="alert"]').count(),0);
  await page.screenshot({path:path.join(out,'reader-cloud-plans.png'),fullPage:true});
  report.checks.push('Plans, continuous-day durations and storage render without seeded products');
  await page.keyboard.press('Escape');
  await page.getByRole('button',{name:'浏览共享预构建工作区',exact:true}).click();
  await page.locator('.shared-catalog[aria-busy="false"]').waitFor();
  assert.equal(await page.locator('.shared-catalog [role="alert"]').count(),0);
  report.checks.push('Course catalog loads existing authorized releases');
  const csrf=(await (await context.request.get(base+'/api/auth/me')).json()).csrf_token;
  const verifier=crypto.randomBytes(32).toString('base64url'),state=crypto.randomBytes(32).toString('base64url');
  const redirect='http://127.0.0.1:49521/callback';
  const authorization=await context.request.post(base+'/api/auth/desktop/authorize',{headers:{Origin:base,'X-CSRF-Token':csrf},data:{redirect_uri:redirect,code_challenge:crypto.createHash('sha256').update(verifier).digest('base64url'),state}});
  assert.equal(authorization.status(),200);const url=new URL((await authorization.json()).redirect_url);assert.equal(url.searchParams.get('state'),state);
  const desktop=await browser.newContext();
  try{
   const exchange=await desktop.request.post(base+'/api/auth/desktop/exchange',{headers:{Origin:base},data:{code:url.searchParams.get('code'),code_verifier:verifier,redirect_uri:redirect}});
   assert.equal(exchange.status(),200);const session=await exchange.json();
   await desktop.addCookies([{name:'__Host-ub_session',value:session.session_token,url:base,httpOnly:true,secure:true,sameSite:'Strict'}]);
   const identity=await desktop.request.get(base+'/api/auth/me');assert.equal(identity.status(),200);assert.equal((await identity.json()).user_id,'reader');
   await logout(desktop);
   assert.equal((await context.request.get(base+'/api/auth/me')).status(),200);
  }catch(e){await desktop.close();throw e;}
  report.checks.push('Production PKCE exchange, same account, independent desktop logout');
  const workspaces=await (await context.request.get(base+'/api/learning-workspaces')).json();
  for(const workspace of workspaces.workspaces){const sprint=await context.request.get(base+'/api/learning-workspaces/'+encodeURIComponent(workspace.learning_workspace_id)+'/exam-sprint');assert.equal(sprint.status(),200);}
  report.existing_learning_workspaces=workspaces.workspaces.length;
  if(workspaces.workspaces.length)report.checks.push('Existing workspaces expose exam settings without rewriting goals');
 }finally{await logout(context);}
 const admin=await login('admin');
 try{
  await admin.page.goto(base+'/admin/users/reader',{waitUntil:'domcontentloaded',timeout:60000});
  await admin.page.getByRole('heading',{name:'云端套餐与续期',exact:true}).waitFor({timeout:60000});
  await admin.page.getByText('套餐商品与开通（所有账号共用商品配置）',{exact:true}).click();
  await admin.page.getByText('尚未配置套餐商品。',{exact:true}).waitFor();
  await admin.page.screenshot({path:path.join(out,'admin-cloud-plans.png'),fullPage:true});
  report.checks.push('Admin product configuration and account entitlements');
  await admin.page.goto(base+'/admin/workspace-releases',{waitUntil:'domcontentloaded',timeout:60000});
  await admin.page.getByRole('heading',{name:'共享工作区发布',exact:true}).waitFor();
  report.checks.push('Admin course release management');
 }finally{await logout(admin.context);}
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.httpFailures,[]);assert.equal(report.model_requests,0);report.status='passed';
}catch(e){report.status='failed';report.failure=String(e);throw e;}
finally{fs.writeFileSync(path.join(out,'public-browser.json'),JSON.stringify(report,null,2));await browser.close();console.log(JSON.stringify(report,null,2));}
