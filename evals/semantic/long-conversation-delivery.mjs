import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { createServer } from 'vite';

const webRoot = fileURLToPath(new URL('../../packages/web/', import.meta.url));

// Runs inside the rendered document. This rubric is specific to the frozen
// learning-rate exercise. Unknown notation is left for human review.
function learningRateOpening(body) {
  const text = body.innerText;
  const number = /(?<![\d.−-])0\.040*(?![\d.])/u.exec(text);
  const formulas = [
    ['loss', /L\s*[（(]\s*w\s*[)）]\s*[=＝]/iu],
    ['derivative', /(?:L\s*[′'’]\s*[（(]\s*w\s*[)）]|[d∂]L\s*[\/／]\s*[d∂]w)\s*[=＝]/iu],
    ['update', /w\s*[_ₜt₀-₉₊\d{}+＋\s]+[=＝]\s*w/iu],
    ['error_recurrence', /e\s*[_ₜt₀-₉₊\d{}+＋\s]+[=＝]\s*[（(]/iu],
    ['contraction', /r\s*[=＝]\s*1\s*[−-]\s*2\s*η/iu],
  ].map(([kind,pattern]) => ({kind, match:pattern.exec(text)})).filter(f => f.match)
    .sort((a,b) => a.match.index-b.match.index);
  const saved = { x:window.scrollX, y:window.scrollY };
  function location(match) {
    if (!match) return null;
    const position = { text: match[0], text_offset: match.index };
    const selection = window.getSelection();
    selection.collapse(body, 0);
    if (window.find(match[0], true, false, false)) {
      const rect = selection.getRangeAt(0).getBoundingClientRect();
      position.document_y = rect.top + window.scrollY;
      position.document_x = rect.left + window.scrollX;
      position.in_opening_viewport = rect.height > 0 && rect.width > 0 && position.document_y >= 0 &&
        position.document_y + rect.height <= window.innerHeight && position.document_x >= 0 &&
        position.document_x + rect.width <= window.innerWidth;
    }
    selection.removeAllRanges();
    window.scrollTo(saved.x,saved.y);
    return position;
  }
  const firstStep = location(number);
  const firstFormula = formulas.length ? { kind:formulas[0].kind, ...location(formulas[0].match) } : null;
  const update = formulas.find(f => f.kind === 'update');
  const updatePosition = update ? location(update.match) : null;
  let status = 'missing_evidence';
  if (Number.isFinite(firstStep?.document_y) && Number.isFinite(firstFormula?.document_y)) {
    status = firstStep.in_opening_viewport && firstStep.text_offset < firstFormula.text_offset &&
      firstStep.document_y <= firstFormula.document_y + 2 ? 'passed' : 'failed';
  }
  const order = { scope:'frozen_learning_rate_handout', status, first_step:firstStep, first_formula:firstFormula, update_formula:updatePosition };
  if (status === 'missing_evidence') order.missing = [
    !firstStep ? 'first_step_0.04' : !Number.isFinite(firstStep.document_y) && 'first_step_visible_position',
    !firstFormula ? 'recognized_formula' : !Number.isFinite(firstFormula.document_y) && 'first_formula_visible_position',
  ].filter(Boolean);
  return order;
}

// Render the exact revision Reader delivered, using the same document assembler
// and sandbox as AgentPresentation. A screenshot is retained for visual review.
export async function captureNarrowPage(view, pngFile, excerpts = []) {
  const host = await createServer({
    root: webRoot, configFile: false, appType: 'custom', logLevel: 'error',
    server: { host: '127.0.0.1', port: 0, hmr: false, watch: null },
    optimizeDeps: { noDiscovery: true },
  });
  let browser;
  try {
    host.middlewares.use('/capture', (_req, res) => {
      res.setHeader('content-type', 'text/html; charset=UTF-8');
      res.end('<!doctype html><html lang="zh-CN"><body style="margin:0"></body></html>');
    });
    await host.listen();
    browser = await chromium.launch();
    const page = await browser.newPage({ viewport: { width: 320, height: 420 }, hasTouch: true });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('http://127.0.0.1:' + host.httpServer.address().port + '/capture');
    await page.evaluate(async view => {
      const { presentationDocument } = await import('/src/presentation-document.ts');
      const frame = document.createElement('iframe');
      frame.name = 'handout';
      frame.setAttribute('sandbox', 'allow-scripts');
      frame.style.cssText = 'display:block;width:100vw;height:100vh;border:0';
      const ready = new Promise(resolve => frame.addEventListener('load', resolve, { once: true }));
      frame.srcdoc = presentationDocument(view);
      document.body.append(frame);
      await ready;
    }, view);
    const body = page.frame({ name: 'handout' }).locator('body');
    const reading = await body.evaluate(async body => {
      await document.fonts.ready;
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return {
        text: body.innerText,
        width: document.documentElement.clientWidth,
        scrollWidth: document.documentElement.scrollWidth,
      };
    });
    reading.opening_order = await body.evaluate(learningRateOpening);
    await page.screenshot({ path: pngFile });
    if (excerpts.length) {
      reading.excerpts = [];
      for (const { name, selector, text } of excerpts) {
        const match = page.frame({ name: 'handout' }).locator(selector).filter({ hasText: text }).first();
        const measured = await match.evaluate(async element => {
          element.scrollIntoView({ block: 'center', behavior: 'instant' });
          await new Promise(resolve => requestAnimationFrame(resolve));
          const rect = element.getBoundingClientRect();
          return { text: element.innerText, document_y: rect.top + window.scrollY, viewport_y: rect.top, scroll_y: window.scrollY };
        });
        const png = path.join(path.dirname(pngFile), path.parse(pngFile).name + '-' + name + '.png');
        await page.screenshot({ path: png });
        reading.excerpts.push({ name, ...measured, screenshot: path.basename(png) });
      }
    }
    return { ...reading, errors, screenshot: path.basename(pngFile) };
  } finally {
    try { await browser?.close(); } finally { await host.close(); }
  }
}

export async function captureDeliveredPage(server, sampleDir, turn) {
  const parts = turn?.outcome?.answer_view?.parts ?? [];
  const pages = parts.filter(part => part.kind === 'presentation');
  const result = { turn_id: turn?.turn_id ?? null, delivered: pages.length, observations: [] };
  if (pages.length) {
    const { current } = await server.api('agent/history');
    if (!current?.turns.some(row => row.turn_id === turn.turn_id)) {
      throw new Error('Delivered turn does not belong to the selected Reader chat');
    }
    for (const [i, { presentation_id, revision }] of pages.entries()) {
      const reference = { presentation_id, revision };
      const view = await server.api('agent/presentation.read', {
        session_id: current.id, turn_id: turn.turn_id, reference,
      }, 'POST');
      const stem = 'delivered-' + (i + 1);
      await fs.writeFile(path.join(sampleDir, stem + '.json'), JSON.stringify(view));
      const reading = await captureNarrowPage(view, path.join(sampleDir, stem + '-320.png'));
      result.observations.push({ reference, ...reading });
    }
  }
  const statuses = result.observations.map(r => r.opening_order.status);
  result.opening_order = { scope:'frozen_learning_rate_handout', status:statuses.includes('failed') ? 'failed' :
    !statuses.length || statuses.includes('missing_evidence') ? 'missing_evidence' : 'passed' };
  if (result.observations.length === 1) result.opening_order = result.observations[0].opening_order;
  if (!pages.length) result.opening_order.reason = 'no_delivered_page';
  await fs.writeFile(path.join(sampleDir, 'delivery.json'), JSON.stringify(result, null, 2) + '\n');
  return result;
}
