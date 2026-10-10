// INV8 browser contracts; credential transactions are exercised by Rust inv7_.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
const reader = process.env.INV_READER_URL ?? 'http://127.0.0.1:18943';
const admin = process.env.INV_ADMIN_URL ?? 'http://127.0.0.1:18944';
const output = process.env.INV_EVIDENCE ?? 'tmp/inv78-browser';
await fs.mkdir(output,{recursive:true});
const browser=await chromium.launch({headless:true});
const context=await browser.newContext({viewport:{width:390,height:844}});
const errors=[],checks=[];
let signed=false, password='fixture-password', starts=0, resets=0, changes=0;
const identity=()=>({user_id:'original',email:'reader@example.com',csrf_token:'csrf',capabilities:{admin:true,presentation:{authoring:true,reason:''}}});
// Serve the admin dev assets under Reader's origin to exercise the actual shared channel.
await context.route('**/admin/**',async route=>{
  const url=new URL(route.request().url());
  const response=await route.fetch({url:admin+url.pathname+url.search});
  await route.fulfill({response});
});
await context.route('**/api/**',async route=>{
  const req=route.request(),path=new URL(req.url()).pathname;
  const reply=(json,status=200)=>route.fulfill({json,status});
  const body=req.method()==='POST'?req.postDataJSON():{};
  if(path==='/api/auth/register/start') {
    starts++; assert.equal(body.password,'fixture-password');
    if(starts===1) return reply({error_code:'INVITE_UNAVAILABLE'},409);
    return reply({request_id:'registration',expires_at:Math.floor(Date.now()/1000)+900,resend_after:1,error_code:'ACCOUNT_MAIL_TIMEOUT'},503);
  }
  if(path==='/api/auth/register/resend') return reply({request_id:'registration',expires_at:Math.floor(Date.now()/1000)+900,resend_after:1});
  if(path==='/api/auth/register/complete') return body.verification_code==='123456'?reply({completed:true}):reply({error_code:'REGISTRATION_CODE_INVALID'},400);
  if(path==='/api/auth/password/forgot') return reply({accepted:true});
  if(path==='/api/auth/password/reset') { resets++; if(body.token==='expired') return reply({error_code:'PASSWORD_RESET_INVALID'},409); assert.equal(body.token,'mail-token'); password=body.new_password; signed=false; return reply({completed:true}); }
  if(path==='/api/auth/login') { if(body.password!==password) return reply({error_code:'AUTH_REQUIRED'},401); signed=true; return reply(identity()); }
  if(!signed) return reply({},401);
  if(path==='/api/auth/me') return reply(identity());
  if(path==='/api/library') return reply({books:[]});
  if(path==='/api/account/password') { assert.equal(req.headers()['x-csrf-token'],'csrf'); if(body.current_password!==password) return reply({error_code:'CURRENT_PASSWORD_INVALID'},400); changes++; password=body.new_password; signed=false; return reply({completed:true}); }
  if(path==='/api/admin/users') return reply({users:[{user_id:'original',email:'reader@example.com',disabled:false,is_admin:true,current_allowance:null,activity:{last_read_at:null,last_question_at:null}}],total:1});
  return reply({},404);
});
const page=await context.newPage();
const observe=p=>{p.setDefaultTimeout(15000);p.on('pageerror',e=>errors.push(String(e)));}; observe(page);
async function login(p, pass=password) {
  await p.getByLabel('邮箱或账号',{exact:true}).fill('reader@example.com');
  await p.getByLabel('密码',{exact:true}).fill(pass);
  await p.getByRole('button',{name:'登录',exact:true}).click();
  await p.getByRole('heading',{name:'选择阅读材料'}).waitFor();
}
const layout=async(p)=>assert(await p.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
try {
  await page.goto(reader); await page.getByRole('button',{name:'注册账号',exact:true}).click();
  await page.getByLabel('邮箱',{exact:true}).fill('reader@example.com');
  async function registration() { await page.getByLabel('密码',{exact:true}).fill('fixture-password'); await page.getByLabel('确认密码',{exact:true}).fill('fixture-password'); await page.getByLabel('内测码',{exact:true}).fill('ABCDE-ABCDE-ABCDE-ABCDE'); await page.getByRole('button',{name:'发送注册验证码'}).click(); }
  await registration(); await page.getByRole('alert').filter({hasText:'内测码已使用'}).waitFor();
  assert.equal(await page.getByLabel('密码',{exact:true}).inputValue(),'');
  await page.screenshot({path:`${output}/register-mobile.png`,fullPage:true}); await layout(page);
  await registration(); await page.getByRole('alert').filter({hasText:'可能已送达'}).waitFor();
  await page.getByRole('button',{name:'重发验证码',exact:true}).click();
  await page.getByRole('status').filter({hasText:'验证码已发送'}).waitFor();
  await page.getByLabel('邮箱验证码').fill('000000'); await page.getByRole('button',{name:'完成注册'}).click();
  await page.getByRole('alert').filter({hasText:'验证码不正确'}).waitFor();
  await page.getByLabel('邮箱验证码').fill('123456'); await page.getByRole('button',{name:'完成注册'}).click();
  await page.getByText('注册成功，请登录。材料与使用额度由管理员开通。',{exact:true}).waitFor();
  await login(page); await page.getByText('账号已创建，等待管理员开通材料。',{exact:true}).waitFor();
  checks.push('Mobile registration: rejected invite, timeout recovery, resend, wrong code, success and email login');
  const other=await context.newPage(); observe(other); await other.goto(reader); await other.getByRole('heading',{name:'选择阅读材料'}).waitFor();
  const adminPage=await context.newPage(); observe(adminPage); await adminPage.goto(`${reader}/admin/users`); await adminPage.getByRole('heading',{name:'账号管理',exact:true}).waitFor();
  await page.getByRole('button',{name:'个人设置',exact:true}).click(); await page.getByRole('button',{name:'修改密码',exact:true}).click();
  const dialog=page.getByRole('dialog');
  async function change(current) {await dialog.getByLabel('当前密码',{exact:true}).fill(current);await dialog.getByLabel('新密码',{exact:true}).fill('changed-password');await dialog.getByLabel('确认新密码',{exact:true}).fill('changed-password');await dialog.getByRole('button',{name:'保存新密码'}).click();}
  await change('wrong'); await dialog.getByRole('alert').filter({hasText:'当前密码不正确'}).waitFor();
  await page.screenshot({path:`${output}/password-mobile.png`,fullPage:true}); await layout(page);
  await change('fixture-password'); await page.getByText('密码已更新，请使用新密码重新登录。',{exact:true}).waitFor();
  await other.getByLabel('邮箱或账号',{exact:true}).waitFor(); await adminPage.getByLabel('邮箱或账号',{exact:true}).waitFor();
  checks.push('Personal password change clears current, other Reader and same-origin Admin sessions');
  await page.getByRole('button',{name:'忘记密码',exact:true}).click(); await page.getByLabel('邮箱',{exact:true}).fill('reader@example.com'); await page.getByRole('button',{name:'发送重置邮件'}).click();
  await page.getByRole('status').filter({hasText:'如该邮箱对应可用账号'}).waitFor();
  await page.screenshot({path:`${output}/forgot-mobile.png`,fullPage:true});
  await page.getByRole('button',{name:'返回登录'}).click(); await login(page);
  await page.setViewportSize({width:1440,height:900}); await page.goto(`${reader}/?account=reset-password#token=mail-token`);
  await page.getByRole('heading',{name:'设置新密码'}).waitFor(); assert.equal(new URL(page.url()).hash,''); assert.equal(resets,0);
  await page.getByLabel('新密码',{exact:true}).fill('reset-password-123'); await page.getByLabel('确认新密码',{exact:true}).fill('reset-password-123');
  await page.screenshot({path:`${output}/reset-desktop.png`,fullPage:true}); await layout(page);
  await page.getByRole('button',{name:'重置密码',exact:true}).click(); await page.getByText('密码已更新，请使用新密码重新登录。',{exact:true}).waitFor();
  await login(page); await page.getByRole('button',{name:'个人设置',exact:true}).click(); await page.getByText('账号：original',{exact:true}).waitFor();
  checks.push('Forgot and signed-in reset deep link: no GET mutation, URL cleared, reset then original-account login');
  await page.goto(`${reader}/?account=reset-password#token=expired`); await page.getByLabel('新密码',{exact:true}).fill('reset-password-123'); await page.getByLabel('确认新密码',{exact:true}).fill('reset-password-123'); await page.getByRole('button',{name:'重置密码',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'链接已失效'}).waitFor(); await page.getByRole('button',{name:'重新申请找回密码'}).click(); await page.getByRole('heading',{name:'找回密码',exact:true}).waitFor();
  await page.goto(`${reader}/?account=reset-password`); await page.getByRole('alert').filter({hasText:'未找到重置链接'}).waitFor();
  for(const p of [page,other,adminPage]) {const storage=await p.evaluate(()=>JSON.stringify(localStorage)+JSON.stringify(sessionStorage)); for(const secret of ['fixture-password','changed-password','reset-password-123','mail-token','ABCDE']) assert(!storage.includes(secret));}
  assert.equal(changes,1); assert.equal(resets,2); assert.deepEqual(errors,[]);
  checks.push('Expired/missing reset links recover; no password, invite or token persisted; zero page errors');
  await fs.writeFile(`${output}/result.json`,JSON.stringify({status:'passed',checks,errors},null,2)); console.log(checks.join('\n'));
} catch(error) { await page.screenshot({path:`${output}/failure.png`,fullPage:true}); throw error; }
finally { await browser.close(); }
