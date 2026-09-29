import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, writeFileSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createBuildControl } from "../src/build-control.ts";
import { harness, textResponse } from "./harness.ts";
import { MetadataModel } from "./control-fixture.ts";

for (const mode of ["runtime", "cancel", "unload"] as const) test(`three owned children ${mode}: all observations survive and all resources close`, { timeout: 90000 }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), "ub-dh5-failures-"));
  const source = path.join(root, "book.md");
  writeFileSync(source, ["# Guide", ...Array.from({ length: 900 }, (_, i) => `Paragraph ${i + 1} contains deterministic semantic evidence for the concurrent build protocol.`)].join("\n\n"));
  let arrived = 0;
  let release!: () => void;
  const all = new Promise<void>(r => { release = r; });
  const cancellation = new AbortController();
  const h = await harness(new MetadataModel(async function* () {
    if (++arrived === 3) release();
    await all;
    if (mode === "cancel") cancellation.abort();
    if (mode === "unload") void control.dispose();
    if (mode === "runtime") throw new Error("PRIVATE-RUNTIME-SENTINEL");
    yield* textResponse("PRIVATE-FINAL-SENTINEL");
  }));
  const trace = path.join(root, "engine-processes.jsonl");
  const config = { executable: process.env.UNDERSTAND_BOOK_TEST_NODE!, prefixArgs: ["--import", "tsx", fileURLToPath(new URL("engine-source-cli.ts", import.meta.url)), "--dh5-trace", trace],
    driverRoot: path.join(root, "registry"), maxOutputTokens: 4096, safetyMarginTokens: 4096, handoffsPerCall: 6 };
  const control = createBuildControl(h.ctx, config, "understand-book-executor");
  const remove = h.ctx.on("user-questions/request", async r => ({ answers: [{ id: r.questions[0].id, selected: ["批准"] }] }));
  try {
    const inv = await control.prepareAndConfirm(h.parent, cancellation.signal, { target_input: source, root_dir: root, pass2: "disabled", max_parallel: 3 }) as any;
    const running = control.run(h.parent, cancellation.signal, inv.invocation_ref);
    await all;
    // A second plugin instance must honor the same durable Engine owner.
    if (mode === "runtime") {
      const other = createBuildControl(h.ctx, config, "understand-book-executor");
      try { assert.equal((await other.run(h.parent, new AbortController().signal, inv.invocation_ref) as any).host_observation.code, "build_owner_active"); }
      finally { await other.dispose(); }
    }
    const result = await running as any;
    assert.equal(arrived, 3);
    assert.deepEqual(h.ctx.agents.list(), [h.parent]);
    const dir = path.join(config.driverRoot, "dsh-observations", inv.invocation_ref);
    const observations = readdirSync(dir).map(f => JSON.parse(readFileSync(path.join(dir, f), "utf8")));
    assert.equal(observations.length, 3);
    assert.ok(observations.every(o => o.calls === 0 && !o.done));
    assert.ok(observations.every(o => o.outcome === (mode === "runtime" ? "runtime" : "cancelled")));
    assert.equal(JSON.stringify(result).includes("PRIVATE-"), false);
    assert.equal(readdirSync(path.join(config.driverRoot, "dsh-controllers")).length, 0);
    const pids = readFileSync(trace, "utf8").trim().split("\n").map(l => JSON.parse(l)).filter(v => v.pid).map(v => v.pid);
    assert.equal(pids.length, 3);
    for (const pid of pids) assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
    console.log(JSON.stringify({ mode, root, observed: observations.length, children_remaining: h.ctx.agents.list().length - 1 }));
  } finally { release(); remove(); await control.dispose(); await h.dispose(); }
});
