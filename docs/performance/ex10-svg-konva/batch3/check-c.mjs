// Batch 3 C1: recurrence, actual Konva shapes, hit graph and real pointer/touch dragging.
import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
const require=createRequire(new URL('../../../../packages/web/package.json',import.meta.url));
const {chromium}=require('@playwright/test');
const root=path.resolve(process.argv[2]);
const view=JSON.parse(fs.readFileSync(path.join(root,'view.json'),'utf8'));
const w=(eta,k)=>{let value=0;for(let i=0;i<k;i++)value-=2*eta*(value-2);return value};
const number=s=>Number(s.replaceAll('−','-'));
const check=(actual,expected,tolerance=.00051)=>({actual,expected,pass:Math.abs(actual-expected)<=tolerance});
const browser=await chromium.launch({headless:true}),samples=[],interactions=[],errors=[];
try {
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});const page=await context.newPage();
    page.on('pageerror',error=>errors.push({viewport,message:error.message}));
    await page.evaluate(initialState=>{window.hostCommits=0;window.presentation={initialState,registerStateReader(fn){window.ex10Read=fn},registerStateRestorer(fn){window.ex10Restore=fn},commitState(){window.hostCommits++}}},view.initial_state);
    await page.setContent(view.content_files[view.entrypoint]);await page.waitForFunction(()=>!!window.presentationScene);
    for(const eta of [0,.2,.5,.8,1,1.1,1.2])for(let k=0;k<=5;k++){
      await page.evaluate(({eta,k})=>window.presentationScene.seek({eta,semantic_state:k,transition_progress:0}),{eta,k});
      const actual=await page.evaluate(()=>{
        const stage=window.Konva.stages[0],xy=(node,x,y)=>node.getAbsoluteTransform().point({x,y});
        const line=n=>{const p=n.points();return {from:xy(n,p[0],p[1]),to:xy(n,p[2],p[3]),visible:n.isVisible(),opacity:n.getAbsoluteOpacity(),pointerLength:n.getClassName()==='Arrow'?n.pointerLength():0,bounds:n.getClientRect({skipShadow:true})}};
        const read=id=>document.getElementById(id).innerText;
        return {state:window.presentationScene.snapshot(),
          ticks:stage.find('.ticklabel').map(n=>({value:Number(n.text()),...xy(n,n.width()/2,0)})),
          ghosts:stage.find('.ghost').map(line),
          arrows:[0,1,2,3,4].map(step=>({step,solid:line(stage.findOne('#arrow-'+step)),guide:line(stage.findOne('#guide-'+step))})),
          dot:xy(stage.findOne('#dot'),0,0),w:read('rWk'),next:read('rNext'),loss:read('rLoss'),error:read('rErr'),live:read('rLive'),
          verdict:read('verdict'),explanation:read('explain'),table:[...document.querySelectorAll('#trajBody tr')].map(row=>[...row.querySelectorAll('td')].map(cell=>cell.innerText))};
      });
      const [lo,hi]=actual.ticks,scale=(hi.x-lo.x)/(hi.value-lo.value),toW=x=>(x-lo.x)/scale+lo.value;
      const geometry=actual.ghosts.map((p,j)=>check(toW(p.from.x),w(eta,j),1e-7));
      const arrows=actual.arrows.filter(a=>a.step<=k).filter(a=>a.step<5).map(a=>{
        const line=a.step===k?a.guide:a.solid;
        return {step:a.step,visible:line.visible,from:check(toW(line.from.x),w(eta,a.step),1e-7),to:check(toW(line.to.x),w(eta,a.step+1),1e-7),length:check(Math.abs((line.to.x-line.from.x)/scale),2*eta*Math.abs(w(eta,a.step)-2),1e-7)};
      });
      const table=actual.table.map((row,j)=>({w:check(number(row[1]),w(eta,j)),loss:check(number(row[2]),(w(eta,j)-2)**2),absError:check(number(row[3]),Math.abs(w(eta,j)-2))}));
      const readings={w:check(number(actual.w),w(eta,k)),loss:check(number(actual.loss),(w(eta,k)-2)**2),error:check(number(actual.error),w(eta,k)-2),next:k<5?check(number(actual.next),w(eta,k+1)):null};
      samples.push({viewport,eta,k,actual,geometry,arrows,table,readings});
      if(eta===.5&&k===1||eta===1.1&&k===5)await page.screenshot({path:path.join(root,`independent-${viewport.width}-eta${eta}-k${k}.png`),fullPage:true});
    }
    await page.evaluate(()=>window.presentationScene.seek({eta:.8,semantic_state:2,transition_progress:0}));
    await page.locator('#drawing').scrollIntoViewIfNeeded();
    const dragInfo=await page.evaluate(()=>{
      const stage=window.Konva.stages[0],dot=stage.findOne('#dot'),p=dot.getAbsolutePosition(),b=stage.container().getBoundingClientRect(),guide=stage.findOne('#guide-2'),g=guide.points(),end=guide.getAbsoluteTransform().point({x:g[2],y:g[3]});
      const sx=b.width/stage.width(),sy=b.height/stage.height();
      const span=axis=>{const hits=[];for(let d=-50;d<=50;d++)if(stage.getIntersection({x:p.x+(axis==='x'?d/sx:0),y:p.y+(axis==='y'?d/sy:0)})===dot)hits.push(d);return hits.length?Math.max(...hits)-Math.min(...hits)+1:0};
      return {start:{x:b.x+p.x*sx,y:b.y+p.y*sy},end:{x:b.x+(p.x+end.x)/2*sx,y:b.y+p.y*sy},hitWidth:span('x'),hitHeight:span('y'),radius:dot.radius()};
    });
    let cdp;
    if(viewport.width===960){await page.mouse.move(dragInfo.start.x,dragInfo.start.y);await page.mouse.down();await page.mouse.move(dragInfo.end.x,dragInfo.end.y,{steps:12})}
    else{cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[dragInfo.start]});for(let i=1;i<=12;i++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:dragInfo.start.x+(dragInfo.end.x-dragInfo.start.x)*i/12,y:dragInfo.start.y}]})}
    const during=await page.evaluate(()=>window.presentationScene.snapshot());
    if(cdp)await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});else await page.mouse.up();
    const after=await page.evaluate(()=>({state:window.presentationScene.snapshot(),hostCommits:window.hostCommits}));
    interactions.push({viewport,dragInfo,during,after,drag_pass:during.semantic_state===2&&!during.playing&&Math.abs(during.transition_progress-.5)<.06,touch_target_pass:dragInfo.hitWidth>=44&&dragInfo.hitHeight>=44});
    await context.close();
  }
  const expectedRegime=new Map([[0,'完全不动'],[.2,'单调收敛（不跳）'],[.5,'一步到位'],[.8,'来回跳但越来越近'],[1,'永远来回跳（不收敛也不发散）'],[1.1,'来回跳且越来越远（振荡发散）'],[1.2,'来回跳且越来越远（振荡发散）']]);
  const zeroUpdateGlyphs=samples.flatMap(s=>s.actual.arrows.filter(a=>a.step<=s.k&&Math.abs(w(s.eta,a.step+1)-w(s.eta,a.step))<1e-9).map(a=>({viewport:s.viewport,eta:s.eta,k:s.k,step:a.step,glyph:a.step===s.k?a.guide:a.solid}))).filter(x=>x.glyph.visible&&x.glyph.pointerLength>0);
  const report={samples,interactions,errors,zero_update_arrowheads:zeroUpdateGlyphs,numeric_pass:samples.every(s=>Object.values(s.readings).every(r=>r===null||r.pass)&&s.table.every(r=>r.w.pass&&r.loss.pass&&r.absError.pass)),
    endpoint_geometry_pass:samples.every(s=>s.geometry.length===6&&s.geometry.every(g=>g.pass)),arrow_geometry_pass:samples.every(s=>s.arrows.every(a=>a.visible&&a.from.pass&&a.to.pass&&a.length.pass)),
    boundary_regime_pass:samples.every(s=>s.actual.verdict===expectedRegime.get(s.eta))};
  fs.writeFileSync(path.join(root,'independent-check.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify({...report,samples:samples.length,zero_update_arrowheads:report.zero_update_arrowheads.slice(0,2)},null,2));
} finally {await browser.close()}
