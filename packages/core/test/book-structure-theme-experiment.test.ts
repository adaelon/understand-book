import { describe, expect, it } from "vitest";
import { themeDelivery, submitThemeStep, themeExperimentReport, replayThemeExperiment, type ThemeExperimentPlan, type ThemeExperimentRun } from "../../../evals/book-structure/themes";
import { planStructureThemes, reopenStructureTheme } from "../src/book-structure-themes";
import { fakeEmbedding } from "./fixtures/embedding-provider";

function fixture() {
  const context: ThemeExperimentPlan["context"] = { catalog: { version: "book_structure_candidates.v1", candidates: [] }, mode: "lexical_only",
    chapters: ["1", "2"].map(unit_lid => ({ unit_lid, title: unit_lid, overview: { text: "已接纳概述", evidence_lids: [unit_lid + ".1"] } })),
    excerpts: ["1", "2"].map(unit_lid => ({ unit_lid, text: "原文机制", lids: [unit_lid + ".1"] })) };
  const directory = planStructureThemes({ themes: [{ ref: "theme:a", question: "机制条件", distinction: "不依赖共享名词", unit_lids: ["1", "2"], member_refs: [] }] }, context);
  const plan: ThemeExperimentPlan = { version: "book_structure_theme_experiment.v1", status: "confirmed", authorization: "test",
    context, fixed_question: "机制条件", prompt: "test prompt", max_steps_per_group: 10, max_input_tokens: 20000, max_output_tokens: 6500, directory,
    embedding: { config_file: "unused", identity: fakeEmbedding().identity, max_calls: 10, max_documents: 10, max_queries: 10 } };
  const run: ThemeExperimentRun = { group: "B", directory: structuredClone(directory), steps: [], embedding_attempts: [] };
  const step = (response: unknown) => {
    const ordinal = run.steps.length;
    run.steps.push({ ordinal, opened_at: "2026-10-01T00:00:00Z", ...themeDelivery(plan, run), status: "pending" });
    return submitThemeStep(plan, run, ordinal, response);
  };
  return { plan, run, step };
}
describe("BSR4 frozen comparison adapter", () => {
  it("records rejected actions without changing state, preserves unknown model usage, and replays all inputs/results", () => {
    const { plan, run, step } = fixture();
    const old = structuredClone(run.directory);
    expect(step({ kind: "read", indices: [99] }).status).toBe("rejected"); expect(run.directory).toEqual(old);
    expect(step({ kind: "read", indices: [0, 1] }).status).toBe("accepted");
    expect(step({ kind: "resolve", result: { ref: "theme:a", name: "机制发展", summary: { text: "不同条件", evidence_lids: ["1.1", "2.1"] },
      stages: plan.context.chapters.map(c => ({ unit_lid: c.unit_lid, development: c.overview, member_refs: [] })), dependencies: [] } }).status).toBe("accepted");
    expect(step({ edits: [], merges: [], unresolved: [] }).complete).toBe(true);
    const report = themeExperimentReport(plan, run);
    expect(report.complete).toBe(true); expect(report.actual_input_tokens).toBeNull(); expect(report.actual_model_calls).toBeNull();
    expect(report.rejected).toBe(1); expect(report.structure!.throughlines).toHaveLength(1);
    expect(replayThemeExperiment(plan, run)).toMatchObject({ state_equal: true, delivery_equal: true, materialization_equal: true });
    const previousResult = run.directory.works[0].result!;
    run.revisions = [{ before_step: run.steps.length, ref: "theme:a", issue: "核对条件" }];
    run.directory = reopenStructureTheme(run.directory, "theme:a", "核对条件");
    expect(step({ kind: "resolve", result: previousResult }).complete).toBe(true);
    expect(replayThemeExperiment(plan, run)).toMatchObject({ steps: 5, state_equal: true, materialization_equal: true });
    run.steps[1].input = "changed";
    expect(() => replayThemeExperiment(plan, run)).toThrow(/delivery mismatch/);
  });
  it("rejects oversized output and blocks materialization of unresolved directory work", () => {
    const { plan, run, step } = fixture();
    plan.max_output_tokens = 1;
    expect(step({ kind: "read", indices: [0, 1] }).error).toContain("reserve");
    expect(themeExperimentReport(plan, run).structure).toBeNull();
    expect(() => submitThemeStep(plan, run, 0, {})).toThrow(/pending/);
  });
});
