// INV5–INV6 browser interactions; real Host/SQLite/mail behavior is covered by Rust inv5_/inv6_.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const readerUrl = process.env.INV_READER_URL ?? 'http://127.0.0.1:18943';
const adminUrl = process.env.INV_ADMIN_URL ?? 'http://127.0.0.1:18944';
const output = process.env.INV_EVIDENCE ?? 'tmp/inv56-browser';
await fs.mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 390, height: 844 } });
const page = await context.newPage();
const errors = [], checks = [];
let signedIn = false, email = null, starts = 0, resend = 0;
const identity = () => ({ user_id: 'legacy', email, csrf_token: 'csrf', capabilities: { admin: true, presentation: { authoring: true, reason: '' } } });
const observe = p => { p.setDefaultTimeout(15000); p.on('pageerror', e => errors.push(String(e))); };
observe(page);
await context.route('**/api/**', async route => {
  const req = route.request(), path = new URL(req.url()).pathname;
  const reply = (json, status = 200) => route.fulfill({ status, json });
  if (path === '/api/auth/login') {
    assert(['legacy',' Reader@Example.COM '].includes(req.postDataJSON().username));
    signedIn = true; return reply(identity());
  }
  if (!signedIn) return reply({},401);
  if (path === '/api/auth/me') return reply(identity());
  if (path === '/api/library') return reply({books:[]});
  if (path === '/api/auth/logout') { signedIn=false; return reply({ok:true}); }
  assert.equal(req.headers()['x-csrf-token'],'csrf');
  const pending = {request_id:'binding',expires_at:Math.floor(Date.now()/1000)+900,resend_after:1};
  if (path.endsWith('/email/start')) {
    starts++;
    const body=req.postDataJSON(); assert.equal(body.email,'Reader@Example.com');
    if (body.current_password === 'wrong') return reply({error_code:'CURRENT_PASSWORD_INVALID'},400);
    assert.equal(body.current_password,'fixture-only-password');
    return reply({...pending,error_code:'ACCOUNT_MAIL_TIMEOUT'},503);
  }
  if (path.endsWith('/email/resend')) { resend++; assert.equal(req.postDataJSON().request_id,'binding'); return reply(pending); }
  if (path.endsWith('/email/complete')) {
    assert.equal(req.postDataJSON().request_id,'binding');
    if (req.postDataJSON().verification_code !== '123456') return reply({error_code:'EMAIL_BINDING_CODE_INVALID'},400);
    email='reader@example.com'; return reply({user_id:'legacy',email,completed:true});
  }
  throw new Error(path);
});
async function login(p, name) {
  await p.getByLabel('邮箱或账号',{exact:true}).fill(name);
  await p.getByLabel('密码',{exact:true}).fill('fixture-only-password');
  await p.getByRole('button',{name:'登录',exact:true}).click();
}
try {
  await page.goto(readerUrl); await login(page,'legacy');
  await page.getByRole('heading',{name:'选择阅读材料'}).waitFor();
  const other=await context.newPage(); observe(other); await other.goto(readerUrl);
  await other.getByRole('heading',{name:'选择阅读材料'}).waitFor();
  await page.getByRole('button',{name:'个人设置',exact:true}).click();
  const dialog=page.getByRole('dialog');
  await dialog.getByLabel('邮箱',{exact:true}).fill('Reader@Example.com');
  await dialog.getByLabel('当前密码',{exact:true}).fill('wrong');
  await dialog.getByRole('button',{name:'发送验证码'}).click();
  await dialog.getByRole('alert').filter({hasText:'当前密码不正确'}).waitFor();
  assert.equal(await dialog.getByLabel('当前密码').inputValue(),'');
  await dialog.getByLabel('当前密码').fill('fixture-only-password');
  await dialog.getByRole('button',{name:'发送验证码'}).click();
  await dialog.getByRole('alert').filter({hasText:'可能已送达'}).waitFor();
  assert.equal(await dialog.locator('input[type=password]').count(),0);
  await page.screenshot({path:`${output}/binding-mobile.png`,fullPage:true});
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  assert(await dialog.evaluate(d => d.scrollWidth <= d.clientWidth));
  await dialog.getByRole('button',{name:'重发验证码',exact:true}).click();
  await dialog.getByRole('status').filter({hasText:'验证码已发送'}).waitFor();
  await dialog.getByLabel('邮箱验证码').fill('000000');
  await dialog.getByRole('button',{name:'确认绑定'}).click();
  await dialog.getByRole('alert').filter({hasText:'验证码不正确'}).waitFor();
  await dialog.getByLabel('邮箱验证码').fill('123456');
  await dialog.getByRole('button',{name:'确认绑定'}).click();
  await dialog.getByText('已验证邮箱：reader@example.com',{exact:true}).waitFor();
  await other.locator('.network-library-head').getByText('reader@example.com',{exact:false}).waitFor();
  assert.equal(starts,2); assert.equal(resend,1);
  assert(!(await page.evaluate(() => JSON.stringify([localStorage,sessionStorage]))).includes('fixture-only-password'));
  checks.push('390px: empty library settings, wrong password, timeout recovery, resend, wrong code, binding and cross-tab email refresh');
  await dialog.getByRole('button',{name:'关闭',exact:true}).click();
  const logoutBox=await page.getByRole('button',{name:'退出登录',exact:true}).boundingBox();
  assert(logoutBox.x >= 0 && logoutBox.x + logoutBox.width <= 390);
  await page.screenshot({path:`${output}/library-mobile.png`,fullPage:true});
  await page.getByRole('button',{name:'个人设置',exact:true}).click();
  await page.setViewportSize({width:1440,height:900}); await page.screenshot({path:`${output}/binding-desktop.png`,fullPage:true});
  await dialog.getByRole('button',{name:'关闭',exact:true}).click();
  await page.getByRole('button',{name:'退出登录',exact:true}).click();
  await login(page,' Reader@Example.COM '); await page.getByRole('heading',{name:'选择阅读材料'}).waitFor();
  await page.getByRole('button',{name:'个人设置',exact:true}).click();
  assert.equal(await page.getByRole('dialog').getByText('账号：legacy',{exact:true}).count(),1);
  assert.equal(await page.getByRole('dialog').locator('form').count(),0);
  checks.push('Email sign-in keeps legacy identity; bound account has no replacement form');
  const adminContext=await browser.newContext({viewport:{width:1440,height:900}});
  const admin=await adminContext.newPage(); observe(admin); let adminSignedIn=false, searched=false;
  await admin.route('**/api/**',async route=>{
    const req=route.request(),url=new URL(req.url()),path=url.pathname;
    if(path==='/api/auth/login') { assert.equal(req.postDataJSON().username,'reader@example.com'); adminSignedIn=true; return route.fulfill({json:identity()}); }
    if(!adminSignedIn) return route.fulfill({status:401,json:{}});
    if(path==='/api/auth/me') return route.fulfill({json:identity()});
    if(path==='/api/admin/users') {
      searched ||= url.searchParams.get('search')==='reader@example.com';
      return route.fulfill({json:{users:[{user_id:'legacy',email,disabled:false,is_admin:true,current_allowance:null,activity:{last_read_at:null,last_question_at:null}}],total:1}});
    }
    return route.fulfill({status:404,json:{}});
  });
  await admin.goto(`${adminUrl}/admin/users`); await login(admin,'reader@example.com');
  await admin.getByRole('heading',{name:'账号管理',exact:true}).waitFor();
  await admin.getByLabel('搜索账号或邮箱',{exact:true}).fill('reader@example.com');
  await Promise.all([admin.waitForResponse(r => r.url().includes('search=reader%40example.com')), admin.getByRole('button',{name:'搜索',exact:true}).click()]);
  assert(searched); assert.equal(await admin.getByRole('cell',{name:'reader@example.com',exact:true}).count(),1);
  assert.equal(await admin.getByRole('link',{name:'legacy',exact:true}).getAttribute('href'),'/admin/users/legacy');
  await admin.screenshot({path:`${output}/admin-email.png`,fullPage:true});
  checks.push('Admin email login, display and search keep management links keyed by legacy user ID');
  assert.deepEqual(errors,[]);
  await fs.writeFile(`${output}/result.json`,JSON.stringify({status:'passed',checks,errors},null,2));
  console.log(checks.join('\n'));
} catch(e) { await page.screenshot({path:`${output}/failure.png`,fullPage:true}); throw e; }
finally { await browser.close(); }
