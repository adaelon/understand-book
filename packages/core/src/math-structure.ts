import { z } from "zod";
import type { LidNode } from "./generated/LidNode";
import type { ProfileArtifactHeader } from "./profile-artifact";
import { ProfileArtifactHeaderZ } from "./zod";

export const MATH_STRUCTURE_VERSION = "math_structure.v1" as const;
export const MathStructureKindZ = z.enum([
  "definition", "theorem", "lemma", "proposition", "corollary", "assumption", "proof", "exercise", "algorithm",
]);
const text = z.string().trim().min(1);
const evidence = z.array(text).min(1);
const quotation = z.object({ quote: text, evidence_lids: evidence }).strict();
export const MathStructureReferenceCandidateZ = quotation.extend({
  relation: z.enum(["uses", "proves", "asks_to_prove", "refers_to"]),
  target_kind: MathStructureKindZ,
  target_label: text,
  scope: z.enum(["local", "external"]),
}).strict();
export const MathStructureCandidateZ = z.object({
  anchor_lid: text,
  kind: MathStructureKindZ,
  label: text.optional(),
  evidence_lids: evidence,
  assumptions: z.array(quotation),
  references: z.array(MathStructureReferenceCandidateZ),
}).strict();
export type MathStructureCandidate = z.infer<typeof MathStructureCandidateZ>;
const reference = MathStructureReferenceCandidateZ.extend({
  resolution: z.enum(["resolved", "missing", "ambiguous", "external"]),
  target_lid: text.optional(),
}).strict();
export const MathStructureItemZ = MathStructureCandidateZ.extend({ references: z.array(reference) }).strict();
export const MathStructureIndexZ = z.object({
  version: z.literal(MATH_STRUCTURE_VERSION),
  header: ProfileArtifactHeaderZ.extend({ profile_id: z.literal("technical_learning") }),
  items: z.array(MathStructureItemZ),
}).strict();
export type MathStructureIndex = z.infer<typeof MathStructureIndexZ>;
export type MathStructureItem = z.infer<typeof MathStructureItemZ>;

export interface MathStructureSource {
  source: string;
  byLid: ReadonlyMap<string, Pick<LidNode, "span">>;
}

/** A rejected source quotation is a candidate error, not a writer crash. */
export class MathStructureQuoteError extends Error {
  constructor(message: string, readonly json_pointer: string) {
    super(message);
    this.name = "MathStructureQuoteError";
  }
  readonly expected = "Copy an exact contiguous quotation from the cited source evidence, preserving whitespace and math markup. Cite all required evidence LIDs; do not paraphrase the quotation.";
}

function evidencePassages(lids: string[], input: MathStructureSource): string[] {
  const nodes = [...new Set(lids)].map((lid) => {
    const node = input.byLid.get(lid);
    if (!node) throw new Error(`math structure evidence LID is unavailable: ${lid}`);
    return node;
  }).sort((a, b) => a.span.start - b.span.start);
  const spans: Array<{ start: number; end: number }> = [];
  for (const node of nodes) {
    const previous = spans.at(-1);
    if (previous && !input.source.slice(previous.end, node.span.start).trim()) previous.end = Math.max(previous.end, node.span.end);
    else spans.push({ ...node.span });
  }
  return spans.map((span) => input.source.slice(span.start, span.end));
}

/** Restore presentation-only quote differences to a unique span of the cited source.
 * The stored candidate stays unchanged; the artifact still passes exact quote validation.
 */
export function restoreMathStructureSourceQuotes(candidates: MathStructureCandidate[], input: MathStructureSource): MathStructureCandidate[] {
  const restore = (quote: string, lids: string[]): string => {
    const passages = evidencePassages(lids, input);
    if (passages.some(passage => passage.includes(quote))) return quote;
    const needle = quote.replace(/\s/gu, "");
    if (!needle) return quote;
    let match: string | undefined;
    for (const passage of passages) {
      let normalized = "";
      const offsets: number[] = [];
      for (let index = 0; index < passage.length; index += 1) {
        if (/\s/u.test(passage[index])) continue;
        normalized += passage[index];
        offsets.push(index);
      }
      const start = normalized.indexOf(needle);
      if (start < 0) continue;
      if (match !== undefined || normalized.indexOf(needle, start + 1) >= 0) return quote;
      match = passage.slice(offsets[start], offsets[start + needle.length - 1] + 1);
    }
    return match ?? quote;
  };
  return candidates.map(item => ({
    ...item,
    ...(item.label === undefined ? {} : { label: restore(item.label, item.evidence_lids) }),
    assumptions: item.assumptions.map(assumption => ({
      ...assumption, quote: restore(assumption.quote, assumption.evidence_lids),
    })),
    references: item.references.map(reference => ({
      ...reference,
      quote: restore(reference.quote, reference.evidence_lids),
      target_label: restore(reference.target_label, reference.evidence_lids),
    })),
  }));
}

