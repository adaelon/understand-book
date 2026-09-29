// Independent closed-form numbers plus actual SVG geometry. Does not alter a candidate.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const view=JSON.parse(fs.readFileSync(path.join(root,'view.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const page=await browser.newPage({viewport:{width:960,height:720}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
const w=(eta,k)=>2-2*(1-2*eta)**k;
const number=s=>Number(s.replaceAll('−','-').replaceAll('–','-').replaceAll(',',''));
const check=(actual,expected,tolerance=.00051)=>({actual,expected,pass:Math.abs(actual-expected)<=tolerance});
const samples=[];
try{
  await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader(fn){window.ex10Read=fn},registerStateRestorer(fn){window.ex10Restore=fn},commitState(){}}},view.initial_state);
  await page.setContent(view.content_files[view.entrypoint]);
  await page.waitForFunction(()=>!!window.presentationScene);
  for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
    await page.evaluate(({eta,k})=>window.ex10Restore({values:{page:{eta,k,semantic_state:k,transition_progress:0}}}),{eta,k});
    const actual=await page.evaluate(()=>{
      const text=id=>document.getElementById(id)?.innerText;
      const svg=document.querySelector('#drawing');
      const screen=node=>{const p=svg.createSVGPoint();p.x=Number(node.getAttribute('cx'));p.y=Number(node.getAttribute('cy'));const q=p.matrixTransform(node.getScreenCTM());return {x:q.x,y:q.y}};
      const ticks=[...svg.querySelectorAll('text')].filter(n=>n.textContent==='-6'||n.textContent==='14').map(n=>{const p=svg.createSVGPoint();p.x=Number(n.getAttribute('x'));p.y=Number(n.getAttribute('y'));return {value:Number(n.textContent),x:p.matrixTransform(n.getScreenCTM()).x}});
      const points=[...svg.querySelectorAll('[data-role="dynamic"]>circle')].slice(0,6).map(screen);
      const arrows=[...svg.querySelectorAll('[data-role="dynamic"]>line')].map(n=>({x1:Number(n.getAttribute('x1')),x2:Number(n.getAttribute('x2')),y1:Number(n.getAttribute('y1')),y2:Number(n.getAttribute('y2')),opacity:n.getAttribute('opacity')}));
      return {state:window.presentationScene.snapshot(),w:text('out-w'),loss:text('out-loss'),delta:text('out-delta'),next:text('out-next'),verdict:text('out-verdict'),points,ticks,arrows,
        table:[...document.querySelectorAll('#steps-body tr')].map(tr=>[...tr.querySelectorAll('th,td')].map(n=>n.innerText))};
    });
    const lo=actual.ticks.find(t=>t.value===-6),hi=actual.ticks.find(t=>t.value===14),scale=(hi.x-lo.x)/20;
    const geometry=actual.points.map((p,j)=>check((p.x-lo.x)/scale-6,w(eta,j),.001));
    const readings={w:check(number(actual.w),w(eta,k)),loss:check(number(actual.loss),(w(eta,k)-2)**2),next:check(number(actual.next),w(eta,Math.min(5,k+1))),
      delta:check(number(actual.delta),k===5?0:w(eta,k+1)-w(eta,k))};
    const table=actual.table.map((row,j)=>({w:check(number(row[1]),w(eta,j)),closed:check(number(row[2]),w(eta,j)),loss:check(number(row[4]),(w(eta,j)-2)**2)}));
    samples.push({eta,k,actual,geometry,readings,table});
  }
  await page.evaluate(()=>window.ex10Restore({values:{page:{eta:.8,k:2,semantic_state:2,transition_progress:.5}}}));
  const middle=await page.evaluate(()=>({snapshot:window.presentationScene.snapshot(),readout:document.querySelector('#result-readout').innerText}));
  await page.locator('#prev').click();
  const previous=await page.evaluate(()=>window.presentationScene.snapshot());
  const paragraphs=await page.locator('p').allTextContents();
  await page.screenshot({path:path.join(root,'independent-desktop.png'),fullPage:true});
  const report={errors,samples,middle,previous,previous_expected_step:2,previous_pass:previous.semantic_state===2,paragraphs,
    numbers_pass:samples.every(s=>Object.values(s.readings).every(x=>x.pass)&&s.table.every(row=>Object.values(row).every(x=>x.pass))),
    endpoint_geometry_pass:samples.every(s=>s.geometry.every(x=>x.pass))};
  fs.writeFileSync(path.join(root,'independent-check.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({cases:samples.length,errors,numbers_pass:report.numbers_pass,endpoint_geometry_pass:report.endpoint_geometry_pass,middle,previous,previous_pass:report.previous_pass},null,2));
}finally{await browser.close()}
