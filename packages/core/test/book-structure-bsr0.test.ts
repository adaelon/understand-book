import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { assertSourceExcerpt, sourceExcerpt, summarizeUsage } from "../../../evals/book-structure/extract";
import { bookStructureRelationEntries, bookStructureSelectedPairs } from "../src/book-structure-relation-routing";
import { materializeBookStructureContributions, type BookStructureContribution } from "../src/book-structure-materialization";
import { applyBookStructureRelationDeltas, validateBookStructureRelationDelta, type BookStructureRelationDelta } from "../src/book-structure-relations";
import type { BookStructureCandidate, BookStructureUnitCard } from "../src/book-structure";
import { collectStructureCandidates, acceptStructureChapterSelection } from "../src/book-structure-candidates";
import { materializeStructureChapterSelections } from "../src/book-structure-materialization";
import { compareThemeExperiments } from "../../../evals/book-structure/compare-themes";
import { fileURLToPath } from "node:url";

const read = (name: string) => JSON.parse(readFileSync(new URL(`../testdata/book-structure/${name}`, import.meta.url), "utf8"));
const baseline = read("baseline.json");
const chapter = read("chapter-8.input.json");
const theme = read("state-reuse.input.json");
const answers = read("acceptance.json");
const card = chapter.chapter.card as BookStructureUnitCard;
const local = baseline.local_contributions.find((c: BookStructureContribution) => c.payload.spine?.some(u => u.lid === "10"));
// A projection of one actual local contribution. No generated substitute output.
const chapterContribution: BookStructureContribution = {
  work_unit_id: local.work_unit_id, artifact_hash: "",
  unit_card_range: { start_ordinal: 0, end_ordinal_exclusive: 1 },
  payload: { spine: local.payload.spine.filter((u: { lid: string }) => u.lid === "10"),
    key_stops: local.payload.key_stops.filter((s: { lid: string }) => s.lid.startsWith("10.")), throughlines: [] },
};

