import { test, expect, type Page } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { createRequire } from 'node:module';
import katex from 'katex';
const require = createRequire(import.meta.url);
const katexPath = require.resolve('katex/dist/katex.min.css');
const mathStyle = readFileSync(katexPath, 'utf8').replace(/url\(([^)]+)\)/g, (_, file) => `url(data:font/woff2;base64,${readFileSync(resolve(dirname(katexPath), file.replace(/["']/g, ''))).toString('base64')})`);

const evidence = resolve('../../docs/performance/presentation-html-export-20261013');
const reference = { presentation_id: 'private-diagram', revision: 2 };
const receipt = { session_id: 'deleted-chat', turn_id: 'private-turn', reference, state_revision: 1, saved_state_ref: 'private-state' };
const html = `<style>${mathStyle}</style><style>.plots{display:flex;gap:16px}.plots>*{width:44%;height:100px}body{font:18px/1.5 sans-serif}h2{font-size:24px}.legend::before{content:'●';color:#b43b25}</style>
<div hidden><input id="internal_state" value="private-control"></div><h2>参数与结果</h2><label>计数 <input id="count" aria-label="计数" type="range" min="0" max="3" value="1"></label>
<div class="plots"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 100"><rect id="bar" y="10" width="60" height="70" fill="#2874b8"/><text id="svg-label" x="8" y="96">SVG 1</text></svg><canvas id="plot" width="240" height="100"></canvas></div>
<p class="legend" id="result">结果 1/3</p><math><mfrac><mn id="numerator">1</mn><mn>3</mn></mfrac><mo>≤</mo><mn>1</mn></math>
${katex.renderToString("p=\\frac{n}{3}")}<button data-source-ref="source-1">来源</button><img alt="版本内资源" width="40" height="30" src="asset.svg">
<script>let count=1;const input=document.querySelector('#count');
function draw(){count=Number(input.value);document.querySelector('#result').textContent='结果 '+count+'/3';document.querySelector('#bar').setAttribute('width',String(count*60));document.querySelector('#svg-label').textContent='SVG '+count;document.querySelector('#numerator').textContent=String(count);const ctx=document.querySelector('#plot').getContext('2d');ctx.clearRect(0,0,240,100);ctx.fillStyle='#c95028';ctx.fillRect(0,10,count*60,70);ctx.fillStyle='#222';ctx.fillText('Canvas '+count,8,96);}
input.oninput=draw;draw();presentation.registerStateReader(()=>({values:{count,alpha:1},visible_step:'比较'}));presentation.registerStateRestorer(async state=>{await new Promise(r=>setTimeout(r,30));input.value=state.values.page.count;draw();});</script>`;

