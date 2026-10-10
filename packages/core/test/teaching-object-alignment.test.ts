import { describe, expect, it } from "vitest";
import { source, proposal, binding } from "./fixtures/teaching-source";
import { acceptFormalObjects, type FormalObjectProposal, type FormalObjects } from "../src/teaching-map";
import { newObjectAlignment, advanceObjectAlignment, mergeObjectAlignmentBranch, alignmentFocus, objectAlignmentInput, type ObjectAlignmentWork } from "../src/teaching-object-alignment";
import type { FormalObjectFragmentResult } from "../src/teaching-object-fragments";
import { fixture as retrievalFixture, cases as retrievalCases } from "../testdata/semantic-retrieval/gold";
import { automaticBuildCandidateCorrection, automaticBuildFailureDiagnosticFromWriterError } from "../src/extractor-contract";

const fragment = (id: string, candidate: FormalObjectProposal): FormalObjectFragmentResult => ({ version: "formal_object_fragment.v1", fragment_id: id,
  source_ranges: source.passages.map(p => ({ lid: p.lid, start: 0, end: p.text.length })), auxiliary_ranges: [], proposal: structuredClone(candidate) });
const advance = (work: ObjectAlignmentWork, action: unknown, previous?: FormalObjects) => advanceObjectAlignment({ work, action, source, previous, operation_id: "alignment-test" });
function readBindings(work: ObjectAlignmentWork, bindings: FormalObjectProposal["objects"][number]["source_bindings"]) {
  for (const b of bindings) {
    const range = b.range_utf16 ?? { start: 0, end: source.passages.find(p => p.lid === b.lid)!.text.length };
    for (let start = range.start; start < range.end; start += 2000)
      work = advance(work, { kind: "read", lid: b.lid, start, end: Math.min(start + 2000, range.end) });
  }
  return work;
}
function resolveAll(work: ObjectAlignmentWork) {
  for (;;) {
    const focus = alignmentFocus(work);
    if (focus.kind !== "candidate") return work;
    work = readBindings(work, focus.object.source_bindings);
    work = advance(work, { kind: "resolve", keys: [focus.key], object: focus.object });
  }
}
describe("bounded cross-fragment object alignment", () => {
  it("supplements only missing inspected evidence after exhaustion, including gaps and the last pending search", () => {
    let work = newObjectAlignment([fragment("f", proposal)]);
    const focus = alignmentFocus(work);
    if (focus.kind !== "candidate") throw new Error("fixture needs a candidate");
    const lid = focus.object.source_bindings[0].lid;
    const length = source.passages.find(p => p.lid === lid)!.text.length;
    work = advance(work, { kind: "read", lid, start: 0, end: 2 });
    work = advance(work, { kind: "read", lid, start: 4, end: length });
    work.steps_since_progress = 32;
    work.search = { query: "pending", offset: 0, keys: [], next_offset: null, preparation_required: true };
    const input = objectAlignmentInput(work, source);
    expect(input.search).toBeUndefined();
    expect(input.retrieval_budget?.next_read).toEqual({ kind: "read", lid, start: 2, end: 4 });
    expect(() => advance(work, { kind: "read", lid, start: 1, end: 4 })).toThrow("retrieval steps are exhausted");
    expect(() => advance(work, { kind: "read", lid: "2.3", start: 0, end: 1 })).toThrow("retrieval steps are exhausted");
    let failure: unknown;
    try { advance(work, { kind: "resolve", keys: [focus.key], object: focus.object }); } catch (error) { failure = error; }
    expect(automaticBuildCandidateCorrection(automaticBuildFailureDiagnosticFromWriterError(failure, { writer_started: true })))
      .toMatchObject({ json_pointer: "/object/source_bindings/0", expected: expect.stringContaining("unread source range") });
    work = advance(work, input.retrieval_budget!.next_read);
    expect(work.steps_since_progress).toBe(33);
    expect(() => advance(work, input.retrieval_budget!.next_read)).toThrow("retrieval steps are exhausted");
    expect(advance(work, { kind: "resolve", keys: [focus.key], object: focus.object }).resolved).toContain(focus.key);
  });
  it("allows the final grounded decision after 32 retrieval steps and rejects more lookup actions", () => {
    let work = newObjectAlignment([fragment("f", proposal)]);
    const focus = alignmentFocus(work);
    if (focus.kind !== "candidate") throw new Error("fixture needs a candidate");
    work.focus_key = focus.key;
    work = readBindings(work, focus.object.source_bindings);
    while (work.steps_since_progress < 31) work = advance(work, { kind: "inspect", key: focus.key });
    expect(objectAlignmentInput(work, source)).not.toHaveProperty("retrieval_budget");
    work = advance(work, { kind: "inspect", key: focus.key });
    expect(objectAlignmentInput(work, source).retrieval_budget).toEqual({ steps_used: 32, step_limit: 32, allowed_actions: ["resolve"] });
    for (const action of [{ kind: "search", query: "source", offset: 0 }, { kind: "inspect", key: focus.key },
      { kind: "read", lid: focus.object.source_bindings[0].lid, start: 0, end: 1 }]) {
      let failure: unknown;
      try { advance(work, action); } catch (error) { failure = error; }
      expect(automaticBuildCandidateCorrection(automaticBuildFailureDiagnosticFromWriterError(failure, { writer_started: true })))
        .toMatchObject({ json_pointer: "/kind", expected: expect.stringContaining("kind=resolve") });
    }
    expect(() => advance({ ...work, read_ranges: [] }, { kind: "resolve", keys: [focus.key], object: focus.object }))
      .toThrow("unread source range");
    const resolved = advance(work, { kind: "resolve", keys: [focus.key], object: focus.object });
    expect(resolved.resolved).toContain(focus.key);
    expect(resolved.steps_since_progress).toBe(0);
    expect(resolved.focus_key).toBeUndefined();
    expect(objectAlignmentInput(resolved, source)).not.toHaveProperty("retrieval_budget");
  });
  it("reduces independent branch decisions and defers overlapping or changed reference decisions", () => {
    const base = newObjectAlignment([fragment("f", proposal)]);
    const [a, b, c] = [base.proposal.objects[2], base.proposal.objects[3], base.proposal.objects[0]];
    const branchFor = (key: string, inspected: string[] = []) => {
      const branch = structuredClone(base); branch.focus_key = key; branch.inspected = inspected;
      return readBindings(branch, base.proposal.objects.filter(o => o.key === key || inspected.includes(o.key)).flatMap(o => o.source_bindings));
    };
    const first = branchFor(a.key, [b.key]);
    const merged = mergeObjectAlignmentBranch({ current: base, branch: first,
      action: { kind: "resolve", keys: [a.key, b.key], object: a }, source, operation_id: "lane-0" })!;
    expect(merged.resolved).toContain(a.key);
    expect(merged.proposal.objects.some(o => o.key === b.key)).toBe(false);
    expect(mergeObjectAlignmentBranch({ current: merged, branch: branchFor(b.key),
      action: { kind: "resolve", keys: [b.key], object: b }, source, operation_id: "lane-1" })).toBeUndefined();
    const independent = mergeObjectAlignmentBranch({ current: merged, branch: branchFor(c.key),
      action: { kind: "resolve", keys: [c.key], object: c }, source, operation_id: "lane-2" });
    expect(independent?.resolved).toEqual(expect.arrayContaining([a.key, c.key]));
    expect(base.resolved).toEqual([]);
    expect(first.read_ranges.length).toBeGreaterThan(0);
    const relation = base.proposal.objects.find(o => o.participants.some(p => p.object_key === b.key));
    if (!relation) throw new Error("fixture requires a relation to the merged object");
    expect(mergeObjectAlignmentBranch({ current: merged, branch: branchFor(relation.key),
      action: { kind: "resolve", keys: [relation.key], object: relation }, source, operation_id: "changed-reference" })).toBeUndefined();
  });
  it("SR6 returns the actual source boundary when an inspected object's paragraph length was not visible", () => {
    const f = retrievalFixture();
    const inspect = advanceObjectAlignment({ ...f, action: { kind: "inspect", key: "later/speed-reworded" }, operation_id: "sr6-read" });
    const length = f.source.passages.find(p => p.lid === "2.2")!.text.length;
    let failure: unknown;
    try { advanceObjectAlignment({ ...f, work: inspect, action: { kind: "read", lid: "2.2", start: 0, end: 2000 }, operation_id: "sr6-read" }); }
    catch (error) { failure = error; }
    const diagnostic = automaticBuildFailureDiagnosticFromWriterError(failure, { writer_started: true });
    expect(diagnostic).toMatchObject({ category: "schema", code: "schema_invalid", phase: "artifact_writer", json_pointer: "/end" });
    expect(automaticBuildCandidateCorrection(diagnostic)).toMatchObject({ json_pointer: "/end", expected: expect.stringContaining(`source_length_utf16=${length}`) });
    expect(() => advanceObjectAlignment({ ...f, work: inspect, action: { kind: "read", lid: "2.2", start: 0, end: 2000 }, operation_id: "sr6-read" }))
      .toThrow(`source_length_utf16=${length}`);
    expect(inspect.read_ranges).toEqual([]);
    const read = advanceObjectAlignment({ ...f, work: inspect, action: { kind: "read", lid: "2.2", start: 0, end: length }, operation_id: "sr6-read" });
    expect(read.read_ranges).toEqual([{ source_id: f.source.source_id, source_revision: f.source.source_revision, lid: "2.2", start: 0, end: length }]);
  });
  it("SR0a rejects legacy alignment work instead of treating it as read-covered", () => {
    const work = newObjectAlignment([fragment("f", proposal)]);
    const legacy = { ...work, version: "formal_object_alignment.v1" } as unknown as ObjectAlignmentWork;
    expect(() => advance(legacy, { kind: "resolve", keys: ["f/speed"], object: work.proposal.objects[0] })).toThrow("contract is stale");
  });
  it.each(["resolve", "identity"] as const)("SR0a %s requires continuous UTF-16 coverage and clears it only on success", kind => {
    const previous = acceptFormalObjects({ source, proposal, operation_id: "ranges-prior" });
    let work = newObjectAlignment([fragment("f", proposal)]);
    if (kind === "identity") {
      work = resolveAll(work);
      work = advance(work, { kind: "inspect", key: "f/speed" }, previous);
    }
    const object = work.proposal.objects.find(o => o.key === "f/speed")!;
    const action = (bindings: typeof object.source_bindings) => kind === "resolve"
      ? { kind, keys: [object.key], object: { ...object, source_bindings: bindings } }
      : { kind, from: [previous.active_refs[0]], to_keys: [object.key], reason: "同一定义", source_bindings: bindings };
    const length = source.passages[0].text.length;
    // Out-of-order, overlapping and repeated reads must coalesce, but a gap must remain a gap.
    for (const [start, end] of [[10, length], [0, 4], [0, 4], [2, 6]])
      work = advance(work, { kind: "read", lid: "1.1", start, end }, previous);
    expect(work.read_ranges).toEqual([
      { ...binding("1.1"), start: 0, end: 6 }, { ...binding("1.1"), start: 10, end: length },
    ]);
    const before = structuredClone(work);
    expect(() => advance(work, action([binding("1.1")]), previous)).toThrow("unread source range");
    expect(work).toEqual(before);
    expect(() => advance(work, action([{ ...binding("1.1"), range_utf16: { start: 5, end: 11 } }]), previous)).toThrow("unread source range");
    expect(advance(work, action([{ ...binding("1.1"), range_utf16: { start: 1, end: 6 } }]), previous).read_ranges).toEqual([]);
    work = advance(work, { kind: "read", lid: "1.1", start: 6, end: 10 }, previous);
    // Reading another LID replaces visible text, without dropping earlier coverage.
    work = advance(work, { kind: "read", lid: "2.3", start: 0, end: 1 }, previous);
    expect(work.read_ranges).toContainEqual({ ...binding("1.1"), start: 0, end: length });
    expect(objectAlignmentInput(work, source, previous).reading!.lid).toBe("2.3");
    const changedRevision = structuredClone(work);
    changedRevision.read_ranges.forEach(r => r.source_revision = "old-source");
    expect(() => advance(changedRevision, action([binding("1.1")]), previous)).toThrow("unread source range");
    const changedSource = structuredClone(work);
    changedSource.read_ranges.forEach(r => r.source_id = "other-source");
    expect(() => advance(changedSource, action([binding("1.1")]), previous)).toThrow("unread source range");
    const done = advance(work, action([binding("1.1")]), previous);
    expect(done.read_ranges).toEqual([]);
    expect(done.reading).toBeUndefined();
    const target = done.proposal.objects.find(o => o.key === "f/velocity")!;
    const inspected = advance(done, { kind: "inspect", key: target.key }, previous);
    const next = kind === "resolve" ? { kind, keys: [target.key], object: target }
      : { kind, from: [previous.active_refs[1]], to_keys: [target.key], reason: "另一定义", source_bindings: target.source_bindings };
    expect(() => advance(inspected, next, previous)).toThrow("unread source range");
  });
  it("SR0 characterizes JSON substring OR matching, self hits and complete current/previous browsing", () => {
    const { work: initial, source: goldSource, previous } = retrievalFixture();
    const step = (work: ObjectAlignmentWork, action: unknown) => advanceObjectAlignment({ work, action, source: goldSource, previous, operation_id: "sr0" });
    let work = initial;
    const keys: string[] = [];
    let offset: number | null = 0;
    do {
      work = step(work, { kind: "search", query: "  ", offset });
      keys.push(...work.search!.keys); offset = work.search!.next_offset;
    } while (offset !== null);
    expect(keys).toEqual([...work.proposal.objects.map(o => o.key), "previous/old-speed"]);
    expect(new Set(keys).size).toBe(keys.length);
    expect(step(work, { kind: "search", query: "", offset: keys.length }).search!.keys).toEqual([]);
    expect(() => step(work, { kind: "search", query: "", offset: keys.length + 1 })).toThrow("offset");
    expect(step(work, { kind: "search", query: "SPEED impossible-token", offset: 0 }).search!.keys).toContain("base/speed");
    // Internal source metadata is searched too; this is the legacy A baseline, not semantic relevance.
    expect(step(work, { kind: "search", query: "source-v1", offset: 0 }).search!.keys).toEqual(keys.slice(0, 6));
    for (const c of retrievalCases.filter(c => c.id.startsWith("low-lexical") || c.id === "previous-paraphrase")) {
      const result = step(initial, { kind: "search", query: c.query, offset: 0 });
      expect(result.search!.keys).toContain(c.focus);
      expect(result.search!.keys).not.toContain(c.compare[0]);
      expect(result.search!.next_offset).toBeNull();
    }
  });
  it("SR0 preserves explicit search across inspect/read and resets decision state after resolve", () => {
    let work = newObjectAlignment([fragment("f", proposal)]);
    work = advance(work, { kind: "search", query: "速度", offset: 0 });
    const search = work.search;
    for (const key of work.proposal.objects.slice(1, 5).map(o => o.key)) work = advance(work, { kind: "inspect", key });
    expect(objectAlignmentInput(work, source).inspected).toHaveLength(3);
    expect(work.inspected).toHaveLength(4);
    work = advance(work, { kind: "read", lid: "1.1", start: 0, end: 10 });
    expect(work.search).toEqual(search);
    expect(objectAlignmentInput(work, source).reading!.text).toBe(source.passages[0].text.slice(0, 10));
    work = readBindings(work, work.proposal.objects[0].source_bindings);
    work = advance(work, { kind: "resolve", keys: ["f/speed"], object: work.proposal.objects[0] });
    expect(work).toMatchObject({ inspected: [], read_ranges: [], steps_since_progress: 0 });
    expect(work.search).toBeUndefined(); expect(work.reading).toBeUndefined();
  });
  it("SR0a rejects inspected synonyms without reading cited original text", () => {
    const duplicate = structuredClone(proposal);
    duplicate.objects = [duplicate.objects[0]]; duplicate.prerequisites = []; duplicate.coverage = [];
    let work = newObjectAlignment([fragment("a", proposal), fragment("b", duplicate)]);
    const action = { kind: "resolve", keys: ["a/speed", "b/speed"], object: work.proposal.objects[0] };
    expect(() => advance(work, action)).toThrow("inspected");
    work = advance(work, { kind: "inspect", key: "b/speed" });
    expect(work.reading).toBeUndefined();
    expect(() => advance(work, action)).toThrow("unread source range");
    expect(objectAlignmentInput(work, source).source_previews[0].text).toBe(source.passages[0].text);
    work = readBindings(work, action.object.source_bindings);
    expect(advance(work, action).proposal.objects).toHaveLength(proposal.objects.length);
  });
  it("SR0a requires read coverage for identity alongside inspected targets and current-source evidence", () => {
    const previous = acceptFormalObjects({ source, proposal, operation_id: "sr0-prior" });
    let work = resolveAll(newObjectAlignment([fragment("f", proposal)]));
    const action = { kind: "identity", from: [previous.active_refs[0]], to_keys: ["f/speed"], reason: "同一定义", source_bindings: [binding("1.1")] };
    expect(() => advance(work, action, previous)).toThrow("uninspected");
    work = advance(work, { kind: "inspect", key: "f/speed" }, previous);
    expect(() => advance(work, { ...action, source_bindings: [binding("2.3")] }, previous)).toThrow("current objects");
    expect(() => advance(work, { ...action, source_bindings: [{ ...binding("1.1"), source_revision: "wrong" }] }, previous)).toThrow("stale");
    expect(work.reading).toBeUndefined();
    expect(() => advance(work, action, previous)).toThrow("unread source range");
    expect(advance(readBindings(work, action.source_bindings), action, previous).proposal.objects.find(o => o.key === "f/speed")!.existing_ref).toEqual(previous.active_refs[0]);
    expect(() => advance(work, { ...action, from: previous.active_refs.slice(0, 2) }, previous)).toThrow("inspected unclaimed");
    for (const ref of previous.active_refs) work = advance(work, { kind: "inspect", key: `previous/${ref.object_id}` }, previous);
    work = advance(work, { kind: "inspect", key: "f/velocity" }, previous);
    expect(() => advance(work, { ...action, from: previous.active_refs.slice(0, 2), to_keys: ["f/speed", "f/velocity"] }, previous)).toThrow("split or merge");
  });
  it("SR0 keeps allowed source membership distinct from actual read coverage", () => {
    let work = newObjectAlignment([fragment("f", proposal)]);
    const replacement = { ...work.proposal.objects[0], source_bindings: [binding("2.3")] };
    const action = { kind: "resolve", keys: [replacement.key], object: replacement };
    expect(() => advance(work, action)).toThrow("uninspected source");
    work = advance(work, { kind: "read", lid: "2.3", start: 0, end: 1 });
    expect(() => advance(work, action)).toThrow("unread source range");
    expect(() => advance(work, { ...action, object: { ...replacement, source_bindings: [{ ...binding("2.3"), source_id: "other" }] } })).toThrow("source binding");
  });
  it("keeps model input bounded independently of unshown Core ledger changes", () => {
    const { work, source: goldSource, previous } = retrievalFixture();
    const changed = structuredClone(work);
    changed.proposal.objects.at(-1)!.meaning = "新的方法说明";
    expect(changed.proposal).not.toEqual(work.proposal);
    expect(objectAlignmentInput(changed, goldSource, previous)).toEqual(objectAlignmentInput(work, goldSource, previous));
  });
  it("merges explicit synonyms, rewrites prerequisites, and preserves homonyms and conditional relations", () => {
    const duplicate: FormalObjectProposal = { objects: [{ ...proposal.objects[0], key: "alias", meaning: "平均速度大小：总路程 / 总时间", candidate_refs: ["concept:alias"] }],
      prerequisites: [], correspondences: [], coverage: [{ unit_lid: "2", object_keys: ["alias"], explanation: "跨块定义", source_bindings: [binding("2.1")] }] };
    let work = newObjectAlignment([fragment("first", proposal), fragment("second", duplicate)]);
    const focus = alignmentFocus(work);
    expect(focus.kind).toBe("candidate");
    const action = { kind: "resolve", keys: ["first/speed", "second/alias"], object: work.proposal.objects[0] };
    expect(() => advance(work, action)).toThrow("inspected");
    work = advance(work, { kind: "search", query: "平均速度", offset: 0 });
    expect(work.search!.keys).toContain("second/alias");
    work = advance(work, { kind: "inspect", key: "second/alias" });
    work = readBindings(work, action.object.source_bindings);
    work = advance(work, action);
    work = resolveAll(work);
    const result = advance(work, { kind: "finish" }).result!;
    expect(result.active_refs).toHaveLength(proposal.objects.length);
    expect(result.candidate_refs["concept:alias"]).toEqual(result.candidate_refs["concept:平均速率"]);
    expect(result.prerequisites[0].prerequisite.object_ref).toEqual(result.candidate_refs["concept:alias"][0]);
    expect(result.objects.filter(o => o.aliases.includes("速度"))).toHaveLength(2);
    expect(result.objects.filter(o => o.kind === "relation").map(o => o.conditions)).toEqual([["等距离", "速率为正"], ["等时间"]]);
    expect(result.coverage).toHaveLength(2);
    expect(result.coverage[1].object_refs).toContainEqual(result.candidate_refs["concept:alias"][0]);
  });
  it("keeps catalog input bounded and reads the requested original range", () => {
    const large = structuredClone(proposal);
    large.objects = Array.from({ length: 1000 }, (_, i) => ({ ...proposal.objects[0], key: `key${i}`, meaning: `定义 ${i} ` + "解释".repeat(200) }));
    large.prerequisites = []; large.coverage.forEach(c => c.object_keys = []);
    let work = newObjectAlignment([fragment("many", large)]);
    const first = objectAlignmentInput(work, source);
    expect(JSON.stringify(first).length).toBeLessThan(6000);
    work = advance(work, { kind: "search", query: "", offset: 0 });
    expect(work.search!.keys).toHaveLength(6);
    expect(work.search!.next_offset).toBe(6);
    work = advance(work, { kind: "search", query: "", offset: 996 });
    expect(work.search!.keys).toHaveLength(4);
    expect(work.search!.next_offset).toBeNull();
    work = advance(work, { kind: "read", lid: "1.1", start: 4, end: 20 });
    expect(objectAlignmentInput(work, source).reading!.text).toBe(source.passages[0].text.slice(4, 20));
    expect(() => advance(work, { kind: "read", lid: "1.1", start: 5, end: 3000 })).toThrow("range");
    expect(() => advance(work, { kind: "finish" })).toThrow("incomplete");
  });
  it("reuses previous identities only after all prior identities are explicitly mapped", () => {
    const previous = acceptFormalObjects({ source, proposal, operation_id: "previous" });
    let work = resolveAll(newObjectAlignment([fragment("new", proposal)]));
    expect(() => advance(work, { kind: "finish" }, previous)).toThrow("incomplete");
    for (const object of previous.objects) {
      const target = work.proposal.objects.find(o => o.meaning === object.meaning)!;
      work = advance(work, { kind: "inspect", key: target.key }, previous);
      work = readBindings(work, target.source_bindings);
      work = advance(work, { kind: "identity", from: [object.ref], to_keys: [target.key], reason: "来源含义延续", source_bindings: target.source_bindings }, previous);
    }
    const result = advance(work, { kind: "finish" }, previous).result!;
    expect(result.active_refs).toEqual(previous.active_refs);
    expect(result.objects).toEqual(previous.objects);
  });
  it.each(["split", "merge"] as const)("preserves explicit %s correspondence and retires old identities", kind => {
    const count = kind === "split" ? 1 : 2;
    const oldProposal = structuredClone(proposal);
    oldProposal.objects = oldProposal.objects.slice(0, count); oldProposal.prerequisites = [];
    oldProposal.coverage.forEach(c => c.object_keys = oldProposal.objects.map(o => o.key));
    const previous = acceptFormalObjects({ source, proposal: oldProposal, operation_id: "old" });
    const next = structuredClone(oldProposal);
    next.objects = kind === "split" ? structuredClone(proposal.objects.slice(0, 2)) : next.objects.slice(0, 1);
    next.coverage.forEach(c => c.object_keys = next.objects.map(o => o.key));
    let work = resolveAll(newObjectAlignment([fragment("next", next)]));
    for (const ref of previous.active_refs) work = advance(work, { kind: "inspect", key: `previous/${ref.object_id}` }, previous);
    for (const object of work.proposal.objects) work = advance(work, { kind: "inspect", key: object.key }, previous);
    work = readBindings(work, [binding("1.1")]);
    work = advance(work, { kind: "identity", from: previous.active_refs, to_keys: work.proposal.objects.map(o => o.key), reason: "实质含义拆并", source_bindings: [binding("1.1")] }, previous);
    const result = advance(work, { kind: "finish" }, previous).result!;
    expect(result.correspondences.at(-1)!.kind).toBe(kind);
    expect(result.active_refs.some(r => previous.active_refs.some(old => old.object_id === r.object_id))).toBe(false);
  });
  it("rejects unresolved references and source evidence not inspected for this decision", () => {
    const work = newObjectAlignment([fragment("f", proposal)]), object = work.proposal.objects[0];
    expect(() => advance(work, { kind: "resolve", keys: [object.key], object: { ...object, component_keys: ["missing"] } })).toThrow("unresolved");
    expect(() => advance(work, { kind: "resolve", keys: [object.key], object: { ...object, source_bindings: [binding("2.3")] } })).toThrow("uninspected");
  });
});
