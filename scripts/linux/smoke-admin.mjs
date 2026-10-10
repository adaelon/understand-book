// ADM7 isolated candidate only. Uses the opt-in adm7_candidate_host fixture, never a Provider.
import { chromium } from "playwright";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
const base = process.env.ADM7_URL ?? "https://localhost:18443";
const output = process.env.ADM7_EVIDENCE ?? "tmp/adm7-candidate";
const fixture = JSON.parse(await fs.readFile(`${output}/info.json`, "utf8"));
await fs.mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true });
const page = await context.newPage();
page.setDefaultTimeout(15000);
const errors = [],
  checks = [];
let phase = "routing";
page.on("pageerror", (error) => errors.push(String(error)));
const passed = (text) => {
  checks.push(text);
  console.log(text);
};
async function login(target, user = "B", password = "fixture-only-password") {
  await target.getByLabel("邮箱或账号", { exact: true }).fill(user);
  await target.getByLabel("密码", { exact: true }).fill(password);
  await target.getByRole("button", { name: "登录", exact: true }).click();
}
async function dialog(label, fill) {
  await page.getByRole("button", { name: label, exact: true }).first().click();
  const modal = page.getByRole("dialog");
  await modal.waitFor();
  if (fill) await fill(modal);
  await modal.getByRole("button", { name: "确认提交" }).click();
  await modal.waitFor({ state: "hidden" });
  await page.getByRole("status").filter({ hasText: "操作已完成" }).waitFor();
}
async function get(path, target = page) {
  const response = await target.request.get(base + path);
  assert.equal(response.status(), 200, path);
  return response.json();
}
try {
  const redirect = await context.request.get(`${base}/admin`, {
    maxRedirects: 0,
  });
  assert.equal(redirect.status(), 308);
  assert.match(redirect.headers().location, /\/admin\/$/);
  const html = await (
    await context.request.get(`${base}/admin/users/A`)
  ).text();
  assert.match(html, /运营后台/);
  for (const resource of html.matchAll(
    /(?:src|href)="(\/admin\/assets\/[^\"]+)"/g,
  )) {
    const response = await context.request.get(base + resource[1]);
    assert.equal(response.status(), 200);
    assert.match(
      response.headers()["content-type"],
      resource[1].endsWith(".css") ? /text\/css/ : /javascript/,
    );
  }
  assert.equal(
    (await context.request.get(`${base}/admin/assets/missing.js`)).status(),
    404,
  );
  assert.equal(
    (await context.request.get(`${base}/admin/missing.css`)).status(),
    404,
  );
  assert.equal(
    (await context.request.get(`${base}/api/admin/users`)).status(),
    401,
  );
  assert.match(
    await (
      await context.request.get(`${base}/admin/LICENSE.shadcn-admin.txt`)
    ).text(),
    /MIT License/,
  );
  passed(
    "Nginx redirect, Admin deep link, MIME, missing asset 404, MIT and API status",
  );
  phase = "login";
  await page.goto(`${base}/admin/users/A`);
  await login(page, "B", "wrong-password");
  await page.getByRole("alert").filter({ hasText: "登录失败" }).waitFor();
  await login(page);
  await page.getByRole("heading", { name: "A", exact: true }).waitFor();
  await page.reload();
  await page.getByRole("heading", { name: "A", exact: true }).waitFor();
  passed(
    "Real Rust login rejects bad password and restores protected deep link",
  );
  phase = "csrf";
  const denied = await page.request.post(`${base}/api/admin/users`, {
    headers: { Origin: base },
    data: {
      user_id: "must-not-exist",
      password: "fixture-only-password",
      operation_id: "csrf-denied",
    },
  });
  assert.equal(denied.status(), 403);
  assert.equal((await denied.json()).error_code, "CSRF_REJECTED");
  assert.equal((await get("/api/admin/users?search=must-not-exist")).total, 0);
  passed("Missing CSRF is refused without creating an account");
  phase = "create";
  await page.goto(`${base}/admin/users`);
  const owner = `adm7-${Date.now()}`;
  await dialog("创建账号", async (modal) => {
    await modal.getByLabel("新账号", { exact: true }).fill(owner);
    await modal.getByLabel("初始密码").fill("fixture-only-password");
  });
  await page.getByRole("link", { name: owner, exact: true }).click();
  await page.getByRole("heading", { name: owner, exact: true }).waitFor();
  await dialog("授权材料", async (modal) => {
    await modal
      .getByLabel("发布材料")
      .selectOption({ value: JSON.stringify(fixture.book) });
  });
  let detail = await get(`/api/admin/users/${owner}`);
  assert.equal(detail.book_grants.total, 1);
  const readerContext = await browser.newContext({ ignoreHTTPSErrors: true });
  const reader = await readerContext.newPage();
  await reader.goto(base);
  await login(reader, owner);
  await reader.getByRole("heading", { name: "选择阅读材料" }).waitFor();
  await reader
    .getByRole("button", { name: fixture.book.book_id, exact: true })
    .waitFor();
  assert.equal(await reader.getByRole("link", { name: "运营后台" }).count(), 0);
  assert.equal(
    (await reader.request.get(`${base}/api/admin/users`)).status(),
    403,
  );
  const forbidden = await readerContext.newPage();
  await forbidden.goto(`${base}/admin/`);
  await forbidden.getByRole("heading", { name: "无管理权限" }).waitFor();
  passed(
    "Online create/grant reaches Reader; normal reader UI and API reject admin access",
  );
  phase = "allowance";
  await dialog("新建额度期");
  await dialog("赠送 / 调整", async (modal) => {
    await modal.getByLabel("额度调整（元）").fill("10.000001");
    await modal.getByLabel("原因", { exact: true }).fill("测试赠送");
  });
  detail = await get(`/api/admin/users/${owner}`);
  assert.equal(detail.current_allowance.balance.available_micro_cny, 10000001);
  await dialog("编辑有效期");
  phase = "lost receipt";
  let posts = 0;
  await page.route("**/api/admin/operations/**", (route) => route.abort());
  await page.route(`**/api/admin/users/${owner}/receipts`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts++;
    const response = await route.fetch();
    assert.equal(response.status(), 200);
    await route.abort();
  });
  await page
    .getByRole("button", { name: "登记收款", exact: true })
    .first()
    .click();
  const receiptDialog = page.getByRole("dialog");
  await receiptDialog
    .getByLabel("实际收款（元）", { exact: true })
    .fill("1.23");
  await receiptDialog.getByLabel("授予 AI 使用额度（元）").fill("2.000001");
  await receiptDialog.getByLabel("收款备注").fill("测试收款");
  await receiptDialog.getByRole("button", { name: "确认提交" }).click();
  await receiptDialog.waitFor({ state: "hidden" });
  await page.getByLabel("待核对操作").waitFor();
  const originalOperation = await page.getByLabel("待核对操作").textContent();
  await page.reload();
  await page.getByLabel("待核对操作").waitFor();
  assert.equal(
    await page.getByLabel("待核对操作").textContent(),
    originalOperation,
  );
  await page.unroute("**/api/admin/operations/**");
  await page.getByRole("button", { name: "按原操作重试" }).click();
  await page.getByLabel("待核对操作").waitFor({ state: "hidden" });
  assert.equal(posts, 1);
  await page.unroute(`**/api/admin/users/${owner}/receipts`);
  detail = await get(`/api/admin/users/${owner}`);
  assert.equal(detail.current_allowance.balance.available_micro_cny, 12000002);
  assert.equal((await get(`/api/admin/users/${owner}/receipts`)).total, 1);
  passed(
    "Exact money, validity, real lost receipt + reload + retry produces one payment and grant",
  );
  phase = "correction";
  await dialog("追加更正", async (modal) => {
    await modal.getByLabel("收款差额（元）").fill("-0.23");
    await modal.getByLabel("额度差额（元）").fill("-1");
    await modal.getByLabel("更正原因").fill("测试追加更正");
  });
  assert.equal(
    (await get(`/api/admin/users/${owner}/receipts`)).items[0]
      .effective_amount_fen,
    100,
  );
  assert.equal(
    (await get(`/api/admin/users/${owner}`)).current_allowance.balance
      .available_micro_cny,
    11000002,
  );
  passed(
    "Receipt correction keeps original payment and applies an explicit allowance delta",
  );
  phase = "account lifecycle";
  await dialog("改密", async (modal) => {
    await modal
      .getByLabel("新密码", { exact: true })
      .fill("replacement-test-password");
  });
  assert.equal((await reader.request.get(`${base}/api/auth/me`)).status(), 401);
  await reader.goto(base);
  await login(reader, owner, "replacement-test-password");
  await reader.getByRole("heading", { name: "选择阅读材料" }).waitFor();
  await dialog("撤销会话");
  assert.equal((await reader.request.get(`${base}/api/auth/me`)).status(), 401);
  await dialog("停用");
  assert.equal((await get(`/api/admin/users/${owner}`)).disabled, true);
  await dialog("启用");
  assert.equal((await get(`/api/admin/users/${owner}`)).disabled, false);
  await dialog("撤销授权");
  assert.equal((await get(`/api/admin/users/${owner}`)).book_grants.total, 0);
  passed(
    "Password, revoke sessions, disable/enable and revoke material use real service state",
  );
  await page.screenshot({
    path: `${output}/account-desktop.png`,
    fullPage: true,
  });
  phase = "reconciliation";
  await page.goto(`${base}/admin/users/A`);
  await page.getByRole("button", { name: "查看 / 核算" }).click();
  await page
    .getByRole("button", { name: "人工核算 / 追加更正", exact: true })
    .click();
  const reconcile = page.getByRole("dialog");
  await reconcile.getByLabel("账号免扣（释放占用）").check();
  await reconcile.getByLabel("处理原因").fill("释放测试占用");
  await reconcile.getByLabel("核算依据").fill("本地测试调用，没有供应商账单");
  await reconcile.getByRole("button", { name: "确认提交" }).click();
  await page.getByRole("status").filter({ hasText: "操作已完成" }).waitFor();
  const charge = await get("/api/admin/charges/adm7-pending");
  assert.equal(charge.account_debit_micro_cny, 0);
  assert.equal(charge.provider_cost_micro_cny, null);
  assert.equal(charge.state, "released");
  passed(
    "Pending charge can be waived without fabricating a zero supplier cost",
  );
  phase = "reader preservation";
  const originalReader = await context.newPage();
  await originalReader.goto(base);
  await originalReader
    .getByRole("button", { name: fixture.book.book_id, exact: true })
    .click();
  const draft = originalReader.locator("textarea").first();
  await draft.fill("保留的未发送草稿");
  await originalReader.getByText("账号", { exact: true }).click();
  const popupPromise = originalReader.waitForEvent("popup");
  await originalReader.getByRole("link", { name: "运营后台" }).click();
  const popup = await popupPromise;
  await popup.getByRole("heading", { name: "账号管理", exact: true }).waitFor();
  assert.equal(await draft.inputValue(), "保留的未发送草稿");
  assert.equal(new URL(originalReader.url()).pathname, "/");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({
    path: `${output}/account-mobile.png`,
    fullPage: true,
  });
  assert(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth + 1,
    ),
  );
  passed(
    "Reader opens Admin in a separate tab preserving draft; mobile page stays within viewport",
  );
  phase = "cross tab logout";
  await popup.getByRole("button", { name: "退出登录", exact: true }).click();
  await page.getByRole("heading", { name: "登录运营后台" }).waitFor();
  await originalReader.getByRole("heading", { name: "回到你的阅读" }).waitFor();
  assert.equal(await page.getByText("10.000001", { exact: true }).count(), 0);
  passed("Admin logout clears the other Admin and Reader tabs");
  assert.deepEqual(errors, []);
  await readerContext.close();
  await fs.writeFile(
    `${output}/result.json`,
    JSON.stringify({ checks, errors, status: "passed" }, null, 2),
  );
} catch (error) {
  await page.screenshot({ path: `${output}/failure.png`, fullPage: true });
  await fs.writeFile(
    `${output}/result.json`,
    JSON.stringify(
      { checks, errors, status: "failed", phase, message: String(error) },
      null,
      2,
    ),
  );
  throw error;
} finally {
  await browser.close();
}
