// UI regression and screenshots against the real admin app with representative API fixtures.
// Run Vite first: pnpm --filter @understand-book/admin dev --host 127.0.0.1 --port 5184
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';

const base = process.env.ADMIN_UI_URL ?? 'http://127.0.0.1:5184';
const output = process.env.ADMIN_UI_OUTPUT ?? 'tmp/admin-usability-20261023';
await fs.mkdir(output, { recursive: true });
const today = new Date(Date.now() + 8 * 3600_000).toISOString().slice(0, 10);
const now = Math.floor(Date.now() / 1000);
const pageOf = items => ({ items, total: items.length, limit: 20, offset: 0 });
const pct = value => ({ value: String(value), nonzero: value !== 0, below_tenth: false });
const emptyCost = { request_count: 0, confirmed_cost_micro_cny: 0, account_debit_micro_cny: 0, platform_covered_micro_cny: 0, active_reserved_micro_cny: 0, active_requests: 0, pending_micro_cny: 0, pending_requests: 0, unknown_cost_requests: 0, reconciliation_requests: 0, estimated_cost_micro_cny: 0, estimated_cost_requests: 0, provider_cost_complete: true, provider_cost_micro_cny: 0 };
const categories = {
  model: { ...emptyCost, request_count: 40, confirmed_cost_micro_cny: 3903378, account_debit_micro_cny: 3503378, platform_covered_micro_cny: 400000, provider_cost_micro_cny: 3903378 },
  web_search: { ...emptyCost, request_count: 8, confirmed_cost_micro_cny: 1280000, account_debit_micro_cny: 1280000, provider_cost_micro_cny: 1280000 },
  web_fetch: { ...emptyCost, request_count: 4, confirmed_cost_micro_cny: 160000, account_debit_micro_cny: 160000, pending_micro_cny: 30000, pending_requests: 1, unknown_cost_requests: 1, provider_cost_complete: false, provider_cost_micro_cny: null },
  service: { ...emptyCost, request_count: 2, estimated_cost_micro_cny: 120000, estimated_cost_requests: 2, provider_cost_complete: false, provider_cost_micro_cny: null },
};
const zero = { ...emptyCost, cost_breakdown: Object.fromEntries(Object.keys(categories).map(k => [k, emptyCost])), period_groups: [] };
const summary = { ...emptyCost, request_count: 54, confirmed_cost_micro_cny: 5343378, account_debit_micro_cny: 4943378, platform_covered_micro_cny: 400000, pending_micro_cny: 30000, pending_requests: 1, unknown_cost_requests: 1, estimated_cost_micro_cny: 120000, estimated_cost_requests: 2, provider_cost_complete: false, provider_cost_micro_cny: null, cost_breakdown: categories, period_groups: [{ user_id: 'reader', period_id: '2026-oct', allowance_basis: { version: 'standard', basis_micro_cny: 10000000 }, percentages: { debited: pct(49.4), active_reserved: pct(0), pending: pct(0.3) } }] };
const tasks = ['理解概率论中的条件独立', '后台整理 · 本周学习进度', 'course-review-20261023-long-reference-for-workspace'].map((id, index) => ({ ...summary, parent_task_ref: id, cost_owner: 'account', user_id: 'reader', confirmed_cost_micro_cny: [3903378,1280000,160000][index], learning_workspace_ids: ['probability-course'], related_learning_workspace_ids: index ? [] : ['revision-workspace'], purposes: ['answer'] }));
const activity = { first_used_at: now - 86400 * 20, last_read_at: now - 1800, last_question_at: now - 3600, active_days: 12, return_days: 8, completed_runs: 46 };
const period = { period_id: '2026-oct', starts_at: now - 86400 * 10, expires_at: now + 86400 * 20, revision: 2, balance: { granted_micro_cny: 15000000, debited_micro_cny: 1000000, active_reserved_micro_cny: 1000000, pending_micro_cny: 0, available_micro_cny: 13000000, allowance_basis: { version: 'standard', basis_micro_cny: 10000000 }, percentages: { granted: pct(150), debited: pct(10), active_reserved: pct(10), pending: pct(0), available: pct(130), deficit: pct(0) } } };
const book = { book_id: '概率论入门', publication_id: '2026-10-v2' };
const user = { user_id: 'reader', email: 'reader@example.test', disabled: false, is_admin: false, current_allowance: period, activity, book_grants: pageOf([book]) };
const charge = { call_id: 'read-web-20261023', user_id: 'reader', cost_owner: 'account', revision: 1, state: 'pending', model: null, entry_kind: 'web_fetch', purpose: 'source_reading', provider_cost_status: 'pending', provider_cost_micro_cny: null, account_debit_micro_cny: null, reserved_micro_cny: 30000, needs_reconciliation: false, created_at: now - 3600, settled_at: null, rate_snapshot: {}, reservation_estimate: {}, provider_usage: null, reports: pageOf([]), reconciliations: pageOf([]) };
const readiness = ready => ({ status: ready ? 'ready' : 'incomplete', materials: [{ published_book_ref: book, available_capabilities: ['text'], missing_capabilities: ready ? [] : ['tutor'], publication_available: true }], audience: [{ user_id: 'reader', ready, missing_grants: ready ? [] : [book] }] });
const releases = ['概率论学习路线', '微积分基础复习', '往期课程归档'].map((name, i) => ({ release_id: `course-${i}`, version: 2, latest_version: 2, archived: i === 2, created_at: now - i * 86400, publisher_user_id: 'operator', visible_to_users: ['reader'], allowed_tiers: ['basic'], readiness_at_publication: readiness(true), availability: readiness(i !== 1), manifest: { name, publisher: 'Understand Book', public_goal: '从概念出发，建立可用于解题的理解。', applicability: '希望系统复习的学习者', prerequisites: '基础代数', materials: [{ published_book_ref: book, required_capabilities: ['text','tutor'] }], topic_links: [], suggested_route: [{ title: '先理解问题，再看形式化定义', description: '从真实例子进入条件概率。', materials: [book] }] } }));
const builds = ['running','awaiting_confirmation','paused','ready'].map((state,i) => ({ task_id: `build-${i}`, name: ['概率论视频课程','数学分析阅读材料','线性代数补充资料','音乐伴听课程'][i], state, revision: 1, created_at: now - 3600, updated_at: now - 60, max_micro_cny: 80000000, reserved_per_execution_micro_cny: 6000000, visible_to_users: ['reader'], pause_requested: false, budget: { max_micro_cny: 80000000, provider_cost_micro_cny: 22400000, estimated_cost_micro_cny: 1200000, pending_micro_cny: 5000000, reserved_micro_cny: 6000000, remaining_micro_cny: 45400000, unknown_count: 2 }, materials: [{ material_id: 'video-1', label: '条件概率与贝叶斯公式', material_kind: 'video', state: 'running', observation: { progress: { stage: 'translation', completed: 3, total: 8, unit: 'batches', message: '已完成的字幕批次会保留。' } } }] }));
const mcp = { month_hk: today.slice(0,7), free_granted: 1000, free_used: 320, free_remaining: 680, free_resets_at: new Date((now + 86400 * 8) * 1000).toISOString(), paid_remaining: 1400, paid_next_expires_at: new Date((now + 86400 * 90) * 1000).toISOString(), call_packs: [{ pack_id: 'pack-20261023', receipt_id: 'receipt-20261023', pack_count: 2, unit_price_fen: 1000, units_per_pack: 1000, price_fen: 2000, total_units: 2000, adjusted_units: -100, used_units: 500, remaining_units: 1400, available_units: 1400, opened_at: new Date((now - 86400) * 1000).toISOString(), expires_at: new Date((now + 86400 * 90) * 1000).toISOString(), revision: 2 }] };
const invites = ['unused','used','disabled'].map((state,i) => ({ invite_id: `invite-${i}`, code: `ABCDE1234F${i}`, state, created_at: now - 86400, actor: 'operator', used_by: state==='used'?'reader':null, used_email: state==='used'?'reader@example.test':null, used_at: state==='used'?now:null, disabled_at: state==='disabled'?now:null }));

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, colorScheme: 'dark' });
const page = await context.newPage();
page.setDefaultTimeout(10000);
const errors = [], requests = [], checks = [];
page.on('pageerror', e => errors.push(String(e)));
await context.route('**/api/**', async route => {
  const request = route.request(), url = new URL(request.url()), path = url.pathname;
  requests.push(`${request.method()} ${path}${url.search}`);
  if (request.method() !== 'GET') return route.fulfill({ status: 409, json: { error_code: 'FIXTURE_READ_ONLY', message: 'UI fixture is read only' } });
  const empty = url.searchParams.get('user_id') === 'empty';
  if (url.searchParams.get('user_id') === 'error-case') return route.fulfill({ status: 503, json: { message: '读取暂时失败，请重试' } });
  let data;
  if (path === '/api/auth/me') data = { user_id: 'operator', email: 'operator@example.test', csrf_token: 'fixture-only', capabilities: { admin: true } };
  else if (path === '/api/admin/cost-summary') data = { as_of: now, from: url.searchParams.get('from'), to: url.searchParams.get('to'), account: empty ? zero : summary, platform: empty ? zero : { ...zero, request_count: 5, confirmed_cost_micro_cny: 22400000, provider_cost_micro_cny: 22400000, platform_covered_micro_cny: 22400000, cost_breakdown: { ...zero.cost_breakdown, model: { ...emptyCost, confirmed_cost_micro_cny: 22400000, provider_cost_micro_cny: 22400000, request_count: 5 } } }, tasks: pageOf(empty ? [] : tasks), accounts: empty ? [] : [{ ...summary,user_id:'reader' }], workspaces: empty ? [] : [{ ...summary,user_id:'reader',learning_workspace_id:'probability-course',workspace_goal:'系统复习概率论与随机变量',archived:false }], account_level: empty ? [] : [{ ...summary,user_id:'reader' }] };
  else if (path.endsWith('/usage')) {
    const metrics = { ...(empty ? emptyCost : categories.model), receipt_original_fen: 20000, receipt_correction_fen: -1000, receipt_net_fen: 19000, active_users: empty ? 0 : 12, returning_users: empty ? 0 : 8, completed_runs: empty ? 0 : 46 };
    data = { from: url.searchParams.get('from'),to:url.searchParams.get('to'),today_date:today,as_of:now,summary:metrics,today:metrics,tasks:pageOf(empty?[]:[{...categories.model,user_id:'reader',cost_owner:'account',reference_kind:'run',reference:'条件独立与贝叶斯推理'}]) };
  } else if (path === '/api/admin/users') data = { users: [user,{...user,user_id:'long-reader-account-name-for-layout-review',email:'long.reader.address@example.test',current_allowance:null,disabled:true}],total:2 };
  else if (path === '/api/admin/users/reader') data = user;
  else if (path.endsWith('/mcp')) data = mcp;
  else if (path.endsWith('/allowance-periods')) data = pageOf([period]);
  else if (path.endsWith('/cloud-plan-purchases')) data = { current: {title:'基础月付套餐',tier:'basic',duration:'month',starts_at:period.starts_at,expires_at:period.expires_at,grant_milli_percent:150000,storage_bytes:1024**3},next:null };
  else if (path === '/api/admin/cloud-plans' || path === '/api/admin/allowance-offers') data = { items: [] };
  else if (path === '/api/admin/books') data = { books: [{published_book_ref:book,is_default:true}],total:1 };
  else if (path === '/api/admin/charges') data = pageOf([charge,{...charge,call_id:'model-20261023',entry_kind:'model',model:'deepseek-chat',purpose:'answer',state:'settled',provider_cost_status:'confirmed',provider_cost_micro_cny:3903378,account_debit_micro_cny:3503378}]);
  else if (path.startsWith('/api/admin/charges/')) data = charge;
  else if (path === '/api/admin/workspace-releases') data = pageOf(releases);
  else if (path.startsWith('/api/admin/workspace-releases/')) data = releases.find(r=>path.includes(r.release_id));
  else if (path === '/api/admin/operator-builds') data = pageOf(builds);
  else if (path.startsWith('/api/admin/operator-builds/')) data = builds.find(b=>path.endsWith(b.task_id));
  else if (path === '/api/admin/invites') { const filtered = invites.filter(i=>!url.searchParams.get('state')||i.state===url.searchParams.get('state'));data={invites:filtered,total:filtered.length}; }
  else if (/\/(receipts|receipt-corrections|allowance-adjustments|operations)$/.test(path)) data = pageOf([]);
  else { errors.push(`Unhandled fixture: ${path}`); return route.fulfill({status:404,json:{message:'Fixture missing'}}); }
  await route.fulfill({ json: data });
});
const click = name => page.getByRole('button',{name,exact:true}).click();
async function shot(name,fullPage=false) {
  await page.waitForFunction(()=>![...document.querySelectorAll('main [role=status]')].some(e=>e.textContent.includes('正在读取')));
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await page.screenshot({path:`${output}/${name}.png`,fullPage,animations:'disabled',caret:'hide'});
}
async function fits(label) {
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),`${label}: page overflow`);
  checks.push(label);
}
try {
  await page.goto(`${base}/admin/usage`);
  await page.getByRole('heading',{name:'费用花在哪里'}).waitFor();
  assert.match(await page.getByRole('region',{name:'账号费用',exact:true}).innerText(),/费用未知 1 次/);
  await fits('Desktop usage layout');await shot('usage-desktop');
  await page.getByRole('region',{name:'账号费用',exact:true}).screenshot({path:`${output}/cost-overview-panel.png`,animations:'disabled'});
  await Promise.all([page.waitForResponse(r=>r.url().includes('/cost-summary?')&&r.url().includes('from=')),click('近 7 天')]);
  const costUrl = new URL(requests.filter(r=>r.includes('/cost-summary?')).at(-1).slice(4),base);
  assert.equal(Date.parse(costUrl.searchParams.get('to'))-Date.parse(costUrl.searchParams.get('from')),6*86400000);
  await page.getByRole('button',{name:'查看归集明细',exact:true}).first().click();
  await page.getByRole('region',{name:'归集调用明细'}).getByText('read-web-20261023',{exact:true}).waitFor();
  assert.ok(requests.some(r=>r.includes('parent_task_ref=')));
  await click('关闭归集明细');await click('工作区');await click('查看工作区明细');
  assert.ok(requests.some(r=>r.includes('learning_workspace_id=probability-course')));
  await click('关闭归集明细');await click('账号级整理');await click('查看整理明细');
  assert.ok(requests.some(r=>r.includes('account_level=true')));
  await click('使用与收款');await shot('usage-activity');
  await click('任务账单');await click('调用明细');await page.getByRole('region',{name:'任务调用明细'}).waitFor();
  assert.ok(requests.some(r=>r.includes('run_ref=')));
  checks.push('Date presets, view switches, and drilldown request filters');
  await page.goto(`${base}/admin/users`);await page.getByRole('link',{name:'reader',exact:true}).waitFor();await fits('Users list');await shot('users-desktop');
  await page.getByRole('link',{name:'reader',exact:true}).click();await page.getByRole('heading',{name:'使用与回访',exact:true}).waitFor();await shot('account-overview');
  assert.equal(await page.getByRole('heading',{name:'MCP 调用次数',exact:true}).count(),0);
  await click('额度与套餐');await page.getByRole('heading',{name:'AI 使用额度',exact:true}).waitFor();await shot('account-allowance');
  assert.match(await page.locator('main').innerText(),/130%/);
  await click('赠送 / 调整');await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');
  await click('MCP 调用包');await page.getByRole('heading',{name:'MCP 调用次数',exact:true}).waitFor();await shot('account-mcp');
  await page.locator('summary').filter({hasText:'已购调用包明细'}).click();await click('更正 MCP 收款 / 次数');await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');
  await click('材料授权');await click('授权材料');await page.getByRole('dialog').waitFor();await page.keyboard.press('Escape');
  await click('费用与账单');await page.getByRole('heading',{name:'服务费用与待核算'}).waitFor();await page.getByRole('button',{name:'查看 / 核算',exact:true}).first().click();await page.getByRole('dialog').waitFor();await shot('charge-detail');await page.keyboard.press('Escape');
  await click('用量分析');await page.getByRole('heading',{name:'账号用量汇总'}).waitFor();checks.push('All six account sections and management dialogs');
  await page.goto(`${base}/admin/operator-builds`);await click('需要操作');assert.equal(await page.locator('tbody tr').count(),2);await click('本页全部');await page.getByRole('button',{name:'查看构建任务',exact:true}).first().click();await page.getByRole('region',{name:'运营构建任务详情'}).waitFor();await shot('build-budget');
  assert.equal(await page.locator('progress').getAttribute('value'),'3');
  await click('查看费用回执');await page.getByRole('region',{name:'构建费用回执'}).waitFor();checks.push('Build filters, actual progress and budget receipts');
  await page.goto(`${base}/admin/workspace-releases`);await click('尚未就绪');assert.equal(await page.locator('tbody tr').count(),1);await click('查看版本');await page.getByRole('region',{name:'共享工作区版本详情'}).waitFor();await shot('release-readiness');checks.push('Workspace readiness and focus on missing capabilities');
  await page.goto(`${base}/admin/invites`);await page.getByRole('heading',{name:'全部内测码'}).waitFor();await Promise.all([page.waitForResponse(r=>r.url().includes('state=unused')),page.getByLabel('内测码状态').selectOption('unused')]);await shot('invites-desktop');checks.push('Invite status filtering');
  await page.setViewportSize({width:390,height:844});
  for (const path of ['usage','users','users/reader','operator-builds','workspace-releases','invites']) {
    await page.goto(`${base}/admin/${path}`);await page.locator('main h1').waitFor();
    await page.waitForFunction(()=>![...document.querySelectorAll('main [role=status]')].some(e=>e.textContent.includes('正在读取')));
    await fits(`Mobile ${path}`);await shot(`mobile-${path.replaceAll('/','-')}`);
  }
  await page.goto(`${base}/admin/users/reader`);await click('额度与套餐');await page.getByRole('heading',{name:'AI 使用额度'}).waitFor();await fits('Mobile allowance');await shot('mobile-allowance');
  await click('MCP 调用包');await page.getByRole('heading',{name:'MCP 调用次数'}).waitFor();await page.locator('summary').filter({hasText:'已购调用包明细'}).click();await fits('Mobile MCP expanded');
  await page.goto(`${base}/admin/usage`);await page.getByRole('heading',{name:'费用花在哪里'}).waitFor();
  await page.getByLabel('账号（留空为全部）').fill('empty');await click('查询期间');await page.getByText('本期暂无调用，发生费用后会显示构成。',{exact:true}).first().waitFor();await fits('Empty costs');await shot('mobile-empty');
  await page.getByLabel('账号（留空为全部）').fill('error-case');await click('查询期间');await page.getByRole('button',{name:'重新读取',exact:true}).waitFor();checks.push('Empty data and fetch failure states');
  await page.setViewportSize({width:1440,height:1000});await page.goto(`${base}/admin/usage`);await page.getByRole('heading',{name:'费用花在哪里'}).waitFor();
  await click('切换主题');await page.waitForFunction(()=>document.documentElement.classList.contains('light'));await shot('usage-light');await fits('Light theme and switch from system dark');
  assert.deepEqual(errors,[]);assert.equal(requests.filter(r=>!r.startsWith('GET ')).length,0);
  await fs.writeFile(`${output}/results.json`,JSON.stringify({checks,errors,requests},null,2));
  console.log(JSON.stringify({checks:checks.length,errors,output},null,2));
} finally { await browser.close(); }
