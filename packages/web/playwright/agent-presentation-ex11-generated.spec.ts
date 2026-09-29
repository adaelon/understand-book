import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

test('EX11 generated artifact: actual drag, playback and Reader scene contract',async({browser})=>{
  test.setTimeout(120000);
  const name=process.env.EX11_SAMPLE!;
  const root=process.env.EX11_RUN_DIR ? path.resolve(process.env.EX11_RUN_DIR) : path.resolve('../../docs/performance/ex11-local-demonstrations',name);
  const results:any[]=[];
  const reportFile=process.env.EX11_WIDTH?`reader-check-${process.env.EX11_WIDTH}.json`:'reader-check.json';
  const selectorFile=path.join(root,'reader-selectors.json');
  const selectors=fs.existsSync(selectorFile)?JSON.parse(fs.readFileSync(selectorFile,'utf8')):name.startsWith('C')
    ? {eta:'#eta',play:'#btn-play',previous:'#btn-prev',next:'#btn-next',result:'#result-panel',scene:'#drawing'}
    : {eta:'#eta',play:'#play',previous:'#prev',next:'#next',result:'#result-readout',scene:'#scene-host'};
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}].filter(v=>!process.env.EX11_WIDTH||v.width===Number(process.env.EX11_WIDTH))){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage();page.setDefaultTimeout(10000);
    await page.request.post('http://127.0.0.1:4175/reset-scene');
    await page.route('**/api/**',async route=>{const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,'http://127.0.0.1:4175')});await route.fulfill({response})});
    await page.addInitScript(()=>{
      const Native=window.MutationObserver;(window as any).__observer=[];
      window.MutationObserver=class extends Native {constructor(callback:MutationCallback){super((records,observer)=>{const start=performance.now();callback(records,observer);(window as any).__observer.push(performance.now()-start)})}};
    });
    const errors:string[]=[];page.on('pageerror',e=>{errors.push(e.message);console.log('EX11_PAGE_ERROR',e.message)});
    page.on('console',m=>{if(m.type()==='error')console.log('EX11_CONSOLE',m.text())});
    await page.addInitScript(()=>window.addEventListener('error',e=>console.error('window error:',e.message))); 
    let saves=0,observes=0;page.on('request',r=>{if(r.url().endsWith('/presentation.state.save'))saves++;if(r.url().endsWith('/presentation.observe'))observes++});
    await page.goto('/agent-presentation-visual.html');
    const frame=page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator(selectors.eta)).toBeVisible();
    await page.getByRole('button',{name:'展开',exact:true}).click();
    await frame.locator(selectors.eta).scrollIntoViewIfNeeded();
    const old=await frame.locator(selectors.eta).inputValue();
    const box=(await frame.locator(selectors.eta).boundingBox())!;
    const min=Number(await frame.locator(selectors.eta).getAttribute('min')),max=Number(await frame.locator(selectors.eta).getAttribute('max'));
    const start=box.x+14+(box.width-28)*(Number(old)-min)/(max-min),end=box.x+14+(box.width-28)*.3,y=box.y+box.height/2;
    const dragHit=await page.evaluate(({x,y})=>{const n=document.elementFromPoint(x,y);return {tag:n?.tagName,classes:n?.className,label:n?.getAttribute('aria-label'),text:n?.textContent?.slice(0,100)}},{x:start,y});
    if(viewport.width===960){await page.mouse.move(start,y);await page.mouse.down();await page.mouse.move(end,y,{steps:12})}
    else{const cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:start,y}]});for(let i=1;i<=12;i++)await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:start+(end-start)*i/12,y}]});(page as any).__cdp=cdp}
    const drag=await frame.locator('body').evaluate((_,sel)=>({eta:(document.querySelector(sel.eta) as HTMLInputElement).value,state:(window as any).presentationScene.snapshot(),text:(document.querySelector(sel.result) as HTMLElement).innerText}),selectors);
    if(viewport.width===960)await page.mouse.up();else await (page as any).__cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    const targets=await frame.locator('button,input[type=range]').evaluateAll(nodes=>nodes.map(n=>{const b=n.getBoundingClientRect();return {id:n.id,width:b.width,height:b.height}}));
    await frame.locator('body').evaluate(()=>{(window as any).presentationScene.seek({semantic_state:2,transition_progress:.5})});
    const middle=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    await frame.locator(selectors.previous).click();const previous=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    await frame.locator(selectors.next).click();
    const forward=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    await frame.locator('body').evaluate(()=>{(window as any).presentationScene.seek({semantic_state:2,transition_progress:.5});(window as any).presentation.commitState()});
    const savedText=await frame.locator(selectors.result).innerText();
    const fixture=await page.request.get('http://127.0.0.1:4175/fixture').then(r=>r.json());
    await expect.poll(async()=>{const view=await page.request.post('http://127.0.0.1:4175/agent/presentation.read',{data:fixture}).then(r=>r.json());return view.restored_state?.values?.page?.transition_progress}).toBe(.5);
    await frame.locator(selectors.scene).scrollIntoViewIfNeeded();await page.screenshot({path:path.join(root,`reader-${viewport.width}.png`)});
    await page.request.post('http://127.0.0.1:4175/reopen');await page.reload();
    await expect(frame.locator(selectors.result)).toBeVisible();
    const restored=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    const restoredText=await frame.locator(selectors.result).innerText();
    await page.getByRole('textbox',{name:'针对当前现场追问'}).fill('解释暂停时这一段运动');await page.getByRole('button',{name:'发送追问',exact:true}).click();
    await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
    const requests=await page.request.get('http://127.0.0.1:4175/requests').then(r=>r.json());
    const message=requests.at(-1).filter((m:any)=>m.role==='User').at(-1).content;
    let performanceResult:any=null;
    await frame.locator('body').evaluate(()=>{(window as any).presentationScene.seek({semantic_state:0,transition_progress:0})});
    await frame.locator(selectors.play).click();await page.waitForTimeout(200);
    const playing=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    await frame.locator(selectors.play).click();
    const paused=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    if(viewport.width===960){
      await frame.locator('body').evaluate(()=>{(window as any).presentationScene.seek({semantic_state:0,transition_progress:0});(window as any).__observer=[]});saves=0;observes=0;
      const measurement=frame.locator('body').evaluate(async()=>await new Promise(resolve=>{const gaps:number[]=[];let last:number|null=null;const start=performance.now();const p95=(a:number[])=>a.length?a.sort((a,b)=>a-b)[Math.ceil(a.length*.95)-1]:0;function tick(now:number){if(last!==null)gaps.push(now-last);last=now;if(now-start>5100)resolve({frames:gaps.length,frameP95:p95(gaps),observerP95:p95((window as any).__observer)});else requestAnimationFrame(tick)}requestAnimationFrame(tick)}));
      await frame.locator(selectors.play).click();performanceResult=await measurement;performanceResult.saves=saves;performanceResult.observes=observes;
      const end=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
      if(end.playing)await frame.locator(selectors.play).click();
      performanceResult.end=await frame.locator('body').evaluate(()=>(window as any).presentationScene.snapshot());
    }
    results.push({identity:fs.existsSync(path.join(root,'origin.json'))?JSON.parse(fs.readFileSync(path.join(root,'origin.json'),'utf8')).origin:fs.existsSync(path.join(root,'view.json'))?'Resident delivered version':'undelivered original candidate mounted by test fixture; diagnostic only',viewport,dragHit,playing,paused,pause_pass:playing.playing&&!paused.playing&&paused.transition_progress>0,drag,drag_live_pass:drag.eta!==old&&drag.state.semantic_state===0&&drag.state.transition_progress===0,targets,middle,previous,forward,previous_pass:previous.semantic_state===2,restored,restored_text_pass:savedText===restoredText,follow_up:message,performance:performanceResult,errors});
    fs.writeFileSync(path.join(root,reportFile),JSON.stringify(results,null,2));
    const recorded=results.at(-1);
    expect(recorded.drag_live_pass).toBe(true);
    expect(recorded.pause_pass).toBe(true);
    expect(recorded.previous_pass).toBe(true);
    expect(recorded.restored_text_pass).toBe(true);
    expect(recorded.errors).toEqual([]);
    await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
  }
  console.log(JSON.stringify(results.map(({follow_up,...r})=>r),null,2));
});
