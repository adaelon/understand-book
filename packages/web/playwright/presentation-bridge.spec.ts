import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";

for (const reject of [false, true]) test(`async restoration ${reject ? 'fails without saving defaults' : 'waits before observation and capture'}`, async ({page}) => {
  const bridge = readFileSync(new URL('../src/presentation-bridge.js', import.meta.url), 'utf8');
  await page.goto('about:blank');
  await page.evaluate(({bridge,reject}) => {
    (window as any).events=[];
    window.addEventListener('message',event=>{
      (window as any).events.push(event.data);
      if(event.data.kind==='observe') (event.source as Window).postMessage({channel:'agent-presentation',kind:'accepted',revision:event.data.revision},'*');
    });
    const frame=document.createElement('iframe');frame.setAttribute('sandbox','allow-scripts');
    frame.srcdoc=`<script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{},restoredState:{values:{page:{time:1.5}}}})</script><p id="result">Default frame</p><script>
      let time=0;presentation.registerStateReader(()=>({values:{time}}));
      presentation.registerStateRestorer(saved=>new Promise((resolve,reject)=>{window.finishRestore=()=>{${reject ? "reject(new Error('decode failed'))" : "time=saved.values.page.time;document.querySelector('#result').textContent='Restored '+time;presentation.commitState();resolve()"}}}));
    </script>`;
    document.body.append(frame);
  },{bridge,reject});
  const frame=page.frameLocator('iframe');
  await expect.poll(()=>frame.locator('body').evaluate(()=>typeof (window as any).finishRestore)).toBe('function');
  await page.locator('iframe').evaluate((f:HTMLIFrameElement)=>f.contentWindow!.postMessage({channel:'agent-presentation',kind:'snapshot',request_id:7},'*'));
  expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>['observe','state'].includes(e.kind)))).toEqual([]);
  await frame.locator('body').evaluate(()=>(window as any).finishRestore());
  if(reject){
    await expect.poll(()=>page.evaluate(()=>(window as any).events.some((e:any)=>e.kind==='error'))).toBe(true);
    expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>['observe','state'].includes(e.kind)))).toEqual([]);
  }else{
    await expect.poll(()=>page.evaluate(()=>(window as any).events.find((e:any)=>e.kind==='state'&&e.request_id===7)?.state.values.page.time)).toBe(1.5);
    expect(await page.evaluate(()=>(window as any).events.filter((e:any)=>e.kind==='observe').map((e:any)=>e.text))).toEqual(['Restored 1.5']);
  }
});

