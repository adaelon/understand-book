import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

for (const reject of [false, true]) test(`async restoration ${reject ? 'fails without saving defaults' : 'waits before readiness and capture'}`, async ({page}) => {
  const bridge = readFileSync(new URL('../src/presentation-bridge.js', import.meta.url), 'utf8');
  await page.goto('about:blank');
  await page.evaluate(({bridge,reject}) => {
    (window as any).events=[];
    window.addEventListener('message',event=>{
      (window as any).events.push(event.data);
    });
    const frame=document.createElement('iframe');frame.setAttribute('sandbox','allow-scripts');
    frame.srcdoc=`<script>(${bridge})({channel:'agent-presentation',sources:[],initialState:{},restoredState:{values:{page:{time:1.5}}}})</script><p id="result">Default frame</p><script>
      let time=0;presentation.registerStateReader(()=>({values:{time}}));
      presentation.registerStateRestorer(saved=>new Promise((resolve,reject)=>{window.finishRestore=()=>{${reject ? "reject(new Error('decode failed'))" : "time=saved.values.page.time;document.querySelector('#result').textContent='Restored '+time;presentation.commitState();resolve()"}}}));
    </script>`;
    document.body.append(frame);
  },{bridge,reject});
  const frame=page.frameLocator('iframe');
  await expect.poll(()=>frame.locator('body').evaluate(()=>typeof (window as any).finishRestore)).toBe('function');
  await page.locator('iframe').evaluate((f:HTMLIFrameElement)=>f.contentWindow!.postMessage({channel:'agent-presentation',kind:'snapshot',request_id:7},'*'));
  expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>['ready','state'].includes(e.kind)))).toEqual([]);
  await frame.locator('body').evaluate(()=>(window as any).finishRestore());
  if(reject){
    await expect.poll(()=>page.evaluate(()=>(window as any).events.some((e:any)=>e.kind==='error'))).toBe(true);
    expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>['ready','state'].includes(e.kind)))).toEqual([]);
  }else{
    await expect.poll(()=>page.evaluate(()=>(window as any).events.find((e:any)=>e.kind==='state'&&e.request_id===7)?.state.values.page.time)).toBe(1.5);
    expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>e.kind==='ready').length)).toBe(1);
  }
});

test("dynamic sources are labeled and checked locally without blocking replaced content", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  const labels = readFileSync(new URL("../src/source-chip.js", import.meta.url), "utf8").replace('export function', 'function');
  await page.goto('about:blank');
  await page.evaluate(({ bridge, labels }) => {
    (window as any).events = [];
    window.addEventListener('message', event => (window as any).events.push(event.data));
    const frame = document.createElement('iframe'); frame.setAttribute('sandbox', 'allow-scripts');
    frame.srcdoc = `<script>${labels};(${bridge})({channel:'agent-presentation',sources:[{source_ref_id:'bound',label:'Book · Evidence'}],initialState:{}})</script>
      <button id="replace" onclick="document.querySelector('#region').innerHTML='<p>Updated result</p><button data-source-ref=bound>invented label</button><a data-source-ref=unknown>invented source</a>'">Replace</button>
      <section id="region"><p>Original result</p></section>`;
    document.body.appendChild(frame);
  }, { bridge, labels });
  const frame = page.frameLocator('iframe');
  await frame.locator('#replace').click();
  await expect(frame.locator('#region p')).toHaveText('Updated result');
  await expect(frame.locator('#region')).toHaveCSS('opacity', '1');
  await expect(frame.locator('[data-presentation-mask]')).toHaveCount(0);
  await expect(frame.locator('[data-source-ref=bound]')).toHaveText('Evidence [1]');
  await expect(frame.locator('[data-source-ref=unknown]')).toHaveText('来源不可用');
  await frame.locator('[data-source-ref=unknown]').click();
  await frame.locator('[data-source-ref=bound]').click();
  await expect.poll(() => page.evaluate(() => (window as any).events.filter((e:any) => e.kind === 'source')))
    .toEqual([{ channel: 'agent-presentation', kind: 'source', source_ref_id: 'bound' }]);
  expect(await page.evaluate(() => (window as any).events.filter((e:any) => e.kind === 'observe'))).toEqual([]);
});

