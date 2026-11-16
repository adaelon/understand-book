import { expect, test, type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Real Rust HTTP fixture: cargo test -p server q4_browser_host -- --ignored --nocapture
// Admin Vite entry: pnpm --filter @understand-book/admin dev --host 127.0.0.1 --port 4184
const backend = 'http://127.0.0.1:4186', control = 'http://127.0.0.1:4187';
const output = resolve('../../docs/performance/allowance-budget-20261108/playwright');

test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) {
    await page.screenshot({ path: info.outputPath('failure.png'), fullPage: true });
    writeFileSync(info.outputPath('body.txt'), await page.locator('body').innerText());
  }
});

test('Q4 percentage balance, real manual top-up, gift, correction, history and account switch', async ({ page, context, browser }) => {
  test.setTimeout(120_000);
  const fixture = await page.request.get(`${control}/fixture`).then(r => r.json());
  let readerCookie = fixture.reader_cookie;
  const failures: string[] = [];
  page.on('pageerror', error => failures.push(error.message));
  const adminContext = await browser.newContext({ viewport: { width: 1440, height: 1100 } });
  const admin = await adminContext.newPage();
  admin.on('pageerror', error => failures.push(error.message));
  const proxy = async (target: typeof context, cookie: () => string) => target.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
      ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: cookie(),
    }});
    if (!response.ok() && !url.pathname.endsWith('source_manifest.json')) console.log('HTTP', response.status(), request.method(), url.pathname, await response.text());
    await route.fulfill({ response });
  });
  await proxy(context, () => readerCookie);
  await proxy(adminContext, () => fixture.admin_cookie);
  await context.route('**/q4-acceptance', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
    createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  const openAllowance = async () => {
    await page.getByRole('button', { name: '使用额度', exact: true }).click();
    await expect(page.getByRole('dialog', { name: 'AI 使用额度' })).toBeVisible();
  };
  const available = page.locator('.allowance-balances .available dd');
  const refresh = async (value: string) => {
    await page.getByRole('button', { name: '刷新额度与明细' }).click();
    await expect(available).toHaveText(value);
  };
  const submit = async (target: Page, title: string) => {
    const dialog = target.getByRole('dialog', { name: title, exact: true });
    await dialog.locator('button[type=submit]').click();
    await expect(dialog).not.toBeVisible();
    await expect(target.getByText('操作已完成', { exact: false })).toBeVisible();
  };
  const field = (title: string, name: string) => admin.getByRole('dialog', { name: title, exact: true }).locator(`[name="${name}"]`);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto('/q4-acceptance');
  await openAllowance();
  await expect(available).toHaveText('96.8%');
  await expect(page.locator('.usage-list').last()).toContainText('低于 0.1%');
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(page.getByRole('navigation', { name: '使用记录分页' })).toContainText('21–22 / 22');
  await expect(page.locator('.usage-list').last()).toContainText('2.3%');

  await admin.goto('http://127.0.0.1:4184/admin/users/A');
  await admin.getByRole('button', { name: '额度与套餐', exact: true }).click();
  await admin.getByText('管理额度加购商品 · 所有账号共用', { exact: true }).click();
  await admin.getByRole('button', { name: '新增加购商品', exact: true }).click();
  await field('新增加购商品', 'title').fill('考试冲刺补充');
  await field('新增加购商品', 'amount').fill('1.23');
  await field('新增加购商品', 'budget').fill('0.5');
  await field('新增加购商品', 'instructions').fill('联系运营，确认实际收款后到账');
  await submit(admin, '新增加购商品');
  await refresh('96.8%');
  await page.getByRole('button', { name: '补充额度', exact: true }).click();
  const offer = page.getByRole('region', { name: '补充额度', exact: true });
  await expect(offer).toContainText('增加的用量相当于本期总额度的 68.5%');
  await expect(offer).toContainText('支付 ¥1.23');
  await expect(available).toHaveText('96.8%');

  await admin.getByRole('button', { name: '收款并补充本期', exact: true }).first().click();
  await field('收款并补充本期', 'amount').fill('1.23');
  await field('收款并补充本期', 'budget').fill('0.5');
  await field('收款并补充本期', 'external_ref').fill('q4-browser-payment');
  await field('收款并补充本期', 'note').fill('浏览器验收人工收款');
  await submit(admin, '收款并补充本期');
  await refresh('98.1%');
  await admin.getByRole('button', { name: '赠送 / 扣回预算', exact: true }).first().click();
  await field('赠送 / 扣回预算', 'budget').fill('0.1');
  await field('赠送 / 扣回预算', 'reason').fill('浏览器验收赠送');
  await submit(admin, '赠送 / 扣回预算');
  await refresh('98.3%');
  await admin.getByRole('button', { name: '费用与账单', exact: true }).click();
  await admin.getByText('收款、额度调整与操作记录', { exact: true }).click();
  await admin.getByRole('button', { name: '追加更正', exact: true }).click();
  await field('追加收款更正', 'amount').fill('-0.23');
  await field('追加收款更正', 'delta').fill('-0.1');
  await field('追加收款更正', 'reason').fill('更正实际收款和授额');
  await submit(admin, '追加收款更正');
  await refresh('98.1%');
  const history = admin.locator('section').filter({ has: admin.getByRole('heading', { name: '收款与额度明细', exact: true }) });
  await expect(history).toContainText('1.23 / 1.00');
  await expect(history).toContainText('0.50 元 AI 预算');
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(page.locator('.usage-list').last()).toContainText('2.3%');
  await page.getByRole('button', { name: '刷新额度与明细' }).click();
  await expect(available).toHaveText('98.1%');
  mkdirSync(output, { recursive: true });
  await page.locator('dialog').evaluate(el => { el.scrollTop = 0; });
  await page.screenshot({ path: resolve(output, 'allowance-desktop.png'), fullPage: true });
  await admin.getByRole('button', { name: '额度与套餐', exact: true }).click();
  await admin.getByRole('heading', { name: '本期 AI 使用预算', exact: true }).scrollIntoViewIfNeeded();
  await admin.screenshot({ path: resolve(output, 'operations-desktop.png'), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.locator('dialog').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.screenshot({ path: resolve(output, 'allowance-mobile.png'), fullPage: true });

  readerCookie = fixture.admin_cookie;
  await page.reload();
  await openAllowance();
  await expect(page.getByRole('dialog')).toContainText('当前没有有效额度期');
  await expect(page.getByRole('dialog')).toContainText('暂无使用记录');
  await expect(page.getByRole('dialog')).not.toContainText('98.1%');
  expect(failures).toEqual([]);
  await adminContext.close();
});

test('Q4 mobile percentage remains on one line with real account data', async ({ page, context }) => {
  const fixture = await page.request.get(`${control}/fixture`).then(r => r.json());
  await context.route(/\/api\/|\/books\//, async route => {
    const request = route.request(), url = new URL(request.url());
    const response = await route.fetch({ url: `${backend}${url.pathname}${url.search}`, headers: {
      ...await request.allHeaders(), host: 'reader.example', origin: 'https://reader.example', cookie: fixture.reader_cookie,
    }});
    await route.fulfill({ response });
  });
  await context.route('**/q4-mobile', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head><body><div id="app"></div><script type="module">
    import {createApp} from '/node_modules/.vite/deps/vue.js';
    import NetworkApp from '/src/NetworkApp.vue'; import '/src/style.css';
    createApp(NetworkApp).mount('#app');
  </script></body></html>` }));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/q4-mobile');
  await page.getByRole('button', { name: '使用额度', exact: true }).click();
  const amount = page.locator('.allowance-balances .available dd');
  await expect(amount).toHaveText(/\d+\.\d+%/);
  expect(await amount.evaluate(el => {
    const node = el.firstChild!;
    const first = document.createRange(), last = document.createRange();
    first.setStart(node, 0); first.setEnd(node, 1);
    last.setStart(node, (node.textContent?.length ?? 1) - 1); last.setEnd(node, node.textContent!.length);
    return Math.abs(first.getBoundingClientRect().top - last.getBoundingClientRect().top) < 1;
  })).toBe(true);
  expect(await page.locator('dialog').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.getByRole('button', { name: '补充额度', exact: true }).click();
  mkdirSync(output, { recursive: true });
  await page.screenshot({ path: resolve(output, 'allowance-mobile.png'), fullPage: true });
});
