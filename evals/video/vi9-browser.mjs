// Run: node --import tsx evals/video/vi9-browser.mjs
import { createRequire } from 'node:module';
import { spawn, spawnSync } from 'node:child_process';
import { createServer as httpsServer } from 'node:https';
import http from 'node:http';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync, existsSync, createWriteStream, rmSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import assert from 'node:assert/strict';
import { publishVideoEnrichmentBatch } from '../../packages/core/src/video-enrichment.ts';
const require = createRequire(new URL('../../packages/web/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const out = process.env.VI9_OUTPUT ? resolve(process.env.VI9_OUTPUT) : resolve('evals/video/generated/vi9', new Date().toISOString().replaceAll(':', '-'));
const input = join(out, 'workspace'); mkdirSync(input, { recursive: true });
const source = resolve('evals/video/generated/vi2/moved-workspace');
const manifest = JSON.parse(readFileSync(join(source, 'video_source_manifest.json')));
const map = JSON.parse(readFileSync(join(source, manifest.transcript_map_path)));
const save = (name, value) => writeFileSync(join(out, name), JSON.stringify(value, null, 2));
for (const rel of ['base.json', 'source.txt', 'video_source_manifest.json', manifest.transcript_map_path, map.raw_result_path, manifest.source.imported_path]) {
  mkdirSync(dirname(join(input, rel)), { recursive: true }); copyFileSync(join(source, rel), join(input, rel));
}
const env = { ...process.env, VI9_OUTPUT: out, VI9_WORKSPACE: input, VITE_MULTI_USER: '1', UNDERSTAND_BOOK_ADDR: '127.0.0.1:8794' };
const children = [];
function start(exe, args, name, cwd = process.cwd()) {
  const child = spawn(exe, args, { env, cwd, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  const log = createWriteStream(join(out, name + '.log')); child.stdout.pipe(log); child.stderr.pipe(log); children.push(child); return child;
}
async function wait(predicate, label, timeout = 120000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await predicate()) return; await new Promise(r => setTimeout(r, 100)); }
  throw new Error('Timed out: ' + label);
}
async function publish(id) {
  save('command.json', { id }); await wait(() => existsSync(join(out, `result-${id}.json`)), `publish ${id}`);
  return JSON.parse(readFileSync(join(out, `result-${id}.json`)));
}
const accepted = (capability, start_ms, end_ms) => [{ capability, accepted: [{ video: manifest.material.video, start_ms, end_ms }], pending: [], failed: [] }];
function batch(id, coverage, artifacts) { return publishVideoEnrichmentBatch({ workspace_dir: input, batch_id: id, material: manifest.material, coverage, artifacts }); }
let browser, proxy, pages = [];
try {
  start('cargo', ['test', '-p', 'server', '--lib', 'video_publication_browser_host', '--', '--ignored', '--nocapture'], 'server');
  start(process.execPath, [join(dirname(require.resolve('vite/package.json')), 'bin/vite.js'), '--port', '5177', '--strictPort'], 'web', resolve('packages/web'));
  const key = join(out, 'localhost.key'), cert = join(out, 'localhost.crt'), config = join(out, 'localhost.cnf');
  writeFileSync(config, '[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=localhost\n[v3]\nsubjectAltName=DNS:localhost\n');
  const tls = spawnSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert, '-days', '2', '-config', config], { windowsHide: true });
  assert.equal(tls.status, 0, tls.stderr?.toString());
  proxy = httpsServer({ key: readFileSync(key), cert: readFileSync(cert) }, (request, response) => {
    const upstream = http.request({ hostname: '127.0.0.1', port: request.url.startsWith('/api/') ? 8794 : 5177,
      path: request.url, method: request.method, headers: request.headers }, result => { response.writeHead(result.statusCode, result.headers); result.pipe(response); });
    upstream.on('error', () => { response.writeHead(502); response.end(); }); response.on('close', () => upstream.destroy()); request.pipe(upstream);
  });
  await new Promise(r => proxy.listen(5176, 'localhost', r));
  await wait(() => existsSync(join(out, 'ready')), 'server ready', 240000);
  const original = JSON.parse(readFileSync(join(out, 'initial.json')));
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  const context = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1440, height: 1000 } });
  const a = await context.newPage(), b = await context.newPage();
  pages = [a,b];
  const errors = []; context.on('page', page => page.on('pageerror', e => errors.push(e.message)));
  for (const page of [a,b]) page.on('pageerror', e => errors.push(e.message));
  for (const [i,page] of pages.entries()) page.on('response', async r => { if (r.status() >= 400) console.log('HTTP', i, r.status(), r.url(), (await r.text().catch(() => '')).slice(0,250)); });
  const responses = { a: [], b: [] };
  for (const [name, page] of [['a',a],['b',b]]) page.on('response', async response => {
    if (response.url().endsWith('/reader/adopt_same_source_publication') && response.ok()) {
      const value = await response.json(); responses[name].push({ publication: value.published_book_ref, status: value.result?.status });
    }
  });
  await a.goto('https://localhost:5176');
  await a.getByLabel('邮箱或账号', { exact: true }).fill('A'); await a.getByLabel('密码', { exact: true }).fill('fixture-only-password');
  await a.getByRole('button', { name: '登录', exact: true }).click();
  for (const page of [a,b]) {
    if (page === b) await b.goto('https://localhost:5176');
    await page.bringToFront();
    await page.getByRole('button', { name: new RegExp(original.reference.book_id) }).click();
    console.log('opened', page === a ? 'A' : 'B');
    await page.locator('.video-reader video').waitFor();
    const textbox = page.getByRole('textbox', { name: '围绕当前阅读内容提问' });
    if (!await textbox.isVisible()) await page.getByRole('button', { name: '问答', exact: true }).click();
    await page.waitForFunction(() => document.querySelector('.video-reader video')?.readyState >= 2);
    await page.locator('.video-reader video').evaluate(v => { v.currentTime = 492; v.playbackRate = 1.5; v.muted = true; });
    await page.waitForFunction(() => { const v = document.querySelector('.video-reader video'); return !v.seeking && v.readyState >= 2; });
    await page.locator('.video-reader video').evaluate(v => v.play());
    await page.waitForFunction(() => !document.querySelector('textarea[data-workspace-input="agent"]')?.disabled);
    await page.locator('.video-reader video').evaluate(v => v.pause());
  }
  const getWorkspace = async page => page.evaluate(async () => {
    const { network } = await import('/src/network-context.ts');
    const w = network.value.workspace; return (await fetch('/api/workspaces/' + w.workspace_id)).json();
  });
  const ref = async page => (await getWorkspace(page)).published_book_ref.publication_id;
  const text = a.getByRole('textbox', { name: '围绕当前阅读内容提问' });
  await a.bringToFront();
  await text.fill('VI9 保留此问题的原发布');
  await wait(async () => { const w = await getWorkspace(a); return w.revision >= 2; }, 'draft saved');
  const playerB = await b.locator('.video-reader video').elementHandle();
  await b.locator('.video-reader video').evaluate(v => v.play());
  const scroll = await b.locator('.video-reader .transcript').evaluate(pane => {
    const segment = pane.querySelectorAll('[data-segment]')[40]; pane.scrollTop = segment.offsetTop - pane.offsetTop;
    return { id: segment.dataset.segment, offset: segment.getBoundingClientRect().top - pane.getBoundingClientRect().top };
  });
  const translations = map.segments.slice(0, 4).map(s => ({ transcript: map.transcript, segment_id: s.segment_id, zh_text: 'VI9 中文批次夹具：' + s.text }));
  batch('zh-1', accepted('translation', map.segments[0].location.start_ms, map.segments[3].location.end_ms), { 'transcript_translations.json': JSON.stringify(translations) });
  const p2 = await publish('zh-1'); assert(!p2.result.error, JSON.stringify(p2));
  await wait(async () => await ref(b) === p2.result.manifest.reference.publication_id, 'idle B adopts translation');
  assert.equal(await ref(a), original.reference.publication_id);
  assert.equal(await playerB.evaluate(v => v === document.querySelector('.video-reader video')), true);
  assert.equal(await playerB.evaluate(v => v.paused), false); assert.equal(await playerB.evaluate(v => v.playbackRate), 1.5);
  await wait(async () => await b.getByLabel('已接入成果').textContent().then(s => s.includes('4 段中文')), 'B projection');
  const scrollAfter = await b.locator('.video-reader .transcript').evaluate((pane, id) => pane.querySelector(`[data-segment="${id}"]`).getBoundingClientRect().top - pane.getBoundingClientRect().top, scroll.id);
  assert(Math.abs(scrollAfter - scroll.offset) < 2, `anchor moved ${scroll.offset} -> ${scrollAfter}`);
  await b.locator('.video-reader video').evaluate(v => v.pause());
  const pausedTime = await playerB.evaluate(v => v.currentTime);
  // Visual producer was launched before zh-1: its catalog intentionally omits that batch.
  rmSync(join(input, 'video_enrichments.json'));
  const frames = {};
  for (const seconds of [585, 600]) {
    const name = `frame-${seconds}.png`, file = join(out, name);
    const result = spawnSync(process.env.FFMPEG || 'ffmpeg', ['-v','error','-y','-ss',String(seconds),'-i',join(input, manifest.source.imported_path),'-frames:v','1',file], { windowsHide: true });
    assert.equal(result.status,0,result.stderr?.toString()); frames[name] = readFileSync(file);
  }
  batch('visual-1', accepted('visual_index', 585000, 601000), { ...frames, 'index.json': JSON.stringify({ fixture: true, frames: [585,600].map(s => ({ path:`frame-${s}.png`,actual_pts_ms:s*1000 })) }) });
  // A enters a real accepted Run while publishing the next independent batch.
  await a.getByRole('button', { name: '发送', exact: true }).click();
  await wait(() => existsSync(join(out, 'answer-started')), 'answer in flight');
  const savedA = await getWorkspace(a);
  const p3 = await publish('visual-1'); assert(!p3.result.error, JSON.stringify(p3));
  assert(p3.result.manifest.files['video-enrichments/zh-1/transcript_translations.json']);
  await wait(async () => await ref(b) === p3.result.manifest.reference.publication_id, 'B adopts cumulative visual batch');
  assert.equal(await ref(a), original.reference.publication_id);
  assert.equal(await playerB.evaluate(v => v.paused), true); assert.equal(await playerB.evaluate(v => v.currentTime), pausedTime);
  writeFileSync(join(out, 'release-answer'), 'release');
  await wait(async () => await ref(a) === p3.result.manifest.reference.publication_id, 'A adopts after saved answer');
  assert.equal((await getWorkspace(a)).selected_chat, savedA.selected_chat);
  await a.getByText('VI9 传输验收回答：问题保持原发布。', { exact: false }).first().waitFor();
  await a.reload(); await a.locator('.video-reader video').waitFor();
  await wait(async () => await ref(a) === p3.result.manifest.reference.publication_id, 'reconnect retains cumulative publication');
  await a.getByText('VI9 传输验收回答：问题保持原发布。', { exact: false }).first().waitFor();
  // Incomplete batch stays unregistered; prior coverage remains readable.
  batch('missing', accepted('visual_index', 610000, 611000), { 'missing.png': new Uint8Array([1]) });
  rmSync(join(input, 'video-enrichments/missing/missing.png'));
  const rejected = await publish('missing'); assert.equal(rejected.result.error, 'PUBLICATION_INVALID');
  assert.equal(await ref(a), p3.result.manifest.reference.publication_id);
  const recovery = await a.evaluate(async originalId => {
    const { api } = await import('/src/api.ts'); return api.adoptVideoPublication(originalId);
  }, original.reference.publication_id);
  assert.equal(recovery.status, 'current'); assert.equal(recovery.video.publication.publication_id, p3.result.manifest.reference.publication_id);
  await b.screenshot({ path: join(out, 'two-batches.png'), fullPage: true });
  assert.deepEqual(errors, []);
  save('verification.json', { date: new Date().toISOString(), material: manifest.material, publications: [original.reference,p2.result.manifest.reference,p3.result.manifest.reference],
    granularity: 'one accepted batch per publication (four translations; two real frames plus index)',
    media_bytes_per_publication: original.files[manifest.source.imported_path], media_bytes_three_snapshots: original.files[manifest.source.imported_path]*3,
    publication_ms: [p2.elapsed_ms,p3.elapsed_ms], two_windows: true, draft_and_inflight_pinned: true, identical_media_element: true,
    playing_and_paused_preserved: true, rate:1.5, english_anchor:{before:scroll.offset,after:scrollAfter}, saved_chat_and_reconnect:true,
    missing_dependency_rejected:true, late_notification_keeps_latest:true, model:'deterministic held answer, no semantic inference',responses });
  console.log(out);
} catch (error) { save('failure.json', { error:String(error),stack:error.stack }); for (const [i,page] of pages.entries()) { save(`page-${i}.json`, { text: await page.locator('body').innerText() }); await page.screenshot({ path:join(out,`failure-${i}.png`) }); } throw error; }
finally { writeFileSync(join(out,'release-answer'),'release'); writeFileSync(join(out,'stop'),'stop'); await browser?.close(); proxy?.closeAllConnections(); proxy?.close(); for (const child of children) child.kill(); }
