// Inspect a real saved continuous Agent answer through the native browser player.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const dir = path.resolve('docs/performance/answer-piano-20261104');
const { url } = JSON.parse(fs.readFileSync(path.join(dir, 'reader.json')));
const turn = JSON.parse(fs.readFileSync(path.join(dir, 'continuous-turn.json')));
const parts = turn.outcome.answer_view.parts;
const phraseIndex = parts.findIndex(part => part.kind === 'music_example' && part.example.kind === 'score' && part.example.duration_ms === 9091);
const chordsIndex = parts.findIndex(part => part.kind === 'music_example' && part.example.kind === 'illustration' && part.example.notes.length === 12);
assert(phraseIndex >= 0 && chordsIndex >= 0, 'Expected a complete motive and the four-chord example');
const phrase = parts[phraseIndex].example;
assert.equal(phrase.notes.length, 20);
assert.deepEqual(phrase.notes.filter(note => note.start_ms === 8182).map(note => note.midi_pitch), [61, 64, 69, 76]);
const browser = await chromium.launch({ headless: true });
const errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.setDefaultTimeout(20000);
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const root = page.locator(`.turn[data-turn-id="${turn.turn_id}"]`);
  await root.locator('.music-example-play').first().waitFor();
  try { await page.locator('.product-tour-skip').waitFor({ timeout: 5000 }); await page.locator('.product-tour-skip').click(); } catch { /* This browser may already know the app. */ }
  await page.evaluate(() => {
    window.pianoTransitions = [];
    let previous = '';
    new MutationObserver(() => {
      const pitches = Array.from(document.querySelectorAll('.piano-key.sounding')).map(key => Number(key.dataset.pitch)).sort((a, b) => a - b);
      const state = pitches.join(',');
      if (previous !== state) window.pianoTransitions.push({ ms: performance.now(), pitches });
      previous = state;
    }).observe(document.body, { attributes: true, attributeFilter: ['class'], subtree: true });
  });
  await root.locator(`.music-example-play[data-music-example-index="${phraseIndex}"]`).click();
  await page.waitForFunction(() => document.querySelector('.piano-key.sounding[data-pitch="81"]'));
  await page.waitForFunction(() => document.querySelector('.piano-key.sounding[data-pitch="62"]'));
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4);
  const cdp = await page.context().newCDPSession(page);
  fs.writeFileSync(path.join(dir, 'continuous-four-keys.png'), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await page.waitForFunction(() => document.querySelector('.piano-status')?.textContent.includes('重听'));
  const phraseTransitions = await page.evaluate(() => window.pianoTransitions);
  await page.evaluate(() => { window.pianoTransitions = []; });
  await root.locator(`.music-example-play[data-music-example-index="${chordsIndex}"]`).click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 3);
  fs.writeFileSync(path.join(dir, 'chord-progression.png'), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await page.waitForFunction(() => document.querySelector('.piano-status')?.textContent.includes('重听'));
  const chordTransitions = await page.evaluate(() => window.pianoTransitions);
  assert.deepEqual(chordTransitions.filter(item => item.pitches.length === 3).map(item => item.pitches), [[60,64,67], [65,69,72], [67,71,74], [60,64,67]]);
  await page.setViewportSize({ width: 390, height: 844 });
  await root.locator(`.music-example-play[data-music-example-index="${phraseIndex}"]`).click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4);
  fs.writeFileSync(path.join(dir, 'continuous-mobile.png'), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  assert(!overflow); assert.equal(errors.length, 0);
  fs.writeFileSync(path.join(dir, 'continuous-browser.json'), JSON.stringify({ turn_id: turn.turn_id, phrase_duration_ms: phrase.duration_ms, phrase_notes: phrase.notes.length, phraseTransitions, chordTransitions, errors, horizontal_overflow: overflow }, null, 2));
  console.log(JSON.stringify({ status: 'passed', phrase_duration_ms: phrase.duration_ms, chord_groups: 4, errors }));
} finally { await browser.close(); }
