import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]),content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
let html=content.content_files[content.entrypoint];
for(const [file,source] of Object.entries(content.content_files))if(file.endsWith('.js'))html=html.replace(`<script data-presentation-library="konva" src="${file}"></script>`,()=>`<script>${source}</script>`);
const css=fs.readFileSync(new URL('../../../packages/web/src/presentation.css',import.meta.url),'utf8');
const browser=await chromium.launch({headless:true}),results=[];
try{
 for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
  const context=await browser.newContext({viewport,hasTouch:viewport.width!==960}),page=await context.newPage();
  await page.evaluate(initialState=>{window.commits=0;window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:()=>{},commitState:()=>window.commits++};},content.initial_state);
  await page.setContent(`<style>${css}</style>${html}`);
  for(const target of ['time','eta']){
   await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
   await page.evaluate(()=>{window.presentationScene.seek({semantic_state:2,transition_progress:.5});Konva.stages[0].draw();});
   const position=await page.evaluate(target=>{const stage=Konva.stages[0],n=stage.find('Circle').find(n=>n.draggable()&&(target==='eta'?n.radius()===9:n.radius()!==9));const p=n.getAbsolutePosition(),b=stage.container().getBoundingClientRect();window.scrollTo(0,window.scrollY+b.y+p.y-innerHeight/2);return {x:p.x,y:p.y};},target);
   const box=await page.locator('#stage-host').boundingBox(),x=box.x+position.x,y=box.y+position.y;
   const before=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),eta:document.querySelector('#eta').value,commits:window.commits}));
   let cdp;
   if(viewport.width===960){await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+30,y,{steps:8});}
   else {cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});for(let i=1;i<=8;i++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:x+30*i/8,y}]});}
   const held=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),eta:document.querySelector('#eta').value,commits:window.commits}));
   if(cdp)await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});else await page.mouse.up();
   const released=await page.evaluate(()=>window.commits);
   results.push({viewport,target,before,held,released,live_pass:target==='eta'?held.eta!==before.eta:held.scene.semantic_state+held.scene.transition_progress!==2.5,commit_boundary_pass:held.commits===before.commits&&released>held.commits});
  }
  await context.close();
 }
}finally{await browser.close();}
fs.writeFileSync(path.join(root,'pointer-check.json'),JSON.stringify(results,null,2));
console.log(JSON.stringify(results,null,2));
if(results.some(r=>!r.live_pass||!r.commit_boundary_pass))process.exitCode=1;
