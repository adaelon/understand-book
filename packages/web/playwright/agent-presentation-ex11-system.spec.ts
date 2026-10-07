import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const host='http://127.0.0.1:'+(process.env.PRESENTATION_TEST_PORT || '4175');

// Batch8 system-1's untouched SVG and bars. Expectations come from T=22-20h.
test('EX11 system: independent latency and actual geometry preserve media position',async({browser})=>{
  test.setTimeout(60000);
  const root=path.resolve(process.env.EX11_RUN_DIR!);const results:any[]=[];
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const context=await browser.newContext({viewport});const page=await context.newPage();
    await page.request.post(host+'/reset-scene');
    await page.route('**/api/**',async route=>{
      const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,host)});
      await route.fulfill({response});
    });
    await page.goto('/agent-presentation-visual.html');
    const frame=page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator('#aPlay')).toBeVisible();
    await page.getByRole('button',{name:'展开',exact:true}).click();
    await frame.locator('body').evaluate(async()=>{await (window as any).presentationScene.seek({semantic_state:2,transition_progress:.35});});
    for(const h of [0,.1,.25,.5,.75,.9,1]) {
      const actual=await frame.locator('body').evaluate((_,h)=>{
        const slider=document.querySelector('#bH') as HTMLInputElement;slider.value=String(h);slider.dispatchEvent(new Event('input',{bubbles:true}));
        const marker=document.querySelector('#marker')!,extra=document.querySelector('#avgSeg') as HTMLElement;
        return {text:document.querySelector('#avgVal')!.textContent,x:Number(marker.getAttribute('cx')),y:Number(marker.getAttribute('cy')),
          extraWidth:extra.getBoundingClientRect().width,trackWidth:extra.parentElement!.clientWidth,scene:(window as any).presentationScene.snapshot()};
      },h);
      const expected=22-20*h;
      const sample={viewport,h,expected,actual};results.push(sample);
      fs.writeFileSync(path.join(root,'independent-system.json'),JSON.stringify(results,null,2));
      expect(Number(actual.text!.match(/[\d.]+/)![0])).toBeCloseTo(expected,1);
      expect(actual.x).toBeCloseTo(40+268*h,2);expect(actual.y).toBeCloseTo(160-6*expected,2);
      expect(Math.abs(actual.extraWidth/actual.trackWidth*22-(20-20*h))).toBeLessThan(.02);
      expect(actual.scene.semantic_state).toBe(2);expect(actual.scene.transition_progress).toBeCloseTo(.35,8);
    }
    await frame.locator('#chart').scrollIntoViewIfNeeded();
    await page.screenshot({path:path.join(root,`system-chart-${viewport.width}.png`)});
    await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
  }
});
