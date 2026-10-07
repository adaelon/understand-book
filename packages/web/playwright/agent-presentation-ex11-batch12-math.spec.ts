import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const host='http://127.0.0.1:'+(process.env.PRESENTATION_TEST_PORT || '4175');

// Coordinates are read from untouched batch12 drawings. Expected relationships
// come from the frozen material: s=rθ and T(h)=22−20h.
test('EX11 batch12 independent formula and rendered geometry',async({browser})=>{
  const root=path.resolve(process.env.EX11_RUN_DIR!);
  const disk=path.basename(root)==='disk-1';
  const records:any[]=[];
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
    const page=await context.newPage();
    const record:any={viewport,cases:[],passed:false};records.push(record);
    try{
      await page.request.post(host+'/reset-scene');
      await page.route('**/api/**',async route=>{
        const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,host)});
        await route.fulfill({response});
      });
      await page.goto('/agent-presentation-visual.html');
      await page.getByRole('button',{name:'展开',exact:true}).click();
      const frame=page.frameLocator('.agent-presentation iframe');
      await expect(frame.locator(disk?'#r2Range':'#hRange')).toBeVisible();
      if(disk){
        for(const radius of [1,1.5,2,3]) for(const angle of [0,.2,Math.PI/2,Math.PI,4.5,6,2*Math.PI]){
          const actual=await frame.locator('body').evaluate((_,v)=>{
            for(const [id,value] of [['r2Range',v.radius],['thetaRange',v.angle]] as const){
              const input=document.getElementById(id) as HTMLInputElement;
              input.value=String(value);input.dispatchEvent(new Event('input',{bubbles:true}));
            }
            const stage=(window as any).Konva.stages.find((s:any)=>s.container().id==='stageHost');
            const groups=stage.getLayers()[0].getChildren();
            const discs=groups.slice(0,2).map((g:any)=>({radius:g.getChildren()[0].radius(),
              spoke:g.getChildren()[5].points(),sweep:g.getChildren()[4].getAttr('sweep')}));
            const bars=groups[2].getChildren();
            return {scene:(window as any).presentationScene.snapshot(),discs,
              track:bars[0].points(),widths:[bars[2].width(),bars[3].width()],text:document.getElementById('result')!.textContent};
          },{radius,angle});
          // Native range controls quantize to their declared step.
          const theta=actual.scene.theta,r=actual.scene.r2;
          expect(Math.abs(theta-angle)).toBeLessThan(.0011);expect(r).toBeCloseTo(radius,6);
          expect(actual.discs[1].radius/actual.discs[0].radius).toBeCloseTo(r,8);
          for(const d of actual.discs){
            expect(d.sweep).toBeCloseTo(theta,8);
            expect(d.spoke[2]).toBeCloseTo(d.radius*Math.cos(theta),7);
            expect(d.spoke[3]).toBeCloseTo(-d.radius*Math.sin(theta),7);
          }
          const unit=(actual.track[2]-actual.track[0])/(6*Math.PI);
          for(const [i,s] of [theta,r*theta].entries()){
            // The page intentionally draws a 2px origin marker at zero.
            expect(actual.widths[i]).toBeCloseTo(Math.max(2,s*unit),7);
            expect(actual.text).toContain(s.toFixed(3));
          }
          record.cases.push({radius,angle,...actual});
        }
      }else{
        for(const h of [0,.1,.25,.5,.75,.9,1]){
          const actual=await frame.locator('body').evaluate((_,h)=>{
            const input=document.getElementById('hRange') as HTMLInputElement;
            input.value=String(h*100);input.dispatchEvent(new Event('input',{bubbles:true}));
            const bars=['hitFill','missFill','mixFixed','mixVar'].map(id=>{
              const el=document.getElementById(id)!;const parent=el.parentElement!;
              const style=getComputedStyle(parent);
              return {width:el.getBoundingClientRect().width,track:parent.getBoundingClientRect().width-parseFloat(style.borderLeftWidth)-parseFloat(style.borderRightWidth)};
            });
            return {bars,text:document.getElementById('bReadout')!.textContent,value:document.getElementById('mixVal')!.textContent};
          },h);
          expect(actual.value).toBe((22-20*h).toFixed(1)+' ms');
          [2,22,2,20*(1-h)].forEach((ms,i)=>expect(Math.abs(actual.bars[i].width-actual.bars[i].track*ms/22)).toBeLessThan(.08));
          record.cases.push({h,...actual});
        }
      }
      record.passed=true;
    }catch(error){record.failure=String(error);}
    finally{
      fs.writeFileSync(path.join(root,'independent-math.json'),JSON.stringify(records,null,2));
      await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
    }
  }
  expect(records.filter(r=>!r.passed).map(r=>({viewport:r.viewport,failure:r.failure}))).toEqual([]);
});
