import { describe, expect, it } from "vitest";
import { acceptFormalObjects } from "../src/teaching-map";
import { source, proposal, binding } from "./fixtures/teaching-source";

describe("formal learning identities", () => {
  it.each(["split", "merge"] as const)("SR0 rejects invalid %s cardinality and disappearing previous identities", kind => {
    const previous = acceptFormalObjects({ source, proposal, operation_id: "sr0-old" });
    const changed = structuredClone(proposal);
    changed.objects.forEach((o, i) => o.existing_ref = previous.active_refs[i]);
    delete changed.objects[0].existing_ref;
    expect(() => acceptFormalObjects({ source, proposal: changed, previous, operation_id: "sr0-unmapped" })).toThrow("identity");
    changed.correspondences = [{ kind, from: [previous.active_refs[0]], to_keys: [changed.objects[0].key], reason: "非法一对一拆并", source_bindings: [binding("1.1")] }];
    expect(() => acceptFormalObjects({ source, proposal: changed, previous, operation_id: "sr0-bad-cardinality" })).toThrow("cardinality");
  });
  const accept = () => acceptFormalObjects({ source, proposal, operation_id: "initial" });
  it("accepts semantic aliases, homonyms, conditional relations and composite content without capability identities", () => {
    const map = accept();
    expect(map.objects).toHaveLength(7);
    expect(map.candidate_refs["concept:平均速率"]).toEqual(map.candidate_refs["concept:平均速度大小"]);
    expect(map.objects[0].ref).not.toEqual(map.objects[1].ref);
    expect(map.objects[0].aliases).toContain("速度");
    expect(map.objects[1].aliases).toContain("速度");
    expect(map.graph_edges.map(e => e.relation_ref)).toEqual([map.objects[5].ref, map.objects[6].ref]);
    expect(map.objects[4].component_refs).toEqual([map.objects[2].ref, map.objects[3].ref]);
    expect(map.prerequisites[0].prerequisite.object_ref).toEqual(map.objects[0].ref);
  });
  it("restores accepted output and revises content without rebinding history", () => {
    const map = JSON.parse(JSON.stringify(accept()));
    expect(acceptFormalObjects({ source, proposal, operation_id: "initial", previous: map })).toEqual(map);
    const changed = structuredClone(proposal);
    changed.objects.forEach((o, i) => o.existing_ref = map.active_refs[i]);
    changed.objects[0].aliases.push("路程平均速率");
    const next = acceptFormalObjects({ source, proposal: changed, operation_id: "revision", previous: map });
    expect(next.active_refs).toEqual(map.active_refs);
    expect(next.objects).toHaveLength(8);
    expect(next.objects.at(-1)?.object_revision).toBe(2);
  });
  it("retains old references after a substantive split and explicit merge", () => {
    const map = accept();
    const changed = structuredClone(proposal);
    changed.objects.forEach((o, i) => o.existing_ref = map.active_refs[i]);
    const original = changed.objects[1];
    changed.objects.splice(1, 1, { ...original, key: "velocity-a", existing_ref: undefined }, { ...original, key: "velocity-b", existing_ref: undefined });
    changed.coverage[0].object_keys = changed.coverage[0].object_keys.filter(k => k !== "velocity").concat("velocity-a", "velocity-b");
    changed.correspondences = [{ kind: "split", from: [map.active_refs[1]], to_keys: ["velocity-a", "velocity-b"], reason: "分别追踪位移和方向条件", source_bindings: [binding("1.1")] }];
    const next = acceptFormalObjects({ source, proposal: changed, operation_id: "split", previous: map });
    expect(next.objects.find(o => o.ref.object_id === map.active_refs[1].object_id)).toEqual(map.objects[1]);
    expect(next.active_refs).not.toContainEqual(map.active_refs[1]);
    const merged = structuredClone(proposal);
    merged.objects.forEach((o, i) => o.existing_ref = i === 1 ? undefined : map.active_refs[i]);
    merged.correspondences = [{ ...changed.correspondences[0], kind: "merge", from: next.correspondences[0].to, to_keys: ["velocity"] }];
    const last = acceptFormalObjects({ source, proposal: merged, operation_id: "merge", previous: next });
    expect(last.correspondences).toHaveLength(2);
    expect(last.active_refs[1]).not.toEqual(map.active_refs[1]);
  });
  it("rejects missing or stale evidence and incomplete coverage", () => {
    const invalid = structuredClone(proposal);
    invalid.objects[0].source_bindings[0].lid = "missing";
    expect(() => acceptFormalObjects({ source, proposal: invalid, operation_id: "bad" })).toThrow("source binding");
    expect(() => acceptFormalObjects({ source: { ...source, source_revision: "new" }, proposal, operation_id: "bad" })).toThrow("stale");
    expect(() => acceptFormalObjects({ source, proposal: { ...proposal, coverage: proposal.coverage.slice(1) }, operation_id: "bad" })).toThrow("coverage");
  });
});
