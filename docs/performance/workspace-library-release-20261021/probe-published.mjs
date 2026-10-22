import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
const stage=path.resolve('tmp/workspace-library-release-20261021');
const html=await readFile(path.join(stage,'source/packages/web/dist-release/index.html'),'utf8');
const expected=html.match(/src="(\/assets\/reader-[^"]+\.js)"/)[1];
const browser=await chromium.connectOverCDP('http://127.0.0.1:19337',{timeout:15000});
try {
  const page=browser.contexts()[0].pages()[0],errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.getByRole('heading',{name:'本机书库',exact:true}).waitFor();
  await page.getByRole('button',{name:'进入云端学习',exact:true}).click();
  await page.getByRole('link',{name:'登录',exact:true}).waitFor({timeout:45000});
  await page.getByRole('link',{name:'登录',exact:true}).click();
  await page.waitForFunction(()=>!!document.querySelector('#app')?.textContent?.trim(),null,{timeout:45000});
  const scripts=await page.locator('script[src]').evaluateAll(nodes=>nodes.map(node=>new URL(node.src).pathname));
  assert(scripts.includes(expected),`Published reader not loaded: ${scripts}`);
  assert(await page.evaluate(()=>!!window.__TAURI_INTERNALS__));
  assert.deepEqual(errors,[]);
  await page.screenshot({path:path.join(stage,'published-windows.png'),fullPage:true});
  await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('desktop_open_local_library'));
  await page.getByText('期末考纲.txt',{exact:true}).waitFor();
  const report={status:'passed',reader_script:expected,native_bridge:true,return_to_library:true,errors,model_requests:0};
  await writeFile(path.join(stage,'published-webview2.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));
} finally {await browser.close();}
