// Specific to batch4's SVG ladder. Closed-form answers are independent of page code.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const report={viewports:[]};
try {
 for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
  const page=await browser.newPage({viewport});const record={viewport,errors:[],samples:[]};report.viewports.push(record);
  page.on('pageerror',e=>record.errors.push(e.message));
  await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>{}};},content.initial_state);
  await page.setContent(content.content_files[content.entrypoint]);
  for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++)for(const progress of [0,.35,.8]){
   const actual=await page.evaluate(async({eta,k,progress})=>{
    const input=document.querySelector('#eta');input.value=String(eta);input.dispatchEvent(new Event('input',{bubbles:true}));
    await window.presentationScene.seek({semantic_state:k,transition_progress:progress});
    const svg=document.querySelector('#svg-ladder');
    const points=Array.from(svg.querySelectorAll('circle')).map(n=>({x:+n.getAttribute('cx'),y:+n.getAttribute('cy'),r:+n.getAttribute('r'),opacity:n.getAttribute('opacity')}));
    const geom=document.querySelector('#svg-geom'),tangent=geom.querySelector('line[clip-path]');
    const vertex=geom.querySelector('line[stroke="#15803d"]');
    const path=geom.querySelector('path').getAttribute('d');
    const first=path.match(/^M([\d.-]+),([\d.-]+)/);
    const line={};for(const a of ['x1','x2','y1','y2','opacity'])line[a]=+tangent.getAttribute(a);
    const curvePoint=geom.querySelector('circle[fill="#111827"]');
    const arrow=geom.querySelector('polygon[fill="#111827"]');
    return {scene:window.presentationScene.snapshot(),text:document.querySelector('#result').innerText,
      caption:document.querySelector('#geomRead').innerText,width:+svg.getAttribute('width'),height:+svg.getAttribute('height'),points,
      tangent:line,firstCurvePoint:[+first[1],+first[2]],targetX:+vertex.getAttribute('x1'),
      curvePoint:[+curvePoint.getAttribute('cx'),+curvePoint.getAttribute('cy')],zeroArrowHidden:arrow.getAttribute('opacity')==='0'};
   },{eta,k,progress});
   const wt=2-2*(1-2*eta)**k,wn=2-2*(1-2*eta)**(k+1),expected=wt+(wn-wt)*progress;
   const shown=Number(actual.text.match(/w = ([\d.e+-]+)/)?.[1]);
   const point=progress>0?actual.points.at(-1):actual.points[k];
   const x=42+(k+progress)/8*(actual.width-56),y=actual.height-34-(Math.max(-3,Math.min(6,expected))+3)/9*(actual.height-58);
   const t=actual.tangent;
   const pxPerW=(actual.targetX-actual.firstCurvePoint[0])/3.5;
   const pxPerL=(170-actual.firstCurvePoint[1])/12.25;
   const slope=-(t.y2-t.y1)/(t.x2-t.x1)*pxPerW/pxPerL;
   const lineAtPoint=t.y1+(t.y2-t.y1)*(actual.curvePoint[0]-t.x1)/(t.x2-t.x1);
   record.samples.push({eta,k,progress,expected,actual,
    number:Math.abs(shown-expected)<.00051,geometry:Math.abs(point.x-x)<.051&&Math.abs(point.y-y)<.051,
    fraction:actual.scene.semantic_state===k&&actual.scene.transition_progress===progress,
    zeroArrows:Math.abs(wn-wt)>1e-9||actual.zeroArrowHidden,
    tangent:{visible:t.opacity===1,slope,expectedSlope:2*(wt-2),missPixels:lineAtPoint-actual.curvePoint[1],
      correct:t.opacity!==1||(Math.abs(slope-2*(wt-2))<.03&&Math.abs(lineAtPoint-actual.curvePoint[1])<.15)}});
  }
  await page.evaluate(()=>{window.presentationScene.seek({semantic_state:0,transition_progress:0});window.scrollTo(0,0);});
  record.controls=await page.locator('input,button').evaluateAll(ns=>ns.map(n=>({id:n.id,type:n.type,text:n.textContent})));
  record.layout=await page.locator('#svg-geom,#svg-ladder,#rows,#pos').evaluateAll(ns=>ns.map(n=>({id:n.id,top:n.getBoundingClientRect().top,height:n.getBoundingClientRect().height})));
  await page.screenshot({path:path.join(root,`independent-${viewport.width}.png`),fullPage:true});
  await page.locator('#svg-geom').screenshot({path:path.join(root,`tangent-${viewport.width}.png`)});
  await page.close();
 }
}finally{await browser.close();fs.writeFileSync(path.join(root,'independent-ladder.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify(report.viewports.map(v=>({viewport:v.viewport,count:v.samples.length,
 failures:Object.fromEntries(['number','geometry','fraction','zeroArrows'].map(k=>[k,v.samples.filter(s=>!s[k]).length])),
 tangentFailures:v.samples.filter(s=>!s.tangent.correct).length,example:v.samples.find(s=>!s.tangent.correct)?.tangent,errors:v.errors})),null,2));
