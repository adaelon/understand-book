// Run with the Web Vite server at http://127.0.0.1:4174.
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const { chromium } = createRequire(new URL('../../../packages/web/package.json', import.meta.url))('@playwright/test');
const base = process.env.YY1_BASE_URL ?? 'http://127.0.0.1:4174';
const output = fileURLToPath(new URL('.', import.meta.url));
const browser = await chromium.launch();
const report = { base, viewports: [], colors: [], errors: [] };

async function sample(locator, name, mode = 'text') {
  await locator.page().waitForTimeout(260); // Read the settled interaction color, after CSS transitions.
  const result = await locator.evaluate((el, mode) => {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const ctx = canvas.getContext('2d');
    const rgba = color => {
      ctx.clearRect(0, 0, 1, 1); ctx.fillStyle = color; ctx.fillRect(0, 0, 1, 1);
      const p = [...ctx.getImageData(0, 0, 1, 1).data]; return [...p.slice(0, 3), p[3] / 255];
    };
    const over = (front, back) => front.slice(0, 3).map((v, i) => v * front[3] + back[i] * (1 - front[3]));
    function background(node) {
      const layers = [];
      for (; node; node = node.parentElement) layers.unshift(rgba(getComputedStyle(node).backgroundColor));
      return layers.reduce((color, layer) => over(layer, color), [255, 255, 255]);
    }
    const s = getComputedStyle(el), bg = background(mode === 'focus' ? el.parentElement : el);
    const fg = over(rgba(mode === 'focus' ? s.outlineColor : s.color), bg);
    const luminance = rgb => rgb.map(v => v / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4)
      .reduce((value, v, i) => value + v * [.2126, .7152, .0722][i], 0);
    const a = luminance(fg), b = luminance(bg);
    return { foreground: fg, background: bg, ratio: (Math.max(a, b) + .05) / (Math.min(a, b) + .05),
      opacity: s.opacity, disabled: el.matches(':disabled'), outline: `${s.outlineWidth} ${s.outlineStyle}`, focusVisible: el.matches(':focus-visible') };
  }, mode);
  report.colors.push({ name, mode, ...result });
  if (mode === 'focus') assert(result.focusVisible && result.outline === '2px solid', `${name}: missing keyboard focus`);
  if (mode !== 'disabled') assert(result.ratio >= (mode === 'focus' ? 3 : 4.5), `${name}: contrast ${result.ratio.toFixed(2)}`);
  else assert(result.disabled, `${name}: expected disabled control`);
}

