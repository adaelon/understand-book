import { randomUUID } from "node:crypto";
import { z } from "zod";
import { canonicalBuildJson } from "./build-intent";
import type { BookStructureSidecar, BookStructureUnitSource } from "./book-structure";
import type { LidNode } from "./generated/LidNode";

const text = z.string().trim().min(1);
export const LearningObjectRefZ = z.object({ source_id: text, object_id: text }).strict();
export type LearningObjectRef = z.infer<typeof LearningObjectRefZ>;
export const SourceRangeZ = z.object({ start: z.number().int().nonnegative(), end: z.number().int().positive() }).strict();
export const SourceBindingZ = z.object({ source_id: text, source_revision: text, lid: text, range_utf16: SourceRangeZ.optional() }).strict();
export type TeachingSourceBinding = z.infer<typeof SourceBindingZ>;
export interface TeachingSource {
  source_id: string;
  source_revision: string;
  passages: Array<{ lid: string; unit_lid: string; text: string; kind?: LidNode["kind"] }>;
}
export function checkTeachingBindings(bindings: TeachingSourceBinding[], source: TeachingSource): void {
  if (!bindings.length) throw new Error("teaching source evidence is required");
  for (const binding of bindings) {
    if (binding.source_id !== source.source_id || binding.source_revision !== source.source_revision
      || !source.passages.some(p => p.lid === binding.lid && p.text.trim())) {
      throw new Error(`teaching source binding is unavailable or stale: ${binding.lid}`);
    }
    if (binding.range_utf16 && (binding.range_utf16.end <= binding.range_utf16.start
      || binding.range_utf16.end > source.passages.find(p => p.lid === binding.lid)!.text.length)) throw new Error("teaching source range invalid");
  }
}

const MeaningZ = z.object({
  meaning: text, kind: z.enum(["concept", "claim", "relation", "composite", "method"]),
  aliases: z.array(text), source_bindings: z.array(SourceBindingZ).min(1),
  conditions: z.array(text),
});
export const ObjectProposalZ = MeaningZ.extend({
  key: text, existing_ref: LearningObjectRefZ.optional(),
  candidate_refs: z.array(text),
  participants: z.array(z.object({ object_key: text, role: text }).strict()),
  component_keys: z.array(text),
}).strict();
export const FormalObjectProposalZ = z.object({
  objects: z.array(ObjectProposalZ),
  prerequisites: z.array(z.object({
    target_key: text, target_capability: text, prerequisite_key: text, prerequisite_capability: text,
    conditions: z.array(text), source_bindings: z.array(SourceBindingZ).min(1),
  }).strict()),
  correspondences: z.array(z.object({
    kind: z.enum(["split", "merge"]), from: z.array(LearningObjectRefZ).min(1), to_keys: z.array(text).min(1),
    reason: text, source_bindings: z.array(SourceBindingZ).min(1),
  }).strict()),
  coverage: z.array(z.object({ unit_lid: text, object_keys: z.array(text), explanation: text,
    source_bindings: z.array(SourceBindingZ).min(1) }).strict()),
}).strict();
export type FormalObjectProposal = z.infer<typeof FormalObjectProposalZ>;
export interface LearningObjectRevision extends z.infer<typeof MeaningZ> {
  ref: LearningObjectRef;
  object_revision: number;
  participants: Array<{ object_ref: LearningObjectRef; role: string }>;
  component_refs: LearningObjectRef[];
}
export interface FormalObjects {
  version: "formal_objects.v1";
  source_id: string;
  source_revision: string;
  revision: number;
  objects: LearningObjectRevision[];
  active_refs: LearningObjectRef[];
  candidate_refs: Record<string, LearningObjectRef[]>;
  graph_edges: Array<{ relation_ref: LearningObjectRef }>;
  prerequisites: Array<{ target: { object_ref: LearningObjectRef; capability: string };
    prerequisite: { object_ref: LearningObjectRef; capability: string };
    conditions: string[]; source_bindings: TeachingSourceBinding[] }>;
  correspondences: Array<{ kind: "split" | "merge"; from: LearningObjectRef[]; to: LearningObjectRef[];
    reason: string; source_bindings: TeachingSourceBinding[] }>;
  coverage: Array<{ unit_lid: string; object_refs: LearningObjectRef[]; explanation: string; source_bindings: TeachingSourceBinding[] }>;
  acceptance: { operation_id: string; proposal: FormalObjectProposal };
}
export const sameObjectRef = (a: LearningObjectRef, b: LearningObjectRef) => a.source_id === b.source_id && a.object_id === b.object_id;

