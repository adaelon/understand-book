import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { projectStructureRetrieval, prepareStructureRetrieval, missingStructureRetrievalTexts, structureRetrievalPage, STRUCTURE_RETRIEVAL } from "../src/book-structure-retrieval";
import { collectStructureCandidates } from "../src/book-structure-candidates";
import { prepareSemanticRetrieval, retrievalDependencies, retrievalPreparationMatches } from "../src/semantic-retrieval-preparation";
import { missingRetrievalTexts, rankSemanticRecords } from "../src/semantic-retrieval-cache";

function fixture() {
  const catalog = collectStructureCandidates([{ contribution_ref: "unit:10", unit_lid: "10", stops: Array.from({ length: 15 }, (_, i) => ({
    id: `s${i}`, lid: `10.${i}`, type: "claim" as const, meaning: `状态复用${i}`, conditions: ["计算条件不变"], aliases: [`KV${i}`],
    reason: { text: "完整材料".repeat(100), evidence_lids: [`10.${i}`] } })),
    reference_scope: { unit_lids: ["10"], dependency_target_lids: [], evidence_by_unit: { "10": Array.from({ length: 15 }, (_, i) => `10.${i}`) } } }]);
  return { catalog, index: projectStructureRetrieval(catalog, []), provider: fakeEmbedding(), workspace: mkdtempSync(path.join(tmpdir(), "bsr3-")) };
}
describe("BSR3 structure retrieval consumer", () => {
  it("indexes available outline questions without embedding display titles or chapter identities", () => {
    const { catalog } = fixture();
    const chapter = { unit_lid: "10", title: "第 8 章", question: "状态为什么可以复用？", overview: { text: "因果性保持旧状态不变", evidence_lids: ["10.1"] } };
    const index = projectStructureRetrieval(catalog, [chapter]);
    const record = index.records.find(r => r.key === "chapter:10")!;
    expect(record.embedding_text).toContain(chapter.question); expect(record.embedding_text).toContain(chapter.overview.text);
    expect(record.embedding_text).not.toContain(chapter.title);
    expect(index.locations[record.key].evidence_lids).toEqual(["10.1"]);
  });
  it("isolates consumers and propagates projection identity through missing/preparation/ranking", async () => {
    const { index, workspace, provider } = fixture();
    const request = { kind: "search" as const, query: "状态" };
    const formal = await prepareSemanticRetrieval({ workspace, provider, records: index.records, request, mode: "semantic_required" });
    const file = path.join(workspace, ".build/semantic-retrieval/cache.json");
    const old = readFileSync(file, "utf8");
    expect(missingStructureRetrievalTexts(workspace, index, provider.identity)).toHaveLength(15);
    const prepared = await prepareStructureRetrieval({ index, workspace, provider, request, mode: "semantic_required", previous: formal });
    expect(readFileSync(file, "utf8")).toBe(old);
    expect(missingStructureRetrievalTexts(workspace, index, provider.identity)).toEqual([]);
    expect(missingRetrievalTexts(workspace, index.records, provider.identity)).toEqual([]);
    expect(retrievalPreparationMatches(prepared, retrievalDependencies(index.records, request, "semantic_required", provider.identity))).toBe(false);
    const cache = JSON.parse(readFileSync(path.join(workspace, ".build/semantic-retrieval/book-structure/cache.json"), "utf8"));
    expect(() => rankSemanticRecords(index.records, cache, prepared.query_embedding!)).toThrow(/identity/);
    expect(rankSemanticRecords(index.records, cache, prepared.query_embedding!, undefined, STRUCTURE_RETRIEVAL.projection_version)).toHaveLength(15);
  });
  it("only embeds changed meanings/conditions and reuses query and preparation across pages", async () => {
    const { catalog, workspace, provider } = fixture();
    const request = { kind: "search" as const, query: "状态" };
    let index = projectStructureRetrieval(catalog, []);
    const first = await prepareStructureRetrieval({ index, workspace, provider, request, mode: "semantic_required" });
    const calls = provider.calls.length;
    expect(structureRetrievalPage(index, first, request, 0).next_offset).toBe(6);
    structureRetrievalPage(index, first, request, 6);
    expect(await prepareStructureRetrieval({ index, workspace, provider, request, mode: "semantic_required", previous: first })).toBe(first);
    expect(provider.calls).toHaveLength(calls);
    catalog.candidates[0].meaning = "新含义"; catalog.candidates[1].conditions = ["新条件"];
    index = projectStructureRetrieval(catalog, []);
    expect(missingStructureRetrievalTexts(workspace, index, provider.identity)).toHaveLength(2);
    expect(() => structureRetrievalPage(index, first, request)).toThrow(/stale/);
    await prepareStructureRetrieval({ index, workspace, provider, request, mode: "semantic_required", previous: first });
    expect(provider.calls.slice(calls).map(c => [c.role, c.texts.length])).toEqual([["document", 2]]);
  });
  it("keeps aliases, full lexical fields, source locations and complete browse with no provider", async () => {
    const { index, workspace, provider } = fixture();
    const request = { kind: "search" as const, query: "KV4" };
    const prepared = await prepareStructureRetrieval({ index, workspace, request, mode: "lexical_only" });
    const hit = structureRetrievalPage(index, prepared, request).items[0];
    expect(hit.match_reasons).toContain("alias"); expect(hit.candidate_ref).toContain("#s4"); expect(hit.evidence_lids).toEqual(["10.4"]);
    expect(hit.embedding_truncated).toBe(true); expect(index.records[0].lexical_fields.join("")).toContain("完整材料".repeat(100));
    const browse = await prepareStructureRetrieval({ index, workspace, provider, request: { kind: "search", query: "" }, mode: "semantic_required" });
    expect(browse.sequence.hits).toHaveLength(15); expect(provider.calls).toHaveLength(0);
  });
  it("rejects tokenizer overflow before any document call instead of silently truncating", async () => {
    const { index, workspace, provider } = fixture(); provider.count_tokens = text => text.includes("14") ? 129 : 10;
    await expect(prepareStructureRetrieval({ index, workspace, provider, request: { kind: "search", query: "状态" }, mode: "semantic_required" })).rejects.toThrow(/tokenizer limit/);
    expect(provider.calls).toHaveLength(0);
  });
});
