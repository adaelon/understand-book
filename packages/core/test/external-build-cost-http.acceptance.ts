/** Run against the ignored q4_browser_host fixture; no real executor or paid call is started. */
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { request as httpRequest } from "node:http";
import { ExternalBuildCostLedger, type ExternalBuildCostBinding } from "../src/external-build-cost";

const fixtureResponse = await fetch("http://127.0.0.1:4187/fixture");
assert.equal(fixtureResponse.status, 200, "start the local q4_browser_host fixture first");
const fixture = await fixtureResponse.json() as { admin_cookie: string; admin_csrf: string };
assert.equal(typeof fixture.admin_cookie, "string");
assert.equal(typeof fixture.admin_csrf, "string");
const root = mkdtempSync(path.join(tmpdir(), "ub-q3-http-"));
const run = randomUUID().replaceAll("-", "");
const task = `q3_http_${run}`;
const recoveryTask = `${task}_recovery`;
const connection = { base_url: "http://127.0.0.1:4186", cookie: fixture.admin_cookie,
  csrf_token: fixture.admin_csrf, origin: "https://reader.example" };
const binding: ExternalBuildCostBinding = { version: "external_build_cost.v1", cost_owner: "platform",
  max_micro_cny: 100, reserved_per_execution_micro_cny: 60, confirmation: `acceptance_${run}` };
const admissions: string[] = [];
const lookups: string[] = [];
const transport: typeof fetch = async (input, init) => {
  const url = String(input);
  if (url.endsWith("/admit")) admissions.push(url);
  if (url.includes("/charges/")) lookups.push(url);
  const headers = new Headers(init?.headers);
  headers.set("Host", "reader.example");
  // Node's fetch may derive Host from the loopback URL. This fixture adapter
  // preserves the public proxy Host expected by the real Rust service.
  return new Promise<Response>((resolve, reject) => {
    const request = httpRequest(url, { method: init?.method, headers: Object.fromEntries(headers.entries()),
      signal: init?.signal ?? undefined }, response => {
      const chunks: Buffer[] = [];
      response.on("data", chunk => chunks.push(Buffer.from(chunk)));
      response.on("error", reject);
      response.on("end", () => resolve(new Response(Buffer.concat(chunks).toString("utf8"), {
        status: response.statusCode, headers: { "Content-Type": "application/json" },
      })));
    });
    request.on("error", reject);
    request.end(init?.body === undefined ? undefined : String(init.body));
  });
};
const ledger = (taskRef: string) => new ExternalBuildCostLedger(taskRef, binding, connection, root, transport);
async function api(url: string, body?: unknown): Promise<any> {
  const response = await transport(`${connection.base_url}/api/admin/${url}`, {
    method: body === undefined ? "GET" : "POST", headers: { Cookie: connection.cookie,
      "X-CSRF-Token": connection.csrf_token, Origin: connection.origin, "Content-Type": "application/json" },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(15_000),
  });
  assert.equal(response.status, 200, `expected fixture API success for ${url.split("/")[0]}`);
  return response.json();
}
async function confirm(taskRef: string, call: string, cost: number) {
  return api(`build-tasks/${taskRef}/receipts`, { call_id: call, status: "confirmed", provider_cost_micro_cny: cost,
    usage: { internal_details: [{ cost_micro_cny: 10 }, { cost_micro_cny: cost - 10 }] },
    evidence: { source: "controlled_provider_execution_receipt", receipt_ref: `${call}_invoice` }, succeeded: true });
}
function assertPlatform(charge: any) {
  assert.equal(charge.cost_owner, "platform");
  assert.equal(charge.user_id, null);
  assert.equal(charge.period_id, null);
  assert.ok(charge.account_debit_micro_cny === null || charge.account_debit_micro_cny === 0);
}

