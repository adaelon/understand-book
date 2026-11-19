// Real Agent answer, published Beethoven score, and browser piano playback.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { startServer } from '../semantic/server.mjs';
import { startProviderRecorder, measuredUsage } from '../semantic/provider-recorder.mjs';

const root = process.cwd();
const stamp = new Date().toISOString().replace(/[:.]/g, '-');
const outputIndex = process.argv.indexOf('--out');
const out = path.resolve(outputIndex >= 0 ? process.argv[outputIndex + 1] : path.join(root, 'docs/performance/answer-piano-20261104', stamp));
fs.mkdirSync(out, { recursive: true });
process.loadEnvFile(path.join(root, '.env'));
process.env.SEMANTIC_EVAL_SERVER = path.join(root, '.tmp/music-example-reader-final.exe');
const publication = { book_id: 'music-beethoven9-tennstedt-m1-release1', publication_id: '01a1a0d2-4e74-7f81-8636-4c91fad083d5' };
const book = path.join(root, '.understand-book/beethoven9-tennstedt/local-library/library/published', publication.book_id, publication.publication_id);
const manifest = JSON.parse(fs.readFileSync(path.join(book, 'music_source_manifest.json'), 'utf8'));
const memory = path.join(root, '.understand-book/answer-piano-20261104', stamp);
const save = (name, value) => fs.writeFileSync(path.join(out, name), JSON.stringify(value, null, 2));
const recorder = await startProviderRecorder(process.env.OPENCODE_BASE_URL, { tokenLimit: 800000 });
const server = await startServer(book, memory, root, { OPENCODE_BASE_URL: recorder.url, UNDERSTAND_BOOK_WEB_DIST: path.join(root, '.tmp/music-example-web') });
save('reader.json', { url: server.url, process_id: server.child.pid, publication });
console.log(JSON.stringify({ reader: server.url, process_id: server.child.pid }));
let browser;
try {
  const question = { draft_id: crypto.randomUUID(), publication, audio: { recording: manifest.recording.source, start_ms: 34370, end_ms: 43640 }, score: null, occurrence_id: 'm1-occurrence-17', focused_part_ids: ['P48'], focused_voices: [{ part_id: 'P48', voice: '1:1' }] };
  const message = '请用第一小提琴第17–21小节解释这条动机怎么往下走、又跳回高处。每小节在回答里放一个能点听的完整钢琴谱例，保留长音、和弦和前后的休止。尤其请让我点听第21小节的四音和弦，比较四音同响和单音。先读原谱核对声部和八度，用短文字说明即可。';
  const descriptor = await server.api('agent/runs', { message, music_question: question, response_language: 'zh-CN' }, 'POST');
  save('request.json', { message, music_question: question, descriptor });
  assert(descriptor.turn_id, JSON.stringify(descriptor));
  const started = Date.now(); let turn;
  while (Date.now() - started < 480000) {
    turn = (await server.api('agent/history')).current?.turns?.find(turn => turn.turn_id === descriptor.turn_id);
    if (turn && turn.status !== 'pending_assistant') break;
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  save('turn.json', turn); save('provider.json', recorder.records); save('usage.json', measuredUsage(recorder.records));
  assert.equal(turn?.status, 'completed', JSON.stringify(turn?.error));
  fs.writeFileSync(path.join(out, 'answer.md'), turn.outcome.answer);
  const examples = turn.outcome.answer_view.parts.filter(part => part.kind === 'music_example').map(part => part.example);
  save('examples.json', examples);
  assert(examples.length >= 5, 'Agent must provide all five measure examples');
  const m21 = examples.find(example => example.title.includes('21') && example.notes.filter(n => [61,64,69,76].includes(n.midi_pitch)).length >= 4);
  assert(m21, 'Expected Beethoven21 example');
  const onset = m21.notes.find(note => note.midi_pitch === 61).start_ms;
  assert.deepEqual(m21.notes.filter(n => n.start_ms === onset).map(n => n.midi_pitch).sort((a,b) => a-b), [61,64,69,76]);
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(server.url, { waitUntil: 'domcontentloaded' });
  await page.locator('.music-example-play').first().waitFor({ timeout: 60000 });
  try { await page.locator('.product-tour-skip').waitFor({ timeout: 5000 }); await page.locator('.product-tour-skip').click(); } catch { /* Tour may already be dismissed. */ }
  page.setDefaultTimeout(20000);
  const index = turn.outcome.answer_view.parts.findIndex(part => part.kind === 'music_example' && part.example.ref_id === m21.ref_id);
  const button = page.locator(`.turn[data-turn-id="${turn.turn_id}"] .music-example-play[data-music-example-index="${index}"]`).first();
  await button.click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4, { timeout: 5000 });
  const sounding = await page.locator('.piano-key.sounding').evaluateAll(keys => keys.map(key => Number(key.dataset.pitch)).sort((a,b) => a-b));
  assert.deepEqual(sounding, [61,64,69,76]);
  const cdp = await page.context().newCDPSession(page);
  fs.writeFileSync(path.join(out, 'desktop-four-keys.png'), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await page.getByRole('button', { name: '收起试听', exact: true }).click();
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('.music-example-play').first().waitFor();
  await page.setViewportSize({ width: 390, height: 844 });
  if (!await button.isVisible()) await page.locator('.music-ask').click();
  await button.click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4);
  fs.writeFileSync(path.join(out, 'mobile-four-keys.png'), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  save('browser.json', { sounding, errors, horizontal_overflow: await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), reloaded: true });
  assert.equal(errors.length, 0);
  console.log(JSON.stringify({ status: 'passed', examples: examples.length, sounding, reader: server.url }));
} catch (error) {
  save('failure.json', { message: error.message, stack: error.stack });
  save('provider.json', recorder.records);
  console.error(error.message); process.exitCode = 1;
} finally {
  await browser?.close();
  // Keep this isolated Reader available for the user's own listening.
  console.log('Reader remains available for listening.');
}
