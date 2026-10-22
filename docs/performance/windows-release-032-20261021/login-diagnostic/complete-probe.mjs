import { chromium } from 'playwright';
import { readFile,writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='tmp/windows-login-20261021';
const url=await readFile(root+'/authorization-url.private.txt','utf8');
const credentials=JSON.parse(await readFile(root+'/credentials.private.json','utf8'));
const browser=await chromium.launch({channel:'msedge',headless:true});
const context=await browser.newContext();
const page=await context.newPage();
const desktop=await chromium.connectOverCDP('http://127.0.0.1:19337');
const native=desktop.contexts()[0].pages()[0];
const result={};
try {
  const login=await context.request.post('https://understandbook.top/api/auth/login',{data:credentials,headers:{Origin:'https://understandbook.top'}});
  result.login_status=login.status();
  assert(login.ok(),'test account login status '+login.status());
  await page.goto(url);
  await page.getByRole('button',{name:'确认登录 Windows',exact:true}).click();
  await page.waitForURL(u=>u.hostname==='127.0.0.1');
  result.callback_page=await page.locator('body').innerText();
  result.native_page=await native.locator('body').innerText();
  result.native_url=native.url();
  result.status=result.callback_page.includes('Signed in.')?'passed':'failed';
} catch(error) {result.status='failed';result.error=String(error).replace(/code=[^&\s]+/g,'code=[redacted]').replace(/state=[^&\s]+/g,'state=[redacted]');result.native_page=await native.locator('body').innerText();}
finally {await writeFile(root+'/baseline.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));await browser.close();await desktop.close();}
