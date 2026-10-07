import { describe, expect, it } from "vitest";
import { prepareBaseline } from "../../../evals/semantic-retrieval/baseline";
import { fixture } from "../testdata/semantic-retrieval/gold";
import { advanceObjectAlignment, alignmentFocus } from "../src/teaching-object-alignment";

describe("SR0 frozen source-grounded retrieval Gold", () => {
  it("materializes all current objects and the paraphrased previous identity through production gates", () => {
    const { source, previous, work: initial } = fixture();
    let work = initial;
    const step = (action: unknown) => { work = advanceObjectAlignment({ work, action, source, previous, operation_id: "sr0-gold-materialization" }); };
    for (;;) {
      const focus = alignmentFocus(work, previous);
      if (focus.kind !== "candidate") break;
      for (const b of focus.object.source_bindings)
        step({ kind: "read", lid: b.lid, start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length });
      step({ kind: "resolve", keys: [focus.key], object: focus.object });
    }
    step({ kind: "inspect", key: "later/speed-reworded" });
    const object = work.proposal.objects.find(o => o.key === "later/speed-reworded")!;
    for (const b of object.source_bindings)
      step({ kind: "read", lid: b.lid, start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length });
    step({ kind: "identity", from: previous.active_refs, to_keys: [object.key], source_bindings: object.source_bindings, reason: "累计行程/耗时与行驶全程距离/历时为同一定义" });
    step({ kind: "finish" });
    expect(work.result!.active_refs).toContainEqual(previous.active_refs[0]);
    expect(work.result!.active_refs).toHaveLength(initial.proposal.objects.length);
    const baseline = prepareBaseline();
    for (const c of baseline.queries) {
      expect(baseline.catalog.some(r => r.key === c.focus)).toBe(true);
      for (const key of [...c.compare, ...c.separate, ...c.same]) expect(baseline.catalog.some(r => r.key === key)).toBe(true);
      for (const lid of c.lids) expect(source.passages.some(p => p.lid === lid)).toBe(true);
      expect(c.same.some(key => c.separate.includes(key))).toBe(false);
    }
  });
  it("records the actual substring gap and one-hop relation/component context without identifiers in embedding text", () => {
    const baseline = prepareBaseline();
    expect(baseline.records).toHaveLength(28);
    const low = baseline.queries.find(q => q.id === "low-lexical-speed")!;
    expect(low.substring).toEqual([low.focus]);
    const relation = baseline.records.find(r => r.key === "base/a-pulls-b")!;
    expect(relation.embedding_text).toContain("施力者: concept; 甲车");
    expect(relation.embedding_text).toContain("受力者: concept; 乙车");
    const composite = baseline.records.find(r => r.key === "base/trip")!;
    expect(composite.embedding_text).toContain("concept; 去程");
    for (const r of baseline.records) {
      expect(r.embedding_text).not.toContain(r.key);
      expect(r.embedding_text).not.toContain("source-v1");
    }
  });
});
