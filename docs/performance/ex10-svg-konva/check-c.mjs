// Inspect rendered HTML and actual transformed Konva objects against an independent recurrence.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const delivered=fs.existsSync(path.join(root,'view.json'));
const view=JSON.parse(fs.readFileSync(path.join(root,delivered?'view.json':'diagnostic-content.json'),'utf8'));
const w=(eta,k)=>{let result=0;for(let i=0;i<k;i++)result-=2*eta*(result-2);return result};
const check=(actual,expected,tolerance=.00051)=>({actual,expected,pass:Math.abs(actual-expected)<=tolerance});
const number=s=>Number(s.replaceAll('−','-'));
const browser=await chromium.launch({headless:true});const samples=[],errors=[];
try{
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const page=await browser.newPage({viewport,hasTouch:viewport.width!==960});page.on('pageerror',e=>errors.push(e.message));
    await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader(fn){window.ex10Read=fn},registerStateRestorer(fn){window.ex10Restore=fn},commitState(){}}},view.initial_state);
    await page.setContent(view.content_files[view.entrypoint]);await page.waitForFunction(()=>!!window.presentationScene);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
      await page.evaluate(({eta,k})=>window.ex10Restore({values:{page:{eta,semantic_state:k,transition_progress:0}}}),{eta,k});
      const actual=await page.evaluate(()=>{
        const stage=window.Konva.stages[0],nodes=stage.find('Shape');
        const transform=(node,x,y)=>node.getAbsoluteTransform().point({x,y});
        const lines=nodes.filter(n=>n.getClassName()==='Line'),axis=lines[0],ap=axis.points();
        const arrows=nodes.filter(n=>n.getClassName()==='Arrow').map(n=>{const p=n.points();return {from:transform(n,p[0],p[1]),to:transform(n,p[2],p[3]),length:n.pointerLength(),width:n.strokeWidth(),opacity:n.getAbsoluteOpacity()}});
        const circles=nodes.filter(n=>n.getClassName()==='Circle').map(n=>({position:transform(n,0,0),radius:n.radius()}));
        const labels=nodes.filter(n=>n.getClassName()==='Text').map(n=>({text:n.text(),position:transform(n,0,0),fontSize:n.fontSize()}));
        return {state:window.presentationScene.snapshot(),axis:{from:transform(axis,ap[0],ap[1]),to:transform(axis,ap[2],ap[3])},arrows,circles,labels,
          table:[...document.querySelectorAll('#r-tbody tr')].map(tr=>[...tr.querySelectorAll('td')].map(n=>n.innerText)),
          marker:document.querySelector('#r-marker').innerText,stepLength:document.querySelector('#r-disp').innerText,
          verdict:document.querySelector('#r-regime').innerText,controls:[...document.querySelectorAll('input')].map(n=>({id:n.id,type:n.type,min:n.min,max:n.max}))};
      });
      const scale=(actual.axis.to.x-actual.axis.from.x)/19;
      const toW=x=>(x-actual.axis.from.x)/scale-6;
      const endpoints=actual.circles.filter(c=>Math.abs(c.position.y-actual.axis.from.y)<1e-8);
      const geometry=endpoints.map((c,j)=>check(toW(c.position.x),w(eta,j),1e-7));
      const table=actual.table.map((row,j)=>({w:check(number(row[1]),w(eta,j)),abs_error:check(number(row[2]),Math.abs(w(eta,j)-2)),delta:j?check(number(row[3]),w(eta,j)-w(eta,j-1)):null}));
      const arrows=[];
      for(let j=0;j<Math.min(k+1,5);j++){
        const label=actual.labels.find(l=>l.text===`${j}→${j+1}`);
        const arrow=label&&actual.arrows.find(a=>Math.abs(a.from.y-(label.position.y+label.fontSize+3))<1e-7);
        const expected=w(eta,j+1)-w(eta,j);
        arrows.push({step:j,expected,expected_css_length:Math.abs(expected*scale),present:!!arrow,
          from:arrow?check(toW(arrow.from.x),w(eta,j),1e-7):null,
          to:arrow?check(toW(arrow.to.x),w(eta,j+1),1e-7):null,
          pass:Math.abs(expected)<1e-9?!arrow:!!arrow&&Math.abs(toW(arrow.to.x)-toW(arrow.from.x)-expected)<1e-7});
      }
      const s={viewport,eta,k,actual,geometry,table,arrows,marker:check(number(actual.marker),w(eta,k)),stepLength:check(number(actual.stepLength),2*eta*Math.abs(w(eta,k===5?4:k)-2))};samples.push(s);
      if(eta===.5&&k===1||viewport.width===320&&eta===.2&&k===4)await page.screenshot({path:path.join(root,`independent-${viewport.width}-eta${eta}-k${k}.png`),fullPage:true});
    }
    await page.close();
  }
  const report={identity:delivered?'Resident delivered version':'undelivered original candidate; diagnostic only',errors,samples,
    numeric_pass:samples.every(s=>s.marker.pass&&s.stepLength.pass&&s.table.every(r=>r.w.pass&&r.abs_error.pass&&(r.delta===null||r.delta.pass))),
    endpoint_geometry_pass:samples.every(s=>s.geometry.length===6&&s.geometry.every(g=>g.pass)),
    arrow_geometry_pass:samples.every(s=>s.arrows.every(a=>a.pass)),
    eta_half_text_failures:samples.filter(s=>s.eta===.5&&s.actual.verdict.includes('乘数为负')).map(s=>({viewport:s.viewport,k:s.k,text:s.actual.verdict})),
    time_slider_present:samples[0].actual.controls.some(c=>c.type==='range'&&c.id!=='eta'),
  };
  fs.writeFileSync(path.join(root,'independent-check.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({...report,samples:samples.length,eta_half_text_failures:report.eta_half_text_failures.slice(0,1),arrow_failures:samples.filter(s=>s.arrows.some(a=>!a.pass)).map(s=>({viewport:s.viewport,eta:s.eta,k:s.k,arrows:s.arrows.filter(a=>!a.pass)})).slice(0,5)},null,2));
}finally{await browser.close()}
