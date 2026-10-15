import { describe, expect, it } from "vitest";
import { planStructureThemes, structureThemePlanningInput, structureThemeInput, applyStructureThemeAction, reconcileStructureThemes, materializeStructureThemes, reopenStructureTheme,
  type StructureThemeContext, type StructureThemeResult } from "../src/book-structure-themes";
import { collectStructureCandidates } from "../src/book-structure-candidates";
import type { BookStructureCandidate } from "../src/book-structure";
import { estimateTokens } from "../src/window";
import { readFileSync } from "node:fs";
import { renderStructureOrganizationInput, type StructureOrganizationInput } from "../src/book-structure-organization";
import { STRUCTURE_ORGANIZATION_PROMPTS } from "../src/book-structure-organization-prompts";

function fixture(count = 11) {
  const chapters = Array.from({ length: count }, (_, i) => ({ unit_lid: String(i + 1), title: `章${i + 1}`,
    overview: { text: `机制${i + 1}`, evidence_lids: [`${i + 1}.1`] } }));
  const catalog = collectStructureCandidates(chapters.map(c => ({ contribution_ref: `unit:${c.unit_lid}`, unit_lid: c.unit_lid,
    stops: [{ id: "s", lid: `${c.unit_lid}.1`, type: "claim", reason: c.overview }],
    reference_scope: { unit_lids: [c.unit_lid], dependency_target_lids: [], evidence_by_unit: { [c.unit_lid]: c.overview.evidence_lids } } })));
  const context: StructureThemeContext = { catalog, chapters, excerpts: chapters.map(c => ({ unit_lid: c.unit_lid, lids: c.overview.evidence_lids, text: c.overview.text })), mode: "lexical_only" };
  const plan = { themes: [{ ref: "theme:one", question: "状态机制如何发展", distinction: "机制与条件，不推断同一对象", unit_lids: chapters.map(c => c.unit_lid), member_refs: catalog.candidates.map(c => c.ref) }] };
  const directory = planStructureThemes(plan, context);
  const result: StructureThemeResult = { ref: "theme:one", name: "状态发展", summary: { text: "从表示到使用", evidence_lids: chapters.slice(0, 2).flatMap(c => c.overview.evidence_lids) },
    stages: chapters.slice(0, 2).map(c => ({ unit_lid: c.unit_lid, development: c.overview, member_refs: [catalog.candidates.find(s => s.unit_lid === c.unit_lid)!.ref] })), dependencies: [] };
  const base: BookStructureCandidate = { spine: chapters.map(c => ({ lid: c.unit_lid, role: "method", summary: c.overview, key_stop_ids: [], depends_on: [] })), key_stops: [], throughlines: [] };
  return { context, directory, plan, result, base };
}
describe("BSR4 theme worksets and grounded dependencies", () => {
  it("keeps thirteen accepted units and all 296 seeds reachable within the plan budget", () => {
    const delivery = JSON.parse(readFileSync(new URL("../testdata/book-structure/theme-plan-capacity-prefix-13.json", import.meta.url), "utf8"));
    expect(estimateTokens(STRUCTURE_ORGANIZATION_PROMPTS.structure_theme_plan + JSON.stringify(delivery) + "\n")).toBeGreaterThan(20000);
    const context = fixture(2).context;
    context.chapters = delivery.input.chapters.map((c: { unit_lid: string; title: string; overview: string; question: string }) =>
      ({ ...c, overview: { text: c.overview, evidence_lids: [] } }));
    context.catalog.candidates = delivery.input.candidate_index.map((c: { ref: string; unit_lid: string; meaning: string }) =>
      ({ ...context.catalog.candidates[0], ...c }));
    context.planning_candidate_refs = context.catalog.candidates.map(c => c.ref);
    const original = structuredClone(context);
    const body = structureThemePlanningInput(context);
    const packet = { phase: "plan", context, body, reference_scope: delivery.reference_scope } as StructureOrganizationInput;
    expect(estimateTokens(STRUCTURE_ORGANIZATION_PROMPTS.structure_theme_plan + renderStructureOrganizationInput(packet))).toBeLessThanOrEqual(20000);
    expect(body.chapters).toEqual(delivery.input.chapters);
    expect(body.candidate_index.map(c => ({ ref: c.ref, unit_lid: c.unit_lid })))
      .toEqual(delivery.input.candidate_index.map((c: { ref: string; unit_lid: string }) => ({ ref: c.ref, unit_lid: c.unit_lid })));
    expect(body.candidate_index).toHaveLength(296);
    expect(context).toEqual(original);
  });
  it("delivers the accepted twelve-chapter plan within capacity without dropping any content", () => {
    const delivery = JSON.parse(readFileSync(new URL("../testdata/book-structure/theme-plan-capacity-prefix-12.json", import.meta.url), "utf8"));
    const packet = { phase: "plan", body: delivery.input, reference_scope: delivery.reference_scope,
      context: { planning_candidate_refs: delivery.input.candidate_index.map((c: { ref: string }) => c.ref) } } as StructureOrganizationInput;
    const pretty = JSON.stringify(delivery, null, 2) + "\n";
    expect(estimateTokens(STRUCTURE_ORGANIZATION_PROMPTS.structure_theme_plan + pretty)).toBeGreaterThan(20000);
    const rendered = renderStructureOrganizationInput(packet);
    expect(estimateTokens(STRUCTURE_ORGANIZATION_PROMPTS.structure_theme_plan + rendered)).toBeLessThanOrEqual(20000);
    expect(JSON.parse(rendered)).toEqual(delivery);
    expect(delivery.input.candidate_index).toHaveLength(259);
    const legacy = { ...packet, context: { ...packet.context, planning_candidate_refs: undefined } } as StructureOrganizationInput;
    expect(renderStructureOrganizationInput(legacy)).toBe(pretty);
  });
  it("keeps planning navigation bounded without repeating chapter citation ledgers", () => {
    const { context } = fixture(8);
    const counts = [0, 7, 11, 17, 17, 20, 21, 23];
    context.chapters.forEach((chapter, i) => {
      chapter.overview = { text: "本章解释容量、带宽与并行放置之间的条件和取舍。".repeat(7),
        evidence_lids: Array.from({ length: 120 }, (_, j) => `${i + 1}.${j + 1}.1`) };
      chapter.question = "容量和通信条件如何改变并行选择？";
    });
    context.catalog.candidates = context.chapters.flatMap((chapter, i) => Array.from({ length: counts[i] }, (_, j) => ({
      ...context.catalog.candidates[i], ref: `unit:${chapter.unit_lid}:fragment:0049#f50-${j}`,
      meaning: "扩大共享通信域使每卡专家权重减少，释放显存容纳更多会话，但继续扩大时状态读取、计算和通信同步增长，容量收益饱和。".repeat(3),
      conditions: ["注意力数据并行，专家并行均分，固定总卡数，每个实例占用一个超节点。".repeat(3)],
    })));
    context.planning_candidate_refs = context.catalog.candidates.map(c => c.ref);
    const legacy = structureThemePlanningInput({ ...context, planning_candidate_refs: undefined });
    expect(estimateTokens(JSON.stringify(legacy, null, 2))).toBeGreaterThan(20000);
    const body = structureThemePlanningInput(context);
    expect(estimateTokens(JSON.stringify(body, null, 2))).toBeLessThan(16000);
    expect(body.candidate_index.map(c => c.ref)).toEqual(context.planning_candidate_refs);
    expect(body.chapters.map(c => c.overview)).toEqual(context.chapters.map(c => c.overview.text));
    expect(body.chapters.map(c => c.question)).toEqual(context.chapters.map(c => c.question));
    expect(context.chapters[0].overview.evidence_lids).toHaveLength(120);
    expect(context.catalog.candidates.at(-1)!.conditions[0]).toContain("固定总卡数");
    expect(legacy.chapters).toEqual(context.chapters);
  });
  it("pages a full-book source directory without granting evidence and keeps original read indices reachable", () => {
    const { context, directory } = fixture(2);
    context.source_index_page_size = 64;
    context.excerpts = Array.from({ length: 9000 }, (_, i) => ({ unit_lid: i < 4500 ? "1" : "2", lids: [`${i < 4500 ? 1 : 2}.${i + 1}`], text: `Body ${i}` }));
    let d = directory;
    let input = structureThemeInput(d, "theme:one", context);
    expect(estimateTokens(JSON.stringify(input))).toBeLessThan(20000);
    expect(input.source_index).toHaveLength(64);
    expect(input.source_page).toEqual({ offset: 0, total: 9000, next_offset: 64 });
    d = applyStructureThemeAction(d, "theme:one", { kind: "source_browse", offset: 8960 }, context);
    input = structureThemeInput(d, "theme:one", context);
    expect(input.source_page!.next_offset).toBeNull();
    expect(input.source_index.at(-1)).toMatchObject({ index: 8999 });
    expect(d.works[0].evidence_by_unit).toEqual({});
    expect(() => applyStructureThemeAction(d, "theme:one", { kind: "source_browse", offset: 9000 }, context)).toThrow(/source.*offset/);
    d = applyStructureThemeAction(d, "theme:one", { kind: "read", indices: [8999] }, context);
    expect(structureThemeInput(d, "theme:one", context).excerpts).toEqual([{ index: 8999, ...context.excerpts[8999] }]);
    d = applyStructureThemeAction(d, "theme:one", { kind: "search", query: "", notes: "read last source" }, context);
    expect(d.works[0].evidence_by_unit["2"]).toEqual(["2.9000"]);
  });
  it("uses accepted macro-route refs as planning seeds while keeping all other candidates in retrieval", () => {
    const { context, directory } = fixture(2);
    const seed = context.catalog.candidates[0], other = context.catalog.candidates[1];
    context.planning_candidate_refs = [seed.ref];
    expect(structureThemePlanningInput(context)).toMatchObject({ candidate_index_scope: "chapter_macro_route_seeds", total_candidates: 2,
      candidate_index: [{ ref: seed.ref }] });
    expect(structureThemePlanningInput(context).candidate_index).toHaveLength(1);
    const input = structureThemeInput(directory, "theme:one", context);
    expect(input.search.items.some(c => c.key === other.ref)).toBe(true);
  });
  it("routes 11 chapters as one theme, supplies the global directory and preserves identity across search pages", () => {
    const { context, directory } = fixture();
    expect(directory.works).toHaveLength(1);
    const first = structureThemeInput(directory, "theme:one", context);
    expect(first.directory).toHaveLength(1); expect(first.search.next_offset).toBe(6);
    const next = applyStructureThemeAction(directory, "theme:one", { kind: "search", query: "", offset: 6 }, context);
    expect(next.works[0].plan).toEqual(directory.works[0].plan);
    expect(structureThemeInput(next, "theme:one", context).search.offset).toBe(6);
    expect(next.works).toHaveLength(1);
  });
  it("never grants evidence from plans/previews and requires inspection for member references", () => {
    const { context, directory, result } = fixture(2);
    expect(() => applyStructureThemeAction(directory, result.ref, { kind: "resolve", result }, context)).toThrow(/undelivered/);
    let next = applyStructureThemeAction(directory, result.ref, { kind: "read", indices: [0, 1] }, context);
    expect(() => applyStructureThemeAction(next, result.ref, { kind: "resolve", result }, context)).toThrow(/inspected/);
    next = applyStructureThemeAction(next, result.ref, { kind: "inspect", refs: result.stages.flatMap(s => s.member_refs) }, context);
    next = applyStructureThemeAction(next, result.ref, { kind: "resolve", result }, context);
    expect(next.works[0].result).toEqual(result);
    expect(directory.works[0].evidence_by_unit).toEqual({});
  });
  it("accepts explained prerequisites only with both endpoints, without deriving dependencies from membership", () => {
    const { context, directory, result, base } = fixture(2);
    const inspected = applyStructureThemeAction(directory, result.ref, { kind: "inspect", refs: result.stages.flatMap(s => s.member_refs) }, context);
    const dependency = { unit_lid: "2", depends_on: "1", prerequisite: result.stages[0].development,
      application: result.stages[1].development, rationale: "第二章的使用步骤要求先理解第一章定义" };
    const bad = { ...result, dependencies: [{ ...dependency, application: dependency.prerequisite }] };
    expect(() => applyStructureThemeAction(inspected, result.ref, { kind: "resolve", result: bad }, context)).toThrow(/foreign/);
    const resolve = (r: StructureThemeResult) => reconcileStructureThemes(applyStructureThemeAction(inspected, r.ref, { kind: "resolve", result: r }, context), { edits: [], merges: [], unresolved: [] }, context);
    expect(materializeStructureThemes(base, resolve(result), context).spine![1].depends_on).toEqual([]);
    const done = resolve({ ...result, dependencies: [dependency] });
    const output = materializeStructureThemes(base, done, context);
    expect(output.spine![1].depends_on).toEqual(["1"]);
    expect(output.key_stops).toHaveLength(2); expect(output.spine![0].key_stop_ids).toEqual([]);
    expect(materializeStructureThemes(output, done, context)).toEqual(output);
    expect(() => reconcileStructureThemes(done, { edits: [], merges: [], unresolved: [] }, context)).toThrow(/already/);
  });
  it("merges a duplicate theme once while retaining selected mechanisms and refusing unsupported edits", () => {
    const { context, plan, result, base } = fixture(2);
    let d = planStructureThemes({ themes: [plan.themes[0], { ...plan.themes[0], ref: "theme:two" }] }, context);
    for (const ref of ["theme:one", "theme:two"]) {
      d = applyStructureThemeAction(d, ref, { kind: "inspect", refs: result.stages.flatMap(s => s.member_refs) }, context);
      d = applyStructureThemeAction(d, ref, { kind: "resolve", result: { ...result, ref } }, context);
    }
    const bad = structuredClone(result); bad.stages[0].development.evidence_lids = ["1.999"];
    expect(() => reconcileStructureThemes(d, { edits: [bad], merges: [], unresolved: [] }, context)).toThrow(/undelivered/);
    const merged = reconcileStructureThemes(d, { edits: [], merges: [{ source_refs: ["theme:one", "theme:two"], result }], unresolved: [] }, context);
    expect(merged.works).toHaveLength(1); expect(merged.redirects).toEqual({ "theme:two": "theme:one" });
    expect(materializeStructureThemes(base, merged, context).throughlines).toHaveLength(1);
    const incomplete = reconcileStructureThemes(d, { edits: [], merges: [], unresolved: ["需要补读机制边界"] }, context);
    expect(() => materializeStructureThemes(base, incomplete, context)).toThrow(/incomplete/);
  });
  it("does not silently degrade missing semantic preparation to lexical results", () => {
    const { context, directory } = fixture(2);
    expect(() => structureThemeInput(directory, "theme:one", { ...context, mode: "semantic_required" })).toThrow(/requires preparation/);
  });
  it("reopens one concrete source-review issue and retains the other accepted themes without rescanning", () => {
    const { context, plan, result } = fixture(2);
    let d = planStructureThemes({ themes: [plan.themes[0], { ...plan.themes[0], ref: "theme:two" }] }, context);
    for (const ref of ["theme:one", "theme:two"]) {
      d = applyStructureThemeAction(d, ref, { kind: "inspect", refs: result.stages.flatMap(s => s.member_refs) }, context);
      d = applyStructureThemeAction(d, ref, { kind: "resolve", result: { ...result, ref } }, context);
    }
    d = reconcileStructureThemes(d, { edits: [], merges: [], unresolved: ["补充容量条件"] }, context);
    const prior = structuredClone(d);
    d = reopenStructureTheme(d, result.ref, "补充容量条件");
    expect(d.works[1]).toEqual(prior.works[1]); expect(d.works[0].result).toBeUndefined();
    expect(structureThemeInput(d, result.ref, context).revision?.previous_result).toEqual(result);
    expect(() => applyStructureThemeAction(d, result.ref, { kind: "resolve", result: { ...result, name: "不同问题" } }, context)).toThrow(/preserve/);
    d = applyStructureThemeAction(d, result.ref, { kind: "resolve", result }, context);
    expect(d.reconciled).toBe(true); expect(d.unresolved).toEqual([]); expect(d.revision).toBeUndefined();
    expect(d.works[1]).toEqual(prior.works[1]); expect(prior.works[0].result).toEqual(result);
  });
});
