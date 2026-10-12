import { describe, expect, it } from "vitest";
import { fixture } from "../testdata/semantic-retrieval/gold";
import { acceptFormalObjects } from "../src/teaching-map";
import { projectRetrievalCatalog, retrievalCatalog } from "../src/semantic-retrieval";

describe("deterministic one-hop retrieval projection", () => {
  it("excludes identity and source metadata, preserves distinct keys and equivalent previous content", () => {
    const { work, source } = fixture();
    const previous = acceptFormalObjects({ source, proposal: work.proposal, operation_id: "projection" });
    const records = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous));
    for (const o of previous.objects) {
      const current = records.find(r => r.key === previous.acceptance.proposal.objects.find(p => p.meaning === o.meaning)!.key)!;
      const old = records.find(r => r.key === `previous/${o.ref.object_id}`)!;
      expect(old.embedding_text).toBe(current.embedding_text);
      expect(old.content_digest).toBe(current.content_digest);
    }
    const changed = structuredClone(work.proposal);
    const rename = (k: string) => `renamed/${k}`;
    changed.objects.forEach(o => { o.key = rename(o.key); o.participants.forEach(p => p.object_key = rename(p.object_key));
      o.component_keys = o.component_keys.map(rename); o.source_bindings[0].source_revision = "changed"; o.candidate_refs = ["new-id"]; });
    expect(projectRetrievalCatalog(retrievalCatalog(changed)).map(r => r.embedding_text))
      .toEqual(projectRetrievalCatalog(retrievalCatalog(work.proposal)).map(r => r.embedding_text));
  });
  it("tracks roles, conditions and referenced content without recursive expansion", () => {
    const { work } = fixture();
    const before = projectRetrievalCatalog(retrievalCatalog(work.proposal));
    work.proposal.objects.find(o => o.key === "base/car-a")!.meaning = "重型甲车";
    const after = projectRetrievalCatalog(retrievalCatalog(work.proposal));
    expect(after.filter((r, i) => r.content_digest !== before[i].content_digest).map(r => r.key))
      .toEqual(["base/a-pulls-b", "base/b-pulls-a", "base/car-a"]);
    const a = after.find(r => r.key === "base/a-pulls-b")!;
    expect(a.embedding_text).toContain("施力者: concept; 重型甲车");
    const relation = work.proposal.objects.find(o => o.key === "base/a-pulls-b")!;
    relation.participants = relation.participants.map((p, i) => ({ ...p, role: relation.participants[1 - i].role }));
    work.proposal.objects.find(o => o.key === "base/trip")!.conditions.push("新条件");
    const next = projectRetrievalCatalog(retrievalCatalog(work.proposal));
    expect(next.find(r => r.key === a.key)!.content_digest).not.toBe(a.content_digest);
    expect(next.find(r => r.key === "base/trip")!.content_digest).not.toBe(after.find(r => r.key === "base/trip")!.content_digest);
  });
  it("does not recursively expand a relation inside a composite", () => {
    const { work } = fixture();
    work.proposal.objects.push({ ...work.proposal.objects.find(o => o.key === "base/trip")!, key: "nested", component_keys: ["base/a-pulls-b"] });
    const project = () => projectRetrievalCatalog(retrievalCatalog(work.proposal));
    const before = project();
    work.proposal.objects.find(o => o.key === "base/car-a")!.meaning += " changed";
    const after = project();
    expect(after.find(r => r.key === "nested")).toEqual(before.find(r => r.key === "nested"));
    expect(after.find(r => r.key === "base/a-pulls-b")!.content_digest).not.toBe(before.find(r => r.key === "base/a-pulls-b")!.content_digest);
  });
  it("uses the effective prior revision and full refs; caps embedding only", () => {
    const { work, previous } = fixture();
    previous.objects.unshift({ ...previous.objects[0], meaning: "old", object_revision: 0 });
    const old = projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)).find(r => r.key === "previous/old-speed")!;
    expect(old.meaning).not.toBe("old");
    work.proposal.objects[0].meaning = "长".repeat(1200);
    const first = projectRetrievalCatalog(retrievalCatalog(work.proposal)).find(r => r.key === "base/speed")!;
    work.proposal.objects[0].aliases.push("not-in-embedding");
    const second = projectRetrievalCatalog(retrievalCatalog(work.proposal)).find(r => r.key === first.key)!;
    expect(Array.from(first.embedding_text)).toHaveLength(1024);
    expect(second.content_digest).toBe(first.content_digest);
    expect(second.lexical_fields).toContain("not-in-embedding");
    previous.active_refs[0] = { ...previous.active_refs[0], source_id: "other" };
    expect(() => retrievalCatalog(work.proposal, previous)).toThrow("reference");
  });
});
