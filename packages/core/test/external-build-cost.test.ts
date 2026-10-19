import { mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { ExternalBuildCostLedger, readExternalBuildCostBinding, type ExternalBuildCostBinding } from "../src/external-build-cost";
import { automaticBuildDriverFailureResponse, createAutomaticBuildInvocation } from "../../../skills/build/automatic-build-driver";

const binding: ExternalBuildCostBinding = { version: "external_build_cost.v1", cost_owner: "platform", max_micro_cny: 1000,
  reserved_per_execution_micro_cny: 100, confirmation: "operator-confirmed-budget" };

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), "ub-q3-cost-"));
  const calls: { suffix: string; body: any }[] = [];
  const charges = new Map<string, any>();
  let receiptFailure = false;
  let admissionResponseFailure = false;
  let chargeQueryFailure = false;
  const fetcher: typeof fetch = async (url, options) => {
    const suffix = String(url).split("/task-q3")[1];
    const body = options?.body ? JSON.parse(String(options.body)) : undefined;
    calls.push({ suffix, body });
    if (suffix === "/admit") {
      charges.set(body.call_id, { ...body, state: "sent" });
      if (admissionResponseFailure) throw new Error("connection lost after admission");
      return Response.json({ admitted: true, call_id: body.call_id });
    }
    if (suffix.startsWith("/charges/")) {
      if (chargeQueryFailure) throw new Error("query offline");
      return charges.has(suffix.slice(9)) ? Response.json({ charge: charges.get(suffix.slice(9)) }) : new Response("", { status: 404 });
    }
    if (suffix === "/receipts" && receiptFailure) throw new Error("offline after terminal");
    return Response.json({});
  };
  const ledger = () => new ExternalBuildCostLedger("task-q3", binding,
    { base_url: "https://reader.example", cookie: "SECRET-COOKIE", csrf_token: "SECRET-CSRF", origin: "https://reader.example" }, root, fetcher);
  const saved = () => readdirSync(path.join(root, "external-build-cost", "task-q3"))
    .map(f => readFileSync(path.join(root, "external-build-cost", "task-q3", f), "utf8")).join("\n");
  return { ledger, calls, saved, failReceipt: (value: boolean) => { receiptFailure = value; },
    failAdmissionResponse: () => { admissionResponseFailure = true; }, failChargeQuery: (value: boolean) => { chargeQueryFailure = value; },
    charges };
}