test("a delayed observation keeps the accepted result and focused slider visible", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  const style = readFileSync(new URL("../src/presentation.css", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(() => {
    (window as any).observations = [];
    (window as any).states = [];
    window.addEventListener("message", event => {
      if (event.data.kind === "observe") (window as any).observations.push({ source: event.source, revision: event.data.revision });
      if (event.data.kind === "state") (window as any).states.push(event.data);
    });
  });
  await page.evaluate(({ bridge, style }) => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<style>${style}</style><script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{}})</script>
      <input id="count" type="range" min="0" max="3" value="2"><output id="result">2/3</output>
      <script>document.querySelector('#count').oninput=e=>document.querySelector('#result').textContent=e.target.value+'/3';</script>`;
    document.body.appendChild(frame);
  }, { bridge, style });
  await expect.poll(() => page.evaluate(() => (window as any).observations.length)).toBe(1);
  await page.evaluate(() => {
    const item = (window as any).observations[0];
    item.source.postMessage({ channel: "agent-presentation", kind: "accepted", revision: item.revision }, "*");
  });
  const frame = page.frameLocator("iframe");
  await expect(frame.locator("#result")).toBeVisible();
  await frame.locator("#count").focus();
  await frame.locator("#count").press("Home");
  await expect.poll(() => page.evaluate(() => (window as any).observations.length)).toBe(2);
  await page.keyboard.press("ArrowRight");
  await expect.poll(() => page.evaluate(() => (window as any).observations.length)).toBe(3);
  for (let frameNumber = 0; frameNumber < 5; frameNumber += 1) {
    const state = await frame.locator("body").evaluate(async body => {
      await new Promise(requestAnimationFrame);
      return { visible: getComputedStyle(body).visibility, focus: document.activeElement?.id,
        result: (document.querySelector("#result") as HTMLElement).getBoundingClientRect().width };
    });
    expect(state.visible).toBe("visible");
    expect(state.focus).toBe("count");
    expect(state.result).toBeGreaterThan(0);
  }
  await page.locator("iframe").evaluate((node: HTMLIFrameElement) => node.contentWindow!.postMessage({ channel: "agent-presentation", kind: "snapshot", request_id: 9 }, "*"));
  expect(await page.evaluate(() => (window as any).states.filter((item: any) => item.request_id === 9))).toHaveLength(0);
  await page.evaluate(() => {
    const item = (window as any).observations[1];
    item.source.postMessage({ channel: "agent-presentation", kind: "accepted", revision: item.revision }, "*");
  });
  expect(await page.evaluate(() => (window as any).states.filter((item: any) => item.request_id === 9))).toHaveLength(0);
  await page.evaluate(() => {
    const item = (window as any).observations[2];
    item.source.postMessage({ channel: "agent-presentation", kind: "accepted", revision: item.revision }, "*");
  });
  await expect.poll(() => page.evaluate(() => (window as any).states.filter((item: any) => item.request_id === 9).length)).toBe(1);
  expect(await page.evaluate(() => (window as any).states.find((item: any) => item.request_id === 9).state.observed_result)).toContain("1/3");
});

test("a pending subtree replacement shows only the last accepted public text", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  const sourceChip = readFileSync(new URL('../src/source-chip.js', import.meta.url), 'utf8').replace('export function', 'function');
  const style = readFileSync(new URL("../src/presentation.css", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(() => {
    (window as any).observations = [];
    window.addEventListener("message", event => {
      if (event.data.kind === "observe") (window as any).observations.push({ source: event.source, revision: event.data.revision });
    });
  });
  await page.evaluate(({ bridge, style, sourceChip }) => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<style>${style}</style><script>${sourceChip}\n(${bridge})({channel:"agent-presentation",sources:[],initialState:{}})</script>
      <button id="replace" onclick="document.querySelector('#region').innerHTML='<p>internal position 1.1</p><button data-source-ref=unknown>invented</button>'">Replace</button>
      <section id="region"><p>Accepted explanation</p></section>`;
    document.body.appendChild(frame);
  }, { bridge, style, sourceChip });
  await expect.poll(() => page.evaluate(() => (window as any).observations.length)).toBe(1);
  await page.evaluate(() => {
    const item = (window as any).observations[0];
    item.source.postMessage({ channel: "agent-presentation", kind: "accepted", revision: item.revision }, "*");
  });
  const frame = page.frameLocator("iframe");
  await expect(frame.locator("#region p")).toHaveText("Accepted explanation");
  await frame.locator("#replace").click();
  await expect.poll(() => page.evaluate(() => (window as any).observations.length)).toBe(2);
  await expect(frame.locator("#region")).toHaveCSS("opacity", "0");
  await expect(frame.locator("#region")).toHaveAttribute("aria-hidden", "true");
  await expect(frame.locator("[data-presentation-mask]")).toHaveAttribute("data-accepted-text", "Accepted explanation");
  await expect(frame.locator("body")).toHaveCSS("visibility", "visible");
});

test("semantic observation preserves the focused control for repeated keyboard input", async ({ page }) => {
  const bridge = readFileSync(new URL("../src/presentation-bridge.js", import.meta.url), "utf8");
  await page.goto("about:blank");
  await page.evaluate(() => window.addEventListener("message", event => {
    if (event.data.kind === "observe") requestAnimationFrame(() => requestAnimationFrame(() =>
      (event.source as Window).postMessage({ channel: "agent-presentation", kind: "accepted", revision: event.data.revision }, "*")));
  }));
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<style>html[data-presentation-pending] body{visibility:hidden}</style>
      <script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{}})</script>
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
      if (event.data.kind === "observe") (event.source as Window).postMessage({ channel: "agent-presentation", kind: "accepted", revision: event.data.revision }, "*");
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{count:2},restoredState:{values:{controls:[{key:'count',type:'range',value:'1'}],page:{count:1}},visible_step:'explain'}})</script>
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
      if (event.data.kind === "observe") (event.source as Window).postMessage({ channel: "agent-presentation", kind: "accepted", revision: event.data.revision }, "*");
      if (event.data.kind === "state") (window as any).scenes.push(event.data);
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{}})</script>
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
      if (event.data.kind === "observe") {
        (event.source as Window).postMessage({ channel: "agent-presentation", kind: "theme", generation: 7, values: {} }, "*");
        (event.source as Window).postMessage({ channel: "agent-presentation", kind: "accepted", revision: event.data.revision }, "*");
      }
      if (event.data.kind === "editing-focus") (window as any).focusEvents.push(event.data);
    });
  });
  await page.evaluate(bridge => {
    const frame = document.createElement("iframe");
    frame.setAttribute("sandbox", "allow-scripts");
    frame.srcdoc = `<style>html[data-presentation-pending] body{visibility:hidden}</style>
      <script>(${bridge})({channel:"agent-presentation",sources:[],initialState:{}})</script><input id="editor" value="draft"><p>Visible result</p>`;
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
