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
  if (contract.work_unit_kind === "structure_unit") {
    const lid = input.excerpts.find((e: any) => e.text.trim()).lid;
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