describe("Q3 external host costs", () => {
  it('uses an operator-confirmed shared budget without creating another confirmation per material', async () => {
    const requests: {url:string;method:string}[]=[];
    const fetcher:typeof fetch=async(url,options)=>{requests.push({url:String(url),method:options?.method??'GET'});return Response.json({cost_owner:'platform',max_micro_cny:2000});};
    const root=mkdtempSync(path.join(tmpdir(),'ub-b1-cost-'));
    const shared=readExternalBuildCostBinding({...binding,task_ref:'operator-task',max_micro_cny:2000},{profile_id:'dsh_native_v4',profile_revision:1});
    const ledger=new ExternalBuildCostLedger('operator-task',shared,{base_url:'https://reader.example',cookie:'operator',csrf_token:'csrf',origin:'https://reader.example'},root,fetcher);
    await ledger.prepare();expect(requests).toEqual([{url:'https://reader.example/api/admin/build-tasks/operator-task',method:'GET'}]);
    const stale=new ExternalBuildCostLedger('operator-task',{...shared,max_micro_cny:1000},{base_url:'https://reader.example',cookie:'operator',csrf_token:'csrf',origin:'https://reader.example'},root,fetcher);
    await expect(stale.prepare()).rejects.toThrow('external_build_budget_changed');
  });
  it("rejects personal payment for both real profiles before reading or creating build data", () => {
    for (const profile_id of ["codex_mcp_v3", "dsh_native_v4"] as const) {
      expect(() => createAutomaticBuildInvocation({ version: "automatic_build_invocation_create.v2",
        target_input: "missing", root_dir: "missing", build_plan_path: "missing", max_parallel: 1,
        quality_profile: "full", created_at: "2026-10-18T00:00:00Z", execution_profile: { profile_id, profile_revision: 1 },
        external_cost: { ...binding, cost_owner: "account", user_id: "reader" } })).toThrow(/personal_payment_unsupported/);
    }
    expect(() => readExternalBuildCostBinding({ ...binding, budget_enforced: true },
      { profile_id: "dsh_native_v4", profile_revision: 1 })).toThrow(/invalid/);
    expect(automaticBuildDriverFailureResponse(new Error("external_build_personal_payment_unsupported: executor capability")))
      .toMatchObject({ action: { kind: "NEEDS_USER", projection: { code: "external_build_personal_payment_unsupported" } } });
  });

  it("preserves distinct actual launches and never treats incomplete stream observations as exact bills", async () => {
    const f = fixture(); const ledger = f.ledger(); await ledger.prepare();
    for (const [id, model_calls, usages, outcome] of [["success", 2, [{ input_tokens: 10 }], "receipt"],
      ["failed", 1, [], "runtime"], ["cancelled", 0, [], "cancelled"]] as const) {
      const call = await ledger.admit(id, "same-unit", "provider", "model");
      await ledger.settle(call, { model_calls, usages: [...usages], outcome, succeeded: outcome === "receipt" });
    }
    const admissions = f.calls.filter(c => c.suffix === "/admit").map(c => c.body);
    expect(admissions.map(c => c.attempt)).toEqual([1, 2, 3]);
    expect(admissions.every(c => c.granularity === "execution_total" && !c.budget_enforced)).toBe(true);
    const receipts = f.calls.filter(c => c.suffix === "/receipts").map(c => c.body);
    expect(receipts.map(c => c.status)).toEqual(["partial", "unknown", "not_sent"]);
    expect(receipts.every(c => c.provider_cost_micro_cny === undefined)).toBe(true);
    await expect(ledger.admit("success", "same-unit", "provider", "model")).rejects.toThrow(/already_recorded/);
    await f.ledger().prepare();
    expect(f.calls.filter(c => c.suffix === "/admit")).toHaveLength(3);
    expect(f.saved()).not.toContain("SECRET");
  });

  it("replays an unsent terminal receipt without resending the model and preserves a crashed launch as unknown", async () => {
    const f = fixture(); const ledger = f.ledger(); await ledger.prepare();
    const terminal = await ledger.admit("terminal", "unit", "provider", "model");
    f.failReceipt(true);
    await expect(ledger.settle(terminal, { model_calls: 1, usages: [{ output_tokens: 7 }], succeeded: true, outcome: "receipt" })).rejects.toThrow();
    await ledger.admit("crashed", "other-unit", "provider", "model");
    f.failReceipt(false);
    await f.ledger().prepare();
    const receipts = f.calls.filter(c => c.suffix === "/receipts").map(c => c.body);
    expect(receipts).toHaveLength(3);
    expect(receipts[1]).toMatchObject({ call_id: "ext_crashed", status: "unknown" });
    expect(receipts[2]).toEqual(receipts[0]);
    expect(f.calls.filter(c => c.suffix === "/admit")).toHaveLength(2);
  });

  it("releases a committed admission when its response was lost before child start", async () => {
    const f = fixture(); const ledger = f.ledger(); await ledger.prepare(); f.failAdmissionResponse();
    await expect(ledger.admit("lost", "unit", "provider", "model")).rejects.toThrow(/connection lost/);
    expect(f.calls.find(c => c.suffix === "/receipts")?.body).toMatchObject({ call_id: "ext_lost", status: "not_sent", succeeded: false });
  });

  it("retains not-sent intent across a failed recovery lookup and skips nonexistent or already settled charges", async () => {
    const f = fixture(); const ledger = f.ledger(); await ledger.prepare();
    await ledger.admit("settled", "done-unit", "provider", "model");
    f.charges.set("ext_settled", { state: "settled" });
    const reconciled = await ledger.admit("reconciled", "done-unit", "provider", "model");
    f.failReceipt(true);
    await expect(ledger.settle(reconciled, { model_calls: 1, usages: [], succeeded: false, outcome: "runtime" })).rejects.toThrow();
    f.failReceipt(false); f.charges.set(reconciled, { state: "settled" });
    f.failAdmissionResponse(); f.failChargeQuery(true);
    await expect(ledger.admit("lost", "unit", "provider", "model")).rejects.toThrow();
    await expect(ledger.admit("rejected", "unit", "provider", "model")).rejects.toThrow();
    f.charges.delete("ext_rejected");
    f.failChargeQuery(false);
    await f.ledger().prepare();
    expect(f.calls.filter(c => c.suffix === "/receipts").slice(1).map(c => c.body)).toEqual([
      expect.objectContaining({ call_id: "ext_lost", status: "not_sent" }),
    ]);
  });
});
