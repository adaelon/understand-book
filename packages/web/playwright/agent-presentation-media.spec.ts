import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const evidence=path.resolve('../../docs/performance/ex11-local-demonstrations/ex11.4');

test('actual media frames, paused recovery and follow-up across Reader viewports',async({browser})=>{
  test.setTimeout(120000);
  const results:any[]=[];
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage(); const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
    await page.request.post('http://127.0.0.1:4175/reset-scene');
    await page.route('**/api/**',async route=>{const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,'http://127.0.0.1:4175')});await route.fulfill({response})});
    await page.goto('/agent-presentation-visual.html');
    const frame=page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator('#play-a')).toBeVisible();
    await page.getByRole('button',{name:'展开',exact:true}).click();
    const decoded=[];
    for(const time of [0,1.5,0.5,0.5,2.2]){
      const record=await frame.locator('#a').evaluate(async(node:HTMLVideoElement,time)=>{
        const scene=(window as any).presentationScene;
        await scene.seek({semantic_state:Math.floor(time),transition_progress:time%1});
        const canvas=document.createElement('canvas');canvas.width=node.videoWidth;canvas.height=node.videoHeight;
        const ctx=canvas.getContext('2d')!;ctx.drawImage(node,0,0);const pixels=ctx.getImageData(0,0,canvas.width,canvas.height).data;
        let x=0,count=0;for(let i=0;i<pixels.length;i+=4)if(pixels[i]<130&&pixels[i+1]>150&&pixels[i+2]>160&&pixels[i+2]>pixels[i]+50){x+=(i/4)%canvas.width;count++}
        return {target:time,actual:scene.snapshot(),centroid:x/count,count,width:node.videoWidth,controls:node.controls,playsInline:node.playsInline};
      },time);
      const actualTime=Math.min(time,2.2-1/30);
      expect(Math.abs(record.actual.media_time-actualTime)).toBeLessThanOrEqual(1/30+1e-6);
      const expectedX=( (-3+6*Math.min(actualTime/2,1))/(8*640/360)+.5)*640;
      expect(Math.abs(record.centroid-expectedX)).toBeLessThan(6);
      expect(record.count).toBeGreaterThan(100);expect(record.controls).toBe(false);expect(record.playsInline).toBe(true);
      decoded.push(record);
    }
    await frame.locator('#middle-a').click();
    await frame.locator('body').evaluate(async()=>{await (window as any).presentationScene.seek({semantic_state:1,transition_progress:.5});});
    await frame.locator('#middle-b').click();
    await frame.locator('#save').click();
    const fixture=await page.request.get('http://127.0.0.1:4175/fixture').then(r=>r.json());
    await expect.poll(async()=>{const view=await page.request.post('http://127.0.0.1:4175/agent/presentation.read',{data:fixture}).then(r=>r.json());return view.restored_state?.values.page.a_time}).toBe(1.5);
    await expect.poll(async()=>{const view=await page.request.post('http://127.0.0.1:4175/agent/presentation.read',{data:fixture}).then(r=>r.json());return view.restored_state?.values.page.b_time}).toBe(1);
    await page.request.post('http://127.0.0.1:4175/reopen');await page.reload();
    try { await expect(frame.locator('#result')).toContainText('A frame 1.500s',{timeout:12000}); }
    catch(error){fs.writeFileSync(path.join(evidence,'restore-failure.json'),JSON.stringify({errors,html:await page.locator('.agent-presentation').innerText(),media:await frame.locator('body').evaluate(()=>Array.from(document.querySelectorAll('video')).map(v=>({id:v.id,time:v.currentTime,ready:v.readyState,seeking:v.seeking,style:getComputedStyle(document.body).visibility}))).catch(()=>[])},null,2));throw error;}
    const restored=await frame.locator('#a').evaluate((node:HTMLVideoElement)=>({time:node.currentTime,paused:node.paused,state:(window as any).presentationScene.snapshot()}));
    expect(restored.time).toBeCloseTo(1.5,2);expect(restored.paused).toBe(true);
    expect(restored.state.active_demo).toBe('b');
    expect(await frame.locator('#b').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeCloseTo(1,2);
    await page.getByRole('textbox',{name:'针对当前现场追问'}).fill('Explain this paused frame');
    await page.getByRole('button',{name:'发送追问',exact:true}).click();
    await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
    const requests=await page.request.get('http://127.0.0.1:4175/requests').then(r=>r.json());
    const followUp=requests.at(-1).filter((m:any)=>m.role==='User').at(-1).content;
    expect(followUp).toContain('"a_time":1.5');expect(followUp).toContain('A frame 1.500s');
    expect(followUp).toContain('"b_time":1');expect(followUp).toContain('"active_demo":"b"');
    await page.getByRole('button',{name:'展开',exact:true}).click();
    await frame.locator('#play-a').click();
    await expect.poll(()=>frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
    await frame.locator('#play-a').click();
    expect(await frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
    await frame.locator('#replay-a').click();
    await frame.locator('#last').scrollIntoViewIfNeeded();
    await expect.poll(()=>frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
    await frame.locator('#a').scrollIntoViewIfNeeded();
    expect(await frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
    await page.screenshot({path:path.join(evidence,`reader-${viewport.width}.png`)});
    expect(errors).toEqual([]);
    results.push({viewport,decoded,restored,followUp,errors});
    fs.writeFileSync(path.join(evidence,'reader.json'),JSON.stringify(results,null,2));
    await context.close();
  }
});

test('media lifecycle pauses peers, hidden host and offscreen clips without resuming',async({page})=>{
  await page.setViewportSize({width:960,height:720});
  await page.request.post('http://127.0.0.1:4175/reset-scene');
  await page.route('**/api/**',async route=>{const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,'http://127.0.0.1:4175')});await route.fulfill({response})});
  await page.goto('/agent-presentation-visual.html');const frame=page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#play-a')).toBeVisible();await page.getByRole('button',{name:'展开',exact:true}).click();
  await frame.locator('#replay-a').click();await expect.poll(()=>frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  await frame.locator('#replay-b').click();await expect.poll(()=>frame.locator('#b').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  expect(await frame.locator('#a').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
  await page.locator('.agent-presentation').evaluate((n:HTMLElement)=>n.style.display='none');
  await expect.poll(()=>frame.locator('#b').evaluate((v:HTMLVideoElement)=>v.paused),{timeout:700}).toBe(true);
  expect(await frame.locator('#b').evaluate((v:HTMLVideoElement)=>v.currentTime)).toBeLessThan(1);
  await page.locator('.agent-presentation').evaluate((n:HTMLElement)=>n.style.display='');
  expect(await frame.locator('#b').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(true);
  await frame.locator('#a').evaluate((v:HTMLVideoElement)=>{
    const added=v.cloneNode() as HTMLVideoElement;added.id='dynamic';v.after(added);
  });
  await frame.locator('#dynamic').scrollIntoViewIfNeeded();
  await frame.locator('#dynamic').evaluate(async(v:HTMLVideoElement)=>{await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);await v.play();});
  await expect.poll(()=>frame.locator('#dynamic').evaluate((v:HTMLVideoElement)=>v.paused)).toBe(false);
  expect(await frame.locator('#dynamic').evaluate((v:HTMLVideoElement)=>v.controls)).toBe(false);
  await frame.locator('#dynamic').evaluate((v:HTMLVideoElement)=>{(window as any).__removedVideo=v;v.remove();});
  await expect.poll(()=>frame.locator('body').evaluate(()=>(window as any).__removedVideo.paused),{timeout:700}).toBe(true);
  fs.writeFileSync(path.join(evidence,'lifecycle.json'),JSON.stringify({peerPaused:true,hiddenHostPaused:true,returnStayedPaused:true,dynamicRemovalPaused:true},null,2));
});
