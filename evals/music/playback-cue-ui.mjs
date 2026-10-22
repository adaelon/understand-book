// Real Chromium/media verification of the Reader component. The command transport is a
// test adapter; live Server/Provider acceptance remains in live-listening.mjs.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';

const root = process.cwd();
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const base = args[0] && !args[0].startsWith('--') ? args[0] : 'http://127.0.0.1:4193';
const cueOnly = args.includes('--cue-only');
const outputPrefix = option('--output-prefix', '');
const output = path.join(root, 'evals/music/results');
const outputPath = name => path.join(output, outputPrefix ? `${outputPrefix}-${name}` : name);
const publicationDirectory = path.resolve(root, option('--publication-dir', '.understand-book/music-acceptance-service/library/published/music-schubert-d780-3-vetter/01a16203-d21b-7f12-b3c7-fe12ede993de'));
const manifest = JSON.parse(fs.readFileSync(path.join(publicationDirectory, 'music_source_manifest.json'), 'utf8'));
const publication = JSON.parse(fs.readFileSync(path.join(publicationDirectory, 'publication.json'), 'utf8')).reference;
const media = file => `/@fs/${path.join(publicationDirectory, file).replaceAll('\\', '/')}`;
const context = { publication, manifest, audio_url: media(manifest.recording.media_path), score_pages: manifest.pages.map(page => ({ page_id: page.page_id, url: media(page.path) })), demonstrations: manifest.demonstrations.map(demo => ({ demonstration_id: demo.demonstration_id, url: media(demo.media_path) })) };
const browser = await chromium.launch({ headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
const errors = [];
page.on('pageerror', error => errors.push(error.message));
const report = { publication, recording: manifest.recording.source, verification: 'Real published media and Reader component; browser-only command transport adapter; no model or human listening claim.', actions: [], errors };
try {
  await page.route(url => url.pathname === '/music-cue-qa-context.json', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify(context) }));
  await page.addInitScript(() => {
    window.__musicPlayCalls = [];
    window.__cuePaints = [];
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function () {
      const cue = document.querySelector('.music-play-cue');
      window.__musicPlayCalls.push({ time: performance.now(), demo: Boolean(this.closest('.music-demo-player')), cue: cue?.textContent ?? null, event_id: cue?.getAttribute('data-event-id'), visible: Boolean(cue && cue.getBoundingClientRect().height > 0) });
      return play.call(this);
    };
    new MutationObserver(() => {
      const cue = document.querySelector('.music-play-cue');
      if (!cue) return;
      const eventId = cue.getAttribute('data-event-id');
      requestAnimationFrame(() => window.__cuePaints.push({ event_id: eventId, time: performance.now() }));
    }).observe(document, { childList: true, subtree: true });
  });
  await page.goto(`${base}/music-reader-visual.html?fixture=/music-cue-qa-context.json`);
  await page.waitForFunction(() => document.querySelector('.music-reader > audio')?.readyState >= 2);
  await page.waitForSelector('.music-score-page svg');
  await page.evaluate(async context => {
    const { api } = await import('/src/api.ts');
    const { musicDispatcher } = await import('/src/music-interaction.ts');
    const { initialRun } = await import('/src/agent-run-state.ts');
    const dispatcher = musicDispatcher();
    let latest, sequence = 0;
    window.__musicResults = [];
    api.musicRespond = async body => {
      if (body.operation === 'claim') return { claimed: true };
      const receipt = { ...body.receipt, receipt_id: `browser-receipt-${sequence}` };
      latest = receipt.receipt_id;
      window.__musicResults.push(receipt);
      return { accepted: true, receipt };
    };
    window.__musicCommand = action => {
      const command_id = `browser-command-${++sequence}`;
      const command = { command_id, run_turn_id: 'browser-turn', session_id: 'browser-session', request: { operation: action ? 'act' : 'observe', publication: context.publication, recording: context.manifest.recording.source, based_on: latest, action } };
      dispatcher.update({ ...initialRun({ book_id: context.publication.book_id, session_id: 'browser-session', turn_id: 'browser-turn' }), music_command: command });
      return command_id;
    };
  }, context);
  const command = async action => {
    const id = await page.evaluate(action => window.__musicCommand(action), action);
    await page.waitForFunction(id => window.__musicResults.some(receipt => receipt.command_id === id), id);
    const receipt = await page.evaluate(id => window.__musicResults.find(receipt => receipt.command_id === id), id);
    assert.notEqual(receipt.status, 'failed', receipt.error);
    report.actions.push({ action: action ?? 'observe', receipt });
    return receipt;
  };
  const cue = manifest.events.find(event => event.listen_for.trim());
  const demoCue = manifest.events.find(event => event.event_id === option('--demo-cue', cue.event_id));
  assert(demoCue, 'Requested demonstration cue must exist in this publication.');
  const demonstration = manifest.demonstrations[0];
  await command();
  await command({ kind: 'seek', time_ms: 4200.125 });
  const demoReceipt = await command({ kind: 'demonstration', demonstration_id: demonstration.demonstration_id, cue_event_id: demoCue.event_id });
  assert.equal(demoReceipt.paused, true);
  assert.equal(demoReceipt.current_ms, 4200.125);
  assert.equal(demoReceipt.demonstration_state.paused, false);
  assert(demoReceipt.demonstration_state.current_ms < demonstration.duration_ms);
  assert.deepEqual(demoReceipt.displayed_cue, { event_id: demoCue.event_id, text: demoCue.listen_for });
  const observedDemo = await command();
  assert.equal(observedDemo.paused, true);
  assert.equal(observedDemo.demonstration_state.paused, false);
  const checkLayout = async (width, height, name) => {
    await page.setViewportSize({ width, height });
    await page.screenshot({ path: outputPath(name) });
    const geometry = await page.evaluate(() => {
      const box = selector => { const node = document.querySelector(selector); const rect = node.getBoundingClientRect(); return { x: rect.x, y: rect.y, width: rect.width, height: rect.height, scroll_height: node.scrollHeight, client_height: node.clientHeight }; };
      return { viewport: { width: innerWidth, height: innerHeight }, cue: box('.music-demo-player .music-play-cue'), controls: box('.music-demo-player audio'), panel: box('.music-demo-player'), text: document.querySelector('.music-demo-player .music-play-cue p').textContent, source_label: document.querySelector('.music-demo-player .music-play-cue small').textContent };
    });
    for (const area of [geometry.panel, geometry.cue, geometry.controls]) assert(area.x >= 0 && area.y >= 0 && area.x + area.width <= width && area.y + area.height <= height, 'The entire cue and media controls must remain inside the viewport.');
    assert.equal(geometry.text, demoCue.listen_for);
    assert(geometry.cue.scroll_height <= geometry.cue.client_height, 'The complete cue must remain visible without clipping.');
    report.visual_readability ??= [];
    report.visual_readability.push(geometry);
  };
  await page.waitForFunction(() => document.querySelector('.music-demo-player audio')?.currentTime > .1);
  await checkLayout(1440, 1000, 'music-demo-cue-desktop.png');
  await checkLayout(390, 844, 'music-demo-cue-mobile.png');
  if (!cueOnly) {
  await page.getByRole('button', { name: '问原声这段', exact: true }).click();
  const questionedDemo = await command();
  assert.equal(questionedDemo.current_ms, 4200.125);
  assert.equal(questionedDemo.paused, true);
  assert.equal(questionedDemo.demonstration_state.paused, true);
  assert(await page.locator('.music-question-anchor').isVisible());
  await page.getByLabel('取消片段提问', { exact: true }).click();
  const demoPause = await command({ kind: 'pause' });
  assert.equal(demoPause.paused, true);
  assert.equal(demoPause.demonstration_state.paused, true);
  await page.setViewportSize({ width: 1440, height: 1000 });
  const original = await command({ kind: 'play', start_ms: cue.start_ms, end_ms: Math.min(cue.end_ms, cue.start_ms + 3500), cue_event_id: cue.event_id });
  assert.equal(original.paused, false);
  assert.equal(original.demonstration_id, null);
  assert.equal(original.demonstration_state, undefined);
  await page.screenshot({ path: outputPath('music-original-cue-desktop.png') });
  await command({ kind: 'pause' });
  await page.getByLabel('播放速度', { exact: true }).selectOption('0.75');
  const userChanged = await command();
  assert.equal(userChanged.displayed_cue, undefined);
  } else {
    await command({ kind: 'pause' });
  }
  const evidence = await page.evaluate(() => ({ calls: window.__musicPlayCalls, paints: window.__cuePaints }));
  assert.equal(evidence.calls.length, cueOnly ? 1 : 2);
  for (const call of evidence.calls) {
    const expected = call.demo ? demoCue : cue;
    assert(call.visible && call.event_id === expected.event_id && call.cue.includes(expected.listen_for));
    assert(evidence.paints.some(paint => paint.event_id === expected.event_id && paint.time < call.time), 'The cue must reach an animation frame before play is invoked.');
  }
  report.cue_before_play = evidence;
  report.status = 'passed';
  assert.deepEqual(errors, []);
} catch (error) {
  report.status = 'failed'; report.failure = String(error);
  await page.screenshot({ path: outputPath('music-cue-failure.png') }).catch(() => {});
  throw error;
} finally {
  fs.writeFileSync(outputPath('music-playback-cue-ui.json'), JSON.stringify(report, null, 2) + '\n');
  await browser.close();
}
console.log(JSON.stringify({ status: report.status, actions: report.actions.length, errors }));