/** Validate citations against the actual source; semantic classification remains the extractor's job. */
export function validateMathStructureCandidates(candidates: MathStructureCandidate[], input: MathStructureSource & {
  anchor_lids: string[];
  visible_lids: string[];
}): void {
  const anchors = new Set(input.anchor_lids);
  const visible = new Set(input.visible_lids);
  const seen = new Set<string>();
  const checkEvidence = (lids: string[]) => {
    if (!lids.length || lids.some((lid) => !visible.has(lid))) throw new Error("math structure evidence is outside the visible source");
    return evidencePassages(lids, input);
  };
  for (const [candidateIndex, candidate] of candidates.entries()) {
    const pointer = `/math_items/${candidateIndex}`;
    if (!anchors.has(candidate.anchor_lid) || seen.has(candidate.anchor_lid)) throw new Error("math structure anchor is out of scope or duplicated");
    seen.add(candidate.anchor_lid);
    const passages = checkEvidence(candidate.evidence_lids);
    if (!candidate.evidence_lids.includes(candidate.anchor_lid)) throw new Error("math structure evidence must include its anchor");
    if (candidate.label && !passages.some((passage) => passage.includes(candidate.label!))) throw new MathStructureQuoteError("math structure label is not quoted from its evidence", `${pointer}/label`);
    for (const [assumptionIndex, assumption] of candidate.assumptions.entries()) {
      if (assumption.evidence_lids.some((lid) => !candidate.evidence_lids.includes(lid))
        || !checkEvidence(assumption.evidence_lids).some((passage) => passage.includes(assumption.quote))) {
        throw new MathStructureQuoteError("math structure assumption is not quoted from the item evidence", `${pointer}/assumptions/${assumptionIndex}/quote`);
      }
    }
    for (const [referenceIndex, ref] of candidate.references.entries()) {
      const referencePassages = checkEvidence(ref.evidence_lids);
      if (!referencePassages.some((passage) => passage.includes(ref.quote))) throw new MathStructureQuoteError("math structure reference is not quoted from its evidence", `${pointer}/references/${referenceIndex}/quote`);
      if (!referencePassages.some((passage) => passage.includes(ref.target_label))) throw new MathStructureQuoteError("math structure target label is not quoted from its evidence", `${pointer}/references/${referenceIndex}/target_label`);
      if (ref.relation === "proves" && candidate.kind !== "proof") throw new Error("only a source proof may emit proves");
      if (ref.relation === "asks_to_prove" && candidate.kind !== "exercise") throw new Error("only an exercise may emit asks_to_prove");
    }
  }
}

const labels: Record<MathStructureCandidate["kind"], RegExp> = {
  definition: /^(?:定义|definition)[:：.]?/iu,
  theorem: /^(?:定理|theorem)[:：.]?/iu,
  lemma: /^(?:引理|lemma)[:：.]?/iu,
  proposition: /^(?:命题|proposition)[:：.]?/iu,
  corollary: /^(?:推论|corollary)[:：.]?/iu,
  assumption: /^(?:假设|assumption)[:：.]?/iu,
  proof: /^(?:证明|proof)[:：.]?/iu,
  exercise: /^(?:习题|练习|exercise|problem)[:：.]?/iu,
  algorithm: /^(?:算法|algorithm)[:：.]?/iu,
};

function labelKey(kind: MathStructureCandidate["kind"], label: string): string {
  return `${kind}:${label.replace(/\s+/gu, "").toLowerCase().replace(labels[kind], "").replaceAll("．", ".")}`;
}

export function buildMathStructureIndex(header: ProfileArtifactHeader, candidates: MathStructureCandidate[], input: MathStructureSource): MathStructureIndex {
  if (header.profile_id !== "technical_learning") throw new Error("math structure belongs to technical_learning");
  const parsed = candidates.map((candidate) => MathStructureCandidateZ.parse(candidate));
  validateMathStructureCandidates(parsed, { ...input, anchor_lids: parsed.map((item) => item.anchor_lid), visible_lids: [...input.byLid.keys()] });
  const byLabel = new Map<string, MathStructureCandidate[]>();
  for (const item of parsed) {
    if (!item.label) continue;
    const key = labelKey(item.kind, item.label);
    const matches = byLabel.get(key) ?? [];
    matches.push(item);
    byLabel.set(key, matches);
  }
  const items = parsed.sort((a, b) => input.byLid.get(a.anchor_lid)!.span.start - input.byLid.get(b.anchor_lid)!.span.start)
    .map((item): MathStructureItem => ({
      ...item,
      references: item.references.map((ref) => {
        if (ref.scope === "external") return { ...ref, resolution: "external" };
        const targets = byLabel.get(labelKey(ref.target_kind, ref.target_label)) ?? [];
        if (targets.length === 1) return { ...ref, resolution: "resolved", target_lid: targets[0].anchor_lid };
        return { ...ref, resolution: targets.length ? "ambiguous" : "missing" };
      }),
    }));
  return MathStructureIndexZ.parse({ version: MATH_STRUCTURE_VERSION, header, items });
}

/** Exact label/anchor lookup; an evidence LID also finds the owning statement or proof. */
export function queryMathStructure(index: MathStructureIndex, query: string): MathStructureItem[] {
  return index.items.filter((item) => item.anchor_lid === query || item.evidence_lids.includes(query)
    || (item.label && labelKey(item.kind, item.label) === labelKey(item.kind, query)));
}
