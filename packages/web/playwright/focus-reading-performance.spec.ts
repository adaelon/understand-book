import { expect,test } from '@playwright/test';
import { mkdir,writeFile,readFile } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { distribution,startFrameTrace } from './focus-reading-trace';

const output=path.resolve(process.env.FP1_OUTPUT??'../../docs/performance/focus-reading-fp1-20261014/results');
const url='/playwright/fixtures/focus-reading.html';
test.beforeAll(async()=>{await mkdir(output,{recursive:true});});
const save=async(name:string,value:unknown)=>writeFile(path.join(output,`${name}.json`),JSON.stringify(value,null,2));

test('environment and material inventory',async({page,browser})=>{
  await page.goto(url);
  await page.waitForFunction(()=>!!(window as any).fp1);
  const raf=await page.evaluate(()=>(window as any).fp1.schedulerSamples());
  await save('environment',{measuredAt:new Date().toISOString(),browser:browser.version(),os:os.version(),platform:os.platform(),cpu:os.cpus()[0]?.model,totalMemory:os.totalmem(),headless:true,network:'local static fixture; no backend / LLM latency',productionBundle:true,rafSchedulerMs:distribution(raf),browserEnvironment:await page.evaluate(()=>({userAgent:navigator.userAgent,dpr:devicePixelRatio,viewport:{width:innerWidth,height:innerHeight},hardwareConcurrency:navigator.hardwareConcurrency})),inventory:JSON.parse(await readFile('../../tmp/focus-reading-fp1/public/fp1/inventory.json','utf8'))});
});

for(const material of ['epub','complex','mixed'])for(const width of [1440,390]) {
  test(`continuous baseline ${material} ${width}`,async({page})=>{
    await page.setViewportSize({width,height:width===390?844:900});
    await page.goto(url);
    await page.waitForFunction(()=>!!(window as any).fp1);
    const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
    const stop=await startFrameTrace(page);
    const cold=await page.evaluate(m=>(window as any).fp1.load(m,0),material);
    await expect(page.locator('.flow-text').first()).toBeVisible();
    // A screenshot is actual rendered-content evidence; ready() alone is not.
    await page.screenshot({path:path.join(output,`continuous-${material}-${width}.png`)});
    cold.firstReadableCaptureUpperBoundMs=await page.evaluate(()=>performance.now())-cold.startedAt;
    const warm=[];
    for(let i=0;i<3;i++)warm.push(await page.evaluate(m=>(window as any).fp1.load(m,0),material));
    const growth=[cold];
    for(let chunk=1;chunk<10;chunk++) {
      const sample=await page.evaluate(({material,chunk})=>(window as any).fp1.load(material,chunk,true),{material,chunk});
      if(chunk===4||chunk===9)growth.push(sample);
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(100);
    }
    for(let i=0;i<6;i++){await page.keyboard.press('ArrowLeft');await page.waitForTimeout(100);}
    const trace=await stop(path.join(output,`continuous-${material}-${width}.trace.json.gz`));
    const report=await page.evaluate(()=>(window as any).fp1.report());
    await save(`continuous-${material}-${width}`,{cold,warmReadyMs:distribution(warm.map(s=>s.totalMs)),growth,report,trace,errors});
    expect(errors).toEqual([]);
    expect(growth[2].leaves).toBe(240);
    expect(trace.filmstripSamples).toBeGreaterThan(0);
    expect(report.inputs.some((i:any)=>i.scrollTop>0)).toBe(true);
  });
}

