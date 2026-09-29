import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const selectors=fs.existsSync(path.join(root,'reader-selectors.json'))?JSON.parse(fs.readFileSync(path.join(root,'reader-selectors.json'),'utf8')):{result:'#readout',play:'#btn-play'};
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
let html=content.content_files[content.entrypoint];
for(const [file,source] of Object.entries(content.content_files))if(file.endsWith('.js'))html=html.replace(`<script data-presentation-library="konva" src="${file}"></script>`,()=>`<script>${source}</script>`);
const css=fs.readFileSync(new URL('../../../packages/web/src/presentation.css',import.meta.url),'utf8');
const w=(eta,k)=>{let v=0;for(let i=0;i<k;i++)v-=2*eta*(v-2);return v;};
const browser=await chromium.launch({headless:true}),samples=[],interactions=[],errors=[];
try{
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage();
    page.on('pageerror',e=>errors.push({viewport,message:e.message}));
    await page.evaluate(initialState=>{window.commits=0;window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>window.commits++};},content.initial_state);
    await page.evaluate(s=>window.ex11Selectors=s,selectors);
    await page.setContent(`<style>${css}</style>${html}`);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++)for(const progress of [0,.5]){
      await page.locator('#eta').evaluate((n,v)=>{n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));},eta);
      await page.evaluate(({k,progress})=>window.presentationScene.seek({semantic_state:k,transition_progress:progress}),{k,progress});
      const actual=await page.evaluate(()=>{
        const stage=Konva.stages[0];
        const grid=stage.find('Line').filter(n=>n.stroke()==='#e2e8f0'&&n.points().length===4&&n.points()[1]===n.points()[3]).map(n=>n.points());
        const ticks=stage.find('Line').map(n=>n.points()).filter(p=>p.length===4);
        const ys=grid.length?grid.map(p=>p[1]):ticks.filter(p=>p[1]===p[3]&&p[2]-p[0]===3).map(p=>p[1]);
        const xs=grid.length?[grid[0][0],grid[0][2]]:ticks.filter(p=>p[0]===p[2]&&p[3]-p[1]===4).map(p=>p[0]);
        const clip={x:Math.min(...xs),y:Math.min(...ys),width:Math.max(...xs)-Math.min(...xs),height:Math.max(...ys)-Math.min(...ys)};
        const current=stage.find('Circle').find(n=>n.radius()===6.5);
        const toW=y=>-8+(clip.y+clip.height-y)/clip.height*20;
        return {scene:window.presentationScene.snapshot(),saved:window.readState(),text:document.querySelector(window.ex11Selectors.result).innerText,
          current:{t:(current.x()-clip.x)/clip.width*8,w:toW(current.y())},
          dots:stage.find('Circle').filter(n=>n.radius()===4&&n.visible()).map(n=>toW(n.y())),
          arrows:stage.find('Arrow').map(n=>n.visible())};
      });
      const expected=w(eta,k)+progress*(w(eta,k+1)-w(eta,k));
      const textNumber=Number(actual.text.match(/当前显示 w\s*=\s*([−\-\d.]+)/)?.[1].replace('−','-'));
      samples.push({viewport,eta,k,progress,actual,expected,
        geometry_pass:Math.abs(actual.current.w-expected)<1e-7&&Math.abs(actual.current.t-k-progress)<1e-7&&actual.dots.length===k+1&&actual.dots.every((v,i)=>Math.abs(v-w(eta,i))<1e-7),
        numeric_pass:Math.abs(textNumber-expected)<.0051,
        zero_arrows_pass:actual.arrows.every((visible,i)=>!visible||Math.abs(w(eta,i+1)-w(eta,i))>1e-10)});
    }
    await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
    await page.evaluate(()=>window.presentationScene.seek({semantic_state:2,transition_progress:.5}));
    const saved=await page.evaluate(()=>window.readState());
    await page.evaluate(saved=>{window.presentationScene.seek({semantic_state:0,transition_progress:0});window.restoreState({values:{page:saved.values},visible_step:saved.visible_step});},saved);
    const restored=await page.evaluate(()=>window.presentationScene.snapshot());
    await page.screenshot({path:path.join(root,`continuous-${viewport.width}.png`)});
    const hit=await page.evaluate(()=>{
      const stage=Konva.stages[0];
      return stage.find('Circle').filter(n=>n.draggable()).map(n=>{
        const p=n.getAbsolutePosition();const span=axis=>{let a=[];for(let d=-40;d<=40;d++)if(stage.getIntersection({x:p.x+(axis==='x'?d:0),y:p.y+(axis==='y'?d:0)})===n)a.push(d);return Math.max(...a)-Math.min(...a)+1;};
        return {radius:n.radius(),width:span('x'),height:span('y')};
      });
    });
    const boundaryHits=[];
    for(const target of [{eta:.8,k:0,radius:6.5},{eta:.8,k:8,radius:6.5},{eta:0,k:0,radius:9},{eta:1.5,k:0,radius:9}]){
      await page.locator('#eta').evaluate((n,v)=>{n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));},target.eta);
      await page.evaluate(k=>window.presentationScene.seek({semantic_state:k,transition_progress:0}),target.k);
      await page.evaluate(()=>Konva.stages[0].draw());
      boundaryHits.push(await page.evaluate(target=>{
        const stage=Konva.stages[0],n=stage.find('Circle').find(n=>n.draggable()&&(target.radius===9?n.radius()===9:n.radius()!==9)),p=n.getAbsolutePosition();
        const span=axis=>{let a=[];for(let d=-40;d<=40;d++)if(stage.getIntersection({x:p.x+(axis==='x'?d:0),y:p.y+(axis==='y'?d:0)})===n)a.push(d);return a.length?Math.max(...a)-Math.min(...a)+1:0;};
        return {...target,width:span('x'),height:span('y')};
      },target));
    }
    await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
    await page.evaluate(()=>window.presentationScene.seek({semantic_state:2,transition_progress:.5}));
    await page.locator(selectors.play).click();await page.waitForTimeout(180);await page.locator(selectors.play).click();
    const paused=await page.evaluate(()=>window.presentationScene.snapshot());
    await page.locator('#eta').evaluate(n=>{n.value='.5';n.dispatchEvent(new Event('input',{bubbles:true}));});
    const reset=await page.evaluate(()=>window.presentationScene.snapshot());
    interactions.push({viewport,saved,restored,hit,boundaryHits,paused,reset,
      restore_pass:restored.semantic_state===2&&restored.transition_progress===.5&&!restored.playing,
      playback_pass:!paused.playing&&(paused.semantic_state>2||paused.transition_progress>.5),
      reset_pass:reset.semantic_state===0&&reset.transition_progress===0&&!reset.playing});
    await context.close();
  }
}finally{await browser.close();}
const report={samples,interactions,errors,numeric_pass:samples.every(s=>s.numeric_pass),geometry_pass:samples.every(s=>s.geometry_pass),zero_arrows_pass:samples.every(s=>s.zero_arrows_pass)};
fs.writeFileSync(path.join(root,'independent-continuous.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify({...report,samples:samples.length},null,2));

if(!report.numeric_pass||!report.geometry_pass||!report.zero_arrows_pass||errors.length||interactions.some(x=>!x.restore_pass||!x.playback_pass||!x.reset_pass||[...x.hit,...x.boundaryHits].some(h=>h.width<44||h.height<44)))process.exitCode=1;
