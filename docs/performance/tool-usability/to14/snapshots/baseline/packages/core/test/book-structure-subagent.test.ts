import { describe, expect, it } from "vitest";
import { openSubagentStep, submitSubagentStep, subagentReport, type SubagentExperiment } from "../../../evals/book-structure/chapter-subagent";
import { newStructureChapterWork } from "../src/book-structure-planning";
import { collectStructureCandidates } from "../src/book-structure-candidates";

function fixture(): SubagentExperiment {
  const summary = { text: "权重与 KV 决定容量", evidence_lids: ["10.2"] };
  return { version: "book_structure_subagent_experiment.v1", authorization: "test", executor: "codex_subagent",
    prompts: { outline: "outline", chapter: "chapter" }, steps: [], chapter: newStructureChapterWork("10"),
    input: { version: "book_structure_chapter_experiment.v1", source_workspace: "test", old_published: {}, historical_estimates: [],
      outline_input: { version: "book_structure_outline_input.v1", preface: [], chapters: [{ unit_lid: "10", title: "第 8 章", overview: summary }] },
      catalog: collectStructureCandidates([{ contribution_ref: "unit:10", unit_lid: "10",
        stops: [{ id: "memory", lid: "10.2", type: "claim", reason: summary }],
        reference_scope: { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": ["10.2"] } } }]),
      excerpts: [{ unit_lid: "10", lids: ["10.2"], text: summary.text }] } };
}
describe("BSR2 isolated subagent experiment adapter", () => {
  it("reuses pending delivery, records rejection without state changes and preserves unknown usage", () => {
    const session = fixture();
    const first = openSubagentStep(session, "2026-10-01T12:00:00.000Z");
    expect(openSubagentStep(session)).toBe(first);
    expect(session.steps).toHaveLength(1);
    expect(submitSubagentStep(session, 0, { chapters: [], themes: [] }).status).toBe("rejected");
    expect(session.outline).toBeUndefined();
    expect(() => submitSubagentStep(session, 0, {})).toThrow(/pending/);
    const report = subagentReport(session);
    expect(report.rejected).toBe(1);
    expect(report.actual_input_tokens).toBeNull();
    expect(report.actual_model_calls).toBeNull();
    expect(report.input_tokens_estimate_sum).toBeGreaterThan(0);
    openSubagentStep(session);
    const oversized = submitSubagentStep(session, 1, { text: "一".repeat(30000) });
    expect(oversized.status).toBe("rejected");
    expect(oversized.error).toContain("output reserve");
  });
  it("runs outline → inspected evidence → reference selection through Core and materializes once", () => {
    const s = fixture(); const summary = s.input.outline_input.chapters[0].overview;
    openSubagentStep(s, "2026-10-01T12:00:00.000Z");
    expect(submitSubagentStep(s, 0, { chapters: [{ unit_lid: "10", question: summary, progression: summary }], themes: [] }).status).toBe("accepted");
    openSubagentStep(s);
    const ref = s.input.catalog.candidates[0].ref;
    expect(submitSubagentStep(s, 1, { kind: "inspect", refs: [ref] }).status).toBe("accepted");
    openSubagentStep(s);
    expect(submitSubagentStep(s, 2, { kind: "select", selection: { unit_lid: "10", role: "application", summary,
      accepted_stop_refs: [ref], macro_stop_refs: [] } }, "2026-10-01T12:01:00.000Z").status).toBe("accepted");
    expect(openSubagentStep(s)).toBeUndefined();
    const report = subagentReport(s);
    expect(report.complete).toBe(true); expect(report.wall_ms).toBe(60000);
    expect(report.new_structure!.key_stops).toHaveLength(1);
    expect(report.new_structure!.spine![0].key_stop_ids).toEqual([]);
    expect(report.external_generation_api_calls).toBe(0);
  });
});
