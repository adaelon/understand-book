import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { acceptStructureOutline, applyStructureChapterAction, newStructureChapterWork, structureChapterInput, reviseStructureChapterWork, continueStructureChapterSelection,
  type StructureOutlineInput, type StructureChapterContext } from "../src/book-structure-planning";
import { collectStructureCandidates } from "../src/book-structure-candidates";
import { renderStructurePlanningInput } from "../src/model-input-renderer";
import { estimateTokens } from "../src/window";
import { createCandidateTransportContract, CODEX_EXECUTOR_TRANSPORT_PROFILE_V2 } from "../src/executor-transport";
// Also typechecks the real experiment entry point without executing it or calling a provider.
import { prepareChapterExperiment, chapterExperimentCli } from "../../../evals/book-structure/chapter";

const chapter = JSON.parse(readFileSync(new URL("../testdata/book-structure/chapter-8.input.json", import.meta.url), "utf8"));
function fixture(count = 9): { context: StructureChapterContext; outlineInput: StructureOutlineInput } {
  const scope = { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": chapter.chapter.card.evidence_lids } };
  const catalog = collectStructureCandidates([{ unit_lid: "10", contribution_ref: chapter.chapter.contribution_ref,
    stops: Array.from({ length: count }, (_, i) => ({ ...chapter.chapter.card.candidate_key_stops[i % 9], id: `candidate-${i}` })), reference_scope: scope }]);
  const summary = chapter.chapter.card.summary;
  const outlineInput: StructureOutlineInput = { version: "book_structure_outline_input.v1",
    chapters: [{ unit_lid: "10", title: chapter.chapter.title, overview: summary }], preface: [] };
  const outline = acceptStructureOutline({ chapters: [{ unit_lid: "10", question: summary, progression: summary }], themes: [] }, outlineInput);
  return { outlineInput, context: { catalog, outline, titles: { "10": chapter.chapter.title },
    excerpts: chapter.evidence.map((e: any) => ({ unit_lid: "10", lids: e.lids, text: e.text })) } };
}
describe("BSR2 outline and chapter selection", () => {
  it("keeps a long 114-stop/44-macro choice through bounded parts without enlarging its transport contract", () => {
    const { context } = fixture(114);
    context.catalog.candidates.forEach(c => c.ref += "-" + "long-reference-".repeat(6));
    let work = newStructureChapterWork("10");
    for (const offset of [24, 48, 72, 96, 0]) work = applyStructureChapterAction(work, { kind: "browse", offset }, context);
    const refs = context.catalog.candidates.map(c => c.ref), summary = context.catalog.candidates[0].reason;
    work = applyStructureChapterAction(work, { kind: "inspect", refs: [refs[0]] }, context);
    const selection = { unit_lid: "10", role: "application", summary, accepted_stop_refs: refs, macro_stop_refs: refs.slice(0, 44) };
    const cap = createCandidateTransportContract(CODEX_EXECUTOR_TRANSPORT_PROFILE_V2).candidate_value_max_estimated_tokens;
    expect(estimateTokens(JSON.stringify({ kind: "select", selection }))).toBeGreaterThan(cap);
    expect(() => applyStructureChapterAction(work, { kind: "select_stops", refs: [refs[0]] }, context)).toThrow(/bounded/);
    work = continueStructureChapterSelection(work, { id: "long-selection", unit_lid: "10", from_action_ordinal: 6, issue: "Keep the complete selection" });
    expect(() => applyStructureChapterAction(work, { kind: "select_stops", refs: ["other:chapter"] }, context)).toThrow(/seen/);
    for (let offset = 0; offset < refs.length; offset += 48) {
      const action = { kind: "select_stops", refs: refs.slice(offset, offset + 48) };
      expect(estimateTokens(JSON.stringify(action))).toBeLessThan(cap);
      work = applyStructureChapterAction(work, action, context);
      expect(work.result).toBeUndefined();
    }
    expect(() => applyStructureChapterAction(work, { kind: "select_stops", refs: [refs[0]] }, context)).toThrow(/duplicate/);
    context.candidate_index_page_size = 64;
    expect(structureChapterInput(work, context).index.items).toEqual([]);
    const browsing = applyStructureChapterAction(work, { kind: "browse", offset: 64 }, context);
    expect(structureChapterInput(browsing, context).index.items.map(c => c.ref)).toEqual(refs.slice(64));
    const inspected = applyStructureChapterAction(browsing, { kind: "inspect", refs: refs.slice(0, 1) }, context);
    expect(structureChapterInput(inspected, context).index.items).toEqual([]);
    expect(structureChapterInput(inspected, context).inspected[0].ref).toBe(refs[0]);
    const { accepted_stop_refs: _refs, ...finalSelection } = selection;
    const final = { kind: "select", selection: finalSelection };
    expect(estimateTokens(JSON.stringify(final))).toBeLessThan(cap);
    expect(() => applyStructureChapterAction(work, { kind: "select", selection: { ...finalSelection, macro_stop_refs: ["unknown"] } }, context)).toThrow(/subset/);
    expect(() => applyStructureChapterAction(work, { kind: "select", selection: { ...finalSelection, summary: { text: "Unsupported", evidence_lids: ["not-delivered"] } } }, context)).toThrow(/undelivered/);
    const result = applyStructureChapterAction(work, final, context).result!;
    expect(result.selection).toEqual(selection);
  });
  it("reopens an accepted chapter with its prior choice and ledgers, requiring delivery of new revision evidence", () => {
    const { context } = fixture();
    context.source_index_page_size = 64; context.candidate_index_page_size = 64;
    const [first, second] = context.catalog.candidates;
    let work = applyStructureChapterAction(newStructureChapterWork("10"), { kind: "inspect", refs: [first.ref], notes: "original notes" }, context);
    work = applyStructureChapterAction(work, { kind: "select", selection: { unit_lid: "10", role: "application", summary: first.reason,
      accepted_stop_refs: [first.ref], macro_stop_refs: [first.ref] } }, context);
    const original = structuredClone(work);
    const revised = reviseStructureChapterWork(work, { id: "source-omission", unit_lid: "10", issue: "Review the omitted mechanism", candidate_refs: [second.ref] }, [first.ref]);
    expect(revised.seen_refs).toEqual(work.seen_refs); expect(revised.evidence_lids).toEqual(work.evidence_lids);
    expect(revised.notes).toBe(""); expect(revised.inspecting).toEqual([]); expect(revised.result).toBeUndefined();
    expect(structureChapterInput(revised, context).revision?.previous_selection).toEqual(work.result!.selection);
    expect(structureChapterInput(revised, context).index.items).toEqual([]);
    const action = { kind: "select", selection: { ...work.result!.selection, summary: second.reason,
      accepted_stop_refs: [first.ref, second.ref], macro_stop_refs: [first.ref, second.ref] } };
    expect(() => applyStructureChapterAction(revised, action, context)).toThrow(/undelivered/);
    const inspected = applyStructureChapterAction(revised, { kind: "inspect", refs: [second.ref] }, context);
    const accepted = applyStructureChapterAction(inspected, action, context);
    expect(accepted.result!.selection).toEqual(action.selection);
    expect(accepted.revision!.previously_inspected_refs).toEqual([first.ref, second.ref]);
    expect(work).toEqual(original);
  });
  it("pages a full chapter source directory, retains global read indices and grants no browsing evidence", () => {
    const { context } = fixture(242);
    context.excerpts = [{ unit_lid: "9", lids: ["9.1"], text: "other chapter" },
      ...Array.from({ length: 900 }, (_, i) => ({ unit_lid: "10", lids: [`10.8.3.${i}`], text: `Source paragraph ${i}` }))];
    let work = newStructureChapterWork("10");
    const legacy = structureChapterInput(work, context);
    expect(legacy.source_index).toHaveLength(900);
    context.source_index_page_size = 64;
    context.candidate_index_page_size = 64;
    const first = structureChapterInput(work, context);
    expect(first.index.items).toHaveLength(64);
    expect(first.index.next_offset).toBe(64);
    expect(first.source_index).toHaveLength(64);
    expect(first.source_index[0].index).toBe(1);
    expect(first.source_page).toEqual({ offset: 0, total: 900, next_offset: 64 });
    expect(estimateTokens(JSON.stringify(first))).toBeLessThan(estimateTokens(JSON.stringify(legacy)));
    work = applyStructureChapterAction(work, { kind: "source_browse", offset: 896 }, context);
    expect(work.evidence_lids).toEqual([]);
    expect(structureChapterInput(work, context).source_index.map(e => e.index)).toEqual([897, 898, 899, 900]);
    expect(() => applyStructureChapterAction(work, { kind: "source_browse", offset: 900 }, context)).toThrow(/outside catalog/);
    work = applyStructureChapterAction(work, { kind: "read", indices: [900] }, context);
    expect(structureChapterInput(work, context).excerpts[0].lids).toEqual(["10.8.3.899"]);
    work = applyStructureChapterAction(work, { kind: "browse", offset: 64 }, context);
    expect(work.evidence_lids).toEqual(["10.8.3.899"]);
    work = applyStructureChapterAction(work, { kind: "inspect", refs: [context.catalog.candidates[0].ref] }, context);
    expect(structureChapterInput(work, context).inspected[0]).toHaveProperty("source_indices");
  });
  it("preserves canonical identities/order and requires delivered chapter/theme evidence", () => {
    const { context, outlineInput } = fixture();
    expect(context.outline.chapters[0].unit_lid).toBe("10");
    expect(() => acceptStructureOutline({ ...context.outline, chapters: [] }, outlineInput)).toThrow(/order/);
    const invalid = structuredClone(context.outline); invalid.chapters[0].question.evidence_lids = ["11.2"];
    expect(() => acceptStructureOutline(invalid, outlineInput)).toThrow(/undelivered/);
    expect(() => acceptStructureOutline({ ...context.outline, themes: [{ question: invalid.chapters[0].question, unit_lids: ["10"] }] }, outlineInput)).toThrow(/each member/);
  });
  it("preview and outline evidence cannot support a new chapter summary; an inspected candidate can", () => {
    const { context } = fixture();
    let work = newStructureChapterWork("10");
    const candidate = context.catalog.candidates[0];
    const action = { kind: "select", selection: { unit_lid: "10", role: "application", summary: candidate.reason,
      accepted_stop_refs: [candidate.ref], macro_stop_refs: [] } };
    expect(() => applyStructureChapterAction(work, action, context)).toThrow(/undelivered/);
    expect(work.seen_refs).toEqual([]);
    work = applyStructureChapterAction(work, { kind: "inspect", refs: [candidate.ref] }, context);
    const input = structureChapterInput(work, context);
    expect(input.inspected[0].reason).toEqual(candidate.reason);
    const accepted = applyStructureChapterAction(work, action, context);
    expect(accepted.result!.selection.accepted_stop_refs).toEqual([candidate.ref]);
    expect(() => applyStructureChapterAction(accepted, action, context)).toThrow(/already accepted/);
  });
  it("requires complete index browsing and rejects wrong-unit or unknown reads", () => {
    const { context } = fixture(30);
    let work = newStructureChapterWork("10");
    expect(structureChapterInput(work, context).index.next_offset).toBe(24);
    const c = context.catalog.candidates[0];
    work = applyStructureChapterAction(work, { kind: "inspect", refs: [c.ref] }, context);
    const select = { kind: "select", selection: { unit_lid: "10", role: "application", summary: c.reason, accepted_stop_refs: [c.ref], macro_stop_refs: [c.ref] } };
    expect(() => applyStructureChapterAction(work, select, context)).toThrow(/complete chapter index/);
    work = applyStructureChapterAction(work, { kind: "browse", offset: 24, notes: "保留显存容量机制" }, context);
    expect(applyStructureChapterAction(work, select, context).result).toBeDefined();
    expect(() => applyStructureChapterAction(work, { kind: "inspect", refs: ["other:11"] }, context)).toThrow(/browsed/);
    expect(() => applyStructureChapterAction(work, { kind: "read", indices: [999] }, context)).toThrow(/outside chapter/);
  });
  it("omits an already-delivered preview during expansion and preserves full browsing and citation state", () => {
    const { context } = fixture(130);
    context.candidate_index_page_size = 64; context.source_index_page_size = 64;
    let work = newStructureChapterWork("10");
    const c = context.catalog.candidates[0];
    work = applyStructureChapterAction(work, { kind: "inspect", refs: [c.ref] }, context);
    const expanded = structureChapterInput(work, context);
    expect(expanded.index.items).toEqual([]);
    expect(expanded.index.preview_already_delivered).toBe(true);
    expect(expanded.index).toMatchObject({ offset: 0, total: 130, next_offset: 64 });
    expect(work.seen_refs).toHaveLength(64);
    expect(expanded.inspected[0].ref).toBe(c.ref);
    const select = { kind: "select", selection: { unit_lid: "10", role: "application", summary: c.reason,
      accepted_stop_refs: [c.ref, context.catalog.candidates[129].ref], macro_stop_refs: [c.ref] } };
    expect(() => applyStructureChapterAction(work, select, context)).toThrow(/complete chapter index/);
    work = applyStructureChapterAction(work, { kind: "browse", offset: 64 }, context);
    expect(structureChapterInput(work, context).index.items).toHaveLength(64);
    expect(work.evidence_lids).toEqual(c.evidence_lids);
    work = applyStructureChapterAction(work, { kind: "browse", offset: 128 }, context);
    expect(applyStructureChapterAction(work, select, context).result!.selection.accepted_stop_refs).toEqual(select.selection.accepted_stop_refs);
  });
  it("source expansion grants only the actual excerpt and keeps bounded model input", () => {
    const { context } = fixture();
    let work = newStructureChapterWork("10");
    work = applyStructureChapterAction(work, { kind: "read", indices: [0] }, context);
    expect(structureChapterInput(work, context).excerpts).toHaveLength(1);
    work = applyStructureChapterAction(work, { kind: "browse", offset: 0 }, context);
    expect(new Set(work.evidence_lids)).toEqual(new Set(context.excerpts[0].lids));
    const rendered = renderStructurePlanningInput(structureChapterInput(work, context));
    expect(rendered).toContain("第 8 章 推理优化");
    expect(rendered).not.toContain('"expected"');
    expect(() => renderStructurePlanningInput(structureChapterInput(work, context), 1)).toThrow(/exceeds budget/);
    expect(typeof prepareChapterExperiment).toBe("function");
    expect(typeof chapterExperimentCli).toBe("function");
  });
});
