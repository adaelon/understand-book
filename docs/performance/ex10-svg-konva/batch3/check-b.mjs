// Batch 3 B1: independent recurrence vs rendered numbers and transformed SVG geometry.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const view=JSON.parse(fs.readFileSync(path.join(root,'view.json'),'utf8'));
const w=(eta,k)=>{let value=0;for(let i=0;i<k;i++)value-=2*eta*(value-2);return value};
const number=s=>Number(s.replaceAll('−','-'));
const check=(actual,expected,tolerance=.000051)=>({actual,expected,pass:Math.abs(actual-expected)<=tolerance});
const browser=await chromium.launch({headless:true});
const samples=[],interactions=[],errors=[];
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const page=await browser.newPage({viewport,hasTouch:viewport.width!==960});
    page.on('pageerror',error=>errors.push({viewport,message:error.message}));
    await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader(fn){window.ex10Read=fn},registerStateRestorer(fn){window.ex10Restore=fn},commitState(){}}},view.initial_state);
    await page.setContent(view.content_files[view.entrypoint]);await page.waitForFunction(()=>!!window.presentationScene);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
      await page.evaluate(({eta,k})=>window.ex10Restore({values:{page:{eta,semantic_state:k,transition_progress:0}}}),{eta,k});
      const actual=await page.evaluate(()=>{
        const svg=document.querySelector('#scene');
        const xy=(node,x,y)=>{const point=svg.createSVGPoint();point.x=x;point.y=y;const p=point.matrixTransform(node.getScreenCTM());return {x:p.x,y:p.y}};
        const ticks=[...svg.querySelectorAll(':scope > g:nth-of-type(2) text')].map(node=>({value:Number(node.textContent),...xy(node,+node.getAttribute('x'),+node.getAttribute('y'))}));
        const endpoints=[...svg.querySelectorAll('[data-role=ghost]')].map(node=>({k:+node.dataset.k,...xy(node,+node.getAttribute('cx'),+node.getAttribute('cy'))}));
        const arrows=[...svg.querySelectorAll('[data-role=arrow]')].map(node=>({step:+node.dataset.step,tag:node.tagName,
          from:xy(node,+(node.getAttribute('x1')??node.getAttribute('cx')),+(node.getAttribute('y1')??node.getAttribute('cy'))),
          to:xy(node,+(node.getAttribute('x2')??node.getAttribute('cx')),+(node.getAttribute('y2')??node.getAttribute('cy')))}));
        return {ticks,endpoints,arrows,state:window.presentationScene.snapshot(),
          table:[...document.querySelectorAll('#tblBody tr')].map(row=>[...row.querySelectorAll('td')].map(cell=>cell.innerText)),
          kvA:document.querySelector('#kvA').innerText,kvB:document.querySelector('#kvB').innerText,verdict:document.querySelector('#verdict').innerText,
          inputs:[...document.querySelectorAll('input')].map(n=>({id:n.id,type:n.type}))};
      });
      const [lo,hi]=actual.ticks,scale=(hi.x-lo.x)/(hi.value-lo.value),toW=x=>(x-lo.x)/scale+lo.value;
      const geometry=actual.endpoints.map(p=>check(toW(p.x),w(eta,p.k),1e-5));
      const arrows=actual.arrows.map(a=>({step:a.step,from:check(toW(a.from.x),w(eta,a.step-1),1e-5),to:check(toW(a.to.x),w(eta,a.step),1e-5),length:check(Math.abs((a.to.x-a.from.x)/scale),2*eta*Math.abs(w(eta,a.step-1)-2),1e-5)}));
      const table=actual.table.map((row,j)=>({w:check(number(row[1]),w(eta,j)),error:check(number(row[2]),w(eta,j)-2),ratio:check(number(row[3]),(1-2*eta)**j,.000006),delta:j<5?check(number(row[4]),w(eta,j+1)-w(eta,j)):null}));
      const expectedLoss=(w(eta,k)-2)**2;
      const loss=check(number(actual.kvB.match(/L\(w[₀₁₂₃₄₅]\) = ([\d.−-]+)/)[1]),expectedLoss,expectedLoss>=100?.0051:.000051);
      samples.push({viewport,eta,k,actual,geometry,arrows,table,loss});
      if(eta===.5&&k===1||eta===1.1&&k===5)await page.screenshot({path:path.join(root,`independent-${viewport.width}-eta${eta}-k${k}.png`),fullPage:true});
    }
    await page.evaluate(()=>window.ex10Restore({values:{page:{eta:.8,semantic_state:2,transition_progress:.5}}}));
    const middle=await page.evaluate(()=>window.presentationScene.snapshot());
    await page.locator('#prev').click();const previous=await page.evaluate(()=>window.presentationScene.snapshot());
    await page.locator('#next').click();const next=await page.evaluate(()=>window.presentationScene.snapshot());
    interactions.push({viewport,middle,previous,next,previous_pass:previous.semantic_state===2&&previous.transition_progress===0,next_pass:next.semantic_state===3&&next.transition_progress===0});
    await page.close();
  }
  const report={samples,interactions,errors,numeric_pass:samples.every(s=>s.loss.pass&&s.table.every(r=>r.w.pass&&r.error.pass&&r.ratio.pass&&(r.delta===null||r.delta.pass))),
    endpoint_geometry_pass:samples.every(s=>s.geometry.length===5&&s.geometry.every(g=>g.pass)),arrow_geometry_pass:samples.every(s=>s.arrows.length===5&&s.arrows.every(a=>a.from.pass&&a.to.pass&&a.length.pass)),
    eta_half_heading_errors:samples.filter(s=>s.eta===.5&&s.actual.kvA.includes('来回跳')).map(s=>({viewport:s.viewport,k:s.k,text:s.actual.kvA})),
    time_slider_present:samples[0].actual.inputs.some(n=>n.type==='range'&&n.id!=='eta')};
  fs.writeFileSync(path.join(root,'independent-check.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({...report,samples:samples.length,eta_half_heading_errors:report.eta_half_heading_errors.slice(0,1)},null,2));
} finally {await browser.close()}
