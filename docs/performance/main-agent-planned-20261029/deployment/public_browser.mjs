import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const require=createRequire('E:/allwork/download/agent/understand-book/packages/web/package.json');
const {chromium,expect}=require('@playwright/test');
const stage=path.dirname(fileURLToPath(import.meta.url));
let input='';for await(const chunk of process.stdin)input+=chunk;
const {account,boards}=JSON.parse(input);input='';
const origin='https://understandbook.top';
const browser=await chromium.launch({headless:true,args:['--disable-http2'],executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'});
const context=await browser.newContext({viewport:{width:1440,height:1000}});
const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
try {
 const login=await context.request.post(origin+'/api/auth/login',{data:account,headers:{Origin:origin}});assert.equal(login.status(),200);
 const books=(await (await context.request.get(origin+'/api/library')).json()).books;
 const reference=books.find(b=>b.published_book_ref.book_id==='logic-of-failure')?.published_book_ref ?? books.find(b=>b.is_default).published_book_ref;
 const manifest=await (await context.request.get(origin+'/api/books/'+reference.book_id+'/publications/'+reference.publication_id+'/manifest')).json();
 const leaf=manifest.tree.find(n=>!n.children?.length&&n.span.end-n.span.start>100)??manifest.tree.find(n=>!n.children?.length);
 const params=new URLSearchParams({...reference,start_lid:leaf.lid,end_lid:leaf.lid,start_utf16:String(leaf.span.start),end_utf16:String(Math.min(leaf.span.end,leaf.span.start+30))});
 for(let attempt=0;attempt<3;attempt++){try{await page.goto(origin+'/?'+params,{waitUntil:'domcontentloaded',timeout:20000});break;}catch(e){if(attempt===2)throw e;}}
 await page.locator('.reader-pane').waitFor({timeout:45000});
 await expect.poll(async()=>await page.locator('.reader-pane').innerText(),{timeout:30000}).not.toBe('');
 await page.evaluate(()=>document.fonts.ready);
 await page.screenshot({path:path.join(stage,'public-reader-desktop.png')});
 await page.setViewportSize({width:390,height:844});
 await page.locator('.reader-pane').waitFor({timeout:15000});
 await page.screenshot({path:path.join(stage,'public-reader-mobile.png')});
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));assert.deepEqual(errors,[]);
 const me=await (await context.request.get(origin+'/api/auth/me')).json();
 await context.request.post(origin+'/api/auth/logout',{data:{},headers:{Origin:origin,'X-CSRF-Token':me.csrf_token}});
 const report={status:'passed',verified_at_utc:new Date().toISOString(),origin,viewports:[1440,390],authenticated_reading:true,book_id:reference.book_id,horizontal_overflow:false,browser_errors:errors,model_requests:0};
 fs.writeFileSync(path.join(stage,'public-browser.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
} finally {await browser.close();}
