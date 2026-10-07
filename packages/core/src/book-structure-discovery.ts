import type { LidNode } from "./generated/LidNode";
import { bookStructureCanonicalTitle, bookStructureUnitHash, type BookStructureUnitSource,
  type BookStructureFragmentObservationV1, type BookStructureUnitArtifact } from "./book-structure";
import type { StructureOutlineInput } from "./book-structure-planning";

export interface BookStructureDiscoveryProgress {
  fragments: { done: number; total: number };
  core_leaves: { done: number; total: number };
  candidates: number;
}

export const STRUCTURE_DISCOVERY_PROMPT = `# BookStructure source discovery

## Automatic Build Executor Envelope

When the caller supplies an automatic_build_executor.v1 envelope, execute input_command and use its stdout as the input below. Write the strict candidate JSON at candidate_path, execute submit_command, and return only its receipt. If a native usage receipt is available, save it at usage_path; never invent exact token counts. Use heartbeat_command while active; on failure execute fail_command and return the failure receipt. Without this envelope, follow the strict JSON contract below.

Discover grounded candidates from this natural section's complete core range.
The provisional outline is orientation only; revise its assumptions when the body disagrees.
Read every core paragraph, preserving mechanisms, conditions, counterexamples and teaching value.
Emit book_structure_fragment_observation.v1 with parent_unit_lid, summary_fragments,
candidate_key_stops, role_hints, dependency_hints (empty), and evidence_lids.
AnchoredText = {"text":string,"evidence_lids":string[]}; keep each text under 600 characters.
Candidate = {"id":string,"lid":string,"type":"definition"|"formula"|"claim"|"example"|"turning_point"|"warning"|"summary",
"meaning":string,"conditions":string[],"reason":AnchoredText,"title"?:string,"aliases"?:string[]}.
Use distinct local IDs for different meanings even at the same LID. State the mechanism in meaning,
its applicable conditions explicitly (empty if unconditional), and why it is worth teaching in reason.
Do not impose a fixed number of points per section. Empty candidates are valid for orientation-only material.
Produce a concise grounded overview in summary_fragments; summaries do not replace any candidates.
Only reference_scope.evidence_by_unit[parent_unit_lid] grants evidence. Titles, outline and graph IDs
are navigation, never proof. Preserve the provenance and conditions of supplied discourse/formula results.
Do not cite a heading or manufacture evidence. Heading-only core packets return empty summaries and candidates.
role_hints uses setup|foundation|method|application|case|synthesis. Output strict JSON only.
`;

/** The discovery path reads the complete canonical span, never the legacy 1200-character preview. */
export function structureDiscoverySources(sources: BookStructureUnitSource[], nodes: LidNode[], source: string) {
  const byLid = new Map(nodes.map(n => [n.lid, n]));
  return sources.map(unit => ({ ...unit, excerpts: unit.leaf_lids.map(lid => {
    const node = byLid.get(lid)!;
    return { lid, text: source.slice(node.span.start, node.span.end).trim() };
  }) }));
}

export const isStructureBodyExcerpt = (text: string) => Boolean(text.trim()) && !/^#{1,6}\s/u.test(text.trim());

export function structureDiscoverySections(unit: BookStructureUnitSource, nodes: LidNode[], source: string) {
  const byLid = new Map(nodes.map(n => [n.lid, n]));
  const sections = nodes.filter(n => (n.kind === "section" || n.kind === "chapter") &&
    (n.lid === unit.unit_lid || n.lid.startsWith(unit.unit_lid + ".")));
  return Object.fromEntries(unit.leaf_lids.map(lid => {
    const section = sections.filter(n => lid === n.lid || lid.startsWith(n.lid + "."))
      .sort((a, b) => b.lid.length - a.lid.length)[0] ?? byLid.get(unit.unit_lid)!;
    return [lid, { lid: section.lid, title: bookStructureCanonicalTitle(section, byLid, source) }];
  }));
}

/** Small source excerpts seed a provisional framework; they make no claim of body coverage. */
export function structureSourceOutline(sources: BookStructureUnitSource[], nodes: LidNode[], source: string): StructureOutlineInput {
  return { version: "book_structure_outline_input.v1", preface: [], chapters: sources.map(unit => {
    const sections = [...new Map(Object.values(structureDiscoverySections(unit, nodes, source)).map(s => [s.lid, s])).values()];
    const body = unit.excerpts.filter(e => isStructureBodyExcerpt(e.text));
    const summarySection = sections.find(s => /小结|总结|summary|conclusion/iu.test(s.title));
    const ending = summarySection ? body.find(e => e.lid.startsWith(summarySection.lid + ".")) : undefined;
    const selected = [...new Map([body[0], ending ?? body[1]].filter(e => e !== undefined).map(e => [e.lid, e])).values()];
    return { unit_lid: unit.unit_lid, title: unit.title ?? "未命名单元", sections,
      overview: { text: selected.map(e => `[${e.lid}] ${e.text.slice(0, 500)}`).join("\n"), evidence_lids: selected.map(e => e.lid) } };
  }) };
}

/** Program accumulation only. Every original observation remains independently addressable on disk. */
export function accumulateStructureDiscovery(source: BookStructureUnitSource,
  observations: BookStructureFragmentObservationV1[]): BookStructureUnitArtifact {
  const summaries = observations.flatMap(o => o.summary_fragments);
  return { content_hash: bookStructureUnitHash(source), output: { unit_card: {
    unit_lid: source.unit_lid, role: observations.flatMap(o => o.role_hints)[0] ?? "setup",
    summary: { text: summaries.map(s => s.text).join("\n"), evidence_lids: [...new Set(summaries.flatMap(s => s.evidence_lids))] },
    candidate_key_stops: [], depends_on: [], evidence_lids: [...new Set(observations.flatMap(o => o.evidence_lids))],
  } } };
}
