import { describe, expect, it } from "vitest";
import { acceptFormalObjects } from "../src/teaching-map";
import { advanceCognitiveWork, newCognitiveWork, collectCognitiveMaterials, cognitiveTaskInput, type CognitiveMaterial, type CognitiveWork } from "../src/cognitive-materials";
import { source, proposal, binding, targets } from "./fixtures/teaching-source";

const objects = acceptFormalObjects({ source, proposal, operation_id: "objects" });
const budget = { searches: 6, preview_chars: 2000, read_chars: 4000, context_chars: 4000 };
export function materialFor(index: number): CognitiveMaterial {
  return { target_id: targets[index].id, kind: index === 1 ? "method" : "reasoning_episode", purpose: targets[index].reason.text,
    object_refs: [objects.active_refs[0]], steps: [{ id: "step1", content: source.passages.find(p => p.lid === targets[index].lid)!.text, source_bindings: [binding(targets[index].lid)] }], connections: [], conditions: [], gaps: [], patterns: [] };
}
describe("source-grounded cognitive construction", () => {
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
