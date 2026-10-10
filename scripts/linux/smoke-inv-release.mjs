// Real isolated candidate + real inbox. No API interception and no model calls.
// INV_CONFIG points to a private JSON file; inbox responses are supplied as files
// in its private_dir: registration-code.txt, reset-link.txt, binding-code.txt.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { randomUUID, randomBytes } from 'node:crypto';

const config = JSON.parse(await fs.readFile(process.env.INV_CONFIG, 'utf8'));
const { base, binding_base: bindingBase, email, admin, private_dir: privateDir, evidence: output } = config;
assert([config.proxy, config.binding_proxy].every(value => new URL(value).hostname === '127.0.0.1'), 'Use isolated loopback candidate proxies');
await fs.mkdir(output, { recursive: true });
await fs.mkdir(privateDir, { recursive: true });
const browser = await chromium.launch({ headless: true });
const errors = [], checks = config.resume_registered ? JSON.parse(await fs.readFile(path.join(output, 'progress.json'), 'utf8')).checks : [];
let phase = 'deployment';
async function context(proxy = config.proxy) {
  const result = await browser.newContext({ proxy: { server: proxy }, ignoreHTTPSErrors: true, viewport: { width: 1440, height: 960 } });
  result.on('page', page => { page.setDefaultTimeout(30000); page.on('pageerror', error => errors.push({ phase, message: error.message })); });
  return result;
}
const adminContext = await context(), readerContext = await context();
const adminPage = await adminContext.newPage(), page = await readerContext.newPage();
const credentials = config.resume_registered ? JSON.parse(await fs.readFile(path.join(privateDir, 'run-credentials.json'), 'utf8'))
  : { password: randomBytes(24).toString('base64url'), changed: randomBytes(24).toString('base64url'), reset: randomBytes(24).toString('base64url') };