async function setup(page: Page, mode = '', pageHtml = html) {
  const view: any = { reference, title: '图解比较', entrypoint: 'index.html', content_files: { 'index.html': pageHtml,
    'asset.svg': '<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><circle cx="15" cy="15" r="12" fill="#20804a"/></svg>' },
    initial_state: {}, restored_state: null, restored_state_revision: null, animation_assets: {},
    sources: [{ source_ref_id: 'source-1', label: '原材料 · 第 2 页' }],
    assumptions: ['计数范围为 0 到 3。'], readable_view: { parts: [{ kind: 'markdown', text: '仅在分母为 3 时比较比例。\n\n$$p=\\frac{n}{3}$$' }], sources: [] } };
  const requests: string[] = [];
  let saved: any;
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname; requests.push(path);
    const input = route.request().postDataJSON();
    let data: any = {};
    if (path.endsWith('presentation.read')) {
      data = structuredClone(view);
      if (path.includes('/memory/')) { data.restored_state = saved; data.restored_state_revision = 1;
        if (mode === 'partial') data.content_files['index.html'] = html.replace('presentation.registerStateRestorer', 'void');
        if (mode === 'mismatch') data.content_files['index.html'] = html.replace('input.value=state.values.page.count', 'input.value=0');
      }
    } else if (path.endsWith('presentation.observe')) data = { accepted: true };
    else if (path.endsWith('presentation.state.save')) { saved ??= { ...input.state, values: { ...input.state.values, page: { alpha: 1, count: input.state.values.page.count } } }; data = receipt; }
    else if (path.endsWith('source.resolve')) {
      if (mode === 'source-failed') return route.fulfill({ status: 404, json: { error: 'unavailable' } });
      data = { label: '原材料 · 第 2 页', material_title: '原来的书', stale: false };
    }
    await route.fulfill({ json: data });
  });
  await page.route('**/rs7-test', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module">
  import {createApp,defineComponent,h,ref,provide} from '/node_modules/.vite/deps/vue.js';
  import Agent from '/src/components/AgentPresentation.vue';import Scene from '/src/components/PresentationSceneCard.vue';import Panel from '/src/components/ShareImagePanel.vue';
  import {sharePresentationKey} from '/src/reading-share.ts';import '/src/style.css';
  createApp(defineComponent({setup(){const share=ref(null),showScene=ref(false);provide(sharePresentationKey,async load=>{share.value=await load();window.lastShare=share.value;});
  return()=>h('main',{style:'max-width:960px;margin:auto'},[h(Agent,{sessionId:'deleted-chat',turnId:'private-turn',reference:${JSON.stringify(reference)}}),h('textarea',{'aria-label':'未发送问题',value:'保留的问题'}),h('button',{onClick:()=>showScene.value=true},'笔记详情'),showScene.value?h(Scene,{receipt:${JSON.stringify(receipt)},memId:'retained-note'}):null,share.value?h(Panel,{source:share.value,onClose:()=>share.value=null}):null]);}})).mount('#app');
  </script></body></html>` }));
  await page.goto('/rs7-test');
  await expect(page.frameLocator('.agent-presentation iframe').locator('h1,h2').first()).toBeVisible();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  await expect.poll(() => !!saved).toBe(true);
  return { requests, saved };
}

test('RS7 current HTML SVG Canvas and formula export unchanged with live scene continuity', async ({ page }) => {
  test.setTimeout(90000); mkdirSync(evidence, { recursive: true });
  await page.setViewportSize({ width: 1280, height: 900 });
  const { requests } = await setup(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await page.getByRole('button', { name: '导出长图' }).click();
  await expect(page.getByRole('dialog', { name: '导出演示长图' })).toBeVisible();
  const initialPng = await page.evaluate(() => (window as any).lastShare.diagram.png);
  await page.getByRole('button', { name: '关闭分享' }).click();
  await frame.locator('#count').fill('3'); await expect(frame.locator('#result')).toHaveText('结果 3/3');
  await frame.locator('body').screenshot({ path: resolve(evidence, 'live-desktop.png') });
  const loadCount = await page.locator('.agent-presentation iframe').getAttribute('data-load-count');
  requests.length = 0;
  await page.getByRole('button', { name: '导出长图' }).click();
  const panel = page.getByRole('dialog', { name: '导出演示长图' }); await expect(panel).toBeVisible();
  const source = await page.evaluate(() => (window as any).lastShare);
  expect(source.parts[0].text).toContain('结果 3/3'); expect(source.parts[0].text).toContain('计数 = 3');
  expect(source.sources).toEqual(['《原来的书》 · 原材料 · 第 2 页']);
  expect(JSON.stringify(source)).not.toMatch(/private-turn|private-state|private-diagram|private-control/);
  // Distinct colours prove the two real rendered drawing surfaces survive rasterization.
  const colours = await page.evaluate(async pngs => Promise.all(pngs.map(async png => {
    const image = new Image(); image.src = png; await image.decode(); const canvas = document.createElement('canvas'); canvas.width=image.width;canvas.height=image.height;
    const ctx=canvas.getContext('2d')!;ctx.drawImage(image,0,0);const p=ctx.getImageData(0,0,canvas.width,canvas.height).data;let blue=0,orange=0,green=0;
    for(let i=0;i<p.length;i+=4){if(p[i]===40&&p[i+1]===116&&p[i+2]===184)blue++;if(p[i]===201&&p[i+1]===80&&p[i+2]===40)orange++;if(p[i]===32&&p[i+1]===128&&p[i+2]===74)green++;}return {blue,orange,green};
  })), [initialPng, source.diagram.png]);
  expect(colours[1].blue).toBeGreaterThan(1000); expect(colours[1].orange).toBeGreaterThan(1000); expect(colours[1].green).toBeGreaterThan(100);
  expect(colours[1].orange / colours[0].orange).toBeCloseTo(3, 1);
  expect(colours[1].blue / colours[0].blue).toBeCloseTo(3, 1);
  await expect(panel.locator('input,textarea,select')).toHaveCount(0);
  await expect(panel.getByRole('navigation', { name: '分享图分页' })).toHaveCount(0);
  await expect(panel.getByRole('button', { name: '下载长图 PNG', exact: true })).toBeEnabled();
  const preview = await panel.locator('.share-preview img').evaluate(async (img: HTMLImageElement) => { await img.decode(); return { bytes:Array.from(new Uint8Array(await (await fetch(img.src)).arrayBuffer())),width:img.naturalWidth,height:img.naturalHeight }; });
  expect([preview.width, preview.height]).toEqual([source.diagram.width * 4, source.diagram.height * 4]);
  expect(Buffer.from(preview.bytes).equals(Buffer.from(source.diagram.png.split(',')[1], 'base64'))).toBe(true);
  const download = page.waitForEvent('download');await panel.getByRole('button', { name: '下载长图 PNG', exact: true }).click();
  const file = await download; const path = resolve(evidence, 'exported-desktop.png'); await file.saveAs(path);
  expect(readFileSync(path).equals(Buffer.from(preview.bytes))).toBe(true);
  await page.screenshot({ path: resolve(evidence, 'panel-desktop.png') });
  await panel.getByRole('button', { name: '关闭分享' }).click();
  expect(await page.locator('.agent-presentation iframe').getAttribute('data-load-count')).toBe(loadCount);
  await expect(frame.locator('#count')).toHaveValue('3'); await expect(page.getByRole('textbox', { name: '未发送问题' })).toHaveValue('保留的问题');
  expect(requests.some(path => /state.save|chat|memory\/save/.test(path))).toBe(false);
  await page.setViewportSize({width:390,height:844});
  await page.getByRole('button',{name:'导出长图'}).click();await expect(panel).toBeVisible();
  await expect(panel.getByRole('button', { name: '下载长图 PNG', exact: true })).toBeEnabled();
  const mobile = await panel.locator('.share-preview img').evaluate(async (img: HTMLImageElement) => { await img.decode(); return { width:img.naturalWidth,height:img.naturalHeight }; });
  const mobileScene = await page.evaluate(() => (window as any).lastShare.diagram);
  expect([mobile.width,mobile.height]).toEqual([mobileScene.width * 4,mobileScene.height * 4]);
  await page.screenshot({ path: resolve(evidence, 'panel-mobile.png') });
});

test('MHA GQA MQA exports readable full-size controls and four pixels per CSS pixel', async ({ page }, info) => {
  test.setTimeout(90000);
  await page.setViewportSize({ width: 1280, height: 900 });
  await setup(page, '', readFileSync(new URL('./fixtures/mha-gqa-mqa.html', import.meta.url), 'utf8'));
  const frame = page.frameLocator('.agent-presentation iframe');
  await expect(frame.locator('#kCap')).toHaveText('1.125 GiB');
  const boxes = (body: HTMLElement) => [...body.querySelectorAll<HTMLButtonElement>('button')].map(button => {
    const range = document.createRange(); range.selectNodeContents(button);
    const box = button.getBoundingClientRect(), text = range.getBoundingClientRect();
    return { text: button.textContent, width: box.width, height: box.height,
      overflow: text.left < box.left || text.right > box.right || text.top < box.top || text.bottom > box.bottom };
  });
  const live = await frame.locator('body').evaluate(boxes);
  expect(live.every(button => !button.overflow)).toBe(true);
  await frame.locator('body').screenshot({ path: info.outputPath('mha-live.png') });
  await page.evaluate(() => window.addEventListener('message', event => {
    if (event.data?.kind === 'static-result') (window as any).capturedHtml = event.data.image.svg;
  }));
  await page.getByRole('button', { name: '导出长图' }).click();
  const panel = page.getByRole('dialog', { name: '导出演示长图' });
  await expect(panel.getByRole('button', { name: '下载长图 PNG', exact: true })).toBeEnabled();
  const download = page.waitForEvent('download');
  await panel.getByRole('button', { name: '下载长图 PNG', exact: true }).click();
  await (await download).saveAs(info.outputPath('mha-export.png'));
  // Reconstruct the frozen DOM to catch controls rewrapping beyond their copied height.
  await page.evaluate(() => {
    const copy = document.createElement('iframe'); copy.id = 'export-layout';
    copy.style.cssText = 'position:fixed;left:-20000px;width:960px;height:640px';
    const svg = new DOMParser().parseFromString((window as any).capturedHtml, 'image/svg+xml');
    copy.srcdoc = '<!doctype html><html>' + svg.querySelector('foreignObject')!.innerHTML + '</html>';
    document.body.append(copy);
  });
  const frozen = page.frameLocator('#export-layout').locator('body');
  await expect(frozen).toBeAttached();
  const exported = await frozen.evaluate(boxes);
  expect(exported.map(button => button.text)).toEqual(live.map(button => button.text));
  exported.forEach((button, index) => {
    expect(button.overflow).toBe(false);
    // Computed CSS serializes fractional pixels with less precision than layout boxes.
    expect(Math.abs(button.width - live[index].width)).toBeLessThan(.05);
    expect(Math.abs(button.height - live[index].height)).toBeLessThan(.05);
  });
  const size = await panel.locator('.share-preview img').evaluate((img: HTMLImageElement) => [img.naturalWidth, img.naturalHeight]);
  const source = await page.evaluate(() => (window as any).lastShare.diagram);
  expect(size).toEqual([source.width * 4, source.height * 4]);
  const capturedHeight = await page.evaluate(() => Number(new DOMParser().parseFromString((window as any).capturedHtml, 'image/svg+xml').documentElement.getAttribute('height')));
  expect(source.height).toBe(capturedHeight + 72);
  const header = await panel.locator('.share-preview img').evaluate((img: HTMLImageElement) => {
    const canvas = document.createElement('canvas'); canvas.width = img.naturalWidth; canvas.height = 72 * 4;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height, 0, 0, canvas.width, canvas.height);
    const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let logo = 0, name = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] === 182 && pixels[i + 1] === 90 && pixels[i + 2] === 23) logo++;
      if (pixels[i] === 145 && pixels[i + 1] === 66 && pixels[i + 2] === 15) name++;
    }
    return { logo, name };
  });
  expect(header.logo).toBeGreaterThan(500);
  expect(header.name).toBeGreaterThan(500);
});

test('high-resolution export resolves quarter-pixel vector detail instead of enlarging a bitmap', async ({ page }) => {
  await setup(page);
  const point = await page.frameLocator('.agent-presentation iframe').locator('body').evaluate(body => {
    const box = document.createElement('div');
    box.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" viewBox="0 0 4 4"><rect width="4" height="4" fill="white"/>'
      + Array.from({ length: 8 }, (_, i) => `<rect x="${i / 2}" width=".25" height="4" fill="black"/>`).join('') + '</svg>';
    body.prepend(box);
    const bounds = box.firstElementChild!.getBoundingClientRect(), root = body.getBoundingClientRect();
    return { x: bounds.left - root.left, y: bounds.top - root.top + 2 };
  });
  await page.getByRole('button', { name: '导出长图' }).click();
  const preview = page.getByRole('dialog', { name: '导出演示长图' }).locator('.share-preview img');
  await expect(preview).toBeVisible();
  const pixels = await preview.evaluate(async (img: HTMLImageElement, point) => {
    await img.decode();
    const canvas = document.createElement('canvas'); canvas.width = img.naturalWidth; canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d')!; ctx.drawImage(img, 0, 0);
    return Array.from(ctx.getImageData(Math.round(point.x * 4), Math.round((point.y + 72) * 4), 16, 1).data).filter((_, i) => i % 4 === 0);
  }, point);
  expect(pixels).toEqual(Array.from({ length: 16 }, (_, i) => i % 2 ? 255 : 0));
});

test('RS7 retained scene uses note read/observe/source after deletion and leaves current count untouched', async ({ page }) => {
  const { requests } = await setup(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#count').fill('3');
  await page.getByRole('button', { name: '笔记详情' }).click();
  requests.length = 0;
  await page.getByRole('button', { name: '分享记录时的图解' }).click();
  await expect(page.getByRole('dialog', { name: '导出演示长图' })).toBeVisible();
  const source = await page.evaluate(() => (window as any).lastShare);
  expect(source.parts[0].text).toContain('结果 1/3'); expect(source.association).toContain('记录时的图解 · 版本 2');
  await expect(frame.locator('#count')).toHaveValue('3');
  expect(requests).toContain('/api/memory/presentation.read');expect(requests).toContain('/api/memory/presentation.observe');
  expect(requests).not.toContain('/api/agent/presentation.read');
  expect(await page.locator('iframe').count()).toBe(1);
});

for (const mode of ['partial','mismatch']) test(`RS7 retained ${mode} fails visibly without exporting initial state`, async ({ page }) => {
  await setup(page,mode);await page.getByRole('button',{name:'笔记详情'}).click();
  await page.getByRole('button',{name:'分享记录时的图解'}).click();
  await expect(page.getByRole('alert')).toContainText(mode === 'partial' ? '无法完整恢复' : '不一致');
  await expect(page.getByRole('dialog',{name:'导出演示长图'})).toHaveCount(0);
  expect(await page.locator('iframe').count()).toBe(1);
});

test('RS7 missing resources and media report concrete failures, then retry succeeds with unavailable source label',async({page})=>{
  await setup(page,'source-failed');const frame=page.frameLocator('.agent-presentation iframe');
  await frame.locator('img').evaluate((img:HTMLImageElement)=>img.src='data:image/png;base64,broken');
  await page.getByRole('button',{name:'导出长图'}).click();
  await expect(page.getByRole('status').filter({hasText:'图解未导出'})).toContainText('图片资源');
  await frame.locator('img').evaluate(img=>img.remove());
  await frame.locator('body').evaluate(body=>body.append(document.createElement('video')));
  await page.getByRole('button',{name:'导出长图'}).click();
  await expect(page.getByRole('status').filter({hasText:'图解未导出'})).toContainText('媒体');
  await frame.locator('video').evaluate(video=>video.remove());
  await page.getByRole('button',{name:'导出长图'}).click();
  await expect(page.getByRole('dialog',{name:'导出演示长图'})).toBeVisible();
  expect((await page.evaluate(()=>(window as any).lastShare)).sources).toEqual(['原材料 · 第 2 页（来源暂不可用）']);
});

test('long HTML exports its original layout and both ends in one unchanged PNG', async ({ page }, info) => {
  test.setTimeout(90000);
  await page.setViewportSize({ width: 1280, height: 900 });
  await setup(page);
  await page.frameLocator('.agent-presentation iframe').locator('body').evaluate(body => {
    const first = document.createElement('div'); first.style.cssText = 'height:60px;background:rgb(17,99,177)'; first.textContent = '图解开头'; body.prepend(first);
    for (let index = 1; index <= 12; index++) {
      const paragraph = document.createElement('p');
      paragraph.textContent = `停靠点 ${index}：` + '从输入沿计算顺序走到结果，记录上下文与权重在这一步的作用。'.repeat(4);
      body.append(paragraph);
    }
    const last = document.createElement('div'); last.style.cssText = 'height:60px;background:rgb(177,99,17)'; last.textContent = '图解结尾'; body.append(last);
  });
  await page.getByRole('button', { name: '导出长图' }).click();
  const panel = page.getByRole('dialog', { name: '导出演示长图' });
  await expect(panel).toBeVisible();
  await expect(panel.locator('.share-preview img')).toBeVisible({ timeout: 45000 });
  await expect(panel.getByRole('alert')).toHaveCount(0);
  const preview = await panel.locator('.share-preview img').evaluate(async (image: HTMLImageElement) => {
    await image.decode();
    const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
    const ctx = canvas.getContext('2d')!; ctx.drawImage(image, 0, 0);
    const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let first = 0, last = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] === 17 && pixels[i + 1] === 99 && pixels[i + 2] === 177) first++;
      if (pixels[i] === 177 && pixels[i + 1] === 99 && pixels[i + 2] === 17) last++;
    }
    return { width: image.naturalWidth, height: image.naturalHeight, first, last,
      bytes: Array.from(new Uint8Array(await (await fetch(image.src)).arrayBuffer())) };
  });
  const source = await page.evaluate(() => (window as any).lastShare.diagram);
  expect([preview.width, preview.height]).toEqual([source.width * 4, source.height * 4]);
  expect(preview.height).toBeGreaterThan(1440);
  expect(Buffer.from(preview.bytes).equals(Buffer.from(source.png.split(',')[1], 'base64'))).toBe(true);
  expect(preview.first).toBeGreaterThan(10000); expect(preview.last).toBeGreaterThan(10000);
  const downloading = page.waitForEvent('download'); await panel.getByRole('button', { name: '下载长图 PNG', exact: true }).click();
  const download = await downloading; const path = info.outputPath('long-diagram.png'); await download.saveAs(path);
  expect(readFileSync(path).equals(Buffer.from(preview.bytes))).toBe(true);
  await expect(panel.getByRole('navigation', { name: '分享图分页' })).toHaveCount(0);
});

test('HTML capture preserves native controls and their current values', async ({ page }) => {
  await setup(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('body').evaluate(body => {
    const style = document.createElement('style');
    style.textContent = '#count { accent-color:rgb(93,33,177) }'; body.append(style);
    const label = document.createElement('label');
    label.innerHTML = '<input id="checked" type="checkbox">已选项目 <input id="text" value="初始"><textarea id="notes">初始</textarea><select id="choice"><option>第一项</option><option>第二项</option></select>';
    body.append(label);
    (body.querySelector('#checked') as HTMLInputElement).checked = true;
    (body.querySelector('#text') as HTMLInputElement).value = '当前输入';
    (body.querySelector('#notes') as HTMLTextAreaElement).value = '当前说明';
    (body.querySelector('#choice') as HTMLSelectElement).selectedIndex = 1;
  });
  await frame.locator('#count').fill('3');
  const slider = await frame.locator('#count').evaluate(input => {
    const box = input.getBoundingClientRect(), root = document.body.getBoundingClientRect();
    return { x: box.right - root.left - 8, y: (box.top + box.bottom) / 2 - root.top };
  });
  await page.evaluate(() => window.addEventListener('message', event => {
    if (event.data?.kind === 'static-result') (window as any).capturedHtml = event.data.image.svg;
  }));
  await page.getByRole('button', { name: '导出长图' }).click();
  const panel = page.getByRole('dialog', { name: '导出演示长图' });
  await expect(panel.getByRole('button', { name: '下载长图 PNG', exact: true })).toBeEnabled();
  const controls = await page.evaluate(() => {
    const doc = new DOMParser().parseFromString((window as any).capturedHtml, 'image/svg+xml');
    return { slider: doc.querySelector('#count')?.tagName, value: doc.querySelector('#count')?.getAttribute('value'),
      checked: doc.querySelector('#checked')?.hasAttribute('checked'), text: doc.querySelector('#text')?.getAttribute('value'),
      notes: doc.querySelector('#notes')?.textContent, selected: doc.querySelector('#choice option[selected]')?.textContent };
  });
  expect(controls).toEqual({ slider: 'input', value: '3', checked: true, text: '当前输入', notes: '当前说明', selected: '第二项' });
  const thumb = await panel.locator('.share-preview img').evaluate(async (img: HTMLImageElement, point) => {
    await img.decode(); const canvas = document.createElement('canvas'); canvas.width = img.naturalWidth; canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d')!; ctx.drawImage(img, 0, 0);
    return Array.from(ctx.getImageData(Math.round(point.x * 4), Math.round((point.y + 72) * 4), 1, 1).data);
  }, slider);
  expect(thumb).toEqual([93,33,177,255]);
});

