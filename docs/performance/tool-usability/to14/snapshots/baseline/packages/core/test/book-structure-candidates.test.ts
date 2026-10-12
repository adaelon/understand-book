import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { collectStructureCandidates, acceptStructureChapterSelection, type StructureCandidateContribution } from "../src/book-structure-candidates";
import { materializeStructureChapterSelections } from "../src/book-structure-materialization";
import { bookStructureCanonicalTitle, buildBookStructureUnitSources } from "../src/book-structure";
import type { LidNode } from "../src/generated/LidNode";

const chapter = JSON.parse(readFileSync(new URL("../testdata/book-structure/chapter-8.input.json", import.meta.url), "utf8"));
export const contributions: StructureCandidateContribution[] = [
  { contribution_ref: chapter.chapter.contribution_ref, unit_lid: "10", stops: chapter.chapter.card.candidate_key_stops,
    reference_scope: { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": chapter.chapter.card.evidence_lids } } },
  ...chapter.recovered_candidates.map((item: any) => ({ contribution_ref: item.contribution_ref, unit_lid: "10", stops: [item.stop],
    reference_scope: { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": item.evidence.flatMap((e: any) => e.lids) } } })),
];

describe("BSR1 persistent candidates and reference assembly", () => {
  it("retains all 18 candidates including continuous scheduling, independent of contribution order", () => {
    const catalog = collectStructureCandidates(contributions, ["10.9", "10.9.3"]);
    expect(catalog.candidates).toHaveLength(18);
    expect(catalog).toEqual(collectStructureCandidates([...contributions].reverse(), ["10.9", "10.9.3"]));
    const continuous = catalog.candidates.find(c => c.lid === "10.9.3.2")!;
    expect(continuous.section_lid).toBe("10.9.3");
    expect(continuous.contribution_ref).toBe("unit:10:fragment:0005");
    expect(continuous.meaning).toContain("连续批处理");
    expect(continuous.reason.evidence_lids).toEqual(["10.9.3.2"]);
  });
  it("keeps distinct meanings on the same LID and only collapses explicit original references", () => {
    const source = structuredClone(contributions[0]);
    source.stops = [source.stops[0]];
    source.stops[0].meaning = "容量约束"; source.stops[0].conditions = ["单卡、状态同时驻留"];
    const original = collectStructureCandidates([source]);
    const duplicate = structuredClone(source); duplicate.contribution_ref = "reduce:later";
    duplicate.stops[0].original_ref = original.candidates[0].ref;
    expect(collectStructureCandidates([duplicate, source, source])).toEqual(original);
    delete duplicate.stops[0].original_ref;
    duplicate.stops[0].meaning = "准入判断";
    expect(collectStructureCandidates([source, duplicate]).candidates).toHaveLength(2);
    duplicate.stops[0].original_ref = original.candidates[0].ref;
    expect(() => collectStructureCandidates([source, duplicate])).toThrow(/changed original/);
    expect(() => collectStructureCandidates([duplicate])).toThrow(/unknown original/);
    const conflict = structuredClone(source); conflict.stops[0].meaning = "changed";
    expect(() => collectStructureCandidates([source, conflict])).toThrow(/conflicting/);
  });
  it("carries all chapter choices while a two-stop macro route leaves the catalog intact", () => {
    const catalog = collectStructureCandidates(contributions);
    const refs = catalog.candidates.map(c => c.ref);
    const selected = acceptStructureChapterSelection({ unit_lid: "10", role: "application", summary: chapter.chapter.card.summary,
      accepted_stop_refs: refs, macro_stop_refs: refs.slice(0, 2) }, catalog, "10", contributions[0].reference_scope);
    const result = materializeStructureChapterSelections(catalog, [selected], ["10"], { "10": chapter.chapter.title });
    expect(result.key_stops).toHaveLength(18);
    expect(result.spine![0].key_stop_ids).toHaveLength(2);
    expect(materializeStructureChapterSelections(catalog, [selected, selected], ["10"], { "10": chapter.chapter.title })).toEqual(result);
    expect(catalog.candidates).toHaveLength(18);
    expect(result.reference_scope!.evidence_by_unit["10"]).toContain("10.9.3.7");
    const another = { ...selected, selection: { ...selected.selection, accepted_stop_refs: refs.slice(1), macro_stop_refs: [] } };
    expect(() => materializeStructureChapterSelections(catalog, [selected, another], ["10"])).toThrow(/conflicting/);
  });
  it("rejects unknown, foreign, duplicate and out-of-scope references", () => {
    const catalog = collectStructureCandidates(contributions);
    const value = { unit_lid: "10", role: "application", summary: chapter.chapter.card.summary,
      accepted_stop_refs: [catalog.candidates[0].ref], macro_stop_refs: [] };
    const accept = (v: unknown) => acceptStructureChapterSelection(v, catalog, "10", contributions[0].reference_scope);
    expect(() => accept({ ...value, accepted_stop_refs: ["missing"] })).toThrow(/unknown or foreign/);
    expect(() => accept({ ...value, accepted_stop_refs: [value.accepted_stop_refs[0], value.accepted_stop_refs[0]] })).toThrow(/duplicate/);
    expect(() => accept({ ...value, macro_stop_refs: [catalog.candidates[1].ref] })).toThrow(/subset/);
    expect(() => accept({ ...value, summary: { text: "new", evidence_lids: ["11.1"] } })).toThrow(/undelivered/);
    const bad = structuredClone(contributions[0]); bad.reference_scope.evidence_by_unit["10"] = [];
    expect(() => collectStructureCandidates([bad])).toThrow(/outside accepted/);
  });
  it("produces identical content and evidence for reordered or regrouped multi-chapter selections", () => {
    const source = contributions[0];
    const second: StructureCandidateContribution = { contribution_ref: "unit:11:fragment:0", unit_lid: "11",
      stops: [{ id: "different-mechanism", lid: "11.2", type: "claim", meaning: "交接状态", conditions: ["传输完成后读取"],
        reason: { text: "说明完成标记", evidence_lids: ["11.2"] } }],
      reference_scope: { unit_lids: ["11"], dependency_target_lids: [], evidence_by_unit: { "11": ["11.2"] } } };
    const catalog = collectStructureCandidates([source, second]);
    const selections = [source, second].map(s => acceptStructureChapterSelection({ unit_lid: s.unit_lid, role: "application",
      summary: s.stops[0].reason, accepted_stop_refs: catalog.candidates.filter(c => c.unit_lid === s.unit_lid).map(c => c.ref), macro_stop_refs: [] }, catalog, s.unit_lid, s.reference_scope));
    const grouped = [[selections[1]], [selections[0], selections[0]]].flat();
    const result = materializeStructureChapterSelections(catalog, selections, ["10", "11"]);
    expect(materializeStructureChapterSelections(catalog, grouped, ["10", "11"])).toEqual(result);
    expect(result.key_stops!.find(s => s.lid === "11.2")!.reason.text).toContain("传输完成后读取");
    const wrongChapter = { ...selections[0].selection, accepted_stop_refs: [catalog.candidates.find(c => c.unit_lid === "11")!.ref] };
    expect(() => acceptStructureChapterSelection(wrongChapter, catalog, "10", source.reference_scope)).toThrow(/foreign/);
  });
  it("extracts an empty-title chapter from its canonical heading without renumbering it", () => {
    const source = "# 封面\n# 前言\n# 第 8 章 推理优化\n正文";
    const nodes: LidNode[] = [
      { lid: "1", path: [1], kind: "chapter", span: { start: 0, end: 5 }, children: ["1.1"] },
      { lid: "1.1", path: [1, 1], kind: "paragraph", span: { start: 0, end: 4 }, children: [] },
      { lid: "2", path: [2], kind: "chapter", span: { start: 5, end: 10 }, children: ["2.1"] },
      { lid: "2.1", path: [2, 1], kind: "paragraph", span: { start: 5, end: 9 }, children: [] },
      { lid: "10", path: [10], kind: "chapter", span: { start: 10, end: source.length }, children: ["10.1", "10.2"] },
      { lid: "10.1", path: [10, 1], kind: "paragraph", span: { start: 10, end: source.indexOf("\n正文") }, children: [] },
      { lid: "10.2", path: [10, 2], kind: "paragraph", span: { start: source.indexOf("正文"), end: source.length }, children: [] },
    ];
    const units = buildBookStructureUnitSources({ lidNodes: nodes, source });
    expect(units.map(u => [u.unit_lid, u.title])).toEqual([["1", "封面"], ["2", "前言"], ["10", "第 8 章 推理优化"]]);
    expect(bookStructureCanonicalTitle(nodes[4], new Map(nodes.map(n => [n.lid, n])), source)).toBe(chapter.chapter.title);
  });
});