test("local rendering preserves the focused control for repeated keyboard input", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:'agent-presentation',sources:[],initialState:{}})</script>
      <input id="count" type="range" min="0" max="3" value="2"><output id="result">2/3</output>
      <script>document.querySelector('#count').oninput=e=>document.querySelector('#result').textContent=e.target.value+'/3';</script>`;
    document.body.appendChild(frame);
  }, bridge);
  const frame = page.frameLocator("iframe");
  await expect(frame.locator("#result")).toBeVisible();
  await frame.locator("#count").press("Home");
  await expect(frame.locator("#result")).toHaveText("0/3");
  await expect(frame.locator("#result")).toBeVisible();
  await expect(frame.locator("#count")).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(frame.locator("#result")).toHaveText("1/3");
});

test("restore runs after page DOMContentLoaded initialization without saving again", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(() => {
    (window as any).events = [];
    window.addEventListener("message", event => {
      (window as any).events.push(event.data);
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:'agent-presentation',sources:[],initialState:{count:2},restoredState:{values:{controls:[{key:'count',type:'range',value:'1'}],page:{count:1}},visible_step:'explain'}})</script>
      <input id="count" type="range" min="0" max="3" value="2"><output id="result"></output>
      <script>document.addEventListener('DOMContentLoaded',()=>{
        const input=document.querySelector('#count'),result=document.querySelector('#result');
        input.value='2'; const render=()=>result.textContent=input.value+'/3';
        input.addEventListener('input',render); render();
        window.presentation.registerStateReader(()=>({values:{count:Number(input.value)}}));
        window.presentation.registerStateRestorer(scene=>{input.value=String(scene.values.page.count);render();});
      });</script>`;
    document.body.appendChild(frame);
  }, bridge);
  await expect(page.frameLocator("iframe").locator("#result")).toHaveText("1/3");
  await page.locator("iframe").evaluate((node: HTMLIFrameElement) => node.contentWindow!.postMessage({ channel: "agent-presentation", kind: "snapshot", request_id: 2 }, "*"));
  await expect.poll(() => page.evaluate(() => (window as any).events.filter((e: any) => e.kind === "state").length)).toBe(1);
  const events = await page.evaluate(() => (window as any).events);
  expect(events.some((e: any) => e.kind === "restore-partial")).toBe(false);
  expect(events.filter((e: any) => e.kind === "state")[0]).toMatchObject({ request_id: 2, state: { values: { page: { count: 1 } } } });
});

test("snapshot reads visible results, custom step and live controls in one browser task", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(() => {
    (window as any).scenes = [];
    window.addEventListener("message", event => {
      if (event.data.kind === "state") (window as any).scenes.push(event.data);
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:'agent-presentation',sources:[],initialState:{}})</script>
      <p hidden>Old result: 99/100</p><p style="display:none">Old step</p>
      <input id="count" type="range" min="0" max="3" value="2"><input id="selected" type="checkbox" checked>
      <output id="result">2/3</output><svg aria-label="Two thirds filled"></svg>
      <script>window.presentation.registerStateReader(()=>({values:{count:Number(document.querySelector('#count').value)},visible_step:'compare'}));</script>`;
    document.body.appendChild(frame);
  }, bridge);
  await expect(page.frameLocator("iframe").locator("#result")).toBeVisible();
  await page.locator("iframe").evaluate((node: HTMLIFrameElement) => node.contentWindow!.postMessage({ channel: "agent-presentation", kind: "snapshot", request_id: 1 }, "*"));
  await expect.poll(() => page.evaluate(() => (window as any).scenes.length)).toBe(1);
  const scene = await page.evaluate(() => (window as any).scenes[0]);
  expect(scene.request_id).toBe(1);
  expect(scene.state.values.page).toEqual({ count: 2 });
  expect(scene.state.values.controls[1].checked).toBe(true);
  expect(scene.state.visible_step).toBe("compare");
  expect(scene.state.observed_result).toContain("2/3");
  expect(scene.state.observed_result).toContain("Two thirds filled");
  expect(scene.state.observed_result).not.toContain("Old");
});

test("editing focus reports only a boolean bound to the host content generation", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.setContent('<button id="outside">Outside</button>');
  await page.evaluate(() => {
    (window as any).focusEvents = [];
    window.addEventListener("message", event => {
      if (event.data.kind === "ready") {
        (event.source as Window).postMessage({ channel: "agent-presentation", kind: "theme", generation: 7, values: {} }, "*");
      }
      if (event.data.kind === "editing-focus") (window as any).focusEvents.push(event.data);
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:'agent-presentation',sources:[],initialState:{}})</script><input id="editor" value="draft"><p>Visible result</p>`;
    document.body.appendChild(frame);
  }, bridge);

  const editor = page.frameLocator("iframe").locator("#editor");
  await expect(editor).toBeVisible();
  await editor.click();
  await expect.poll(() => page.evaluate(() => (window as any).focusEvents.at(-1))).toMatchObject({
    kind: "editing-focus",
    generation: 7,
    editing: true,
  });
  await page.locator("#outside").click();
  await expect.poll(() => page.evaluate(() => (window as any).focusEvents.at(-1))).toMatchObject({
    kind: "editing-focus",
    generation: 7,
    editing: false,
  });
  expect(await page.evaluate(() => Object.keys((window as any).focusEvents[0]).sort())).toEqual([
    "channel", "editing", "generation", "kind",
  ]);
});

