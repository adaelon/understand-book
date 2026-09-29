import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
const browser=await chromium.launch({headless:true});
const report={identity:'untouched delivered Canvas page; independent arithmetic and observed canvas drawing calls',viewports:[]};
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const page=await browser.newPage({viewport,hasTouch:viewport.width!==960});
    const record={viewport,errors:[],samples:[]};report.viewports.push(record);
    page.on('pageerror',e=>record.errors.push(e.message));
    await page.evaluate(initialState=>{
      window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>{}};
      const proto=CanvasRenderingContext2D.prototype;
      for(const name of ['clearRect','beginPath','moveTo','lineTo','closePath','arc','fill','strokeRect']) {
        const original=proto[name];
        proto[name]=function(...args){
          this.__audit??={arcs:[],triangles:[],box:null,path:[]};const a=this.__audit;
          if(name==='clearRect'){a.arcs=[];a.triangles=[];a.box=null;}
          if(name==='beginPath')a.path=[];
          if(['moveTo','lineTo','closePath'].includes(name))a.path.push([name,...args]);
          if(name==='arc'){a.arcs.push(args);a.path.push(['arc',...args]);}
          if(name==='strokeRect')a.box=args;
          if(name==='fill'&&a.path.length===4&&a.path[0][0]==='moveTo'&&a.path[1][0]==='lineTo'&&a.path[2][0]==='lineTo'&&a.path[3][0]==='closePath')a.triangles.push(a.path);
          return original.apply(this,args);
        };
      }
    },content.initial_state);
    await page.setContent(content.content_files[content.entrypoint]);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
      const actual=await page.evaluate(({eta,k})=>{
        const input=document.querySelector('#eta');input.value=String(eta);input.dispatchEvent(new Event('input',{bubbles:true}));
        window.presentationScene.seek({semantic_state:k,transition_progress:0});
        const cv=document.querySelector('#cvB'),audit=cv.getContext('2d').__audit;
        return {eta:Number(input.value),state:window.presentationScene.snapshot(),rows:Array.from(document.querySelectorAll('#tblBody tr')).map(tr=>Array.from(tr.querySelectorAll('td')).map(td=>td.innerText)),box:audit.box,arcs:audit.arcs,arrowheads:document.querySelector('#cvA').getContext('2d').__audit.triangles.length};
      },{eta,k});
      const expected=Array.from({length:k+1},(_,i)=>2-2*Math.pow(1-2*eta,i));
      const [x,y,width,height]=actual.box;
      const points=actual.arcs.map(a=>({step:(a[0]-x)/width*8,w:10-(a[1]-y)/height*16}));
      const visible=expected.map((w,step)=>({w,step})).filter(p=>p.w>=-6&&p.w<=10);
      record.samples.push({eta,k,actual,expected,points,
        parameterAvailable:actual.eta===eta,
        numbers:actual.rows.every((r,i)=>Math.abs(Number(r[1])-expected[i])<.00051),
        geometry:points.length===visible.length&&points.every((p,i)=>Math.abs(p.step-visible[i].step)<1e-6&&Math.abs(p.w-visible[i].w)<1e-6),
        zeroArrows:eta!==.5||k<=1||actual.arrowheads===1});
    }
    await page.locator('#resetBtn').click();await page.locator('#playBtn').click();
    const before=await page.evaluate(()=>window.presentationScene.snapshot());
    await page.waitForTimeout(650);
    record.playback={before,after:await page.evaluate(()=>window.presentationScene.snapshot()),button:await page.locator('#playBtn').innerText()};
    await page.locator('#playBtn').click();
    record.fractional=await page.evaluate(()=>{window.presentationScene.seek({semantic_state:2,transition_progress:.5});return window.presentationScene.snapshot();});
    await page.evaluate(()=>{const n=document.querySelector('#eta');n.value='.5';n.dispatchEvent(new Event('input',{bubbles:true}));window.presentationScene.seek({semantic_state:4,transition_progress:0});});
    await page.locator('#cvA').screenshot({path:path.join(root,`zero-arrows-${viewport.width}.png`)});
    await page.screenshot({path:path.join(root,`independent-${viewport.width}.png`),fullPage:true});
    await page.close();
  }
}finally{await browser.close();fs.writeFileSync(path.join(root,'independent-canvas.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify(report.viewports.map(v=>({viewport:v.viewport,samples:v.samples.length,unavailable:v.samples.filter(x=>!x.parameterAvailable).length,numericFailures:v.samples.filter(x=>!x.numbers).length,geometryFailures:v.samples.filter(x=>!x.geometry).length,zeroArrowFailures:v.samples.filter(x=>!x.zeroArrows).length,playback:v.playback,fractional:v.fractional,errors:v.errors})),null,2));
