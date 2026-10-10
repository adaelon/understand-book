import { test, expect, type Page } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { createRequire } from 'node:module';
import katex from 'katex';
const require = createRequire(import.meta.url);
const katexPath = require.resolve('katex/dist/katex.min.css');
const mathStyle = readFileSync(katexPath, 'utf8').replace(/url\(([^)]+)\)/g, (_, file) => `url(data:font/woff2;base64,${readFileSync(resolve(dirname(katexPath), file.replace(/["']/g, ''))).toString('base64')})`);

const evidence = resolve('../../docs/performance/reading-share-rs7-20261010');
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

async function setup(page: Page, mode = '') {
  const view: any = { reference, title: '图解比较', entrypoint: 'index.html', content_files: { 'index.html': html,
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
  await expect(page.frameLocator('.agent-presentation iframe').locator('#result')).toBeVisible();
  await page.getByRole('button', { name: '记一下', exact: true }).click();
  await expect.poll(() => !!saved).toBe(true);
  return { requests, saved };
}

test('RS7 current SVG Canvas DOM and formula freeze together; landscape/portrait PNG and live scene continuity', async ({ page }) => {
  test.setTimeout(90000); mkdirSync(evidence, { recursive: true });
  await page.setViewportSize({ width: 1280, height: 900 });
  const { requests } = await setup(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await page.getByRole('button', { name: '导出当前图解' }).click();
  await expect(page.getByRole('dialog', { name: '生成分享图' })).toBeVisible();
  const initialPng = await page.evaluate(() => (window as any).lastShare.diagram.png);
  await page.getByRole('button', { name: '关闭分享' }).click();
  await frame.locator('#count').fill('3'); await expect(frame.locator('#result')).toHaveText('结果 3/3');
  const loadCount = await page.locator('.agent-presentation iframe').getAttribute('data-load-count');
  requests.length = 0;
  await page.getByRole('button', { name: '导出当前图解' }).click();
  const panel = page.getByRole('dialog', { name: '生成分享图' }); await expect(panel).toBeVisible();
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
  for (const orientation of ['landscape', 'portrait']) {
    await panel.locator('select').filter({ has: page.locator('option[value="portrait"]') }).selectOption(orientation);
    await panel.getByRole('button', { name: '预览图片', exact: true }).click();
    await expect(panel.locator('.share-preview img')).toBeVisible({ timeout: 45000 });
    const pageCount = Number((await panel.locator('.share-preview nav span').textContent()).split('/')[1]);
    for (let index = 0; index < pageCount; index++) {
    const preview = await panel.locator('.share-preview img').evaluate(async (img: HTMLImageElement) => ({ bytes:Array.from(new Uint8Array(await (await fetch(img.src)).arrayBuffer())),width:img.naturalWidth,height:img.naturalHeight }));
    expect([preview.width, preview.height]).toEqual(orientation === 'landscape' ? [1440,1080] : [1080,1440]);
    const download = page.waitForEvent('download');await panel.getByRole('button', { name: '下载 PNG', exact: true }).click();
    const file = await download; const path = resolve(evidence, index === 0 ? `${orientation}.png` : `${orientation}-${index + 1}.png`); await file.saveAs(path);
    expect([...readFileSync(path)]).toEqual(preview.bytes);
      if (index + 1 < pageCount) await panel.getByRole('button', { name: '下一页' }).click();
    }
  }
  await page.screenshot({ path: resolve(evidence, 'panel-desktop.png') });
  await panel.getByRole('button', { name: '关闭分享' }).click();
  expect(await page.locator('.agent-presentation iframe').getAttribute('data-load-count')).toBe(loadCount);
  await expect(frame.locator('#count')).toHaveValue('3'); await expect(page.getByRole('textbox', { name: '未发送问题' })).toHaveValue('保留的问题');
  expect(requests.some(path => /state.save|chat|memory\/save/.test(path))).toBe(false);
  await page.setViewportSize({width:390,height:844});
  await page.getByRole('button',{name:'导出当前图解'}).click();await expect(panel).toBeVisible();
  await page.screenshot({ path: resolve(evidence, 'panel-mobile.png') });
});

test('RS7 retained scene uses note read/observe/source after deletion and leaves current count untouched', async ({ page }) => {
  const { requests } = await setup(page);
  const frame = page.frameLocator('.agent-presentation iframe');
  await frame.locator('#count').fill('3');
  await page.getByRole('button', { name: '笔记详情' }).click();
  requests.length = 0;
  await page.getByRole('button', { name: '分享记录时的图解' }).click();
  await expect(page.getByRole('dialog', { name: '生成分享图' })).toBeVisible();
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
  await expect(page.getByRole('dialog',{name:'生成分享图'})).toHaveCount(0);
  expect(await page.locator('iframe').count()).toBe(1);
});

test('RS7 missing resources and media report concrete failures, then retry succeeds with unavailable source label',async({page})=>{
  await setup(page,'source-failed');const frame=page.frameLocator('.agent-presentation iframe');
  await frame.locator('img').evaluate((img:HTMLImageElement)=>img.src='data:image/png;base64,broken');
  await page.getByRole('button',{name:'导出当前图解'}).click();
  await expect(page.getByRole('status').filter({hasText:'图解未导出'})).toContainText('图片资源');
  await frame.locator('img').evaluate(img=>img.remove());
  await frame.locator('body').evaluate(body=>body.append(document.createElement('video')));
  await page.getByRole('button',{name:'导出当前图解'}).click();
  await expect(page.getByRole('status').filter({hasText:'图解未导出'})).toContainText('媒体');
  await frame.locator('video').evaluate(video=>video.remove());
  await page.getByRole('button',{name:'导出当前图解'}).click();
  await expect(page.getByRole('dialog',{name:'生成分享图'})).toBeVisible();
  expect((await page.evaluate(()=>(window as any).lastShare)).sources).toEqual(['原材料 · 第 2 页（来源暂不可用）']);
});
