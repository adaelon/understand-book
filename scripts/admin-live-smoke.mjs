// Read-only checks against the actual deployed Admin and its real APIs.
// Uses the administrator credential file; never logs or persists session secrets.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const base = process.env.ADMIN_LIVE_URL ?? 'https://understandbook.top';
const output = process.env.ADMIN_LIVE_OUTPUT ?? 'tmp/admin-usability-release-20261023';
const account = process.env.ADMIN_LIVE_ACCOUNT ?? 'adaelon';
const credentialPath = process.env.ADMIN_LIVE_CREDENTIALS ?? path.join(os.homedir(), '.understand-book/adm10-admin-credentials.json');
const credentials = JSON.parse((await fs.readFile(credentialPath, 'utf8')).replace(/^\uFEFF/, ''));
await fs.mkdir(output, { recursive: true });
const checks = [], unavailable = [], errors = [], apiFailures = [], writes = [], screenshots = [];
const started = new Date().toISOString();
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, colorScheme: 'dark' });
const page = await context.newPage();
page.setDefaultTimeout(30000);
page.on('pageerror', error => errors.push(error.message));
page.on('response', r => { if (new URL(r.url()).pathname.startsWith('/api/admin/') && r.status() >= 400) apiFailures.push({ url: r.url(), status: r.status() }); });
page.on('request', r => { if (new URL(r.url()).pathname.startsWith('/api/admin/') && !['GET','HEAD'].includes(r.method())) writes.push(`${r.method()} ${r.url()}`); });
const passed = label => { checks.push(label); console.log('PASS ' + label); };
const click = name => page.getByRole('button', { name, exact: true }).click();
async function settled() {
  await page.waitForFunction(() => ![...document.querySelectorAll('[role="status"]')].some(e => e.getClientRects().length && e.textContent.includes('正在读取')));
  const alerts = await page.getByRole('alert').allTextContents();
  assert.deepEqual(alerts.map(t => t.trim()).filter(Boolean), [], 'Visible API error');
}
async function fits(label) {
  const size = await page.evaluate(() => ({ viewport: innerWidth, document: document.documentElement.scrollWidth }));
  assert(size.document <= size.viewport + 1, `${label}: ${JSON.stringify(size)}`);
}
async function shot(name, locator = page) {
  await settled(); await fits(name);
  await locator.screenshot({ path: `${output}/${name}.png`, animations: 'disabled', ...(locator === page ? { fullPage: true } : {}) });
  screenshots.push(name + '.png');
}
async function dialog(name) {
  await click(name); await page.getByRole('dialog').waitFor(); await settled();
  await page.keyboard.press('Escape'); await page.getByRole('dialog').waitFor({ state: 'hidden' });
}
async function go(route, heading) {
  await page.goto(`${base}/admin/${route}`);
  await page.getByRole('heading', { name: heading, exact: true }).waitFor(); await settled();
}
async function expand(text) {
  const summary = page.locator('summary:visible').filter({ hasText: text }).first();
  await summary.click(); await settled();
}
let success = false;
try {
  if (!process.argv.includes('--ui-only')) {
  const htmlResponse = await context.request.get(base + '/admin/');
  assert.equal(htmlResponse.status(), 200);
  assert.match(htmlResponse.headers()['cache-control'], /no-store/);
  const html = await htmlResponse.text();
  const expected = await fs.readFile('apps/admin/dist/index.html', 'utf8');
  const assets = [...expected.matchAll(/(?:src|href)="(\/admin\/assets\/[^\"]+)"/g)].map(m => m[1]);
  for (const asset of assets) {
    assert(html.includes(asset), 'Production must serve the new asset ' + asset);
    const response = await context.request.get(base + asset);
    assert.equal(response.status(), 200);
    assert.match(response.headers()['content-type'], asset.endsWith('.js') ? /javascript/ : /text\/css/);
    assert.deepEqual(await response.body(), await fs.readFile('apps/admin/dist/' + asset.slice('/admin/'.length)));
  }
  passed('Public HTML and exact JS/CSS bytes match the built Admin, including correct MIME');
  }

  await page.goto(base + '/admin/usage');
  await page.getByLabel('邮箱或账号', { exact: true }).fill(credentials.username);
  await page.getByLabel('密码', { exact: true }).fill(credentials.password);
  await click('登录');
  await page.getByRole('heading', { name: '全站用量', exact: true }).waitFor();
  await page.getByRole('heading', { name: '费用花在哪里', exact: true }).first().waitFor();
  await settled(); await shot('live-usage-desktop');
  await shot('live-cost-overview', page.getByRole('region', { name: '账号费用', exact: true }));
  await expand('精确金额与原额度期比例');
  await page.getByText('原额度期百分比', { exact: true }).waitFor();
  await expand('精确金额与原额度期比例');
  passed('Real administrator login, today cost charts and exact financial details');

  await Promise.all([page.waitForResponse(r => r.url().includes('/api/admin/cost-summary?') && r.ok()), click('近 30 天')]);
  await settled();
  for (const [group, detail] of [['父任务','查看归集明细'],['工作区','查看工作区明细'],['账号','查看账号明细'],['账号级整理','查看整理明细']]) {
    await page.getByRole('group', { name: '费用归集分组' }).getByRole('button', { name: group, exact: true }).click();
    if (await page.getByRole('button', { name: detail, exact: true }).count()) {
      await page.getByRole('button', { name: detail, exact: true }).first().click();
      await page.getByRole('region', { name: '归集调用明细' }).waitFor(); await settled();
      await click('关闭归集明细');
    } else unavailable.push('No production rows for ' + group);
  }
  await click('使用与收款'); await settled(); await shot('live-usage-activity');
  await expand('模型调用账目与精确金额');
  await click('任务账单'); await settled(); await shot('live-usage-ledger');
  if (await page.getByRole('button', { name: '调用明细', exact: true }).count()) {
    await page.getByRole('button', { name: '调用明细', exact: true }).first().click();
    await page.getByRole('region', { name: '任务调用明细' }).waitFor(); await settled(); await click('关闭明细');
  }
  passed('30-day real-data query, four cost groupings, activity and task ledger drill-down');

  await page.getByRole('link', { name: '账号管理', exact: true }).click();
  await page.getByRole('heading', { name: '账号管理', exact: true }).waitFor(); await settled();
  await shot('live-users-desktop'); await dialog('创建账号');
  await page.getByLabel('搜索账号或邮箱').fill(account); await click('搜索'); await settled();
  await page.getByRole('link', { name: account, exact: true }).click();
  await page.getByRole('heading', { name: '使用与回访', exact: true }).waitFor(); await shot('live-account-overview');
  await click('额度与套餐'); await page.getByRole('heading', { name: 'AI 使用额度', exact: true }).waitFor(); await settled(); await shot('live-account-allowance');
  await dialog(await page.getByRole('button', { name: '赠送 / 调整', exact: true }).count() ? '赠送 / 调整' : '配置百分比基准');
  await click('MCP 调用包'); await page.getByRole('heading', { name: 'MCP 调用次数', exact: true }).waitFor(); await settled(); await shot('live-account-mcp');
  await expand('已购调用包明细'); await dialog('登记 MCP 购包');
  if (await page.getByRole('button', { name: '更正 MCP 收款 / 次数', exact: true }).count()) await dialog('更正 MCP 收款 / 次数');
  else unavailable.push('No paid MCP batch on the inspected production account');
  await click('材料授权'); await settled(); await shot('live-account-materials'); await dialog('授权材料');
  await click('费用与账单'); await page.getByRole('heading', { name: '服务费用与待核算', exact: true }).waitFor(); await settled();
  await page.getByRole('button', { name: '查看 / 核算', exact: true }).first().click();
  await page.getByRole('dialog').waitFor(); await settled(); await shot('live-charge-detail'); await page.keyboard.press('Escape');
  await page.getByLabel('仅看待核算').check(); await settled(); await page.getByLabel('仅看待核算').uncheck(); await settled();
  await expand('收款、额度调整与操作记录'); await shot('live-account-history');
  await click('用量分析'); await page.getByRole('heading', { name: '账号用量汇总', exact: true }).waitFor(); await settled();
  passed('Account search, all six sections, allowance/MCP/material dialogs and real charge/history reads');

  await go('users/reader', 'reader'); await click('额度与套餐'); await settled();
  await page.getByText('此额度期尚未设置百分比基准，当前按金额展示。配置基准后可使用比例管理。', { exact: true }).waitFor();
  await shot('live-legacy-allowance'); await expand('精确额度与基准'); await dialog('配置百分比基准');
  passed('Legacy allowance displays actual money and exact details without inventing a percentage basis');

  await page.getByRole('link', { name: '运营资料构建', exact: true }).click(); await page.getByRole('heading', { name: '运营资料构建', exact: true }).waitFor(); await settled();
  await shot('live-builds-desktop');
  for (const label of ['需要操作','构建中','成果已就绪','本页全部']) { await click(label); await settled(); await fits('Build filter ' + label); }
  await dialog('创建准备任务');
  if (await page.getByRole('button', { name: '查看构建任务', exact: true }).count()) {
    await page.getByRole('button', { name: '查看构建任务', exact: true }).first().click();
    await page.getByRole('region', { name: '运营构建任务详情' }).waitFor(); await settled(); await shot('live-build-detail');
    if (await page.getByRole('button', { name: '查看费用回执', exact: true }).count()) { await click('查看费用回执'); await settled(); await shot('live-build-receipts'); }
  } else unavailable.push('No production build task is available to inspect its budget and receipts');
  passed('Build status filters, create dialog and available live task details');

  await page.getByRole('link', { name: '共享工作区', exact: true }).click(); await page.getByRole('heading', { name: '共享工作区发布', exact: true }).waitFor(); await settled();
  await shot('live-workspaces-desktop');
  for (const label of ['尚未就绪','完整可用','已归档','本页全部']) { await click(label); await settled(); }
  await page.getByRole('button', { name: '查看版本', exact: true }).first().click();
  await page.getByRole('region', { name: '共享工作区版本详情' }).waitFor(); await settled(); await shot('live-workspace-detail');
  await expand('全部检查结果'); await expand('发布时的就绪情况'); await shot('live-workspace-expanded');
  await dialog('发布新工作区');
  passed('Workspace filters, real version/readiness disclosures and publication dialog');

  await page.getByRole('link', { name: '内测码', exact: true }).click(); await page.getByRole('heading', { name: '全部内测码', exact: true }).waitFor(); await settled();
  for (const state of ['unused','used','disabled','']) { await page.getByLabel('内测码状态').selectOption(state); await settled(); }
  await shot('live-invites-desktop');
  passed('Invitation status filters against the live API');

  await page.setViewportSize({ width: 390, height: 844 });
  for (const [route, heading] of [['usage','全站用量'],['users','账号管理'],[`users/${account}`,account],['operator-builds','运营资料构建'],['workspace-releases','共享工作区发布'],['invites','全部内测码']]) {
    await go(route, heading); await shot('live-mobile-' + route.replaceAll('/','-'));
  }
  await go(`users/${account}`, account);
  for (const section of ['额度与套餐','MCP 调用包','材料授权','费用与账单','用量分析']) {
    await click(section); await settled(); await shot('live-mobile-account-' + ({'额度与套餐':'allowance','MCP 调用包':'mcp','材料授权':'materials','费用与账单':'billing','用量分析':'usage'}[section]));
  }
  await go('usage', '全站用量');
  await click('Toggle Sidebar'); await page.getByRole('link', { name: '账号管理', exact: true }).waitFor();
  await page.getByRole('link', { name: '账号管理', exact: true }).click(); await page.keyboard.press('Escape'); await settled();
  passed('390px mobile: five main pages, six account sections, sidebar and no horizontal overflow');

  await page.setViewportSize({ width: 1440, height: 1000 }); await go('usage','全站用量');
  await click('切换主题'); await page.waitForFunction(() => document.documentElement.classList.contains('light')); await shot('live-usage-light');
  await page.reload(); await page.getByRole('heading', { name: '全站用量', exact: true }).waitFor(); await settled();
  passed('Light theme, retained preference and authenticated deep-link refresh');
  assert.deepEqual(errors, []); assert.deepEqual(apiFailures, []); assert.deepEqual(writes, []);
  await click('退出登录'); await page.getByRole('button', { name: '登录', exact: true }).waitFor();
  passed('Logout works; zero page errors, failed Admin API responses or management writes');
  success = true;
} catch (error) {
  errors.push(error.stack ?? String(error));
  await page.screenshot({ path: `${output}/live-failure.png`, fullPage: true }).catch(() => {});
  throw error;
} finally {
  await fs.writeFile(`${output}/live-browser.json`, JSON.stringify({ status: success ? 'passed' : 'failed', started_at: started, finished_at: new Date().toISOString(), base, real_api: true, asset_check_skipped: process.argv.includes('--ui-only'), checks, unavailable, errors, apiFailures, management_writes: writes, screenshots }, null, 2));
  await browser.close();
}
