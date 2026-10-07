import { ScriptedModel } from "./harness.ts";
export class MetadataModel extends ScriptedModel {
  async resolveModel(provider: string, model: string) { return { provider, id: model, name: model, context: { contextWindow: 1000000 } }; }
}

export function objects(value: unknown): any[] {
  if (typeof value === "string") { try { return [JSON.parse(value)]; } catch { return []; } }
  if (value && typeof value === "object") return Object.values(value).flatMap(objects);
  return [];
}

export function candidateFor(contract: any, semantic: string): unknown {
  if (contract.stage === "pass1") return { nodes: [{ id: "claim:1.2:atomic", type: "claim", name: "Transactions group operations into one atomic change.", occurrences: [], source_lid: "1.2" }], edges: [] };
  if (contract.stage === "profile_sidecar") return { discourse_items: JSON.parse(semantic.match(/visible_lids: (\[[^\n]+\])/u)![1]).map((lid: string) => ({ lid, mode: "informative", relations: [] })) };
  if (contract.stage === "pass2") return { edges: [] };
  const input = JSON.parse(semantic.slice(semantic.indexOf("{"), semantic.lastIndexOf("}") + 1));
  if (contract.stage === "formal_objects") {
    const binding = (lid: string) => ({ source_id: input.source.source_id, source_revision: input.source.source_revision, lid });
    const passage = input.source.passages.find((p: any) => p.text.includes("Transactions group"));
    return { objects: [{ key: "transaction", kind: "concept", meaning: passage.text, aliases: [],
      source_bindings: [binding(passage.lid)], conditions: [], candidate_refs: [], participants: [], component_keys: [] }],
      prerequisites: [], correspondences: [], coverage: input.structure.spine.map((unit: any) => ({
        unit_lid: unit.lid, object_keys: ["transaction"], explanation: "Defines atomic transactions.",
        source_bindings: input.source.passages.filter((p: any) => p.unit_lid === unit.lid).map((p: any) => binding(p.lid)),
      })) };
  }
  if (contract.stage === "cognitive_materials") {
    const reading = input.work.readings.find((p: any) => p.lid === input.target.lid);
    if (!reading) return { kind: "read", lid: input.target.lid };
    return { kind: "finish", material: { target_id: input.target.id, kind: "definition",
      object_refs: input.objects.map((o: any) => o.ref), purpose: input.target.reason.text,
      steps: [{ id: "definition", content: reading.text, source_bindings: [{
        source_id: input.source_id, source_revision: input.source_revision, lid: reading.lid,
      }] }], connections: [], conditions: [], gaps: [], patterns: [] } };
  }
  if (contract.stage === "teaching_publish") return { samples: input.samples.map((sample: any) => ({
    sample_id: sample.sample_id, verdict: "pass", reason: "The definition agrees with the supplied original passage.",
    source_bindings: sample.evidence_bindings,
  })) };
  if (contract.work_unit_kind === "structure_unit") {
    const lid = input.excerpts.find((e: any) => e.text.includes("Transactions group")).lid;
    return { unit_card: { unit_lid: input.unit_lid, role: "foundation", summary: { text: "Transactions group operations atomically.", evidence_lids: [lid] },
      candidate_key_stops: [{ id: "stop-1", lid, type: "definition", reason: { text: "Defines the transaction boundary.", evidence_lids: [lid] } }], depends_on: [], evidence_lids: [lid] } };
  }
  if (contract.work_unit_kind === "structure_stitch") {
    return { spine: input.unit_cards.map((card: any) => ({ lid: card.unit_lid, role: card.role, summary: card.summary, key_stop_ids: card.candidate_key_stops.map((s: any) => s.id), depends_on: [] })),
      key_stops: input.unit_cards.flatMap((card: any) => card.candidate_key_stops), throughlines: [] };
  }
  if (contract.work_unit_kind === "structure_relation_select") return { groups: [] };
  if (contract.work_unit_kind === "structure_relation_delta") return { new_throughlines: [], extend_throughlines: [], merge_throughlines: [], add_dependencies: [] };
  throw new Error(`unsupported fixture candidate ${contract.stage}/${contract.work_unit_kind}`);
}