test('locating a registered teaching region reveals the same objects without changing their model state', async ({ page }) => {
  const bridge = readFileSync(new URL('../src/presentation-bridge.js', import.meta.url), 'utf8');
  await page.goto('about:blank');
  await page.evaluate(bridge => {
    (window as any).events = [];
    window.addEventListener('message', event => (window as any).events.push(event.data));
    const frame = document.createElement('iframe'); frame.setAttribute('sandbox', 'allow-scripts');
    frame.srcdoc = `<script>(${bridge})({channel:'teaching',sources:[],initialState:{},teachingBinding:{regions:[{region_id:'retained'}]}})</script>
      <p style="height:900px">章节路线只选择甲和丙。</p><section data-teaching-region="retained"><p>候选目录仍然保留乙。</p><input id="selection" value="甲、丙"></section>
      <script>presentation.registerStateReader(()=>({values:{selected:document.querySelector('#selection').value}}));</script>`;
    document.body.append(frame);
  }, bridge);
  await expect.poll(() => page.evaluate(() => (window as any).events.some((event: any) => event.kind === 'ready'))).toBe(true);
  const frame = page.frameLocator('iframe');
  await page.locator('iframe').evaluate((element: HTMLIFrameElement) => element.contentWindow!.postMessage({ channel: 'teaching', kind: 'locate-region', region_id: 'retained' }, '*'));
  await expect.poll(() => frame.locator('body').evaluate(() => window.scrollY)).toBeGreaterThan(0);
  await expect(frame.locator('#selection')).toHaveValue('甲、丙');
  expect(await frame.locator('[data-teaching-region]').evaluate(element => element.getAnimations().length)).toBe(1);
  expect(await page.evaluate(() => (window as any).events.filter((event: any) => event.kind === 'state'))).toEqual([]);
  await page.locator('iframe').evaluate((element: HTMLIFrameElement) => element.contentWindow!.postMessage({ channel: 'teaching', kind: 'locate-region', region_id: 'selection' }, '*'));
  await expect.poll(() => page.evaluate(() => (window as any).events.filter((event: any) => event.kind === 'region-located').at(-1))).toMatchObject({ region_id: 'selection', located: false });
});

test('only explicitly revealed, registered help is acknowledged; local exploration and prefilled wording stay distinct', async ({ page }) => {
  const bridge = readFileSync(new URL('../src/presentation-bridge.js', import.meta.url), 'utf8');
  await page.goto('about:blank');
  await page.evaluate(bridge => {
    (window as any).events = [];
    window.addEventListener('message', event => {
      (window as any).events.push(event.data);
      if (event.data.kind === 'teaching-help') (event.source as Window).postMessage({ channel: 'teaching', kind: 'teaching-help-recorded', request_id: event.data.request_id }, '*');
    });
    const frame = document.createElement('iframe'); frame.setAttribute('sandbox', 'allow-scripts');
    frame.srcdoc = `<script>(${bridge})({channel:'teaching',sources:[],initialState:{},teachingBinding:{regions:[{region_id:'help'}]}})</script>
      <input id="selected" type="checkbox"><section id="help" hidden>没有选用乙，不等于删除乙。<span hidden>不应当作已展示</span></section>
      <button id="reveal" onclick="document.querySelector('#help').hidden=false;presentation.reportTeachingHelp('invite','explanation','help').then(()=>document.querySelector('#done').textContent='已记录')">看解释</button>
      <button id="prefill" onclick="presentation.requestTeachingAction('invite','乙仍然存在。')">用这段文字继续</button><output id="done"></output>`;
    document.body.append(frame);
  }, bridge);
  const frame = page.frameLocator('iframe');
  await expect(frame.locator('#selected')).toBeVisible();
  await frame.locator('#selected').check();
  const hiddenError = await frame.locator('body').evaluate(async () => {
    try { await (window as any).presentation.reportTeachingHelp('invite', 'hint', 'help'); return ''; }
    catch (failure) { return (failure as Error).message; }
  });
  expect(hiddenError).toContain('尚未显示');
  expect(await page.evaluate(() => (window as any).events.filter((event: any) => event.kind === 'teaching-help'))).toEqual([]);
  await frame.locator('#reveal').click();
  await expect(frame.locator('#done')).toHaveText('已记录');
  const help = await page.evaluate(() => (window as any).events.find((event: any) => event.kind === 'teaching-help'));
  expect(help).toMatchObject({ move_id: 'invite', help_kind: 'explanation', region_id: 'help' });
  expect(help.text).toContain('没有选用乙，不等于删除乙');
  expect(help.text).not.toContain('不应当作已展示');
  await frame.locator('#prefill').click();
  await expect.poll(() => page.evaluate(() => (window as any).events.find((event: any) => event.kind === 'teaching-action'))).toMatchObject({ move_id: 'invite', response: '乙仍然存在。' });
  expect(await page.evaluate(() => (window as any).events.filter((event: any) => event.kind === 'teaching-help'))).toHaveLength(1);
});