async function shot(page, name) {
  await page.evaluate(() => document.fonts.ready);
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${name}: horizontal overflow`);
  await page.screenshot({ path: `${output}${name}.png`, fullPage: true });
}

async function library(page) {
  const reference = { book_id: '理解复杂系统：从概念到实践', publication_id: 'original' };
  await page.addInitScript(() => sessionStorage.setItem('understand-book:workspace:reader', JSON.stringify({ workspace_id: 'this-window' })));
  await page.route('**/api/**', route => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/auth/me') return route.fulfill({ json: { user_id: 'reader', csrf_token: 'csrf', email: 'reader@example.com' } });
    if (path === '/api/library') return route.fulfill({ json: { books: [
      { published_book_ref: { ...reference, publication_id: 'new' }, is_default: true },
      { published_book_ref: reference }, { published_book_ref: { book_id: '学习的艺术', publication_id: 'other' } },
    ] } });
    if (path.endsWith('/resumption')) return route.fulfill({ json: { workspace_id: 'this-window', published_book_ref: reference,
      selected_chat: 'original-chat', position_label: '段落 · 反馈与系统边界',
      position_excerpt: '理解一个系统，先分清系统的边界，再观察其中的反馈。相同的行动，在不同条件下可能产生不同的结果。',
      last_question: '正反馈一定会让系统失稳吗？如果同时存在负反馈，应该怎样判断最后的变化？' } });
    if (path === '/api/workspaces/this-window') return route.fulfill({ json: { workspace_id: 'this-window', generation: 2, revision: 8, published_book_ref: reference, selected_chat: 'original-chat', reader: {} } });
    if (path.endsWith('/attach')) return route.fulfill({ status: 409, json: { message: '现场已更新，请重试', error_code: 'WORKSPACE_REVISION_CONFLICT' } });
    return route.fulfill({ status: 404, json: {} });
  });
  await page.route('**/yy1-library', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js'; import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css'; createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  await page.goto(`${base}/yy1-library`);
  await page.locator('.resumption-continue').waitFor();
}

try {
  for (const width of [1440, 390]) {
    const page = await browser.newPage({ viewport: { width, height: 900 } });
    page.on('pageerror', error => report.errors.push(error.message));
    report.viewports.push({ width, height: 900 });
    await page.goto(`${base}/home.html`);
    const cta = page.locator('.home-button-dark');
    await sample(cta, `home CTA ${width}`);
    await cta.hover(); await page.waitForTimeout(240);
    await sample(cta, `home CTA hover ${width}`);
    await page.mouse.move(0, 0); await page.keyboard.press('Tab'); await cta.focus();
    await sample(cta, `home CTA focus ${width}`, 'focus');
    await cta.blur();
    for (const img of await page.locator('.home-screenshot-preview > img').all()) {
      await img.scrollIntoViewIfNeeded(); await img.evaluate(el => el.decode());
    }
    await page.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
    await page.screenshot({ path: `${output}home-hero-${width}.png` });
    await shot(page, `home-${width}`);
    const homeLink = page.locator('.home-hero .home-text-link');
    await sample(homeLink, `home text link ${width}`);
    await homeLink.hover(); await page.waitForTimeout(240);
    await sample(homeLink, `home text link hover ${width}`);
    await page.getByRole('radio', { name: '长和宽一样时，面积最大', exact: true }).check();
    await sample(page.locator('.home-feedback-choices label.selected'), `home selected choice ${width}`);
    await sample(page.locator('.home-story-tabs button.active'), `home selected tab ${width}`);
    await page.evaluate(() => document.activeElement?.blur());
    await page.locator('#learning-story').screenshot({ path: `${output}home-selected-${width}.png` });
    await page.getByRole('button', { name: '04 换个条件', exact: false }).click();
    await sample(page.locator('.home-verify'), `home verify disabled ${width}`, 'disabled');
    await page.getByRole('radio', { name: '50 平方米', exact: true }).check();
    await sample(page.locator('.home-verify'), `home verify enabled ${width}`);
    const register = page.locator('.home-button-light');
    await page.keyboard.press('Tab'); await register.focus();
    await sample(register, `home register ${width}`);
    await sample(register, `home dark section focus ${width}`, 'focus');

    await library(page);
    const resume = page.locator('.resumption-continue');
    await sample(resume, `library resume ${width}`);
    await resume.hover(); await page.waitForTimeout(240);
    await sample(resume, `library resume hover ${width}`);
    await page.keyboard.press('Tab'); await resume.focus();
    await sample(resume, `library resume focus ${width}`, 'focus');
    await shot(page, `library-${width}`);
    await resume.click(); await page.getByRole('alert').filter({ hasText: '现场已更新' }).waitFor();
    await shot(page, `library-error-${width}`);
    await page.unroute('**/api/**');

    await page.goto(`${base}/mobile-workspace-visual.html?long-chat&task&learning`);
    if (width < 1024) await page.locator('.workspace-mobile-nav').getByRole('button', { name: '问答', exact: true }).click();
    else await page.getByRole('button', { name: '问答全屏', exact: true }).click();
    const input = page.locator('.agent-input textarea'), send = page.locator('.send-agent');
    await input.fill('接着上一节，看看条件改变后会发生什么。'); await input.blur();
    if (width >= 1024) await sample(page.locator('.history-button span').first(), `chat history badge ${width}`);
    await sample(send, `chat send ${width}`);
    await send.hover(); await page.waitForTimeout(240);
    await sample(send, `chat send hover ${width}`);
    await page.keyboard.press('Tab'); await send.focus();
    await sample(send, `chat send focus ${width}`, 'focus');
    await shot(page, `chat-${width}`);
    await input.fill(''); await input.blur();
    await sample(send, `chat send disabled ${width}`, 'disabled');
    await shot(page, `chat-disabled-${width}`);
    const text = await page.locator('.answer-markdown p').first().evaluate(el => {
      const s = getComputedStyle(el), box = el.getBoundingClientRect();
      return { color: s.color, fontSize: s.fontSize, lineHeight: s.lineHeight, width: box.width };
    });
    report.viewports.at(-1).answer = text;

    await page.route('**/missing-annotation-visual.pdf', route => route.fulfill({ contentType: 'application/pdf',
      body: readFileSync(new URL('../../../packages/core/test/fixtures/hybrid-foundation-goldset/v1/licensed-two-column-formula/paper.pdf', import.meta.url)) }));
    await page.goto(`${base}/pdf-annotation-visual.html`);
    await page.locator('.pdf-text-layer span').filter({ hasText: /^Left Formula$/ }).waitFor();
    const marker = page.getByRole('button', { name: '打开 2 条 PDF 笔记' });
    await sample(marker, `PDF note marker ${width}`);
    await sample(page.getByRole('button', { name: '隐藏笔记标记' }), `PDF selected tool ${width}`);
    await marker.click(); await page.locator('.pdf-annotation-surface').waitFor();
    await sample(page.locator('.pdf-annotation-surface .note-kind').first(), `PDF note label ${width}`);
    await shot(page, `pdf-notes-${width}`);
    await page.close();
  }
  assert.deepEqual(report.errors, []);
  report.status = 'passed';
} catch (error) {
  report.status = 'failed'; report.failure = error.message; throw error;
} finally {
  writeFileSync(`${output}verification.json`, JSON.stringify(report, null, 2) + '\n');
  await browser.close();
}