function merge(ranges:{start:number;end:number}[]) {
  const out:{start:number;end:number}[]=[];
  for(const r of ranges){const last=out.at(-1);if(last&&r.start<=last.end)last.end=Math.max(last.end,r.end);else out.push({start:r.start,end:r.end});}
  return out;
}
for(const [width,height,mobile] of [[1440,900,false],[1024,900,false],[390,844,true],[844,390,true],[320,844,true]] as const) {
  test(`local columns preserve source and overflow ${width}`,async({page})=>{
    await page.setViewportSize({width,height});
    await page.goto(url);await page.waitForFunction(()=>!!(window as any).fp1);
    const measurements=[];
    for(const font of [19,22]) {
      await page.evaluate(({font,mobile})=>(window as any).fp1.columns(font,mobile),{font,mobile});
      for(const material of ['epub','complex','mixed']) {
        const ready=await page.evaluate(m=>(window as any).fp1.load(m,0),material);
        const mapping=await page.evaluate(()=>(window as any).fp1.mapColumns());
        for(const leaf of mapping.leaves){
          expect(merge(leaf.runs)).toEqual(leaf.expected);
          for(let i=1;i<leaf.runs.length;i++) {
            expect(leaf.runs[i].start).toBeGreaterThanOrEqual(leaf.runs[i-1].end);
            expect(leaf.runs[i].page,`${material} ${font} ${leaf.lid} ${JSON.stringify(leaf.runs.slice(i-1,i+1))}`).toBeGreaterThanOrEqual(leaf.runs[i-1].page);
          }
        }
        measurements.push({material,font,ready,mapping});
      }
    }
    const realStress=[];
    for(const material of ['epub','complex','mixed']) {
      const ready=await page.evaluate(m=>(window as any).fp1.load(m,'stress'),material);
      const mapping=await page.evaluate(()=>(window as any).fp1.mapColumns());
      const reused=await page.evaluate(()=>(window as any).fp1.mapColumns());
      expect(reused.leaves).toEqual(mapping.leaves);
      expect(reused.mapBuilds).toBe(0);
      for(const leaf of mapping.leaves)expect(merge(leaf.runs)).toEqual(leaf.expected);
      realStress.push({material,ready,mapping,reused});
    }
    await page.evaluate(()=>(window as any).fp1.load('complex','stress'));
    await page.evaluate(()=>(window as any).fp1.stress());
    const stress=await page.evaluate(()=>(window as any).fp1.mapColumns());
    const long=stress.leaves.find((s:any)=>s.lid==='stress.long');
    expect(new Set(long.runs.map((r:any)=>r.page)).size).toBeGreaterThan(1);
    for(const leaf of stress.leaves)expect(merge(leaf.runs)).toEqual(leaf.expected);
    // Native cross-column range + canonical source map: every piece is retained.
    const selection=await page.locator('[data-lid="stress.long"]').evaluate(el=>{const r=document.createRange();r.selectNodeContents(el);const s=getSelection()!;s.removeAllRanges();s.addRange(r);return {text:s.toString(),rectCount:r.getClientRects().length};});
    expect(selection.rectCount).toBeGreaterThan(2);
    expect(selection.text).toContain('保留原文');
    const overflow=await page.locator('[data-lid="stress.code"] .asset-source').evaluate(el=>{
      const before={clientHeight:el.clientHeight,scrollHeight:el.scrollHeight,clientWidth:el.clientWidth,scrollWidth:el.scrollWidth};
      el.scrollTop=el.scrollHeight;el.scrollLeft=el.scrollWidth;
      return {...before,scrollTop:el.scrollTop,scrollLeft:el.scrollLeft,tail:el.textContent?.includes('line 99:')};
    });
    expect(overflow.scrollTop).toBeGreaterThan(0);expect(overflow.scrollLeft).toBeGreaterThan(0);expect(overflow.tail).toBe(true);
    await save(`columns-${width}`,{measurements,realStress,stress,overflow,selection:{characters:selection.text.length,rectCount:selection.rectCount},resources:await page.evaluate(()=>(window as any).fp1.resources())});
    await page.evaluate(()=>getSelection()?.removeAllRanges());
    await page.screenshot({path:path.join(output,`columns-${width}.png`)});
  });
}

for(const width of [1440,390])test(`bounded local window budget ${width}`,async({page})=>{
  await page.setViewportSize({width,height:width===390?844:900});
  await page.goto(url);await page.waitForFunction(()=>!!(window as any).fp1);
  await page.evaluate(mobile=>(window as any).fp1.columns(22,mobile),width===390);
  const samples=[];
  for(const material of ['epub','complex','mixed'])for(const chunk of (material==='complex'?[0,4,9,'images']:[0,4,9])){
    await page.evaluate(({material,chunk})=>(window as any).fp1.load(material,chunk),{material,chunk});
    const fit=await page.evaluate(()=>(window as any).fp1.fitLocalWindow());
    samples.push({material,chunk,...fit});
    expect(fit.selected,`${material}/${chunk}`).toBeGreaterThan(0);
    expect(fit.resident.columns).toBeLessThanOrEqual(fit.maxPages);
    if(chunk==='images') {
      const images=await page.locator('.image-rendered').evaluateAll(images=>images.map(el=>{const image=el as HTMLImageElement;const r=image.getBoundingClientRect();return {width:r.width,height:r.height,naturalWidth:image.naturalWidth,naturalHeight:image.naturalHeight};}));
      expect(images.length).toBeGreaterThan(0);
      for(const image of images){expect(image.naturalWidth).toBeGreaterThan(0);expect(image.height).toBeLessThanOrEqual((width===390?844:900)-110);}
      await page.screenshot({path:path.join(output,`images-${width}.png`)});
    }
  }
  await save(`bounded-${width}`,samples);
});

test('original PDF baseline uses real canvas and text layer',async({page})=>{
  await page.goto(url);await page.waitForFunction(()=>!!(window as any).fp1);
  const stop=await startFrameTrace(page);
  const start=Date.now();
  await page.evaluate(()=>(window as any).fp1.pdf());
  await expect.poll(()=>page.locator('canvas').evaluateAll(cs=>cs.some(c=>(c as HTMLCanvasElement).width>1&&(c as HTMLCanvasElement).height>1)),{timeout:60_000}).toBe(true);
  await expect.poll(()=>page.locator('.pdf-text-layer').first().textContent()).not.toBe('');
  const firstCanvasAndTextMs=Date.now()-start;
  await page.waitForTimeout(300);
  const samples=[await page.evaluate(()=>(window as any).fp1.resources())];
  const list=page.locator('.pdf-page-list');
  for(let i=1;i<=6;i++){
    await list.evaluate((el,i)=>{el.scrollTop=i*el.clientHeight*.8;},i);
    await page.waitForTimeout(300);
    samples.push(await page.evaluate(()=>(window as any).fp1.resources()));
  }
  await page.screenshot({path:path.join(output,'pdf.png')});
  const trace=await stop(path.join(output,'pdf.trace.json.gz'));
  await save('pdf',{firstCanvasAndTextMs,samples,trace});
  expect(Math.max(...samples.map(s=>s.canvasPixels))).toBeLessThanOrEqual(12_000_000);
});
