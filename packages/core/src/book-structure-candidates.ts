import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import type { AnchoredText, BookStructureKeyStop, BookStructureSpineRole } from "./book-structure";
import type { BookStructureReferenceScope } from "./book-structure-evidence";

/** Inputs are current accepted contributions, never unvalidated model output. */
export interface StructureCandidateContribution {
  contribution_ref: string;
  unit_lid: string;
  stops: Array<BookStructureKeyStop & { meaning?: string; conditions?: string[]; aliases?: string[]; original_ref?: string }>;
  reference_scope: BookStructureReferenceScope;
}

export interface StructureStopCandidate {
  ref: string;
  contribution_ref: string;
  local_id: string;
  unit_lid: string;
  section_lid: string;
  lid: string;
  type: BookStructureKeyStop["type"];
  title?: string;
  meaning: string;
  conditions: string[];
  aliases?: string[];
  reason: AnchoredText;
  evidence_lids: string[];
}

export interface StructureCandidateCatalog {
  version: "book_structure_candidates.v1";
  candidates: StructureStopCandidate[];
}

export const structureCandidateRef = (contribution: string, localId: string) => `${contribution}#${encodeURIComponent(localId)}`;

export function collectStructureCandidates(contributions: StructureCandidateContribution[], sections: string[] = []): StructureCandidateCatalog {
  const candidates = new Map<string, StructureStopCandidate>();
  const aliases: Array<{ original: string; candidate: StructureStopCandidate }> = [];
  for (const source of contributions) {
    const allowed = new Set(source.reference_scope.evidence_by_unit[source.unit_lid] ?? []);
    for (const stop of source.stops) {
      const evidence = [...new Set([stop.lid, ...stop.reason.evidence_lids])].sort();
      if (!stop.reason.evidence_lids.length || evidence.some(lid => !allowed.has(lid))) {
        throw new Error(`candidate evidence outside accepted contribution: ${source.contribution_ref}#${stop.id}`);
      }
      const candidate: StructureStopCandidate = {
        ref: structureCandidateRef(source.contribution_ref, stop.id), contribution_ref: source.contribution_ref,
        local_id: stop.id, unit_lid: source.unit_lid,
        section_lid: sections.filter(lid => stop.lid === lid || stop.lid.startsWith(lid + "."))
          .sort((a, b) => b.length - a.length)[0] ?? source.unit_lid,
        lid: stop.lid, type: stop.type, ...(stop.title ? { title: stop.title } : {}),
        // Legacy cards have no separate conditions field: keep their entire anchored
        // statement, rather than inventing conditions from a title or a LID.
        meaning: stop.meaning ?? stop.reason.text, conditions: [...(stop.conditions ?? [])],
        ...(stop.aliases ? { aliases: [...stop.aliases] } : {}),
        reason: structuredClone(stop.reason), evidence_lids: evidence,
      };
      if (stop.original_ref) { aliases.push({ original: stop.original_ref, candidate }); continue; }
      const previous = candidates.get(candidate.ref);
      if (previous && !isDeepStrictEqual(previous, candidate)) throw new Error(`conflicting candidate reference: ${candidate.ref}`);
      candidates.set(candidate.ref, candidate);
    }
  }
  for (const { original, candidate } of aliases) {
    const previous = candidates.get(original);
    if (!previous) throw new Error(`unknown original candidate reference: ${original}`);
    const content = ({ ref: _ref, contribution_ref: _source, local_id: _id, ...rest }: StructureStopCandidate) => rest;
    if (!isDeepStrictEqual(content(previous), content(candidate))) throw new Error(`changed original candidate: ${original}`);
  }
  return { version: "book_structure_candidates.v1", candidates: [...candidates.values()].sort((a, b) => a.ref.localeCompare(b.ref)) };
}

export interface StructureChapterSelection {
  unit_lid: string;
  role: BookStructureSpineRole;
  summary: AnchoredText;
  accepted_stop_refs: string[];
  macro_stop_refs: string[];
}
export interface AcceptedStructureChapterSelection {
  selection: StructureChapterSelection;
  reference_scope: BookStructureReferenceScope;
}

export const structureAnchoredTextSchema = z.object({ text: z.string().min(1).max(2400), evidence_lids: z.array(z.string().min(1)).min(1) }).strict();
const selectionSchema = z.object({
  unit_lid: z.string().min(1), role: z.enum(["setup", "foundation", "method", "application", "case", "synthesis"]),
  summary: structureAnchoredTextSchema, accepted_stop_refs: z.array(z.string()), macro_stop_refs: z.array(z.string()),
}).strict();

export function acceptStructureChapterSelection(value: unknown, catalog: StructureCandidateCatalog,
  unitLid: string, deliveredScope: BookStructureReferenceScope): AcceptedStructureChapterSelection {
  const selection = selectionSchema.parse(value);
  const candidates = new Map(catalog.candidates.map(c => [c.ref, c]));
  if (selection.unit_lid !== unitLid) throw new Error("chapter selection unit changed");
  for (const ref of selection.accepted_stop_refs) {
    if (candidates.get(ref)?.unit_lid !== unitLid) throw new Error(`unknown or foreign chapter candidate: ${ref}`);
  }
  if (new Set(selection.accepted_stop_refs).size !== selection.accepted_stop_refs.length
    || new Set(selection.macro_stop_refs).size !== selection.macro_stop_refs.length) throw new Error("duplicate chapter selection reference");
  if (selection.macro_stop_refs.some(ref => !selection.accepted_stop_refs.includes(ref))) throw new Error("macro route must be a subset of chapter choices");
  const allowed = new Set(deliveredScope.evidence_by_unit[unitLid] ?? []);
  if (selection.summary.evidence_lids.some(lid => !allowed.has(lid))) throw new Error("chapter explanation cites undelivered evidence");
  return { selection, reference_scope: { unit_lids: [unitLid], dependency_target_lids: [],
    evidence_by_unit: { [unitLid]: [...new Set(selection.summary.evidence_lids)].sort() } } };
}
