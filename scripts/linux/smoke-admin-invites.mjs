// INV2 UI acceptance with an in-memory HTTP contract fixture; Rust inv2_ tests cover real SQLite/Host.
import { chromium } from "playwright";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
const base = process.env.INV2_UI_URL ?? "http://127.0.0.1:18942";
const output = process.env.INV2_EVIDENCE ?? "tmp/inv2-browser";
await fs.mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"], viewport: { width: 390, height: 844 } });
const page = await context.newPage();
page.setDefaultTimeout(25000);
const checks = [], errors = [], batches = new Map();
let posts = 0, lookupOffline = true, mode = "timeout", missingId, disablePosts = 0;
const now = 1791520000;
const invite = (id, state = "unused") => ({ invite_id: id, code: "ABCDE23456FGHJK789AB", actor: "B", operation_id: "seed", state, created_at: now,
  used_by: state === "used" ? "reader-A" : null, used_email: state === "used" ? "reader@example.com" : null, used_at: state === "used" ? now : null, disabled_at: state === "disabled" ? now : null });
const items = [invite("seed-used", "used"), invite("seed-disabled", "disabled")];
page.on("pageerror", error => errors.push(String(error)));
await page.route("**/api/**", async route => {
  const request = route.request(), url = new URL(request.url()), path = url.pathname;
  const reply = (value, status = 200) => route.fulfill({status, contentType: "application/json", body: JSON.stringify(value)});
  if (path === "/api/auth/me") return reply({user_id: "B", csrf_token: "fixture", capabilities: {admin: true}});
  if (path === "/api/admin/invites") {
    const state = url.searchParams.get("state"), selected = items.filter(i => !state || i.state === state), offset = Number(url.searchParams.get("offset"));
    return reply({invites: selected.slice(offset, offset + 20), total: selected.length});
  }
  if (path === "/api/admin/invite-batches" && request.method() === "POST") {
    posts++;
    const body = request.postDataJSON();
    assert.equal(request.headers()["x-csrf-token"], "fixture");
    if (mode === "missing") { missingId = body.operation_id; mode = "retry"; return route.abort(); }
    if (mode === "retry") assert.equal(body.operation_id, missingId);
    let batch = batches.get(body.operation_id);
    if (!batch) {
      const generated = Array.from({length: body.count}, (_, i) => ({...invite(`${body.operation_id}-${i}`), operation_id: body.operation_id}));
      batch = {...body, actor: "B", created_at: now, invites: generated};
      batches.set(body.operation_id,batch); items.unshift(...generated);
    }
    if (mode === "timeout") {
      mode = "normal";
      // Let the page's actual 15s AbortSignal timeout expire after the commit.
      await new Promise(resolve => setTimeout(resolve, 16000));
      return route.abort().catch(() => {});
    }
    return reply(batch);
  }
  if (path.startsWith("/api/admin/invite-batches/")) {
    if (lookupOffline) return reply({error_code: "UNAVAILABLE", message: "fixture lookup offline"},503);
    const batch = batches.get(path.split("/").at(-1));
    return batch ? reply(batch) : reply({error_code: "NOT_FOUND"},404);
  }
  if (path.endsWith("/disable")) {
    disablePosts++;
    const target = items.find(i => i.invite_id === path.split("/").at(-2));
    assert(target);
    target.state = "disabled"; target.disabled_at = now;
    if (disablePosts === 1) return route.abort();
    return reply(target);
  }
  throw new Error(`Unexpected API request: ${path}`);
});
try {
  await page.goto(`${base}/admin/invites`);
  await page.getByRole("heading",{name:"内测码",exact:true}).waitFor();
  await page.getByLabel("生成数量（1–100）").fill("2");
  await page.getByRole("button",{name:"生成内测码",exact:true}).click();
  await page.getByRole("button",{name:"核对原批次",exact:true}).waitFor({state:"visible"});
  await page.waitForFunction(() => !document.querySelector('button[type="submit"]') || document.querySelector('[aria-label="待核对批次"] button')?.disabled === false);
  const original = await page.getByLabel("待核对批次").textContent();
  await page.reload();
  await page.getByLabel("待核对批次").waitFor();
  assert.equal(await page.getByLabel("待核对批次").textContent(),original);
  lookupOffline = false;
  await page.getByRole("button",{name:"按原操作重试"}).click();
  await page.getByLabel("本次批次").waitFor();
  await page.getByLabel("待核对批次").waitFor({state:"hidden"});
  assert.equal(posts,1); assert.equal(batches.size,1); assert.equal(items.length,4);
  checks.push("Actual 15s timeout after simulated commit, reload and lookup recover the original batch with one POST");
  await page.getByRole("button",{name:"复制本批未使用码"}).click();
  assert.deepEqual((await page.evaluate(() => navigator.clipboard.readText())).split(/\r?\n/),["ABCDE-23456-FGHJK-789AB","ABCDE-23456-FGHJK-789AB"]);
  await page.getByLabel("本次批次").getByRole("button",{name:"复制",exact:true}).first().click();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()),"ABCDE-23456-FGHJK-789AB");
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
  await page.screenshot({path:`${output}/mobile-batch.png`,fullPage:true});
  checks.push("390px page stays inside viewport; single and batch copy write formatted codes to real clipboard");
  await page.getByLabel("本次批次").getByRole("button",{name:"停用",exact:true}).first().click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button",{name:"确认停用"}).click();
  await dialog.getByRole("alert").filter({hasText:"停用结果尚未确认"}).waitFor();
  await dialog.getByRole("button",{name:"确认停用"}).click();
  await dialog.waitFor({state:"hidden"});
  assert.equal(disablePosts,2);
  assert.equal(await page.getByLabel("本次批次").getByRole("button",{name:"停用",exact:true}).count(),1);
  await page.getByRole("button",{name:"收起批次"}).click();
  await page.getByLabel("内测码状态").selectOption("used");
  await page.getByText("邮箱：reader@example.com",{exact:false}).waitFor();
  assert.equal(await page.getByRole("button",{name:"停用",exact:true}).count(),0);
  await page.screenshot({path:`${output}/mobile-used.png`,fullPage:true});
  await page.getByLabel("内测码状态").selectOption("disabled");
  await page.getByText("共 2 条",{exact:false}).waitFor();
  checks.push("Unknown disable response can be retried on the same code; used account/email and disabled filter are visible");
  mode = "missing";
  await page.getByLabel("生成数量（1–100）").fill("1");
  await page.getByRole("button",{name:"生成内测码",exact:true}).click();
  await page.getByRole("alert").filter({hasText:"生成结果尚未确认"}).waitFor();
  await page.getByRole("button",{name:"按原操作重试"}).click();
  await page.getByLabel("本次批次").waitFor();
  assert.equal(posts,3); assert.equal(batches.size,2); assert.equal(items.length,5);
  checks.push("A missing batch is retried only after GET 404 using the same operation id and frozen count");
  await page.setViewportSize({width:1440,height:1000});
  await page.getByLabel("内测码状态").selectOption("");
  await page.getByText("共 5 条",{exact:false}).waitFor();
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
  await page.screenshot({path:`${output}/desktop.png`,fullPage:true});
  assert.deepEqual(errors,[]);
  checks.push("Desktop listing renders all states without page errors");
  await fs.writeFile(`${output}/result.json`,JSON.stringify({status:"passed",checks,posts,batches:batches.size,errors},null,2));
  console.log(checks.join("\n"));
} catch (error) {
  await page.screenshot({path:`${output}/failure.png`,fullPage:true});
  await fs.writeFile(`${output}/result.json`,JSON.stringify({status:"failed",checks,errors,message:String(error)},null,2));
  throw error;
} finally { await browser.close(); }
