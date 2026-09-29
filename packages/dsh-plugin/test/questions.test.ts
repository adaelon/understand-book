import assert from "node:assert/strict";
import { test } from "node:test";
import { harness } from "./harness.ts";

test("installed question service preserves full plans and rejects missing, cancelled and delegated callers", async () => {
  const h = await harness();
  const question = { id: "dh0-plan", question: "确认合成测试计划", detail: "FULL-PLAN-SENTINEL",
    options: [{ label: "批准" }, { label: "拒绝" }], intent: { kind: "plan-review" as const, approve: "批准" } };
  try {
    await assert.rejects(h.ctx.userQuestions.ask({ agent: h.parent, questions: [question] }), { code: "NO_PROVIDER" });
    const abort = new AbortController(); abort.abort();
    await assert.rejects(h.ctx.userQuestions.ask({ agent: h.parent, signal: abort.signal, questions: [question] }), { code: "ASK_ABORTED" });
    for (const selected of [["批准"], ["拒绝"], []]) {
      const remove = h.ctx.on("user-questions/request", async request => {
        assert.equal(request.agent, h.parent);
        assert.deepEqual(request.questions, [question]);
        return { answers: [{ id: question.id, selected }] };
      });
      try { assert.deepEqual(await h.ctx.userQuestions.ask({ agent: h.parent, questions: [question] }), { answers: [{ id: question.id, selected }] }); }
      finally { remove(); }
    }
    const run = await h.ctx.subagents.start("spawn", { parent: h.parent, signal: new AbortController().signal,
      prompt: [{ type: "text", text: "probe" }] });
    try {
      await assert.rejects(h.ctx.userQuestions.ask({ agent: run.localAgent!, questions: [question] }), { code: "DELEGATED_CALLER" });
      await run.result;
    } finally { await run.dispose(); }
  } finally { await h.dispose(); }
});