describe("BSR0 source-backed baseline", () => {
  it("keeps chapter candidates, final stops and accepted-task estimates separate", () => {
    expect(baseline.counts).toEqual({ cards: 14, chapter_candidates: 138, tasks: 702, spine: 14,
      throughlines: 53, two_unit_throughlines: 49, key_stops: 34, dependencies: 1, reference_occurrences: 622 });
    expect(baseline.tasks).toHaveLength(702);
    const sum = (field: string) => baseline.tasks.reduce((n: number, t: { budget: Record<string, number> }) => n + t.budget[field], 0);
    expect(sum("estimated_rendered_tokens")).toBe(2817329);
    expect(sum("estimated_prompt_tokens")).toBe(507377);
    expect(sum("input_delivery_overhead_tokens")).toBe(655467);
    expect(new Set(baseline.tasks.map((t: { budget: { max_candidate_tokens: number } }) => t.budget.max_candidate_tokens))).toEqual(new Set([1024]));
    expect(baseline.chapter_number_suspects).toHaveLength(13);
    expect(baseline.actual_usage.tokens.input_tokens).toEqual({ known_subtotal: null, known_attempts: 0, unknown_attempts: 720 });
  });

  it("does not turn absent or partially known usage into zero actual cost", () => {
    const result = summarizeUsage([{ usage: {} }, { usage: { input_tokens: 120 } }, { usage: { input_tokens: 0, output_tokens: 30 } }]);
    expect(result.input_tokens).toEqual({ known_subtotal: 120, known_attempts: 2, unknown_attempts: 1 });
    expect(result.output_tokens).toEqual({ known_subtotal: 30, known_attempts: 1, unknown_attempts: 2 });
    expect(result.cached_input_tokens.known_subtotal).toBeNull();
  });

  it("uses Core source character spans for Chinese text and joins split inline formulas", () => {
    const source = "前言\n缓存 $K$ 可复用。";
    const nodes = [
      { lid: "10.1", kind: "paragraph", span: { start: 3, end: 6 }, children: [] },
      { lid: "10.2", kind: "formula", span: { start: 6, end: 9 }, children: [] },
      { lid: "10.3", kind: "paragraph", span: { start: 9, end: source.length }, children: [] },
    ];
    expect(sourceExcerpt(nodes, source, { from: "10.1", to: "10.3" }).text).toBe("缓存 $K$ 可复用。");
    expect(() => assertSourceExcerpt(nodes, source, { lid: "10.2", text: "$K$" })).not.toThrow();
    expect(() => assertSourceExcerpt(nodes, source, { lid: "10.2", text: "$V$" })).toThrow(/differs from canonical/);
    expect(() => sourceExcerpt(nodes, source, { from: "missing", to: "10.3" })).toThrow(/missing source/);
    expect(() => sourceExcerpt(nodes, source, { from: "10.3", to: "10.1" })).toThrow(/reversed/);
  });

  it("ships every answer anchor in the source packet, with labels kept in a separate file", () => {
    const chapterLids = new Set(chapter.evidence.flatMap((e: { lids: string[] }) => e.lids));
    const themeLids = new Set(theme.evidence.flatMap((e: { lids: string[] }) => e.lids));
    for (const requirement of answers.chapter.requirements) for (const lid of requirement.evidence) expect(chapterLids.has(lid), lid).toBe(true);
    for (const requirement of answers.theme.required_comparisons) for (const lid of requirement.evidence) expect(themeLids.has(lid), lid).toBe(true);
    for (const packet of [chapter, theme]) expect(JSON.stringify(packet)).not.toMatch(/"(?:expected|requirements|must_remain_distinct|negative_dependency|future_comparison)":/);
    expect(card.candidate_key_stops).toHaveLength(9);
    expect(chapter.recovered_candidates).toHaveLength(9);
    const continuous = chapter.recovered_candidates.find((c: { ref: string }) => c.ref.endsWith("#stop-10-5-continuous"));
    expect(continuous.stop.lid).toBe("10.9.3.2");
    expect(continuous.evidence[0].text).toContain("迭代边界");
    expect(card.candidate_key_stops.some(s => s.lid === continuous.stop.lid)).toBe(false);
  });

  it("locates loss before deterministic assembly: the accepted local payload contains only two chapter stops", () => {
    expect(chapterContribution.payload.key_stops).toHaveLength(2);
    const result = materializeBookStructureContributions([chapterContribution], ["10"]);
    expect(result.candidate.key_stops!.map(s => s.lid)).toEqual(chapter.published.key_stops.map((s: { lid: string }) => s.lid));
    expect(materializeBookStructureContributions([chapterContribution, chapterContribution], ["10"])).toEqual(result);
  });

  it("keeps the legacy delivery-only gate separate from the theme semantic judgment", () => {
    const entry = bookStructureRelationEntries(baseline.final).find(e => e.id === "unit:10");
    expect(entry!.name).toBe("未命名单元");
    const counterexample = dependencyCounterexample();
    expect(counterexample.accepted.add_dependencies).toHaveLength(1);
    expect(counterexample.result.candidate.spine!.find(u => u.lid === "13")!.depends_on).toContain("12");
    expect(counterexample.candidate.spine!.find(u => u.lid === "13")!.depends_on).not.toContain("12");
  });
});

