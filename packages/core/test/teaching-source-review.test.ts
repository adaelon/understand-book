import { describe, expect, it } from "vitest";
import { newTeachingReviewWork, teachingReviewInput, advanceTeachingReview, reviewSource, type TeachingSample } from "../src/teaching-source-review";
import { teachingSamples } from "../src/teaching-build";
import { acceptFormalObjects } from "../src/teaching-map";
import { source, binding, proposal } from "./fixtures/teaching-source";
import type { CognitiveMaterials } from "../src/cognitive-materials";
describe("bounded independent teaching source review", () => {
  const sample: TeachingSample = { sample_id: "object:speed:1", content: { meaning: "速率是总路程除以总时间" }, evidence_lids: ["1.1"], evidence_bindings: [binding("1.1")] };
  it("requires all cited ranges before deciding the whole sample and persists a tail contradiction", () => {
    const long = structuredClone(source);
    long.passages[0].text = "重复背景。".repeat(2000) + "末尾更正：这不是速率的定义。";
    let work = newTeachingReviewWork(long, sample);
    const finish = { kind: "finish", review: { samples: [{ sample_id: sample.sample_id, verdict: "fail", reason: "末尾更正否定该定义", source_bindings: [binding("1.1")] }] } };
    expect(() => advanceTeachingReview(long, sample, work, finish)).toThrow("coverage incomplete");
    while (teachingReviewInput(long, sample, work).next_range) {
      const range = teachingReviewInput(long, sample, work).next_range!;
      work = advanceTeachingReview(long, sample, work, { kind: "read", ...range });
      work = JSON.parse(JSON.stringify(work));
    }
    work = advanceTeachingReview(long, sample, work, { kind: "retain", notes: "末尾存在直接更正，拒绝样本" });
    expect(teachingReviewInput(long, sample, work).readings[0].text).toContain("末尾更正");
    expect(advanceTeachingReview(long, sample, work, finish).review!.samples[0].verdict).toBe("fail");
  });
  it("reviews actual cited ranges without expanding an unrelated unit", () => {
    const ranged = { ...sample, evidence_bindings: [{ ...binding("1.1"), range_utf16: { start: 0, end: 8 } }] };
    const original = reviewSource(source, ranged);
    expect(original.passages).toHaveLength(1);
    expect(original.passages[0].text).toBe(source.passages[0].text.slice(0, 8));
    const work = advanceTeachingReview(source, ranged, newTeachingReviewWork(source, ranged), { kind: "read", lid: "1.1", start: 0, end: 8 });
    expect(teachingReviewInput(source, ranged, work).remaining_ranges).toBe(0);
  });
  it("covers connection/pattern evidence and empty units alongside populated units", () => {
    const emptyUnit = structuredClone(proposal);
    emptyUnit.coverage[1].object_keys = [];
    const objects = acceptFormalObjects({ source, proposal: emptyUnit, operation_id: "review" });
    const materials: CognitiveMaterials = { version: "cognitive_materials.v1", source_id: source.source_id, source_revision: source.source_revision,
      formal_objects_revision: objects.revision, coverage: [], materials: [{ target_id: "test", kind: "definition", object_refs: [objects.active_refs[0]], purpose: "test",
        steps: [{ id: "a", content: "定义", source_bindings: [binding("1.1")] }], connections: [{ from: "a", to: "a", meaning: "连接", source_bindings: [binding("2.1")] }],
        patterns: [{ name: "测量", organization: "方法", source_bindings: [binding("2.2")] }], gaps: [], conditions: [] }] };
    const samples = teachingSamples(objects, materials);
    expect(samples.find(s => s.sample_id === "material:test")!.evidence_lids).toEqual(["1.1", "2.1", "2.2"]);
    expect(samples.some(s => s.sample_id === "coverage:2")).toBe(true);
    expect(samples.find(s => s.sample_id.startsWith(`object:${objects.active_refs[0].object_id}:`))!.evidence_lids).toContain("1.2");
  });
});
