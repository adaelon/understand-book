// Run against an already deployed isolated candidate. This test never submits a model question.
// stdin: {"reader":"password","reader-b":"password"}; secrets are not written to evidence.
import { chromium, webkit } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';

const base = process.env.READER_URL;
const output = process.env.READER_EVIDENCE_DIR;
assert(base?.startsWith('https://') && output, 'Set HTTPS READER_URL and READER_EVIDENCE_DIR');
const credentials = JSON.parse(await new Promise((resolve, reject) => {
  let input = ''; process.stdin.setEncoding('utf8');
  process.stdin.on('data', chunk => { input += chunk; });
  process.stdin.on('end', () => resolve(input)); process.stdin.on('error', reject);
}));
const engine = process.env.READER_BROWSER ?? 'chromium';
assert(['chromium', 'webkit'].includes(engine), 'READER_BROWSER must be chromium or webkit');
const testCertificate = process.env.READER_TEST_CERTIFICATE === '1';
const browser = await ({ chromium, webkit }[engine]).launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: testCertificate });
await context.addInitScript(() => {
  for (const type of ['error', 'unhandledrejection']) window.addEventListener(type, event => {
    console.error('MU11_BROWSER_EXCEPTION', type, String(event.reason ?? event.message));
  });
});
const errors = [], checks = [], failedRequests = [];
let phase = 'login';
context.on('page', page => {
  page.on('pageerror', e => errors.push({ phase, message: e.message, stack: e.stack }));
  page.on('requestfailed', r => failedRequests.push({ phase, method: r.method(), url: r.url(), error: r.failure()?.errorText }));
  page.on('console', m => { if (m.type() === 'error' && /(?:TypeError|ReferenceError):|MU11_BROWSER_EXCEPTION/.test(m.text())) errors.push(m.text()); });
});
const page = await context.newPage();
page.setDefaultTimeout(30_000);
async function login(owner) {
  await page.goto(base);
  await page.getByLabel('邮箱或账号', { exact: true }).fill(owner);
  await page.getByLabel('密码', { exact: true }).fill(credentials[owner]);
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await page.getByRole('heading', { name: '选择阅读材料' }).waitFor();
}
async function open(target, book) {
  const escaped = book.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  await target.getByRole('button', { name: new RegExp(`^${escaped}(?: · 版本 |$)`) }).first().waitFor();
  const single = target.getByRole('button', { name: book, exact: true });
  if (await single.count() === 1) await single.click();
  else {
    const versions = target.getByRole('button', { name: new RegExp(`^${escaped} · 版本 `) });
    const labels = await versions.allTextContents();
    assert(labels.length > 1 && new Set(labels).size === labels.length, 'Published versions must have distinct labels');
    await target.getByRole('button', { name: new RegExp(`^${escaped} · 版本 \\d+（默认）$`) }).click();
    checks.push('distinct publication labels and default selection');
  }
  await target.locator('.workspace-shell').waitFor();
}
async function logout() {
  const response = page.waitForResponse(r => r.url().endsWith('/api/auth/logout'));
  await page.getByRole('button', { name: '退出登录', exact: true }).click();
  assert.equal((await response).status(), 200);
  await page.getByRole('button', { name: '登录', exact: true }).waitFor();
}
const book = process.env.READER_BOOK ?? 'quickstart-demo';
try {
  await login('reader'); await open(page, book);
  await page.locator('.new-chat').waitFor();
  checks.push('reader login and actual Reader');
  phase = 'second tab';
  const other = await context.newPage(); await other.goto(base); await open(other, book);
  // Assert the persisted handles through their actual key, without depending on its object format.
  const workspaces = await Promise.all([page, other].map(p => p.evaluate(() => Object.entries(sessionStorage).filter(([key]) => /workspace/.test(key)))));
  assert.notDeepEqual(workspaces[0], workspaces[1], 'Independent tabs must own different workspace handles');
  checks.push('independent browser tabs');
  await page.setViewportSize({ width: 390, height: 844 });
  const input = page.locator('textarea[data-workspace-input="agent"]');
  await input.fill('MU11 private unsent draft');
  await page.setViewportSize({ width: 844, height: 390 });
  assert.equal(await input.inputValue(), 'MU11 private unsent draft');
  checks.push('rotation keeps draft');
  phase = 'close second tab';
  await other.close();
  await page.setViewportSize({ width: 1440, height: 1000 });
  await fs.mkdir(output, { recursive: true });
  await page.screenshot({ path: path.join(output, `${engine}-reader.png`) });
  phase = 'logout reader';
  await logout();
  phase = 'login reader-b';
  await login('reader-b'); await open(page, book);
  assert.equal(await input.inputValue(), '');
  checks.push('account switch clears private draft');
  phase = 'reload';
  await page.reload(); await page.locator('.workspace-shell').waitFor();
  checks.push('refresh restores reader');
  // WebKit reports a pageerror for a fetch cancelled by navigation even without a
  // DOM error/unhandledrejection. Keep that evidence, but require a matching
  // cancelled request in this deliberate navigation; real JS errors still fail.
  const navigationCancellations = errors.filter(error => typeof error === 'object'
    && engine === 'webkit' && ['reload', 'close second tab'].includes(error.phase)
    && error.stack?.startsWith('Fetch API cannot load ')
    && failedRequests.some(request => request.phase === error.phase
      && request.error === 'Load request cancelled' && error.stack.includes(request.url)));
  const unexpectedErrors = errors.filter(error => !navigationCancellations.includes(error));
  assert.deepEqual(unexpectedErrors, []);
  await logout();
  await fs.writeFile(path.join(output, `${engine}-browser.json`), JSON.stringify({ base, engine,
    test_certificate: testCertificate, device: 'Playwright desktop with resized viewport', checks,
    errors: unexpectedErrors, navigationCancellations }, null, 2));
  console.log(JSON.stringify({ engine, checks }));
} finally {
  await fs.mkdir(output, { recursive: true });
  await fs.writeFile(path.join(output, `${engine}-diagnostics.json`), JSON.stringify({ phase, checks, errors, failedRequests }, null, 2));
  await browser.close();
}
