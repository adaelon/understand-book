import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const host = 'http://127.0.0.1:' + (process.env.PRESENTATION_TEST_PORT || '4175');

test('EX11 disk: real slider and playback reach the exact saved endpoint', async ({browser}) => {
  test.setTimeout(120000);
  const root = path.resolve(process.env.EX11_RUN_DIR!);
  const records: any[] = [];
  for (const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const context = await browser.newContext({viewport, hasTouch:viewport.width !== 960});
    const page = await context.newPage();
    const record: any = {viewport, passed:false}; records.push(record);
    try {
      await page.request.post(host + '/reset-scene');
      await page.route('**/api/**', async route => {
        const response = await route.fetch({url:route.request().url().replace(/^.*\/api/, host)});
        await route.fulfill({response});
      });
      await page.goto('/agent-presentation-visual.html');
      await page.getByRole('button', {name:'展开', exact:true}).click();
      const frame = page.frameLocator('.agent-presentation iframe');
      const snapshot = () => frame.locator('body').evaluate(() => (window as any).presentationScene.snapshot());
      const endpoint = (scene: any) => {
        expect(scene.theta).toBeCloseTo(2*Math.PI, 10);
        expect(scene.semantic_state).toBe(8);
        expect(scene.transition_progress).toBe(0);
        expect(scene.playing).toBe(false);
      };
      const slider = frame.locator('#thetaRange');
      await slider.scrollIntoViewIfNeeded();
      const box = (await slider.boundingBox())!;
      const x = box.x + 8, y = box.y + box.height/2, end = box.x + box.width - 2;
      if (viewport.width === 960) {
        await page.mouse.move(x,y); await page.mouse.down();
        await page.mouse.move(end,y,{steps:12}); await page.mouse.up();
      } else {
        const cdp = await context.newCDPSession(page);
        await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});
        for(let i=1;i<=12;i++) await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:x+(end-x)*i/12,y}]});
        await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
      }
      record.drag = {scene:await snapshot(), value:await slider.inputValue(), text:await frame.locator('#result').innerText()};
      endpoint(record.drag.scene);
      await frame.locator('body').evaluate(() => (window as any).presentation.commitState());
      const fixture = await page.request.get(host+'/fixture').then(r=>r.json());
      await expect.poll(async () => {
        const view = await page.request.post(host+'/agent/presentation.read',{data:fixture}).then(r=>r.json());
        record.savedState = view.restored_state;
        return view.restored_state?.values?.page?.theta;
      }).toBeCloseTo(2*Math.PI,10);
      await page.request.post(host+'/reopen'); await page.reload();
      await expect(frame.locator('#thetaRange')).toBeVisible();
      await expect.poll(async ()=>(await snapshot()).theta).toBeCloseTo(2*Math.PI,10);
      record.restored = await snapshot(); endpoint(record.restored);
      await frame.locator('body').evaluate(async () => (window as any).presentationScene.seek({semantic_state:7,transition_progress:.9}));
      await frame.locator('#play').click();
      await expect.poll(async ()=>(await snapshot()).semantic_state).toBe(8);
      record.playedToEnd = await snapshot(); endpoint(record.playedToEnd);
      record.middle = [];
      for (const [step,progress] of [[2,.35],[7,.999]]) {
        await frame.locator('body').evaluate(async (_,v) => (window as any).presentationScene.seek({semantic_state:v.step,transition_progress:v.progress}),{step,progress});
        const before = await snapshot();
        expect(before.theta).toBeCloseTo(2*Math.PI*(step+progress)/8,10);
        await frame.locator('body').evaluate(() => (window as any).presentation.commitState());
        const currentFixture = await page.request.get(host+'/fixture').then(r=>r.json());
        await expect.poll(async () => {
          const view = await page.request.post(host+'/agent/presentation.read',{data:currentFixture}).then(r=>r.json());
          return view.restored_state?.values?.page?.theta;
        }).toBeCloseTo(before.theta,10);
        await page.request.post(host+'/reopen'); await page.reload();
        await expect(frame.locator('#thetaRange')).toBeVisible();
        const restored = await snapshot();
        record.middle.push({step,progress,before,restored});
        expect(restored.theta).toBeCloseTo(before.theta,10);
        expect(restored.semantic_state).toBe(step);
        expect(restored.transition_progress).toBeCloseTo(progress,10);
      }
      record.passed = true;
    } catch(error) { record.failure = String(error); }
    finally {
      fs.writeFileSync(path.join(root,'endpoint-check.json'),JSON.stringify(records,null,2));
      await page.unrouteAll({behavior:'ignoreErrors'}); await context.close();
    }
  }
  expect(records.filter(r=>!r.passed).map(({viewport,failure})=>({viewport,failure}))).toEqual([]);
});
