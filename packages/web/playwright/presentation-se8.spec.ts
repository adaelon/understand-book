import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

const run=process.env.SE8_RUN;
test.skip(!run,'SE8_RUN names a completed paired experiment');

test('SE8 R1 exact static revision remains readable in three environments',async({browser})=>{
  const expected=JSON.parse(fs.readFileSync(path.resolve('../../docs/performance/explanation-evolution/se8/r1-expected.json'),'utf8'));
  const baseline=JSON.parse(fs.readFileSync(path.resolve('../../docs/performance/explanation-evolution/se8/r1-base.json'),'utf8'));
  const records:any[]=[];
  for(const arm of ['B','A']){
    const file=path.join(run!,`R1-${arm}/native/version.json`);
    if(!fs.existsSync(file)){records.push({arm,status:'not_delivered'});continue;}
    const version=JSON.parse(fs.readFileSync(file,'utf8'));
    expect(version.reference.presentation_id).toBe(baseline.reference.presentation_id);
    expect(version.reference.revision).toBe(2);
    expect(version.content.content_files['index.html']).toBe(expected.html);
    expect(version.content.readable_content).toBe(expected.readable_content);
    expect(version.content.method_refs??[]).toEqual([]);
    for(const key of ['title','entrypoint','assumptions','source_bindings','initial_state','state_contract','animation_assets'])
      expect(version.content[key]).toEqual(baseline.content[key]);
    for(const viewport of [{width:320,height:420},{width:640,height:240},{width:960,height:720}]){
      const context=await browser.newContext({viewport,hasTouch:viewport.width!==960});
      try{
        const page=await context.newPage();
        await page.route('**/se8-static',route=>route.fulfill({contentType:'text/html',body:`<!doctype html><html><body style="margin:0"><iframe sandbox="allow-scripts" style="border:0;width:100%;height:100vh"></iframe><script type="module">import {presentationDocument} from '/src/presentation-document.ts';window.openStatic=v=>{document.querySelector('iframe').srcdoc=presentationDocument({...v.content,reference:v.reference,sources:[],readable_view:{parts:[],sources:[]}},'se8-static')};</script></body></html>`}));
        await page.goto('/se8-static');await page.waitForFunction(()=>typeof(window as any).openStatic==='function');
        await page.evaluate(v=>(window as any).openStatic(v),version);
        const frame=page.frameLocator('iframe');
        await expect(frame.locator('.relation')).toHaveText(expected.new_text);
        for(const selector of ['#definition','#example-one','#example-two']){
          await frame.locator(selector).scrollIntoViewIfNeeded();await expect(frame.locator(selector)).toBeVisible();
          await page.screenshot({path:path.join(run!,`R1-${arm}/${viewport.width}-${selector.slice(1)}.png`)});
        }
        expect(await frame.locator('body').evaluate(()=>document.documentElement.scrollWidth<=document.documentElement.clientWidth+1)).toBe(true);
        expect(await frame.locator('button,input,canvas,svg,video').count()).toBe(0);
        records.push({arm,viewport,status:'passed',reference:version.reference});
      }finally{await context.close();}
    }
  }
  fs.writeFileSync(path.join(run!,'r1-reader.json'),JSON.stringify({records},null,2));
  expect(records.filter(r=>r.status==='passed').length).toBe(6);
});
