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
  if(contract.stage==='course'){
    const lids=input.source.passages.map((p:any)=>p.lid);
    if(input.task_kind==='scope')return {title:'Atomic changes',question:'What belongs to one transaction?',route:'chapters',unit_lids:input.structure.spine.map((u:any)=>u.lid)};
    if(input.task_kind==='plan')return {language:'en',unit_title:'Transaction boundary',lessons:[{id:'atomic',question:'What does one atomic change mean?',objective:'Explain the boundary described in the definition.',prerequisites:'Understand an operation and a change.',estimated_minutes:20,duration_basis:'Read and reason for 10 minutes, then apply and review for 10.',source_lids:lids,blueprint:'Use the definition to group operations; the final task asks what the group is treated as.'}]};
    if(input.task_kind==='lesson')return {introduction:'Read the definition and identify what is grouped.',prediction:{question:'What does a transaction group?',options:[]},experiment:{kind:'inquiry',activity:'reading',instruction:'Identify the grouping and its result.',material:input.source.passages.map((p:any)=>p.text).join('\n\n'),response_prompt:'Quote the relevant words and explain the relationship.'},explanation:'The operations are grouped into one atomic change.',task:{question:'How does the definition describe the result of grouping operations?',options:[]},hint:'Look at the phrase following operations.',assessment:{rule:{kind:'open',items:[{id:'atomic',criterion:'Explains that the group forms one atomic change.',source_lids:lids}]},source_lids:lids,source_sufficient:true,feedback:'after_submit'},solution:'The resulting group is treated as one atomic change.',reflection_prompt:'Connect the operations with the resulting change.',reading:{before_experiment:'Find the transaction definition.',after_experiment:'Compare your explanation with the phrase one atomic change.'}};
    if(input.task_kind==='solve')return {solutions:input.questions.map((q:any)=>({question:q.question,reasoning:'The definition groups operations into one atomic change.',answer:'Operations are grouped into one atomic change.'})),issues:[]};
    if(input.task_kind==='review')return {accepted:true,checked_lids:lids,reasoning:'The activity and both independent solutions use the supplied definition.',issues:[]};
  }
  if (contract.stage === "book_structure") {
    if (input.phase === "outline") return { chapters: input.input.chapters.map((c: any) => ({ unit_lid: c.unit_lid,
      question: c.overview, progression: c.overview })), themes: [] };
    if (input.phase === "plan") return { themes: [] };
    if (input.phase === "reconcile") return { edits: [], merges: [], unresolved: [] };
    if (input.phase === "chapter") {
      const c = input.input;
      if (!c.inspected.length) return c.index.items.length ? { kind: "inspect", refs: [c.index.items[0].ref] }
        : c.excerpts.length ? { kind: "select", selection: { unit_lid: c.unit_lid, role: "foundation",
          summary: { text: "Atomic transaction boundary.", evidence_lids: c.excerpts[0].lids }, accepted_stop_refs: [], macro_stop_refs: [] } }
          : { kind: "read", indices: [c.source_index[0].index] };
      return { kind: "select", selection: { unit_lid: c.unit_lid, role: "foundation", summary: c.inspected[0].reason,
        accepted_stop_refs: c.seen_refs, macro_stop_refs: [c.inspected[0].ref] } };
    }
    if (contract.work_unit_kind === "structure_fragment") {
      const lid = input.parent_unit_lid, evidence = input.reference_scope.evidence_by_unit[lid];
      return { version: "book_structure_fragment_observation.v1", parent_unit_lid: lid,
        summary_fragments: evidence.length ? [{ text: "Transactions group operations atomically.", evidence_lids: [evidence[0]] }] : [],
        candidate_key_stops: evidence.slice(0, 1).map((e: string) => ({ id: "atomic", lid: e, type: "definition",
          meaning: "Transactions group operations into one atomic change.", conditions: [],
          reason: { text: "Defines an atomic transaction.", evidence_lids: [e] } })),
        role_hints: ["foundation"], dependency_hints: [], evidence_lids: evidence };
    }
  }
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

