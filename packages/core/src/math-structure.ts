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
  for (const candidate of candidates) {
    if (!anchors.has(candidate.anchor_lid) || seen.has(candidate.anchor_lid)) throw new Error("math structure anchor is out of scope or duplicated");
    seen.add(candidate.anchor_lid);
    const passages = checkEvidence(candidate.evidence_lids);
    if (!candidate.evidence_lids.includes(candidate.anchor_lid)) throw new Error("math structure evidence must include its anchor");
    if (candidate.label && !passages.some((passage) => passage.includes(candidate.label!))) throw new Error("math structure label is not quoted from its evidence");
    for (const assumption of candidate.assumptions) {
      if (assumption.evidence_lids.some((lid) => !candidate.evidence_lids.includes(lid))
        || !checkEvidence(assumption.evidence_lids).some((passage) => passage.includes(assumption.quote))) {
        throw new Error("math structure assumption is not quoted from the item evidence");
      }
    }
    for (const ref of candidate.references) {
      const referencePassages = checkEvidence(ref.evidence_lids);
      if (!referencePassages.some((passage) => passage.includes(ref.quote))) throw new Error("math structure reference is not quoted from its evidence");
      if (!referencePassages.some((passage) => passage.includes(ref.target_label))) throw new Error("math structure target label is not quoted from its evidence");
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
