import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const require=createRequire(path.resolve('packages/web/package.json'));
const {chromium,expect}=require('@playwright/test');
const account=JSON.parse(fs.readFileSync(0,'utf8'));
const origin='https://understandbook.top', output=path.resolve('tmp/workspace-library-release-20261021');
const browser=await chromium.launch({headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
const report={status:'running',errors:[],model_requests:0};
let csrf='',page;
try {
  const login=await context.request.post(origin+'/api/auth/login',{data:account,headers:{Origin:origin}});
  assert.equal(login.status(),200);
  csrf=(await(await context.request.get(origin+'/api/auth/me')).json()).csrf_token;
  page=await context.newPage();
  page.on('pageerror',error=>report.errors.push(error.message));
  page.on('request',request=>{if(request.method()==='POST'&&/\/agent\/runs?(\?|$)/.test(request.url()))report.model_requests++;});
  await page.goto(origin+'/index.html',{waitUntil:'domcontentloaded',timeout:60000});
  await page.getByRole('heading',{name:'学习工作区',exact:false}).waitFor({timeout:60000});
  const shelf=page.getByRole('button',{name:'全部资料',exact:true});
  if(await shelf.count())await shelf.click();
  const video=page.locator('.network-book-card').filter({hasText:'CS336 第 2 讲：PyTorch 与资源核算'});
  await expect(video).toHaveCount(1,{timeout:30000});
  await video.locator('img').evaluate(img=>img.decode());
  await video.scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(output,'public-library.png'),fullPage:true});
  report.video_title=await video.locator('strong').last().innerText();
  report.cover_loaded=true;
  await page.getByRole('button',{name:'浏览共享预构建工作区',exact:true}).click();
  const course=page.locator('.shared-card').filter({hasText:'CS336 第 2 讲：PyTorch 与资源核算'});
  await course.getByRole('button',{name:'查看目标与路线'}).click({timeout:30000});
  const manifest=page.locator('.shared-manifest');
  await expect(manifest).toBeVisible({timeout:30000});
  await expect(manifest).toContainText('CS336 第 2 讲：PyTorch 与资源核算');
  await expect(manifest).not.toContainText('19ac880d');
  await expect(manifest).not.toContainText('external');
  await manifest.screenshot({path:path.join(output,'public-course-route.png')});
  assert.deepEqual(report.errors,[]);assert.equal(report.model_requests,0);
  report.shared_route_uses_readable_title=true;report.status='passed';
} catch(error) {
  report.failure=String(error);
  if(page){report.page_url=page.url();report.page_text=(await page.locator('body').innerText()).slice(0,2000);await page.screenshot({path:path.join(output,'public-library-failure.png'),fullPage:true});}
  throw error;
} finally {
  if(csrf)await context.request.post(origin+'/api/auth/logout',{data:{},headers:{Origin:origin,'X-CSRF-Token':csrf}});
  fs.writeFileSync(path.join(output,'public-library.json'),JSON.stringify(report,null,2));
  await browser.close();console.log(JSON.stringify(report));
}
