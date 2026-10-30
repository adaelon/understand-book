// Render the real Admin with controlled HTTP samples; server collection is tested separately.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const { chromium } = createRequire(path.join(root, 'packages/web/package.json'))('@playwright/test');
const output = path.join(root, 'docs/performance/server-monitor-20261028');
mkdirSync(output, { recursive:true });
const browser = await chromium.launch({headless:true});
const page = await browser.newPage({viewport:{width:1440,height:1100}});
const errors=[];
page.on('pageerror', error => errors.push(error.message));
let requests=0, failed=false, unavailable=false;
function report() {
  const now=Math.floor(Date.now()/1000);
  const history=Array.from({length:80}, (_, index) => {
    const wave=Math.sin(index/9), load=48+index/20+wave*6;
    return {
      sampled_at:now-(79-index)*5,service_uptime_seconds:345600+index*5,
      memory:{total_bytes:8*1024**3,used_bytes:8*1024**3*load/100,available_bytes:8*1024**3*(100-load)/100,used_percent:load},
      disk:{total_bytes:100*1024**3,used_bytes:62.4*1024**3,available_bytes:32.6*1024**3,used_percent:62.4},
      network:{interfaces:['eth0'],receive_bytes_per_second:270000+wave*180000,transmit_bytes_per_second:900000+Math.cos(index/7)*550000},
      activity:{online_users:Math.round(32+index/10+wave*3),active_runs:Math.round(4+wave*2),queued_runs:index>60?2:0,active_run_limit:12,today_turns:128,today_completed_turns:119},
      issues:[],
    };
  });
  const current=history.at(-1);
  if(unavailable){current.memory=null;current.network=null;current.issues=['暂时无法读取服务器内存。','暂时无法读取默认路由网卡流量。'];}
  return {current,history,interval_seconds:5,history_seconds:600,online_window_seconds:90,timezone:'Asia/Hong_Kong',platform:'linux'};
}
await page.route('**/api/**', route => {
  const pathname=new URL(route.request().url()).pathname;
  if(pathname==='/api/auth/me')return route.fulfill({json:{user_id:'operator',email:'operator@example.com',csrf_token:'fixture',capabilities:{admin:true}}});
  if(pathname==='/api/account/presence')return route.fulfill({json:{ok:true}});
  if(pathname==='/api/admin/monitor'){
    requests++;
    return failed?route.fulfill({status:503,json:{message:'监控连接暂时不可用',error_code:'UNAVAILABLE'}}):route.fulfill({json:report()});
  }
  return route.fulfill({status:404,json:{}});
});
try {
  await page.goto(`${process.env.MONITOR_ADMIN_URL??'http://127.0.0.1:18967'}/admin/monitor`);
  await page.getByRole('heading',{name:'实时监控',exact:true}).waitFor();
  await page.getByText('实时更新',{exact:true}).waitFor();
  await page.getByRole('slider',{name:'在线人数采样时间'}).fill('10');
  await page.evaluate(()=>window.scrollTo(0,0));
  await page.screenshot({path:path.join(output,'desktop.png'),fullPage:true});
  const first=requests;
  await page.waitForTimeout(5500);assert(requests>first,'automatic refresh');
  await page.getByRole('button',{name:'暂停更新',exact:true}).click();
  const paused=requests;
  await page.waitForTimeout(5500);assert.equal(requests,paused,'pause stops polling');
  await Promise.all([page.waitForResponse(response=>response.url().endsWith('/api/admin/monitor')),page.getByRole('button',{name:'立即刷新',exact:true}).click()]);
  assert.equal(requests,paused+1,'manual refresh while paused');
  failed=true;
  await page.getByRole('button',{name:'恢复更新',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'保留上次采样'}).waitFor();
  await page.screenshot({path:path.join(output,'connection-error.png'),fullPage:true});
  assert(await page.getByText('今日对话轮次',{exact:true}).isVisible());
  failed=false;unavailable=true;
  await page.getByRole('button',{name:'立即刷新',exact:true}).click();
  await page.getByText('暂时无法读取服务器内存。 暂时无法读取默认路由网卡流量。',{exact:true}).waitFor();
  await page.screenshot({path:path.join(output,'partial-unavailable.png'),fullPage:true});
  unavailable=false;
  await page.getByRole('button',{name:'立即刷新',exact:true}).click();
  await page.getByText('实时更新',{exact:true}).waitFor();
  await page.getByRole('button',{name:'切换主题'}).click();
  await page.screenshot({path:path.join(output,'dark.png'),fullPage:true});
  await page.getByRole('button',{name:'切换主题'}).click();
  await page.setViewportSize({width:390,height:844});
  await page.screenshot({path:path.join(output,'mobile.png'),fullPage:true});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,'no mobile page overflow');
  await page.getByText('统计口径',{exact:true}).click();
  assert(await page.getByText('AI 对话数',{exact:true}).isVisible());
  assert.deepEqual(errors,[]);
  writeFileSync(path.join(output,'browser-results.json'),JSON.stringify({passed:true,requests,scenarios:['desktop 1440','mobile 390','dark theme','automatic refresh','pause and resume','manual refresh','failure retains data','partial collection failure','chart selection','definitions'],pageErrors:errors},null,2));
} catch (error) {
  await page.screenshot({path:path.join(root,'tmp/monitor-browser-failure.png'),fullPage:true});
  console.error(JSON.stringify({url:page.url(),errors,body:await page.locator('body').innerText()},null,2));
  throw error;
} finally {await browser.close();}
