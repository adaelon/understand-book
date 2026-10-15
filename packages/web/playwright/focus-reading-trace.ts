import type { Page } from '@playwright/test';
import { gzipSync } from 'node:zlib';
import { writeFile } from 'node:fs/promises';

export const distribution = (values:number[]) => {
  const sorted=[...values].sort((a,b)=>a-b);
  const at=(q:number)=>sorted.length?sorted[Math.max(0,Math.ceil(sorted.length*q)-1)]:null;
  return {count:values.length,p50:at(.5),p95:at(.95),p99:at(.99),max:at(1)};
};

/** Pace from the actual input, so a delayed automation command never makes
 * the next nominal one-second turn catch up as a rapid second input. */
export async function waitAfterInput(page: Page, observer: 'fp4' | 'fp6', periodMs: number) {
  await page.evaluate(async ({ observer, periodMs }) => {
    const input = (window as any)[observer].samples().at(-1).input;
    await new Promise(resolve => setTimeout(resolve, Math.max(1, input + periodMs - performance.now())));
  }, { observer, periodMs });
}

/** P3 drives the public button on the browser clock, excluding automation's
 * actionability delay while keeping exactly one second between actual inputs. */
export async function pacedReaderClick(page: Page, observer: 'fp4' | 'fp6', name: string, direction: number) {
  await page.evaluate(async ({ observer, name, direction }) => {
    const measurement = (window as any)[observer], previous = measurement.samples().at(-1);
    if (previous) await new Promise(resolve => setTimeout(resolve, Math.max(1, previous.input + 1000 - performance.now())));
    measurement.arm(name);
    const host = observer === 'fp4' ? '.focus-reader-body' : '.pdf-reader-pane';
    document.querySelector<HTMLButtonElement>(`${host} .focus-page-${direction > 0 ? 'next' : 'prev'}`)!.click();
  }, { observer, name, direction });
  await page.waitForFunction(({ observer, name }) => {
    const sample = (window as any)[observer].samples().at(-1);
    return sample?.name === name && sample.end !== null;
  }, { observer, name });
}

/** CDP frame evidence stays distinct from RAF scheduling and readiness marks. */
export async function startFrameTrace(page:Page, inputMark = /^fp1-input-\d+$/) {
  const session=await page.context().newCDPSession(page);
  // Real frame events and captured pixels are the evidence during a trace.
  // Per-frame getComputedStyle diagnostics would force main-thread updates
  // alongside an otherwise compositor-only animation, especially at 165Hz.
  await page.evaluate(() => { (window as any).focusTraceActive = true; });
  const framesOnly = process.env.FP7_FRAME_EVENTS_ONLY === '1';
  const initialScreenshot = framesOnly ? undefined : (await page.screenshot({ type: 'jpeg', quality: 80 })).toString('base64');
  const start = () => session.send('Tracing.start',{categories:`devtools.timeline,disabled-by-default-devtools.timeline.frame,${framesOnly ? '' : 'disabled-by-default-devtools.screenshot,'}blink.user_timing,cc,benchmark`,transferMode:'ReturnAsStream'});
  const segments: any[] = [];
  const collect = async () => {
    const complete=new Promise<any>(resolve=>session.once('Tracing.tracingComplete',resolve));
    await session.send('Tracing.end');
    const {stream}=await complete;
    let raw='';
    while(true){const chunk=await session.send('IO.read',{handle:stream});raw+=chunk.base64Encoded?Buffer.from(chunk.data,'base64').toString():chunk.data;if(chunk.eof)break;}
    await session.send('IO.close',{handle:stream});
    segments.push(JSON.parse(raw));
  };
  await start();
  const stop = async (file:string) => {
    await collect();
    const events = segments.flatMap(segment => segment.traceEvents);
    const raw = JSON.stringify({ ...segments[0], traceEvents: events });
    await writeFile(file,gzipSync(raw));
    await session.detach();
    await page.evaluate(() => { delete (window as any).focusTraceActive; });
    const counts:Record<string,number>={};
    for(const event of events)if(/DrawFrame|Presented|SubmitCompositorFrame|DroppedFrame|Screenshot|PipelineReporter/.test(event.name))counts[event.name]=(counts[event.name]??0)+1;
    const shots=events.filter((e:any)=>e.name==='Screenshot'&&e.args?.snapshot).sort((a:any,b:any)=>a.ts-b.ts);
    const marks=events.filter((e:any)=>inputMark.test(e.name)).sort((a:any,b:any)=>a.ts-b.ts);
    const captured:number[]=[];
    for(const [index,mark] of marks.entries()){
      const before=shots.filter((s:any)=>s.ts<mark.ts).at(-1);
      const next=marks[index+1]?.ts??Infinity;
      const after=shots.find((s:any)=>s.ts>=mark.ts&&s.ts<next&&before&&s.args.snapshot!==before.args.snapshot);
      if(after)captured.push((after.ts-mark.ts)/1000);
    }
    const layouts=events.filter((e:any)=>e.name==='Layout'&&e.dur);
    const paints=events.filter((e:any)=>e.name==='Paint'&&e.dur);
    return {initialScreenshot,framesOnly,segments:segments.length,eventCounts:counts,filmstripSamples:shots.length,handlerMarkToChangedCaptureMs:distribution(captured),layoutMs:layouts.reduce((n:number,e:any)=>n+e.dur/1000,0),paintMs:paints.reduce((n:number,e:any)=>n+e.dur/1000,0),physicalPresentation:{supported:false,inputToPresentedFrameMs:null,droppedFrameRatio:null,reason:'Capture and frame events describe browser rendering, not physical display presentation. RAF alone is not presentation evidence.'}};
  };
  // Call only between settled interactions. WebView2 caps each trace at 450
  // pictures; short segments retain all actual frames and original timestamps.
  return Object.assign(stop, { checkpoint: async () => { await collect(); await start(); } });
}
