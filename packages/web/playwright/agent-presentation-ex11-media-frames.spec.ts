import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const host='http://127.0.0.1:'+(process.env.PRESENTATION_TEST_PORT || '4175');

// Capture actual decoded Reader frames. Domain event order is judged against
// the frozen input using these images, never inferred from snapshot fields.
test('EX11 decoded frames at forward, fractional, backward and repeated positions', async ({browser})=>{
  test.setTimeout(120000);
  const root=path.resolve(process.env.EX11_RUN_DIR!);
  const selectors=JSON.parse(fs.readFileSync(path.join(root,'reader-selectors.json'),'utf8'));
  const results:any[]=[];
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage();
    const record:any={viewport,errors:[],frames:[]};results.push(record);
    page.on('pageerror',e=>record.errors.push(e.message));
    try {
      await page.request.post(host+'/reset-scene');
      await page.route('**/api/**',async route=>{
        const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,host)});
        await route.fulfill({response});
      });
      await page.goto('/agent-presentation-visual.html');
      const frame=page.frameLocator('.agent-presentation iframe');
      await expect(frame.locator(selectors.play)).toBeVisible();
      await page.getByRole('button',{name:'展开',exact:true}).click();
      const last=selectors.lastStep;
      expect(Number.isInteger(last)).toBe(true);
      const targets=[...Array.from({length:last+1},(_,step)=>[step,0]),[2,.35],[1,.2],[1,.2]];
      for(let i=0;i<targets.length;i++) {
        const [step,progress]=targets[i];
        const actual=await frame.locator('body').evaluate(async(_,target)=>{
          await (window as any).presentationScene.seek({semantic_state:target.step,transition_progress:target.progress});
          const videos=Array.from(document.querySelectorAll('video')).map(video=>{
            const canvas=document.createElement('canvas');canvas.width=video.videoWidth;canvas.height=video.videoHeight;
            canvas.getContext('2d')!.drawImage(video,0,0);
            return {time:video.currentTime,duration:video.duration,paused:video.paused,controls:video.controls,
              readyState:video.readyState,width:video.videoWidth,height:video.videoHeight,png:canvas.toDataURL('image/png')};
          });
          return {scene:(window as any).presentationScene.snapshot(),videos};
        },{step,progress});
        for(let j=0;j<actual.videos.length;j++) {
          const video=actual.videos[j];
          expect(video.width).toBeGreaterThan(0);expect(video.readyState).toBeGreaterThanOrEqual(2);
          expect(video.controls).toBe(false);expect(video.paused).toBe(true);
          if(viewport.width===960) fs.writeFileSync(path.join(root,`decoded-${i}-${j}.png`),Buffer.from(video.png.split(',')[1],'base64'));
        }
        const pngs=actual.videos.map(v=>v.png);
        if(i===targets.length-1) expect(pngs).toEqual(record.previousPngs);
        record.previousPngs=pngs;
        record.frames.push({target:{step,progress},scene:actual.scene,visibleText:await frame.locator(selectors.result).innerText(),
          videos:actual.videos.map(({png,...meta})=>meta)});
        expect(actual.scene.semantic_state).toBe(step);
        expect(actual.scene.transition_progress).toBeCloseTo(progress,8);
      }
      await frame.locator('video').first().scrollIntoViewIfNeeded();
      // A decoded offscreen frame needs a paint after scrolling into the Reader.
      await frame.locator('video').first().evaluate(async()=>{
        await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
      });
      await page.screenshot({path:path.join(root,`media-reader-${viewport.width}.png`)});
      expect(record.errors).toEqual([]);record.passed=true;
    } catch(error) {record.passed=false;record.failure=String(error);}
    finally {
      delete record.previousPngs;
      fs.writeFileSync(path.join(root,'decoded-frames.json'),JSON.stringify(results,null,2));
      await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
    }
  }
  expect(results.filter(r=>!r.passed).map(r=>({viewport:r.viewport,failure:r.failure}))).toEqual([]);
});
