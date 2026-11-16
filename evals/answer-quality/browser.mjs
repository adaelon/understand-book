import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';

// Use the real Reader and its production command dispatcher. Never fabricate
// playback receipts, source observations, or learner listening responses.
export async function browserTurn({ server, request, directory, timeout }) {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  const evidence = { interface: 'real_reader_browser', errors: [], music_responses: [] };
  const pending = [];
  page.setDefaultTimeout(30000);
  await page.addLocatorHandler(page.locator('.product-tour-overlay'), () => page.locator('.product-tour-skip').click());
  page.on('pageerror', error => evidence.errors.push(error.message));
  page.on('response', response => {
    if (response.request().method() !== 'POST' || new URL(response.url()).pathname !== '/api/agent/music/respond') return;
    pending.push((async () => {
      try { evidence.music_responses.push({ status: response.status(), request: response.request().postDataJSON(), response: await response.json() }); }
      catch (error) { evidence.errors.push(`Music receipt: ${error.message}`); }
    })());
  });
  try {
    // Keep the original API question exactly the same. On load the Reader
    // restores the active run from history and subscribes to its real events.
    const descriptor = await server.api('agent/runs', request, 'POST');
    evidence.descriptor = descriptor;
    const deadline = Date.now() + timeout;
    await page.goto(server.url, { waitUntil: 'domcontentloaded' });
    if (await page.locator('.music-reader').count()) {
      await page.waitForFunction(() => document.querySelector('.music-reader > audio')?.readyState > 0);
      await page.locator('.music-ask').click();
    }
    let turn;
    while (Date.now() < deadline) {
      const history = await server.api('agent/history');
      turn = history.current?.turns?.find(item => item.turn_id === descriptor.turn_id);
      if (turn && turn.status !== 'pending_assistant') break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    if (!turn || turn.status === 'pending_assistant') throw new Error('Reader browser turn timed out');
    if (!turn.outcome) throw new Error(`Reader browser turn: ${turn.error?.error_code ?? turn.status}`);
    evidence.turn_status = turn.status;
    const rendered = page.locator(`.turn[data-turn-id="${descriptor.turn_id}"] .ans-text`);
    if (await rendered.count()) {
      fs.writeFileSync(path.join(directory, 'browser-answer.txt'), await rendered.innerText());
      await rendered.scrollIntoViewIfNeeded();
    }
    return turn.outcome;
  } finally {
    try { await page.screenshot({ path: path.join(directory, 'browser.png'), fullPage: true }); }
    catch (error) { evidence.errors.push(`Screenshot: ${error.message}`); }
    await browser.close();
    await Promise.allSettled(pending);
    fs.writeFileSync(path.join(directory, 'browser.json'), JSON.stringify(evidence, null, 2));
  }
}