describe("BSR0 minimal counterexamples resolved by their named slices", () => {
  it("BSR1: canonical titles keep relation entry 10 at chapter 8", () => {
    const input: BookStructureCandidate = { spine: baseline.final.spine.filter((u: { lid: string }) => ["1", "2", "10"].includes(u.lid)), key_stops: baseline.final.key_stops };
    const entry = bookStructureRelationEntries({ ...input, unit_titles: { "10": chapter.chapter.title } }).find(e => e.id === "unit:10");
    expect(entry!.name).toBe(chapter.chapter.title);
  });

  it("BSR1: chapter accepted references preserve all nine choices beyond the actual two-stop stitch", () => {
    const scope = { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": card.evidence_lids } };
    const catalog = collectStructureCandidates([{ contribution_ref: chapter.chapter.contribution_ref, unit_lid: "10", stops: card.candidate_key_stops, reference_scope: scope }]);
    const selection = acceptStructureChapterSelection({ unit_lid: "10", role: card.role, summary: card.summary,
      accepted_stop_refs: catalog.candidates.map(c => c.ref), macro_stop_refs: catalog.candidates.slice(0, 2).map(c => c.ref) }, catalog, "10", scope);
    const result = materializeStructureChapterSelections(catalog, [selection], ["10"]);
    expect(new Set(result.key_stops!.map(s => s.lid))).toEqual(new Set(card.candidate_key_stops.map(s => s.lid)));
  });

  it("same retained content survives regrouping; loss requires a changed model selection", () => {
    const make = (lid: string, rangeStart: number): BookStructureContribution => ({
      work_unit_id: `one:${lid}`, artifact_hash: "", unit_card_range: { start_ordinal: rangeStart, end_ordinal_exclusive: rangeStart + 1 },
      payload: { spine: baseline.final.spine.filter((u: { lid: string }) => u.lid === lid),
        key_stops: baseline.final.key_stops.filter((s: { lid: string }) => s.lid.startsWith(lid + ".")), throughlines: [] },
    });
    const separate = [make("9", 0), make("10", 1)];
    const joined: BookStructureContribution = { work_unit_id: "joined", artifact_hash: "", unit_card_range: { start_ordinal: 0, end_ordinal_exclusive: 2 },
      payload: { spine: separate.flatMap(c => c.payload.spine!), key_stops: separate.flatMap(c => c.payload.key_stops!), throughlines: [] } };
    const stopContent = (items: BookStructureContribution[]) => materializeBookStructureContributions(items, ["9", "10"]).candidate.key_stops!
      .map(({ id: _id, ...stop }) => stop).sort((a, b) => a.lid.localeCompare(b.lid));
    expect(stopContent(separate)).toEqual(stopContent([joined]));
  });

  it("BSR4: real B/C resolver outputs preserve six source groups without the unsupported training-to-Agent prerequisite", () => {
    const result = compareThemeExperiments(fileURLToPath(new URL("../../../docs/performance/book-structure-bsr4-20261001/", import.meta.url)));
    for (const group of result.groups) {
      expect(group.verification).toMatchObject({ delivery_equal: true, state_equal: true, materialization_equal: true });
      expect(group.report.complete).toBe(true);
      expect(group.unsupported_fixed_dependency_absent).toBe(true);
      expect(group.targeted_revision_preserved_other_themes).toBe(true);
      const fixed = group.report.themes.find(t => t?.ref === group.fixed_theme_ref)!;
      const evidence = fixed.stages.flatMap(s => s.development.evidence_lids);
      for (const lid of ["10.10.2.4", "10.10.3.17", "11.8.3.14"]) expect(evidence).toContain(lid);
      expect(group.fixed_theme_source_mapping).toHaveLength(6);
      for (const mechanism of group.fixed_theme_source_mapping) expect(mechanism.stages.length).toBeGreaterThan(0);
    }
  });
});

function dependencyCounterexample() {
    const negative = answers.theme.negative_dependency;
    const candidate: BookStructureCandidate = { spine: baseline.final.spine.filter((u: { lid: string }) => ["12", "13"].includes(u.lid)).map((u: object) => ({ ...u, key_stop_ids: [] })), key_stops: [], throughlines: [] };
    const entries = bookStructureRelationEntries(candidate);
    const delta: BookStructureRelationDelta = { new_throughlines: [], extend_throughlines: [], merge_throughlines: [],
      add_dependencies: [{ unit_lid: negative.unit_lid, depends_on: negative.depends_on, evidence_lids: negative.evidence_lids }] };
    // The existing gate correctly checks delivery, not the semantic truth of a model proposal.
    const accepted = validateBookStructureRelationDelta(delta, { version: "book_structure_relation_input.v1", work_unit_id: "counterexample",
      entries, reference_scope: { unit_lids: ["12", "13"], dependency_target_lids: ["12", "13"], evidence_by_unit: { "12": ["12.12.4.3"], "13": ["13.13.2.15"] } } });
    expect(bookStructureSelectedPairs([{ groups: [{ member_ids: entries.map(e => e.id) }] }])).toHaveLength(1);
    const result = applyBookStructureRelationDeltas(candidate, [{ work_unit_id: "counterexample", delta: accepted }]);
    return { candidate, accepted, result };
}
