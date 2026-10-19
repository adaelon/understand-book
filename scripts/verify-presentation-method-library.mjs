import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const methodsRoot=path.join(root,'skills/presentation/methods');
const catalog=JSON.parse(fs.readFileSync(path.join(methodsRoot,'catalog.json'),'utf8'));
const args=process.argv.slice(2),galleryOnly=args.includes('--gallery-only'),selected=new Set(args.filter(s=>!s.startsWith('--')));
for(const id of selected)assert.ok(catalog.methods.some(m=>m.id===id),'Unknown method '+id);
const output=path.resolve(process.env.METHOD_LIBRARY_OUTPUT??path.join(root,'tmp/method-library-verification'));fs.mkdirSync(output,{recursive:true});
const report={status:'running',methods:[],gallery:[],provider_calls:0};
if(!galleryOnly){
 const linked=catalog.methods.filter(m=>m.id!=='spatial-linked-views'&&(!selected.size||selected.has(m.id)));
 if(linked.length){const dest=path.join(output,'linked');execFileSync(process.execPath,[path.join(root,'scripts/verify-linked-methods.mjs'),...linked.map(m=>m.id)],{env:{...process.env,LINKED_OUTPUT:dest},stdio:'inherit'});report.methods.push({ids:linked.map(m=>m.id),verification:JSON.parse(fs.readFileSync(path.join(dest,'report.json'),'utf8'))});}
 if(!selected.size||selected.has('spatial-linked-views')){const dest=path.join(output,'spatial');execFileSync(process.execPath,[path.join(root,'scripts/verify-spatial-method.mjs')],{env:{...process.env,SPATIAL_OUTPUT:dest},stdio:'inherit'});report.methods.push({ids:['spatial-linked-views'],verification:JSON.parse(fs.readFileSync(path.join(dest,'report.json'),'utf8'))});}
}
const web=createRequire(path.join(root,'packages/web/package.json')),{chromium}=web('@playwright/test');
const server=http.createServer((req,res)=>{const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname),file=path.join(methodsRoot,pathname==='/'?'index.html':pathname);try{const data=fs.readFileSync(file);res.setHeader('Content-Type',file.endsWith('.html')?'text/html; charset=utf-8':file.endsWith('.js')?'application/javascript; charset=utf-8':'text/plain; charset=utf-8');res.end(data);}catch{res.writeHead(404);res.end();}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url='http://127.0.0.1:'+server.address().port;
const browser=await chromium.launch({channel:process.env.METHOD_LIBRARY_BROWSER??'msedge',headless:true});
try{
 for(const width of [320,960,1440]){
  const page=await browser.newPage({viewport:{width,height:1000}}),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(url);
  for(let i=0;i<catalog.methods.length;i++){
   const m=catalog.methods[i];await page.locator(`[data-index="${i}"]`).click();assert.equal(await page.locator('#preview-title').textContent(),m.name);
   const demo=m.id==='spatial-linked-views'?page.frameLocator('#demo').frameLocator('iframe'):page.frameLocator('#demo');
   const title=JSON.parse(fs.readFileSync(path.join(root,m.root,'example/author.json'),'utf8')).title;
   await demo.locator('h1').waitFor();assert.equal(await demo.locator('h1').textContent(),title);
   assert.equal(await demo.locator('body').evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,m.id+' embedded overflow');
  }
  await page.locator('[data-index="1"]').click();await page.frameLocator('#demo').locator('#next').click();await page.frameLocator('#demo').locator('body').evaluate(()=>scrollTo(0,0));
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false);assert.deepEqual(errors,[]);await page.screenshot({path:path.join(output,`gallery-${width}.png`),fullPage:true});
  report.gallery.push({width,selection_count:catalog.methods.length,status:'passed'});await page.close();
 }
 report.status='passed';
}catch(e){report.status='failed';report.error=e.stack;throw e;}finally{fs.writeFileSync(path.join(output,'browser-results.json'),JSON.stringify(report,null,2)+'\n');await browser.close();await new Promise(resolve=>server.close(resolve));}
console.log(JSON.stringify({status:report.status,gallery:report.gallery,output}));