/** Semantics belong to the proposer; this gate never merges names or graph node IDs. */
export function acceptFormalObjects(input: { source: TeachingSource; proposal: unknown; operation_id: string;
  previous?: FormalObjects; allocate_id?: () => string }): FormalObjects {
  const proposal = FormalObjectProposalZ.parse(input.proposal);
  const { source, previous } = input;
  if (previous && previous.source_id !== source.source_id) throw new Error("formal objects belong to another source");
  if (previous?.acceptance.operation_id === input.operation_id) {
    if (previous.source_revision !== source.source_revision || canonicalBuildJson(previous.acceptance.proposal) !== canonicalBuildJson(proposal)) {
      throw new Error("formal object acceptance operation changed");
    }
    return previous;
  }
  const keys = new Map<string, LearningObjectRef>();
  for (const object of proposal.objects) {
    if (keys.has(object.key)) throw new Error(`duplicate object key: ${object.key}`);
    if (object.existing_ref && !previous?.active_refs.some(ref => sameObjectRef(ref, object.existing_ref!))) {
      throw new Error("existing object identity is not active in this source");
    }
    const ref = object.existing_ref ?? { source_id: source.source_id, object_id: (input.allocate_id ?? randomUUID)() };
    if ([...keys.values()].some(other => sameObjectRef(other, ref))) throw new Error("two proposals reuse one object identity");
    keys.set(object.key, ref);
  }
  const get = (key: string) => {
    const ref = keys.get(key);
    if (!ref) throw new Error(`unresolved formal object key: ${key}`);
    return ref;
  };
  const revisions = [...(previous?.objects ?? [])];
  for (const object of proposal.objects) {
    checkTeachingBindings(object.source_bindings, source);
    if (object.kind === "relation" && object.participants.length < 2) throw new Error("learnable relation requires participants and roles");
    if (object.kind === "composite" && !object.component_keys.length) throw new Error("composite object requires components");
    const ref = get(object.key);
    if (object.component_keys.includes(object.key)) throw new Error("object cannot contain itself");
    const old = revisions.filter(item => sameObjectRef(item.ref, ref)).at(-1);
    const content = { ref, meaning: object.meaning, kind: object.kind, aliases: object.aliases,
      source_bindings: object.source_bindings, conditions: object.conditions,
      participants: object.participants.map(p => ({ object_ref: get(p.object_key), role: p.role })),
      component_refs: object.component_keys.map(get) };
    if (!old || canonicalBuildJson({ ...old, object_revision: undefined }) !== canonicalBuildJson(content)) {
      revisions.push({ ...content, object_revision: (old?.object_revision ?? 0) + 1 });
    }
  }
  const correspondences = proposal.correspondences.map(c => {
    checkTeachingBindings(c.source_bindings, source);
    if (c.kind === "split" ? c.from.length !== 1 || c.to_keys.length < 2 : c.from.length < 2 || c.to_keys.length !== 1) {
      throw new Error("invalid split/merge cardinality");
    }
    const to = c.to_keys.map(get);
    if (to.some(ref => previous?.objects.some(o => sameObjectRef(o.ref, ref)))) throw new Error("split/merge must create new identities");
    for (const from of c.from) {
      if (!previous?.active_refs.some(ref => sameObjectRef(ref, from)) || [...keys.values()].some(ref => sameObjectRef(ref, from))) {
        throw new Error("split/merge requires retired active identities");
      }
    }
    return { kind: c.kind, from: c.from, to, reason: c.reason, source_bindings: c.source_bindings };
  });
  for (const old of previous?.active_refs ?? []) {
    if (![...keys.values()].some(ref => sameObjectRef(ref, old)) && !correspondences.some(c => c.from.some(ref => sameObjectRef(ref, old)))) {
      throw new Error("removing an identity requires an explicit correspondence");
    }
  }
  const units = new Set(source.passages.map(p => p.unit_lid));
  if (proposal.coverage.length !== units.size || new Set(proposal.coverage.map(c => c.unit_lid)).size !== units.size) throw new Error("formal object coverage incomplete");
  for (const c of proposal.coverage) {
    checkTeachingBindings(c.source_bindings, source);
    if (!units.has(c.unit_lid) || c.source_bindings.some(b => !source.passages.some(p => p.lid === b.lid && p.unit_lid === c.unit_lid))) throw new Error("coverage cites another unit");
  }
  const candidateRefs: Record<string, LearningObjectRef[]> = {};
  for (const object of proposal.objects) for (const id of object.candidate_refs) (candidateRefs[id] ??= []).push(get(object.key));
  return { version: "formal_objects.v1", source_id: source.source_id, source_revision: source.source_revision,
    revision: (previous?.revision ?? 0) + 1, objects: revisions, active_refs: [...keys.values()], candidate_refs: candidateRefs,
    graph_edges: proposal.objects.filter(o => o.kind === "relation").map(o => ({ relation_ref: get(o.key) })),
    prerequisites: proposal.prerequisites.map(p => {
      checkTeachingBindings(p.source_bindings, source);
      return { target: { object_ref: get(p.target_key), capability: p.target_capability },
        prerequisite: { object_ref: get(p.prerequisite_key), capability: p.prerequisite_capability },
        conditions: p.conditions, source_bindings: p.source_bindings };
    }), correspondences: [...(previous?.correspondences ?? []), ...correspondences],
    coverage: proposal.coverage.map(c => ({ unit_lid: c.unit_lid, object_refs: c.object_keys.map(get), explanation: c.explanation, source_bindings: c.source_bindings })),
    acceptance: { operation_id: text.parse(input.operation_id), proposal } };
}

/** Uses the accepted public asset packet; graph edges remain recall hints. */
export function formalObjectInput(source: TeachingSource, units: BookStructureUnitSource[], structure: BookStructureSidecar, previous?: FormalObjects) {
  return { source, structure, candidates: units.map(unit => ({ unit_lid: unit.unit_lid,
    graph_nodes: unit.graph_nodes, graph_edges: unit.graph_edges, discourse_items: unit.discourse_items,
    formula_semantics: unit.formula_semantics, pass2_edges: unit.pass2_edges })),
    previous_objects: previous?.active_refs.map(ref => previous.objects.filter(o => sameObjectRef(o.ref, ref)).at(-1)) ?? [] };
}
