// Exercise the real saved Agent answer, including decoded sampled audio.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const source = path.resolve('docs/performance/answer-piano-20261104');
const dir = path.resolve('docs/performance/salamander-lossless-20261104');
const audioDir = path.resolve('.tmp/salamander-lossless-pitch-audit');
fs.mkdirSync(dir, { recursive: true }); fs.mkdirSync(audioDir, { recursive: true });
const { url } = JSON.parse(fs.readFileSync(path.join(source, 'reader.json')));
const turn = JSON.parse(fs.readFileSync(path.join(source, 'continuous-turn.json')));
const parts = turn.outcome.answer_view.parts;
const phraseIndex = parts.findIndex(part => part.kind === 'music_example' && part.example.kind === 'score' && part.example.duration_ms === 9091);
const chordsIndex = parts.findIndex(part => part.kind === 'music_example' && part.example.kind === 'illustration' && part.example.notes.length === 12);
assert(phraseIndex >= 0 && chordsIndex >= 0);
const phrase = parts[phraseIndex].example;
const pitches = [...new Set(phrase.notes.map(note => note.midi_pitch))].sort((a, b) => a - b);
const browser = await chromium.launch({ headless: true });
const report = { url, turn_id: turn.turn_id, errors: [], requests: [], recordings: [] };
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.setDefaultTimeout(25000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('request', request => { if (request.url().includes('/piano/salamander-v3/') && /\.(flac|mp3)$/.test(request.url())) report.requests.push(request.url()); });
  await page.addInitScript(() => {
    const resume = AudioContext.prototype.resume;
    AudioContext.prototype.resume = async function() {
      await resume.call(this);
      this.pianoCaptureReady ??= this.audioWorklet.addModule(URL.createObjectURL(new Blob([`
        class PianoCapture extends AudioWorkletProcessor {
          constructor() { super(); this.samples = new Float32Array(2048); this.position = 0; }
          process(inputs) {
            const input = inputs[0]?.[0];
            if (input) {
              if (!this.position) this.firstFrame = currentFrame;
              this.samples.set(input, this.position); this.position += input.length;
              if (this.position === this.samples.length) {
                this.port.postMessage({ frame: this.firstFrame, samples: this.samples }, [this.samples.buffer]);
                this.samples = new Float32Array(2048); this.position = 0;
              }
            }
            return true;
          }
        }
        registerProcessor('piano-capture', PianoCapture);
      `], { type: 'text/javascript' })));
      await this.pianoCaptureReady;
    };
    const nativeConnect = AudioNode.prototype.connect;
    AudioNode.prototype.connect = function(target, ...args) {
      const result = nativeConnect.call(this, target, ...args);
      if (target instanceof AudioDestinationNode && this instanceof GainNode) {
        window.pianoTap?.disconnect(); window.pianoMute?.disconnect();
        const tap = new AudioWorkletNode(target.context, 'piano-capture', { channelCount: 1, channelCountMode: 'explicit' }), mute = target.context.createGain();
        mute.gain.value = 0; window.pianoSamples = []; window.pianoFrames = []; window.pianoSampleRate = target.context.sampleRate;
        tap.port.onmessage = event => { if (window.pianoTap === tap) { window.pianoSamples.push(Array.from(event.data.samples)); window.pianoFrames.push(event.data.frame); } };
        nativeConnect.call(this, tap); nativeConnect.call(tap, mute); nativeConnect.call(mute, target);
        window.pianoTap = tap; window.pianoMute = mute;
      }
      return result;
    };
    const create = BaseAudioContext.prototype.createBufferSource;
    BaseAudioContext.prototype.createBufferSource = function() {
      const voice = create.call(this), start = voice.start;
      voice.start = function(time, offset) {
        (window.pianoVoices ??= []).push({ time, offset, playbackRate: voice.playbackRate.value, detune: voice.detune.value, duration: voice.buffer.duration });
        return start.call(this, time, offset);
      };
      return voice;
    };
  });
  await page.goto(url, { waitUntil: 'domcontentloaded' });
  const root = page.locator(`.turn[data-turn-id="${turn.turn_id}"]`);
  await root.locator('.music-example-play').first().waitFor();
  try { await page.locator('.product-tour-skip').waitFor({ timeout: 5000 }); await page.locator('.product-tour-skip').click(); } catch { /* Already dismissed. */ }
  await page.evaluate(() => {
    window.pianoTransitions = []; let previous = '';
    new MutationObserver(() => {
      const pitches = Array.from(document.querySelectorAll('.piano-key.sounding')).map(key => Number(key.dataset.pitch)).sort((a, b) => a - b);
      const state = pitches.join(',');
      if (previous !== state) window.pianoTransitions.push({ ms: performance.now(), pitches });
      previous = state;
    }).observe(document.body, { attributes: true, attributeFilter: ['class'], subtree: true });
  });
  // Hold the cold sample responses briefly so the loading state is observable.
  await page.route('**/piano/salamander-v3/*.flac', async route => {
    await new Promise(resolve => setTimeout(resolve, 250)); await route.continue();
  });
  await root.locator(`.music-example-play[data-music-example-index="${phraseIndex}"]`).click();
  await page.getByRole('status').filter({ hasText: '正在准备钢琴音色' }).waitFor();
  await page.waitForFunction(() => document.querySelector('.piano-key.sounding[data-pitch="81"]'));
  await page.unroute('**/piano/salamander-v3/*.flac');
  assert(report.requests.every(url => url.endsWith('.flac')), 'The piano must use lossless source files');
  assert.equal(report.requests.length, 7, 'The phrase should load seven recorded keys');
  report.phrase_samples = [...report.requests];
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4);
  report.chord_pitches = await page.locator('.piano-key.sounding').evaluateAll(keys => keys.map(key => Number(key.dataset.pitch)));
  assert.deepEqual(report.chord_pitches, [61, 64, 69, 76]);
  const cdp = await page.context().newCDPSession(page);
  const screenshot = async name => fs.writeFileSync(path.join(dir, name), Buffer.from((await cdp.send('Page.captureScreenshot', { format: 'png' })).data, 'base64'));
  await screenshot('four-keys.png');
  await page.waitForFunction(() => document.querySelector('.piano-status')?.textContent.includes('重听'));
  report.phrase_transitions = await page.evaluate(() => window.pianoTransitions);
  report.phrase_voices = await page.evaluate(() => window.pianoVoices);
  assert.equal(report.phrase_voices.length, 20);
  assert(report.phrase_voices.slice(-4).every(voice => voice.time === report.phrase_voices.at(-1).time));
  const pcm = await page.evaluate(() => ({ sample_rate: window.pianoSampleRate, samples: window.pianoSamples.flat() }));
  let square = 0, peak = 0; for (const sample of pcm.samples) { square += sample * sample; peak = Math.max(peak, Math.abs(sample)); }
  report.phrase_audio = { sample_rate: pcm.sample_rate, samples: pcm.samples.length, peak, rms: Math.sqrt(square / pcm.samples.length) };
  assert(peak > 0.01 && peak < 1, 'Real phrase audio should be audible and unclipped');
  await page.evaluate(() => { window.pianoTransitions = []; });
  await root.locator(`.music-example-play[data-music-example-index="${chordsIndex}"]`).click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 3);
  await screenshot('chord-progression.png');
  await page.waitForFunction(() => document.querySelector('.piano-status')?.textContent.includes('重听'));
  report.chord_transitions = await page.evaluate(() => window.pianoTransitions);
  assert.deepEqual(report.chord_transitions.filter(item => item.pitches.length === 3).map(item => item.pitches), [[60,64,67], [65,69,72], [67,71,74], [60,64,67]]);
  const requestsBeforeRepeat = report.requests.length;
  await page.setViewportSize({ width: 390, height: 844 });
  await root.locator(`.music-example-play[data-music-example-index="${phraseIndex}"]`).click();
  await page.waitForFunction(() => document.querySelectorAll('.piano-key.sounding').length === 4);
  await screenshot('mobile-four-keys.png');
  assert.equal(report.requests.length, requestsBeforeRepeat, 'Replay should reuse decoded recordings');
  report.mobile_overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
  assert(!report.mobile_overflow);
  await page.setViewportSize({ width: 1440, height: 1000 });
  for (const pitch of pitches) {
    await page.locator(`.piano-key[data-pitch="${pitch}"]`).click();
    await page.waitForFunction(pitch => document.querySelector(`.piano-key.sounding[data-pitch="${pitch}"]`), pitch);
    await page.waitForFunction(() => window.pianoSamples?.length >= 14);
    const data = await page.evaluate(() => ({ sample_rate: window.pianoSampleRate, samples: window.pianoSamples.flat(), frames: window.pianoFrames }));
    assert(data.frames.every((frame, index) => index === 0 || frame - data.frames[index - 1] === 2048), 'Captured PCM must be contiguous');
    const floats = Float32Array.from(data.samples), file = path.join(audioDir, `${pitch}.f32`);
    fs.writeFileSync(file, Buffer.from(floats.buffer));
    report.recordings.push({ midi_pitch: pitch, sample_rate: data.sample_rate, samples: floats.length, contiguous: true, file });
  }
  const credits = await page.request.get(url + '/piano/salamander-v3/credits.html');
  assert.equal(credits.status(), 200); assert((await credits.text()).includes('Alexander Holm'));
  assert.deepEqual(report.errors, []);
  fs.writeFileSync(path.join(dir, 'browser.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ status: 'passed', phrase_samples: report.phrase_samples.length, voices: report.phrase_voices.length, chord: report.chord_pitches, audio: report.phrase_audio, pitch_recordings: report.recordings.length, errors: report.errors }));
} finally { await browser.close(); }
