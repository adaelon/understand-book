import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdtempSync, writeFileSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, textResponse } from "./harness.ts";
import { MetadataModel } from "./control-fixture.ts";
import { costFixture } from "./external-cost-fixture.ts";

for (const mode of ["denied", "cancelled", "failed", "receipt-offline"] as const) {
  test(`Q3 ${mode}: actual host admission and terminal uncertainty`, { timeout: 120000 }, async () => {
    const root = mkdtempSync(path.join(tmpdir(), "ub-q3-host-"));
    const source = path.join(root, "book.md");
    writeFileSync(source, "# Transactions\n\nTransactions group operations into one atomic change.\n");
    const costs = await costFixture();
    if (mode === "denied") costs.rejectAdmissions();
    if (mode === "receipt-offline") costs.receiptsOffline(true);
    const abort = new AbortController();
    const h = await harness(new MetadataModel(async function* () {
      assert.ok(costs.admissions().length > 0, "first provider request requires admission");
      if (mode === "cancelled") abort.abort();
      if (mode === "failed") throw new Error("provider interrupted without usage");
      yield* textResponse("finished without executor receipt");
    }));
    const config = { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!,
      prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url))],
      driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096,
      handoffsPerCall: 1, externalCost: costs.config };
    const control = createBuildControl(h.ctx, config, "understand-book-executor");
    const remove = h.ctx.on("user-questions/request", async r => ({ answers: [{ id: r.questions[0].id, selected: ["批准"] }] }));
    try {
      const inv = await control.prepareAndConfirm(h.parent, new AbortController().signal,
        { target_input: source, root_dir: root, pass2: "disabled" }) as any;
      const result = await control.run(h.parent, abort.signal, inv.invocation_ref) as any;
      assert.equal(result.yielded, undefined, 'a failure or budget boundary at the handoff limit must not auto-resume');
      assert.equal(costs.admissions().length, 1);
      if (mode === "denied") {
        assert.equal(h.model.requests.length, 0);
        assert.equal(result.cost_issue, "admission_blocked");
        assert.equal(costs.receipts().length, 0);
        assert.equal(existsSync(path.join(config.driverRoot, "dsh-observations", inv.invocation_ref)), false,
          "budget denial must not record an executor failure or consume its retry allowance");
      } else {
        assert.equal(h.model.requests.length, 1);
        assert.equal(costs.receipts().length, 1);
        assert.equal(costs.receipts()[0].status, "unknown");
        assert.equal(costs.receipts()[0].succeeded, false);
        if (mode === "cancelled") assert.equal(costs.receipts()[0].evidence.outcome, "cancelled");
      }
      if (mode === "receipt-offline") {
        assert.equal(result.cost_issue, "receipt_pending");
        const dir = path.join(config.driverRoot, "external-build-cost", inv.invocation_ref);
        const saved = readdirSync(dir).map(f => JSON.parse(readFileSync(path.join(dir, f), "utf8")));
        assert.equal(saved[0].state, "receipt_pending");
        costs.receiptsOffline(false);
        // The next driver session reconciles the identical pending receipt before any new admission.
        const before = costs.calls.length;
        await control.run(h.parent, new AbortController().signal, inv.invocation_ref);
        const subsequent = costs.calls.slice(before);
        assert.ok(subsequent.findIndex(c => c.path.endsWith("/receipts")) >= 0);
        assert.deepEqual(costs.receipts()[1], costs.receipts()[0]);
      }
      assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    } finally { remove(); await control.dispose(); await h.dispose(); await costs.close(); }
  });
}
