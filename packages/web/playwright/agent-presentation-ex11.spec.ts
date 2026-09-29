import { expect, test } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

test('EX11 formal supply: real point drag, save, reopen and follow-up in three viewports', async ({ browser }) => {
  const evidence = path.resolve('../../docs/performance/ex11-local-demonstrations/ex11.1');
  const results: unknown[] = [];
  for (const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]) {
    const touch = viewport.width !== 960;
    const context = await browser.newContext({viewport,hasTouch:touch});
    const page = await context.newPage();
    await page.request.post('http://127.0.0.1:4175/reset-scene');
    await page.route('**/api/**', async route => {
      const response = await route.fetch({url:route.request().url().replace(/^.*\/api/,'http://127.0.0.1:4175')});
      await route.fulfill({response});
    });
    const errors: string[]=[];
    page.on('pageerror',error=>{errors.push(error.message);console.log('EX11_PAGE_ERROR',error.message)});
    page.on('console',message=>{if(message.type()==='error')console.log('EX11_CONSOLE',message.text())});
    await page.addInitScript(()=>window.addEventListener('error',event=>console.error('window error:',event.message)));
    await page.goto('/agent-presentation-visual.html');
    const frame = page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator('#readout')).toContainText('80.00');
    await page.getByRole('button',{name:'展开',exact:true}).click();
    await frame.locator('#scene').scrollIntoViewIfNeeded();
    const box = (await frame.locator('#scene').boundingBox())!;
    const x=box.x+80,y=box.y+50;
    if(touch){
      const cdp=await context.newCDPSession(page);
      await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x,y}]});
      for(let i=1;i<=8;i++) await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:x+i*12,y}]});
      // Read while finger is still down: a release-only update is a failure.
      await expect(frame.locator('#readout')).not.toContainText('80.00');
      await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    }else{
      await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+96,y,{steps:8});
      await expect(frame.locator('#readout')).not.toContainText('80.00');await page.mouse.up();
    }
    const geometry=await frame.locator('body').evaluate(()=>{
      const stage=(window as any).Konva.stages[0],point=stage.findOne('Circle'),line=stage.findOne('Line');
      return {version:(window as any).Konva.version,stages:(window as any).Konva.stages.length,x:point.x(),points:line.points(),diameter:point.radius()*2*point.getAbsoluteScale().x};
    });
    expect(geometry.version).toBe("10.7.0");expect(geometry.stages).toBe(1);expect(geometry.x).toBeGreaterThan(150);expect(geometry.points[2]).toBe(geometry.x);expect(geometry.diameter).toBeGreaterThanOrEqual(44);
    const fixture=await page.request.get('http://127.0.0.1:4175/fixture').then(r=>r.json());
    await expect.poll(async()=>{
      const view=await page.request.post('http://127.0.0.1:4175/agent/presentation.read',{data:fixture}).then(r=>r.json());
      return view.restored_state?.values?.page?.x;
    }).toBe(geometry.x);
    await page.screenshot({path:path.join(evidence,`reader-${viewport.width}.png`)});
    await page.request.post('http://127.0.0.1:4175/reopen');await page.reload();
    await expect(frame.locator('#readout')).toContainText(geometry.x.toFixed(2));
    await expect(page.getByText('已恢复上次保存的现场')).toBeVisible();
    await page.getByRole('textbox',{name:'针对当前现场追问'}).fill('解释当前点的位置');
    await page.getByRole('button',{name:'发送追问',exact:true}).click();
    await expect(page.getByTestId('follow-up-status')).toHaveText('追问已完成');
    const requests=await page.request.get('http://127.0.0.1:4175/requests').then(r=>r.json());
    const message=requests.at(-1).filter((m:any)=>m.role==='User').at(-1).content;
    expect(message).toContain(`"x":${geometry.x}`);expect(errors).toEqual([]);
    results.push({viewport,touch,geometry,errors,restored:true,follow_up:true});
    await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
  }
  fs.writeFileSync(path.join(evidence,'reader-check.json'),JSON.stringify(results,null,2));
});
