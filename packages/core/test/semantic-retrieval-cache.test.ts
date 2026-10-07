import { mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { fakeEmbedding } from "./fixtures/embedding-provider";
import { fixture } from "../testdata/semantic-retrieval/gold";
import { projectRetrievalCatalog, retrievalCatalog } from "../src/semantic-retrieval";
import { prepareRetrievalVectors, rankSemanticRecords } from "../src/semantic-retrieval-cache";

const workspace = () => mkdtempSync(path.join(tmpdir(), "sr-cache-"));
const projection = () => { const { work, previous } = fixture(); return projectRetrievalCatalog(retrievalCatalog(work.proposal, previous)); };
describe("regenerable semantic vectors", () => {
  it("reuses unchanged text and updates independent/referencing objects only", async () => {
    const { work } = fixture(), provider = fakeEmbedding(), root = workspace();
    provider.limits.max_input_tokens = 10000;
    const prepare = () => prepareRetrievalVectors({ workspace: root, provider, records: projectRetrievalCatalog(retrievalCatalog(work.proposal)) });
    expect((await prepare()).embedded_documents).toBe(27);
    expect((await prepare()).embedded_documents).toBe(0);
    work.proposal.objects.find(o => o.key === "base/temperature")!.meaning = "new temperature";
    expect((await prepare()).embedded_documents).toBe(1);
    work.proposal.objects.find(o => o.key === "base/car-a")!.meaning = "heavy car";
    expect((await prepare()).embedded_documents).toBe(3);
    work.proposal.objects.find(o => o.key === "base/out")!.meaning = "new outward journey";
    // Both conditional relations reference the outward leg, as does the composite.
    expect((await prepare()).embedded_documents).toBe(4);
    work.proposal.objects.push({ ...work.proposal.objects[0], key: "identical-text" });
    const shared = await prepare();
    expect(shared.embedded_documents).toBe(0);
    expect(shared.cache.records).toHaveLength(28);
  });
  it("rebuilds deterministic ranks after deletion without touching action/identity files", async () => {
    const records = projection(), provider = fakeEmbedding(), root = workspace();
    provider.limits.max_input_tokens = 10000;
    mkdirSync(path.join(root, ".build", "teaching"), { recursive: true });
    writeFileSync(path.join(root, ".build", "teaching", "action.json"), "accepted-action");
    writeFileSync(path.join(root, "formal_objects.json"), "published-identity");
    const first = await prepareRetrievalVectors({ workspace: root, records, provider });
    const query = { provider: provider.identity, vector: [1, 2, 3] };
    const rank = rankSemanticRecords(records, first.cache, query);
    rmSync(path.join(root, ".build", "semantic-retrieval"), { recursive: true });
    const second = await prepareRetrievalVectors({ workspace: root, records, provider });
    expect(rankSemanticRecords(records, second.cache, query)).toEqual(rank);
    expect(readFileSync(path.join(root, ".build", "teaching", "action.json"), "utf8")).toBe("accepted-action");
    expect(readFileSync(path.join(root, "formal_objects.json"), "utf8")).toBe("published-identity");
    const invalid = structuredClone(second.cache);
    invalid.vectors[records[0].content_digest] = [1];
    expect(() => rankSemanticRecords(records, invalid, query)).toThrow("vector");
    writeFileSync(path.join(root, ".build", "semantic-retrieval", "cache.json"), JSON.stringify(invalid));
    expect((await prepareRetrievalVectors({ workspace: root, records, provider })).embedded_documents).toBe(1);
  });
  it("invalidates model/config/projection changes, retains completed batches after failure", async () => {
    const records = projection(), provider = fakeEmbedding(), root = workspace();
    provider.limits.max_input_tokens = 10000;
    const prepare = (projection_version?: string) => prepareRetrievalVectors({ workspace: root, records, provider, projection_version });
    await prepare();
    for (const change of [() => provider.identity.model_id = "next", () => provider.identity.embedding_config.normalize = true]) {
      change(); expect((await prepare()).embedded_documents).toBe(28);
    }
    expect((await prepare("next-projection")).embedded_documents).toBe(28);
    let batches = 0;
    const original = provider.embed_documents;
    provider.embed_documents = async (texts, options) => { if (++batches === 2) throw new Error("offline"); return original(texts, options); };
    await expect(prepare()).rejects.toThrow("offline");
    provider.embed_documents = original;
    expect((await prepare()).embedded_documents).toBe(20);
    const cache = (await prepare()).cache;
    expect(() => rankSemanticRecords(records, cache, { provider: { ...provider.identity, dimensions: 4 }, vector: [1, 2, 3, 4] })).toThrow("identity");
  });
  it("filters exited keys and self before ranking and breaks ties by canonical key", async () => {
    const records = projection(), provider = fakeEmbedding(), root = workspace();
    provider.limits.max_input_tokens = 10000;
    provider.embed_documents = async texts => ({ vectors: texts.map(() => [1, 0, 0]) });
    const { cache } = await prepareRetrievalVectors({ workspace: root, records, provider });
    const current = [records[3], records[1], records[3], records[2]];
    expect(rankSemanticRecords(current, cache, { provider: provider.identity, vector: [1, 0, 0] }, records[2].key).map(r => r.key))
      .toEqual([records[1].key, records[3].key]);
  });
});
