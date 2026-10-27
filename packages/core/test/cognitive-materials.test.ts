import { describe, expect, it } from "vitest";
import { acceptFormalObjects } from "../src/teaching-map";
import { advanceCognitiveWork, newCognitiveWork, collectCognitiveMaterials, cognitiveTaskInput, prepareCognitiveTaskInput, type CognitiveMaterial, type CognitiveWork } from "../src/cognitive-materials";
import { source, proposal, binding, targets } from "./fixtures/teaching-source";

const objects = acceptFormalObjects({ source, proposal, operation_id: "objects" });
const budget = { searches: 6, preview_chars: 2000, read_chars: 4000, context_chars: 4000 };
export function materialFor(index: number): CognitiveMaterial {
  return { target_id: targets[index].id, kind: index === 1 ? "method" : "reasoning_episode", purpose: targets[index].reason.text,
    object_refs: [objects.active_refs[0]], steps: [{ id: "step1", content: source.passages.find(p => p.lid === targets[index].lid)!.text, source_bindings: [binding(targets[index].lid)] }], connections: [], conditions: [], gaps: [], patterns: [] };
}
describe("source-grounded cognitive construction", () => {
  it("delivers the required target before the first action without changing the unissued ledger", () => {
    const initial = newCognitiveWork(source, targets[1]);
    const prepared = prepareCognitiveTaskInput(source, targets[1], objects, initial, budget);
    const original = source.passages.find(p => p.lid === targets[1].lid)!.text;
    expect(initial).toMatchObject({ readings: [], used: { read_chars: 0 } });
    expect(prepared.packet.work.readings).toEqual([{ lid: "2.2", start: 0, end: original.length, text: original }]);
    expect(prepared.packet.work.read_ranges).toEqual([{ lid: "2.2", start: 0, end: original.length }]);
    expect(prepared.work.used.read_chars).toBe(original.length);
    expect(advance(prepared.work, { kind: "finish", material: materialFor(1) }, 1).status).toBe("complete");
    const recalled = advance(prepared.work, { kind: "read", lid: "2.2" }, 1);
    expect(recalled.used).toEqual(prepared.work.used);
    const resumed = JSON.parse(JSON.stringify(prepared.work)) as CognitiveWork;
    expect(prepareCognitiveTaskInput(source, targets[1], objects, resumed, budget).work).toBe(resumed);
  });
  it("prefetches at most 2000 UTF-16 units and still rejects unseen parts of a long target", () => {
    const large = structuredClone(source);
    large.passages.find(p => p.lid === "2.1")!.text = "等距离条件。".repeat(500);
    const prepared = prepareCognitiveTaskInput(large, targets[0], objects, newCognitiveWork(large, targets[0]), budget);
    expect(prepared.packet.work.read_ranges).toEqual([{ lid: "2.1", start: 0, end: 2000 }]);
    expect(prepared.work.used.read_chars).toBe(2000);
    const finish = (material: CognitiveMaterial) => advanceCognitiveWork({ source: large, target: targets[0], objects,
      work: prepared.work, budget, discourse: [], action: { kind: "finish", material } });
    const material = materialFor(0);
    expect(() => finish(material)).toThrow("unread source range");
    material.steps[0].source_bindings[0].range_utf16 = { start: 0, end: 2000 };
    expect(finish(material).status).toBe("complete");
    material.steps[0].source_bindings[0].range_utf16 = { start: 1999, end: 2001 };
    expect(() => finish(material)).toThrow("unread source range");
  });
  it("charges only source text that survives context, reading and complete packet limits", () => {
    for (const limits of [{ ...budget, context_chars: 17 }, { ...budget, read_chars: 11 }]) {
      const prepared = prepareCognitiveTaskInput(source, targets[0], objects, newCognitiveWork(source, targets[0]), limits);
      const expected = Math.min(limits.context_chars, limits.read_chars);
      expect(prepared.work.used.read_chars).toBe(expected);
      expect(prepared.packet.work.readings[0].text.length).toBe(expected);
      expect(prepared.packet.work.read_ranges).toEqual([{ lid: "2.1", start: 0, end: expected }]);
    }
    const initial = newCognitiveWork(source, targets[0]);
    const fits = (body: unknown) => (body as ReturnType<typeof cognitiveTaskInput>).work.readings.every(r => r.text.length <= 23);
    const prepared = prepareCognitiveTaskInput(source, targets[0], objects, initial, budget, fits);
    expect(prepared.work.used.read_chars).toBe(23);
    expect(prepared.packet.work.read_ranges).toEqual([{ lid: "2.1", start: 0, end: 23 }]);
    const omitted = prepareCognitiveTaskInput(source, targets[0], objects, initial, budget,
      body => !(body as ReturnType<typeof cognitiveTaskInput>).work.readings.length);
    expect(omitted.work).toBe(initial);
    expect(omitted.packet.work.readings).toEqual([]);
    expect(omitted.packet.work.read_ranges).toEqual([]);
    expect(omitted.work.used.read_chars).toBe(0);
    const interrupted = { ...initial, stop_reason: "budget_exhausted" as const };
    expect(prepareCognitiveTaskInput(source, targets[0], objects, interrupted, budget).work).toBe(interrupted);
  });
  it("reads long LIDs by range, rejects overclaiming and recalls persisted ranges without charging twice", () => {
    const large = structuredClone(source);
    large.passages.find(p => p.lid === "2.1")!.text = "等距离速率条件。".repeat(2500) + "必要定义在末尾。";
    const limits = { ...budget, read_chars: 30000, context_chars: 2000 };
    const step = (work: CognitiveWork, action: unknown) => advanceCognitiveWork({ work, action, source: large, target: targets[0], objects, discourse: [], budget: limits });
    let work = step(newCognitiveWork(large, targets[0]), { kind: "read", lid: "2.1" });
    expect(work.readings[0]).toMatchObject({ start: 0, end: 2000 });
    work = step(work, { kind: "search", query: "必要定义", offset: 0, limit: 6 });
    expect(work.searches[0].ranges![0].start).toBeGreaterThan(18000);
    expect(work.searches[0].summaries[0]).toContain("必要定义");
    expect(() => step(work, { kind: "finish", material: materialFor(0) })).toThrow("unread source range");
    for (let start = 2000; start < 18000; start += 2000) work = step(work, { kind: "read", lid: "2.1", start, end: start + 2000 });
    const text = large.passages.find(p => p.lid === "2.1")!.text;
    work = step(work, { kind: "read", lid: "2.1", start: text.length - 8, end: text.length });
    const used = work.used.read_chars;
    work = step(JSON.parse(JSON.stringify(work)), { kind: "read", lid: "2.1", start: 0, end: 2000 });
    expect(work.used.read_chars).toBe(used);
    expect(work.readings.at(-1)!.start).toBe(0);
    const packet = cognitiveTaskInput(large, targets[0], objects, work, limits);
    expect(packet.work.readings.reduce((sum, r) => sum + r.text.length, 0)).toBeLessThanOrEqual(2000);
    expect(packet.work.read_ranges).toHaveLength(10);
    const material = materialFor(0);
    material.steps[0].source_bindings = [{ ...binding("2.1"), range_utf16: { start: text.length - 8, end: text.length } }];
    expect(step(work, { kind: "finish", material }).status).toBe("complete");
    material.steps[0].source_bindings[0].range_utf16 = { start: 18000, end: 18010 };
    expect(() => step(work, { kind: "finish", material })).toThrow("unread source range");
  });
  it("pages the whole object catalog, exposes inspected records and preserves working notes", () => {
    let work = newCognitiveWork(source, targets[0]);
    work = advance(work, { kind: "search_objects", query: "", offset: 0 });
    expect(work.object_searches![0].refs).toHaveLength(6);
    expect(cognitiveTaskInput(source, targets[0], objects, work, budget).objects).toHaveLength(6);
    work = advance(work, { kind: "search_objects", query: "", offset: 6 });
    expect(work.object_searches!.at(-1)!.exhausted).toBe(true);
    work = advance(work, { kind: "inspect_object", ref: objects.active_refs[0] });
    expect(cognitiveTaskInput(source, targets[0], objects, work, budget).inspected_object!.ref).toEqual(objects.active_refs[0]);
    work = advance(work, { kind: "retain", notes: "待回读 1.1 的定义" });
    expect(cognitiveTaskInput(source, targets[0], objects, work, budget).work.notes).toBe("待回读 1.1 的定义");
  });
  function advance(work: CognitiveWork, action: unknown, index = 0, limits = budget) {
    return advanceCognitiveWork({ work, action, source, target: targets[index], objects, discourse: [], budget: limits });
  }
  it("retrieves an unconnected necessary definition and rereads it before use", () => {
    let work = newCognitiveWork(source, targets[0]);
    work = advance(work, { kind: "read", lid: "2.1" });
    work = advance(work, { kind: "search", query: "平均速率", offset: 0, limit: 12 });
    expect(work.searches[0].lids).toContain("1.1");
    work = advance(work, { kind: "preview", lid: "1.1" });
    const material = materialFor(0);
    material.steps.unshift({ id: "definition", content: "平均速率等于路程除以时间", source_bindings: [binding("1.1")] });
    expect(() => advance(work, { kind: "finish", material })).toThrow("unread");
    work = advance(work, { kind: "read", lid: "1.1" });
    expect(advance(work, { kind: "finish", material }).status).toBe("complete");
  });
  it("preserves a genuine omitted connection with searchable and readable evidence", () => {
    let work = newCognitiveWork(source, targets[2]);
    work = advance(work, { kind: "read", lid: "2.3" }, 2);
    work = advance(work, { kind: "search", query: "测量记录", offset: 0, limit: 12 }, 2);
    const material = materialFor(2);
    material.gaps = [{ description: "未给出各段记录，平均值不能支持各段都更快", source_bindings: [binding("2.3")], searched_queries: ["测量记录"], inspected_lids: ["2.3"] }];
    const completed = advance(work, { kind: "finish", material }, 2);
    expect(completed.material?.gaps[0].source_bindings[0].lid).toBe("2.3");
  });
  it("budget exhaustion remains incomplete, resumes from persisted readings, and cannot publish", () => {
    const limits = { ...budget, read_chars: 1 };
    const work = advance(newCognitiveWork(source, targets[0]), { kind: "read", lid: "2.1" }, 0, limits);
    expect(work).toMatchObject({ status: "incomplete", stop_reason: "budget_exhausted", readings: [] });
    expect(() => collectCognitiveMaterials(source, targets, objects, [work])).toThrow("incomplete");
    const resumed = advance(JSON.parse(JSON.stringify(work)), { kind: "read", lid: "2.1" });
    expect(resumed.readings).toHaveLength(1);
    expect(advance(resumed, { kind: "read", lid: "2.1" }).used).toEqual(resumed.used);
  });
  it("accepts a method without manufacturing an argument chain", () => {
    const read = advance(newCognitiveWork(source, targets[1]), { kind: "read", lid: "2.2" }, 1);
    const done = advance(read, { kind: "finish", material: materialFor(1) }, 1);
    expect(done.material).toMatchObject({ kind: "method", connections: [], gaps: [] });
  });
});
