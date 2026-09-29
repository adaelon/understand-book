// Independent recurrence and actual Konva geometry; no edits to model output.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const content=JSON.parse(fs.readFileSync(path.join(root,'content.json'),'utf8'));
let html=content.content_files[content.entrypoint];
for(const [file,source] of Object.entries(content.content_files))if(file.endsWith('.js'))html=html.replace(`<script data-presentation-library="konva" src="${file}"></script>`,()=>`<script>${source}</script>`);
const css=fs.readFileSync(new URL('../../../packages/web/src/presentation.css',import.meta.url),'utf8');
const recurrence=(eta,k)=>{let w=0;for(let i=0;i<k;i++)w-=2*eta*(w-2);return w;};
const browser=await chromium.launch({headless:true}),samples=[],interactions=[],errors=[];
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage();
    page.on('pageerror',e=>errors.push({viewport,message:e.message}));
    await page.evaluate(initialState=>{window.commits=0;window.presentation={initialState,registerStateReader:f=>window.readState=f,registerStateRestorer:f=>window.restoreState=f,commitState:()=>window.commits++};},content.initial_state);
    await page.setContent(`<style>${css}</style>${html}`);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
      await page.locator('#eta').evaluate((n,v)=>{n.value=String(v);n.dispatchEvent(new Event('input',{bubbles:true}));},eta);
      await page.evaluate(k=>window.presentationScene.seek({semantic_state:k,transition_progress:0}),k);
      const actual=await page.evaluate(()=>{
        const stage=Konva.stages[0],bg=stage.getLayers()[0],dyn=stage.getLayers()[1];
        const vertical=bg.find('Line').find(n=>n.points().length===4&&n.points()[0]===n.points()[2]);
        const p=vertical.points(),dots=dyn.find('Group')[0].find('Circle');
        return {state:window.readState(),text:document.querySelector('#liveOut').innerText,
          points:dots.map(n=>({x:n.x(),w:-8+(p[3]-n.y())/(p[3]-p[1])*20})),
          plot:{top:p[1],bottom:p[3]},scene:window.presentationScene.snapshot()};
      });
      samples.push({viewport,eta,k,actual,expected:recurrence(eta,k),geometry_pass:actual.points.every((n,i)=>Math.abs(n.w-recurrence(eta,i))<1e-7),
        numeric_pass:actual.text.includes(recurrence(eta,k).toFixed(2).replace('-','−'))||actual.text.includes(recurrence(eta,k).toFixed(2))});
    }
    await page.locator('#eta').evaluate(n=>{n.value='.8';n.dispatchEvent(new Event('input',{bubbles:true}));});
    await page.evaluate(()=>window.presentationScene.seek({semantic_state:2,transition_progress:.5}));
    const before=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),saved:window.readState(),firstGraphicY:document.querySelector('canvas').getBoundingClientRect().y}));
    await page.screenshot({path:path.join(root,`density-${viewport.width}-eta0.8-k2-p0.5.png`)});
    await page.evaluate(saved=>{window.presentationScene.seek({semantic_state:0,transition_progress:0});window.restoreState({values:{page:saved.values},visible_step:saved.visible_step});},before.saved);
    const restored=await page.evaluate(()=>window.presentationScene.snapshot());
    // Actual hit graph, not the visible knob radius, and real held pointer motion.
    await page.locator('#stage-host').evaluate(n=>n.scrollIntoView({block:'end'}));
    const drag=await page.evaluate(()=>{
      const stage=Konva.stages[0],knob=stage.findOne(n=>n.draggable()),p=knob.getAbsolutePosition(),b=stage.container().getBoundingClientRect();
      const span=axis=>{let hits=[];for(let d=-40;d<=40;d++)if(stage.getIntersection({x:p.x+(axis==='x'?d:0),y:p.y+(axis==='y'?d:0)})===knob)hits.push(d);return Math.max(...hits)-Math.min(...hits)+1;};
      return {x:b.x+p.x,y:b.y+p.y,hitWidth:span('x'),hitHeight:span('y')};
    });
    const commitsBefore=await page.evaluate(()=>window.commits);
    let cdp;
    if(viewport.width===960){await page.mouse.move(drag.x,drag.y);await page.mouse.down();await page.mouse.move(drag.x+35,drag.y,{steps:8});}
    else{cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:drag.x,y:drag.y}]});for(let i=1;i<=8;i++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:drag.x+35*i/8,y:drag.y}]});}
    const during=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),saved:window.readState(),commits:window.commits}));
    if(cdp)await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});else await page.mouse.up();
    const after=await page.evaluate(()=>({scene:window.presentationScene.snapshot(),commits:window.commits}));
    interactions.push({viewport,before,restored,drag,during,after,commitsBefore,intermediate_seek:before.scene.transition_progress===.5,
      restored_same:JSON.stringify(before.scene)===JSON.stringify(restored),hit_pass:drag.hitWidth>=44&&drag.hitHeight>=44,
      live_drag:during.saved.values.eta!==.8,reset_on_parameter_change:during.scene.semantic_state===0&&during.scene.transition_progress===0});
    await context.close();
  }
} finally {await browser.close();}
const report={samples,interactions,errors,numeric_pass:samples.every(s=>s.numeric_pass),geometry_pass:samples.every(s=>s.geometry_pass)};
fs.writeFileSync(path.join(root,'independent-check.json'),JSON.stringify(report,null,2));
console.log(JSON.stringify({...report,samples:samples.length},null,2));
