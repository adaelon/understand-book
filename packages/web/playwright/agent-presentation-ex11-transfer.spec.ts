import {test, expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

// Selectors are taken from each untouched generated page; expectations come from
// the frozen input and the shared Reader contract, not from its implementation.
test('EX11 transfer: real controls, exact saved scene and follow-up', async ({browser}) => {
  test.setTimeout(150000);
  const root = path.resolve(process.env.EX11_RUN_DIR!);
  const selectors = JSON.parse(fs.readFileSync(path.join(root, 'reader-selectors.json'), 'utf8'));
  const records: any[] = [];
  for (const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const context = await browser.newContext({viewport, hasTouch:viewport.width !== 960});
    const page = await context.newPage();
    page.setDefaultTimeout(12000);
    const record: any = {viewport, errors:[], passed:false};
    records.push(record);
    page.on('pageerror', error => record.errors.push(error.message));
    try {
      await page.request.post('http://127.0.0.1:4175/reset-scene');
      await page.route('**/api/**', async route => {
        const response = await route.fetch({url:route.request().url().replace(/^.*\/api/, 'http://127.0.0.1:4175')});
        await route.fulfill({response});
      });
      await page.goto('/agent-presentation-visual.html');
      const frame = page.frameLocator('.agent-presentation iframe');
      // Ignore IEEE-754 round-off (e.g. 3.525 vs 3.5250000000000004), while
      // preserving mathematical scene precision well below a visible change.
      const stable = (value:any):any => typeof value === 'number' ? Math.round(value*1e10)/1e10
        : Array.isArray(value) ? value.map(stable)
        : value && typeof value === 'object' ? Object.fromEntries(Object.entries(value).map(([k,v])=>[k,stable(v)])) : value;
      const snapshot = async () => stable(await frame.locator('body').evaluate(() => (window as any).presentationScene.snapshot()));
      const seek = (step:number, progress:number) => frame.locator('body').evaluate(async (_, target) => {
        await (window as any).presentationScene.seek({semantic_state:target.step, transition_progress:target.progress});
      }, {step, progress});
      await expect(frame.locator(selectors.play)).toBeVisible();
      await page.getByRole('button', {name:'展开', exact:true}).click();
      record.initial = await snapshot();
      record.drags = [];
      for (const parameter of selectors.parameters ?? (selectors.parameter ? [selectors.parameter] : [])) {
        const control = frame.locator(parameter);
        await control.scrollIntoViewIfNeeded();
        const before = await control.inputValue();
        const box = (await control.boundingBox())!;
        const min = Number(await control.getAttribute('min'));
        const max = Number(await control.getAttribute('max'));
        const start = box.x+14+(box.width-28)*(Number(before)-min)/(max-min);
        const end = box.x+14+(box.width-28)*(Number(before) > (max+min)/2 ? .25 : .75);
        const y = box.y+box.height/2;
        if (viewport.width === 960) {
          await page.mouse.move(start,y); await page.mouse.down(); await page.mouse.move(end,y,{steps:12});
          record.drag = {before, held:await control.inputValue(), scene:await snapshot()};
          await page.mouse.up();
        } else {
          const cdp = await context.newCDPSession(page);
          await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:start,y}]});
          for(let i=1;i<=12;i++) await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:start+(end-start)*i/12,y}]});
          record.drag = {before, held:await control.inputValue(), scene:await snapshot()};
          await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
        }
        expect(record.drag.held).not.toBe(before);
        expect(record.drag.scene.playing).toBe(false);
        record.drags.push({parameter,...record.drag});
      }
      record.positions = [];
      for(const [step,progress] of [[2,.35],[1,.2],[1,.2]]) {
        await seek(step,progress);
        record.positions.push({scene:await snapshot(), text:await frame.locator(selectors.result).innerText(),
          media:await frame.locator('video').evaluateAll(nodes=>nodes.map(n=>({time:(n as HTMLVideoElement).currentTime,paused:(n as HTMLVideoElement).paused,controls:(n as HTMLVideoElement).controls})))});
      }
      expect(record.positions[1]).toEqual(record.positions[2]);
      expect(record.positions[0].scene.playing).toBe(false);
      await seek(2,.35);
      await frame.locator(selectors.previous).click();
      await expect.poll(async()=>{const s=await snapshot();return s.semantic_state+s.transition_progress;}).toBeLessThan(2.35);
      record.previous = await snapshot();
      expect(record.previous.playing).toBe(false);
      expect(record.previous.semantic_state + record.previous.transition_progress).toBeLessThan(2.35);
      await seek(0,0);
      await frame.locator(selectors.play).click();
      record.playing = await snapshot();
      await page.waitForTimeout(650);
      record.advanced = await snapshot();
      await frame.locator(selectors.play).click();
      record.paused = await snapshot();
      expect(record.paused.playing).toBe(false);
      record.playbackAdvanced = record.paused.semantic_state + record.paused.transition_progress > 0;
      record.playbackGeometry = await frame.locator('video').evaluateAll(ns=>ns.map(n=>({time:n.currentTime,paused:n.paused,rect:n.getBoundingClientRect().toJSON(),viewport:{width:innerWidth,height:innerHeight}})));
      // Optional second demo is configured only for the mixed-process input.
      if (selectors.secondDemo) {
        await frame.locator(selectors.secondDemo).click(); await seek(1,.25);
      }
      if (selectors.firstDemo) await frame.locator(selectors.firstDemo).click();
      await seek(2,.35);
      record.savedSnapshot = await snapshot();
      record.savedText = await frame.locator(selectors.result).innerText();
      await frame.locator('body').evaluate(() => (window as any).presentation.commitState());
      const fixture = await page.request.get('http://127.0.0.1:4175/fixture').then(r=>r.json());
      await expect.poll(async()=> {
        const view = await page.request.post('http://127.0.0.1:4175/agent/presentation.read',{data:fixture}).then(r=>r.json());
        record.savedState = view.restored_state;
        const normalize = (s:string) => s.replace(/\s+/g,'');
        return view.restored_state?.observed_result && normalize(view.restored_state.observed_result).includes(normalize(record.savedText));
      }).toBe(true);
      record.targets = await frame.locator('button,input[type=range]').evaluateAll(nodes=>nodes.map(n=>{
        const box=n.getBoundingClientRect(); return {id:n.id,width:box.width,height:box.height};
      }).filter(n=>n.width>0&&n.height>0));
      await frame.locator(selectors.result).scrollIntoViewIfNeeded();
      await page.screenshot({path:path.join(root,`reader-${viewport.width}.png`)});
      await page.request.post('http://127.0.0.1:4175/reopen'); await page.reload();
      await expect(frame.locator(selectors.play)).toBeVisible();
      await expect.poll(snapshot).toEqual(record.savedSnapshot);
      record.restoredSnapshot = await snapshot();
      expect(await frame.locator(selectors.result).innerText()).toBe(record.savedText);
      await page.getByRole('textbox',{name:'针对当前现场追问'}).fill('解释我保存的这一步和当前参数');
      await page.getByRole('button',{name:'发送追问',exact:true}).click();
      await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
      const requests = await page.request.get('http://127.0.0.1:4175/requests').then(r=>r.json());
      record.followUp = requests.at(-1).filter((m:any)=>m.role==='User').at(-1).content;
      expect(record.followUp).toContain(JSON.stringify(record.savedState.values.page));
      expect(record.targets.every((n:any)=>n.width>=44&&n.height>=44)).toBe(true);
      expect(record.errors).toEqual([]);
      // Video play() can start asynchronously; inspect the state while it is
      // actually advancing, rather than requiring immediate playback on click.
      record.playbackReported = record.advanced.playing === true;
      record.passed = record.playbackReported && record.playbackAdvanced;
      if(!record.passed) record.failure = !record.playbackAdvanced ? 'Play control did not advance the scene' : 'Page advanced during playback while snapshot().playing reported false';
    } catch(error) {
      record.failure = String(error);
      record.host = await page.locator('.agent-presentation').innerText().catch(()=> 'unavailable');
      await page.screenshot({path:path.join(root,`reader-failed-${viewport.width}.png`)}).catch(()=>{});
    } finally {
      fs.writeFileSync(path.join(root,'reader-check.json'),JSON.stringify(records,null,2));
      await page.unrouteAll({behavior:'ignoreErrors'}); await context.close();
    }
  }
  expect(records.filter(record=>!record.passed).map(({viewport,failure})=>({viewport,failure}))).toEqual([]);
});
