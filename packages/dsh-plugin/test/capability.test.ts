import assert from "node:assert/strict";
import { test } from "node:test";
import { Context } from "@deepseek-ai/cordis";
import { checkHarnessCapabilities } from "../src/capability-check.ts";
import { harness } from "./harness.ts";

test("missing host services are unsupported; the real fixed host exposes the required seams", async () => {
  assert.equal(checkHarnessCapabilities(new Context()).status, "unsupported");
  const h = await harness();
  try { assert.deepEqual(checkHarnessCapabilities(h.ctx), { status: "available", missing: [] }); }
  finally { await h.dispose(); }
});
