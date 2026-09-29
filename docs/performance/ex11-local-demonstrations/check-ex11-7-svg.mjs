// Geometry adapter for batch3's untouched SVG learning-rate page.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const report={identity:'untouched delivered SVG; independent closed form and actual element coordinates',viewports:[]};
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const page=await browser.newPage({viewport,hasTouch:viewport.width!==960});
    const record={viewport,errors:[],samples:[]}; report.viewports.push(record);
    page.on('pageerror',e=>record.errors.push(e.message));
    await page.route('http://ex11.test/**',async route=>{
      const file=new URL(route.request().url()).pathname.slice(1)||content.entrypoint;
      const body=content.content_files[file];
      if(body===undefined) return route.fulfill({status:404,body:''});
      return route.fulfill({contentType:file.endsWith('.svg')?'image/svg+xml':'text/html',body});
    });
    await page.addInitScript(initialState=>{
      window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>{}};
    },content.initial_state);
    await page.goto('http://ex11.test/');
    await page.waitForFunction(()=>window.presentationScene);
    record.controls=await page.locator('input,button').evaluateAll(nodes=>nodes.map(n=>({id:n.id,type:n.type,text:n.textContent,aria:n.getAttribute('aria-label')})));
    for(const eta of [0,.2,.5,.8,1,1.1,1.2]) for(let k=0;k<=5;k++) for(const progress of [0,.35,.8]) {
      const actual=await page.evaluate(async({eta,k,progress})=>{
        const input=document.querySelector('#etaSlider');input.value=String(eta);input.dispatchEvent(new Event('input',{bubbles:true}));
        await window.presentationScene.seek({semantic_state:k,transition_progress:progress});
        const svg=document.querySelector('#mainRail');
        const axis=svg.querySelector('line[stroke="#c6cdd7"]');
        const point=svg.querySelector('circle[fill="#f97316"]');
        return {eta:Number(input.value),scene:window.presentationScene.snapshot(),text:document.querySelector('#resultLine').innerText,
          axis:[Number(axis.getAttribute('x1')),Number(axis.getAttribute('x2'))],x:Number(point.getAttribute('cx')),
          activeArrows:svg.querySelectorAll('line[marker-end="url(#mMain)"]').length,
          paraArrows:document.querySelectorAll('#parabola line[marker-end]').length,
          paraText:document.querySelector('#paraNote').innerText};
      },{eta,k,progress});
      const w0=2-2*Math.pow(1-2*eta,k), w1=2-2*Math.pow(1-2*eta,k+1);
      const expected=w0+(w1-w0)*progress;
      const [left,right]=actual.axis;
      const expectedX=Math.max(left+1,Math.min(right-1,left+(expected+10)/20*(right-left)));
      const shown=Number(actual.text.match(/｜ w = ([^ ]+)/)?.[1]);
      record.samples.push({eta,k,progress,expected,actual,parameter:actual.eta===eta,
        number:Math.abs(shown-expected)<=.00501,geometry:Math.abs(actual.x-expectedX)<1e-6,
        fraction:actual.scene.semantic_state===k&&actual.scene.transition_progress===progress,
        zeroArrows:Math.abs(w1-w0)>1e-12||(actual.activeArrows===0&&actual.paraArrows===0)});
    }
    await page.locator('#etaSlider').fill('0.8');
    await page.evaluate(()=>window.presentationScene.seek({semantic_state:1,transition_progress:.35}));
    record.layout=await page.locator('#mainRail,#ref0,.controls,h1').evaluateAll(nodes=>nodes.map(n=>({id:n.id||n.tagName,top:n.getBoundingClientRect().top,height:n.getBoundingClientRect().height,width:n.getBoundingClientRect().width})));
    await page.screenshot({path:path.join(root,`independent-${viewport.width}.png`),fullPage:true});
    await page.close();
  }
} finally {
  await browser.close();fs.writeFileSync(path.join(root,'independent-svg.json'),JSON.stringify(report,null,2));
}
console.log(JSON.stringify(report.viewports.map(v=>({viewport:v.viewport,count:v.samples.length,
  failures:Object.fromEntries(['parameter','number','geometry','fraction','zeroArrows'].map(k=>[k,v.samples.filter(s=>!s[k]).length])),errors:v.errors,layout:v.layout})),null,2));