const { password, changed, reset } = credentials;
const noteText = 'INV9 candidate private record ' + randomUUID();
await fs.writeFile(path.join(privateDir, 'run-credentials.json'), JSON.stringify({ email, password, changed, reset }), { mode: 0o600 });
async function api(ctx, origin, route, body, status = 200) {
  const headers = { Origin: origin };
  if (body !== undefined && !route.startsWith('/auth/')) {
    const me = await ctx.request.get(origin + '/api/auth/me');
    assert.equal(me.status(), 200, 'Authenticated identity required');
    headers['X-CSRF-Token'] = (await me.json()).csrf_token;
  }
  const response = body === undefined ? await ctx.request.get(origin + '/api' + route)
    : await ctx.request.post(origin + '/api' + route, { headers, data: body });
  const json = await response.json();
  assert.equal(response.status(), status, `${route}: ${json.error_code ?? 'unexpected response'}`);
  return json;
}
async function inbox(name) {
  console.log('WAITING_FOR_' + name.toUpperCase().replaceAll('-', '_'));
  const file = path.join(privateDir, name + '.txt');
  const deadline = Date.now() + 14 * 60_000;
  while (Date.now() < deadline) {
    try { const value = (await fs.readFile(file, 'utf8')).trim(); if (value) { await fs.unlink(file); return value; } }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  throw new Error('Inbox response not received within 14 minutes: ' + name);
}
async function login(target, username, pass) {
  await target.getByLabel('邮箱或账号', { exact: true }).fill(username);
  await target.getByLabel('密码', { exact: true }).fill(pass);
  await target.getByRole('button', { name: '登录', exact: true }).click();
}
async function accountSettings(target) {
  const button = target.getByRole('button', { name: '个人设置', exact: true });
  if (!await button.isVisible()) await target.locator('.network-account summary').click();
  await button.click();
}
async function workspace(ctx, origin, publication) {
  const attachment = randomUUID();
  const result = await api(ctx, origin, '/workspaces', { published_book_ref: publication, attachment_id: attachment });
  return { result, attachment };
}
async function workspaceAction(ctx, origin, w, action, body) {
  const latest = await api(ctx, origin, `/workspaces/${w.result.workspace_id}`);
  return api(ctx, origin, `/workspaces/${latest.workspace_id}/${action}`, {
    ...body, attachment_id: w.attachment, generation: latest.generation, expected_revision: latest.revision,
  });
}
async function passed(message) {
  checks.push(message); console.log('PASS: ' + message);
  await fs.writeFile(path.join(output, 'progress.json'), JSON.stringify({ phase, checks, errors }, null, 2));
}
try {
  let identity, registrationSentAt = 0;
  assert.equal((await adminContext.request.get(base + '/admin', { maxRedirects: 0 })).status(), 308);
  assert.equal((await adminContext.request.get(base + '/admin/assets/inv9-missing.js')).status(), 404);
  await adminPage.goto(base + '/admin/invites');
  await login(adminPage, admin.username, admin.password);
  await adminPage.getByRole('heading', { name: '内测码', exact: true }).waitFor();
  await adminPage.reload();
  await adminPage.getByRole('heading', { name: '内测码', exact: true }).waitFor();
  for (const src of await adminPage.locator('script[src]').evaluateAll(nodes => nodes.map(node => node.getAttribute('src')))) {
    assert.match((await adminContext.request.get(new URL(src, base).href)).headers()['content-type'], /javascript/);
  }
  if (!config.resume_registered) {
  await adminContext.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: base });
  const batchResponse = adminPage.waitForResponse(response => response.url().endsWith('/api/admin/invite-batches') && response.request().method() === 'POST');
  await adminPage.getByRole('button', { name: '生成内测码', exact: true }).click();
  const batch = await (await batchResponse).json();
  assert.equal(batch.invites.length, 1);
  await adminPage.getByRole('button', { name: '复制本批未使用码', exact: true }).click();
  const invite = await adminPage.evaluate(() => navigator.clipboard.readText());
  assert.equal(invite.replaceAll('-', ''), batch.invites[0].code);
  await passed('Nginx Admin deep link, refresh, resource MIME/404, real invite generation and clipboard');

  phase = 'registration';
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(base + '/?account=register');
  await page.getByLabel('邮箱', { exact: true }).fill(email);
  await page.getByLabel('密码', { exact: true }).fill(password);
  await page.getByLabel('确认密码', { exact: true }).fill(password);
  await page.getByLabel('内测码', { exact: true }).fill(invite);
  const registration = page.waitForResponse(response => response.url().endsWith('/api/auth/register/start'));
  await page.getByRole('button', { name: '发送注册验证码' }).click();
  const started = await registration;
  assert.equal(started.status(), 200, 'Registration mail: ' + (await started.json()).error_code);
  registrationSentAt = Date.now();
  const registrationCode = await inbox('registration-code');
  await page.getByLabel('邮箱验证码').fill(registrationCode);
  await page.getByRole('button', { name: '完成注册', exact: true }).click();
  await page.getByText('注册成功，请登录。材料与使用额度由管理员开通。', { exact: true }).waitFor();
  await login(page, email, password);
  await page.getByText('账号已创建，等待管理员开通材料。', { exact: true }).waitFor();
  identity = await api(readerContext, base, '/auth/me');
  assert.equal(identity.email, email); assert.equal(identity.capabilities.admin, false);
  assert.deepEqual((await api(readerContext, base, '/library')).books, []);
  assert.equal((await api(readerContext, base, '/account/allowance')).current_allowance, null);
  // The unavailable code is rejected before the different address can receive mail.
  const rejected = await api(readerContext, base, '/auth/register/start', { email: 'inv9-second@example.com', password, invite_code: invite }, 409);
  assert.equal(rejected.error_code, 'INVITE_UNAVAILABLE');
  await page.screenshot({ path: path.join(output, 'registered-empty-library.png'), fullPage: true });
  await passed('Real registration inbox verified; reused invite rejected; new account has no books, allowance or admin role');
  } else {
    await page.goto(base); await login(page, email, password);
    await page.getByRole('heading', { name: '选择阅读材料' }).waitFor();
    identity = await api(readerContext, base, '/auth/me');
  }

  phase = 'grant-and-read';
  const books = (await api(adminContext, base, '/admin/books?limit=100')).books;
  const book = books.find(book => book.is_default && book.published_book_ref.book_id === 'quickstart-demo');
  assert(book, 'Candidate must contain quickstart-demo');
  const publication = book.published_book_ref;
  const owner = identity.user_id;
  if (!config.resume_registered) {
  await api(adminContext, base, `/admin/users/${owner}/book-grants`, { operation_id: randomUUID(), published_book_ref: publication });
  const now = Math.floor(Date.now() / 1000);
  const period = (await api(adminContext, base, `/admin/users/${owner}/allowance-periods`, { operation_id: randomUUID(), starts_at: now - 60, expires_at: now + 86400 })).allowance_period;
  await api(adminContext, base, `/admin/users/${owner}/allowance-adjustments`, { operation_id: randomUUID(), period_id: period.period_id, revision: 0, delta_micro_cny: 1000000, reason: 'INV9 isolated acceptance' });
  }
  const granted = await api(readerContext, base, '/account/allowance');
  assert(granted.current_allowance);
  await page.reload();
  await page.getByRole('button', { name: 'quickstart-demo', exact: true }).click();
  await page.locator('.workspace-shell').waitFor();
  await page.setViewportSize({ width: 1440, height: 960 });
  const w = await workspace(readerContext, base, publication);
  const fingerprint = await api(readerContext, base, `/books/${publication.book_id}/publications/${publication.publication_id}/source_fingerprint`);
  const lid = w.result.reader.viewport?.anchor_lid ?? '1.1';
  const saved = await workspaceAction(readerContext, base, w, 'memory/save', { type: 'note', content: noteText,
    note_placement: { kind: 'lid_block', lid, source_fingerprint: fingerprint.source_fingerprint } });
  const note = saved.result?.record ?? saved.result;
  assert(note?.mem_id, 'Private note must be committed');
  await page.screenshot({ path: path.join(output, 'reader-desktop.png'), fullPage: true });
  await passed('Admin explicitly granted a book and allowance; real Reader opened; private note committed without model calls');

  phase = 'change-password';
  const other = await readerContext.newPage(), sameIdentityAdmin = await readerContext.newPage();
  await other.goto(base); await sameIdentityAdmin.goto(base + '/admin/');
  await sameIdentityAdmin.getByRole('heading', { name: '无管理权限', exact: true }).waitFor();
  await accountSettings(page);
  await page.getByRole('button', { name: '修改密码', exact: true }).click();
  await page.getByLabel('当前密码', { exact: true }).fill(password);
  await page.getByLabel('新密码', { exact: true }).fill(changed);
  await page.getByLabel('确认新密码', { exact: true }).fill(changed);
  await page.getByRole('button', { name: '保存新密码' }).click();
  await page.getByText('密码已更新，请使用新密码重新登录。', { exact: true }).waitFor();
  await other.getByLabel('邮箱或账号', { exact: true }).waitFor();
  await sameIdentityAdmin.getByLabel('邮箱或账号', { exact: true }).waitFor();
  await api(readerContext, base, '/auth/login', { username: email, password }, 401);
  await login(page, email, changed);
  await page.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  await passed('Personal password change revoked old password and all Reader/Admin sessions');

  phase = 'forgot-password';
  const cooldown = Math.max(0, registrationSentAt + 61_000 - Date.now());
  if (cooldown) await new Promise(resolve => setTimeout(resolve, cooldown));
  await page.goto(base + '/?account=forgot-password');
  await page.getByLabel('邮箱', { exact: true }).fill(email);
  const forgot = page.waitForResponse(response => response.url().endsWith('/api/auth/password/forgot'));
  await page.getByRole('button', { name: '发送重置邮件' }).click();
  assert.equal((await forgot).status(), 200);
  const link = new URL(await inbox('reset-link'));
  assert.equal(link.origin, base); assert.equal(link.searchParams.get('account'), 'reset-password');
  await page.goto(link.href);
  await page.getByRole('heading', { name: '设置新密码' }).waitFor();
  assert.equal(new URL(page.url()).hash, '');
  await page.getByLabel('新密码', { exact: true }).fill(reset);
  await page.getByLabel('确认新密码', { exact: true }).fill(reset);
  await page.getByRole('button', { name: '重置密码', exact: true }).click();
  await page.getByText('密码已更新，请使用新密码重新登录。', { exact: true }).waitFor();
  await login(page, email, reset);
  await page.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  assert.equal((await api(readerContext, base, '/auth/me')).user_id, owner);
  assert.deepEqual(await api(readerContext, base, '/account/allowance'), granted);
  const restored = await workspace(readerContext, base, publication);
  const recalled = await workspaceAction(readerContext, base, restored, 'memory/recall', { type: 'note' });
  assert(recalled.result.some(record => record.mem_id === note.mem_id && record.content === noteText));
  await page.getByRole('button', { name: 'quickstart-demo', exact: true }).click();
  await page.locator('.workspace-shell').waitFor();
  await passed('Real inbox reset link opened through Nginx; token cleared; same account, allowance and private note survived');

  phase = 'legacy-binding';
  const legacyContext = await context(config.binding_proxy), legacy = await legacyContext.newPage();
  await legacy.goto(bindingBase); await login(legacy, admin.username, admin.password);
  await legacy.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  const beforeIdentity = await api(legacyContext, bindingBase, '/auth/me');
  const beforeBooks = await api(legacyContext, bindingBase, '/library');
  const beforeAllowance = await api(legacyContext, bindingBase, '/account/allowance');
  const oldPublication = beforeBooks.books.find(book => book.published_book_ref.book_id === 'quickstart-demo').published_book_ref;
  const oldWorkspace = await workspace(legacyContext, bindingBase, oldPublication);
  const oldRecords = await workspaceAction(legacyContext, bindingBase, oldWorkspace, 'memory/recall', {});
  assert.equal(beforeIdentity.email, null);
  await accountSettings(legacy);
  await legacy.getByLabel('邮箱', { exact: true }).fill(email);
  await legacy.getByLabel('当前密码', { exact: true }).fill(admin.password);
  const binding = legacy.waitForResponse(response => response.url().endsWith('/api/account/email/start'));
  await legacy.getByRole('button', { name: '发送验证码', exact: true }).click();
  assert.equal((await binding).status(), 200);
  await legacy.getByLabel('邮箱验证码').fill(await inbox('binding-code'));
  await legacy.getByRole('button', { name: '确认绑定', exact: true }).click();
  await legacy.getByText('邮箱已绑定，可以使用此邮箱或原账号登录。', { exact: true }).waitFor();
  const afterIdentity = await api(legacyContext, bindingBase, '/auth/me');
  assert.equal(afterIdentity.user_id, beforeIdentity.user_id); assert.equal(afterIdentity.email, email);
  assert.equal(afterIdentity.capabilities.admin, true);
  assert.deepEqual(await api(legacyContext, bindingBase, '/library'), beforeBooks);
  assert.deepEqual(await api(legacyContext, bindingBase, '/account/allowance'), beforeAllowance);
  assert.deepEqual((await workspaceAction(legacyContext, bindingBase, oldWorkspace, 'memory/recall', {})).result, oldRecords.result);
  await legacy.getByRole('button', { name: '关闭', exact: true }).click();
  await legacy.getByRole('button', { name: '退出登录', exact: true }).click();
  await login(legacy, email, admin.password);
  await legacy.getByRole('heading', { name: '选择阅读材料' }).waitFor();
  assert.equal((await api(legacyContext, bindingBase, '/auth/me')).user_id, beforeIdentity.user_id);
  await passed('Legacy account received binding code and retained user ID, admin role, books, allowance and private records; email login succeeded');
  assert.deepEqual(errors, []);
  await fs.writeFile(path.join(output, 'result.json'), JSON.stringify({ status: 'passed', checks, errors, model_requests: 0, real_inbox: true }, null, 2));
  // Keep the candidate login private for follow-up acceptance; never include it in evidence.
  await fs.writeFile(path.join(privateDir, 'candidate-reader.json'), JSON.stringify({ username: email, password: reset, user_id: owner }), { mode: 0o600 });
} catch (error) {
  await fs.writeFile(path.join(output, 'failure.json'), JSON.stringify({ phase, checks, errors, message: error.message }, null, 2));
  throw error;
} finally { await browser.close(); }
