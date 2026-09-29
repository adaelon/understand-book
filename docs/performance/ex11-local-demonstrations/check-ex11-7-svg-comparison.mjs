// Observations for batch6 learning-1. Answers use the independent closed form;
// only selectors and axis transforms are adapted to the untouched SVG page.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const report={viewports:[]};
const clamp=w=>Math.max(-4,Math.min(8,w));
const exact=(eta,k,p)=>{const w=2-2*(1-2*eta)**k;return w+p*((2-2*(1-2*eta)**(k+1))-w);};
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const page=await browser.newPage({viewport});
    const record={viewport,errors:[],samples:[]};report.viewports.push(record);
    page.on('pageerror',e=>record.errors.push(e.message));
    await page.evaluate(initialState=>{window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>{}};},content.initial_state);
    await page.setContent(content.content_files[content.entrypoint]);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++)for(const progress of [0,.35,.8]) {
      const actual=await page.evaluate(async({eta,k,progress})=>{
        const input=document.querySelector('#eta');input.value=String(eta);input.dispatchEvent(new Event('input',{bubbles:true}));
        await window.presentationScene.seek({semantic_state:k,transition_progress:progress});
        const attrs=(node,names)=>Object.fromEntries(names.map(a=>[a,Number(node.getAttribute(a))]));
        const loss=document.querySelector('#svgLoss'),line=document.querySelector('#svgLine');
        const tangent=loss.querySelector('line[stroke-dasharray="3 3"]');
        return {scene:window.presentationScene.snapshot(),text:document.querySelector('#r1').innerText,
          loss:{...attrs(loss,['width','height']),dot:attrs(loss.lastElementChild.querySelector('circle'),['cx','cy'])},
          line:{...attrs(line,['width','height']),dot:attrs(line.lastElementChild.querySelector('circle'),['cx','cy'])},
          tangent:tangent?attrs(tangent,['x1','x2','y1','y2']):null,
          zeroArrows:!document.querySelector('#svgLoss polygon[fill="#c05621"],#svgLine polygon[fill="#c05621"]'),
          comparisons:Array.from(document.querySelectorAll('#compare .cmp')).map(c=>{const svg=c.querySelector('svg');return {...attrs(svg,['width','height']),dot:attrs(svg.lastElementChild.querySelector('circle'),['cx','cy']),text:c.querySelector('.cmp-v').innerText};})};
      },{eta,k,progress});
      const expected=exact(eta,k,progress),clipped=clamp(expected),wt=exact(eta,k,0),wn=exact(eta,k+1,0);
      const shown=Number(actual.text.match(/此刻 w = ([\d.e+-]+)/)?.[1]);
      const lossX=40+(clipped+4)/12*(actual.loss.width-56);
      const lossY=actual.loss.height-30-(clipped-2)**2/36*(actual.loss.height-48);
      const lineX=14+(clipped+4)/12*(actual.line.width-28);
      let tangent=true;
      if(actual.tangent) {
        const t=actual.tangent,pxPerW=(actual.loss.width-56)/12,pxPerL=(actual.loss.height-48)/36;
        const slope=-(t.y2-t.y1)/(t.x2-t.x1)*pxPerW/pxPerL;
        const px=40+(wt+4)*pxPerW,py=actual.loss.height-30-(wt-2)**2*pxPerL;
        tangent=Math.abs(slope-2*(wt-2))<.001&&Math.abs(t.y1+(t.y2-t.y1)*(px-t.x1)/(t.x2-t.x1)-py)<.01;
      }
      const comparisons=actual.comparisons.every((c,i)=>{
        const w=exact([.25,.75,1,1.2][i],k,progress),x=30+(k+progress)/12*(c.width-42),y=c.height-22-(clamp(w)+4)/12*(c.height-40);
        return Math.abs(c.dot.cx-x)<.01&&Math.abs(c.dot.cy-y)<.01&&Math.abs(Number(c.text.match(/w=([\d.e+-]+)/)?.[1])-w)<.0051;
      });
      record.samples.push({eta,k,progress,expected,actual,number:Math.abs(shown-expected)<.00051,
        geometry:Math.abs(actual.loss.dot.cx-lossX)<.01&&Math.abs(actual.loss.dot.cy-lossY)<.01&&Math.abs(actual.line.dot.cx-lineX)<.01,
        fraction:actual.scene.semantic_state===k&&actual.scene.transition_progress===progress,
        zeroArrows:Math.abs(wn-wt)>1e-9||actual.zeroArrows,tangent,comparisons});
    }
    await page.evaluate(async()=>{await window.presentationScene.seek({semantic_state:0,transition_progress:0});window.scrollTo(0,0);});
    record.layout=await page.locator('#pos,#compare,#svgLoss,#svgLine').evaluateAll(ns=>ns.map(n=>({id:n.id,top:n.getBoundingClientRect().top,height:n.getBoundingClientRect().height})));
    await page.screenshot({path:path.join(root,`independent-${viewport.width}.png`),fullPage:true});
    await page.close();
  }
}finally{await browser.close();fs.writeFileSync(path.join(root,'independent-comparison.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify(report.viewports.map(v=>({viewport:v.viewport,count:v.samples.length,errors:v.errors,
  failures:Object.fromEntries(['number','geometry','fraction','zeroArrows','tangent','comparisons'].map(k=>[k,v.samples.filter(s=>!s[k]).length]))})),null,2));