try {
  const first = ledger(task);
  await first.prepare();
  await first.prepare();
  assert.equal(admissions.length, 0, "preparing or replaying a budget never starts an execution");
  const call = await first.admit(`launch_${run}`, `unit_${run}`, "controlled-fixture", "fixture-model");
  const terminal = { model_calls: 2, usages: [{ input_tokens: 13, output_tokens: 7 }], succeeded: true, outcome: "completed" };
  await first.settle(call, terminal);
  await first.settle(call, terminal);
  const partial = await api(`charges/${call}`);
  assertPlatform(partial);
  assert.equal(partial.state, "pending");
  assert.equal(partial.provider_cost_status, "pending");
  assert.equal(partial.provider_cost_micro_cny, null);
  assert.equal(partial.reports.total, 1, "repeated incomplete observation is one receipt fact");
  const observed = await api(`build-tasks/${task}`);
  assert.equal(observed.pending_micro_cny, 60);
  assert.equal(observed.remaining_micro_cny, 40);

  const late = await confirm(task, call, 70);
  assertPlatform(late.charge);
  assert.equal(late.charge.state, "settled");
  assert.equal(late.budget.provider_cost_micro_cny, 70);
  assert.equal(late.budget.platform_covered_micro_cny, 70);
  assert.equal(late.budget.debited_micro_cny, 0);
  assert.deepEqual(await confirm(task, call, 70), late, "late complete execution totals are idempotent");
  const rows = await api(`charges?parent_task_ref=${task}`);
  assert.equal(rows.total, 1, "the execution total and its internal details are charged at one level");
  assert.equal(rows.items[0].provider_cost_micro_cny, 70);

  const admissionsBeforeRestart = admissions.length;
  await ledger(task).prepare();
  assert.equal(admissions.length, admissionsBeforeRestart, "a new client never replays a completed launch");
  await assert.rejects(() => ledger(task).admit(`launch_${run}`, `unit_${run}`, "controlled-fixture", "fixture-model"), /already_recorded/);
  let startsAfterBudgetExhaustion = 0;
  await assert.rejects(async () => {
    await ledger(task).admit(`blocked_${run}`, `next_unit_${run}`, "controlled-fixture", "fixture-model");
    startsAfterBudgetExhaustion += 1;
  }, /http_409/);
  assert.equal(startsAfterBudgetExhaustion, 0);
  assert.equal((await api(`charges?parent_task_ref=${task}`)).total, 1);

  const interrupted = ledger(recoveryTask);
  await interrupted.prepare();
  const completedWithoutLocalReceipt = await interrupted.admit(`settled_${run}`, `settled_unit_${run}`, "controlled-fixture", "fixture-model");
  await confirm(recoveryTask, completedWithoutLocalReceipt, 25);
  const beforeRecovery = admissions.length;
  const beforeLookups = lookups.length;
  await ledger(recoveryTask).prepare();
  assert.equal(admissions.length, beforeRecovery);
  assert.ok(lookups.length > beforeLookups, "recovery reads the service's actual charge state");
  assert.equal((await api(`charges/${completedWithoutLocalReceipt}`)).reports.total, 1,
    "a server-settled launch is not replaced with an unknown receipt");
  const pendingLaunch = await ledger(recoveryTask).admit(`interrupted_${run}`, `pending_unit_${run}`, "controlled-fixture", "fixture-model");
  const beforePendingRecovery = admissions.length;
  await ledger(recoveryTask).prepare();
  assert.equal(admissions.length, beforePendingRecovery);
  const pending = await api(`charges/${pendingLaunch}`);
  assertPlatform(pending);
  assert.equal(pending.state, "pending");
  assert.equal(pending.provider_cost_micro_cny, null);
  assert.equal(pending.reports.total, 1);
  const recoveredBudget = await api(`build-tasks/${recoveryTask}`);
  assert.equal(recoveredBudget.pending_micro_cny, 60);
  assert.equal(recoveredBudget.provider_cost_micro_cny, 25);

  for (const taskRef of [task, recoveryTask]) {
    const directory = path.join(root, "external-build-cost", taskRef);
    for (const file of readdirSync(directory)) {
      const saved = readFileSync(path.join(directory, file), "utf8");
      assert.ok(!saved.includes(fixture.admin_cookie) && !saved.includes(fixture.admin_csrf),
        "runtime credentials must not be written into the task registry");
    }
  }
  console.log(JSON.stringify({ status: "Q3_HTTP_ACCEPTANCE_PASS", scenarios: [
    "budget preparation", "partial receipt replay", "late total with diagnostic details", "insufficient budget stops launch",
    "settled recovery reads service state", "interrupted recovery preserves pending cost", "credentials remain runtime-only",
  ], ledger_cost_micro_cny: 95, account_debit_micro_cny: 0, real_paid_calls: 0 }));
} finally {
  // Only the fresh fixture registry is removed; the host and its ledger belong to the root runner.
  assert.equal(path.dirname(path.resolve(root)), path.resolve(tmpdir()));
  assert.ok(path.basename(root).startsWith("ub-q3-http-"));
  rmSync(root, { recursive: true, force: true });
}
