import { z } from "zod";
import { checkTeachingBindings, SourceBindingZ, type FormalObjects, type TeachingSource, type TeachingSourceBinding } from "./teaching-map";
import { advanceCognitiveWork, cognitiveRangeWasRead, newCognitiveWork, type CognitiveWork } from "./cognitive-materials";

export const TeachingReviewZ = z.object({ samples: z.array(z.object({ sample_id: z.string().min(1), verdict: z.enum(["pass", "fail"]),
  reason: z.string().trim().min(1), source_bindings: z.array(SourceBindingZ).min(1) }).strict()).min(1) }).strict();
export type TeachingReview = z.infer<typeof TeachingReviewZ>;
export interface TeachingSample { sample_id: string; content: unknown; evidence_lids: string[]; evidence_bindings: TeachingSourceBinding[] }
export interface TeachingReviewWork {
  version: "teaching_review_work.v1";
  reading: CognitiveWork;
  review?: TeachingReview;
}
const target = (sample: TeachingSample) => ({ id: sample.sample_id, lid: sample.evidence_lids[0], type: "claim" as const,
  reason: { text: "独立对照教学样本与原文", evidence_lids: sample.evidence_lids } });
export function newTeachingReviewWork(source: TeachingSource, sample: TeachingSample): TeachingReviewWork {
  return { version: "teaching_review_work.v1", reading: newCognitiveWork(source, target(sample)) };
}
export function reviewRanges(source: TeachingSource, sample: TeachingSample) {
  const raw = sample.evidence_bindings.map(b => ({ lid: b.lid, start: b.range_utf16?.start ?? 0,
    end: b.range_utf16?.end ?? source.passages.find(p => p.lid === b.lid)!.text.length }));
  const result: typeof raw = [];
  for (const lid of new Set(raw.map(r => r.lid))) for (const range of raw.filter(r => r.lid === lid).sort((a, b) => a.start - b.start)) {
    const last = result.at(-1);
    if (last?.lid === lid && range.start <= last.end) last.end = Math.max(last.end, range.end);
    else result.push({ ...range });
  }
  return result;
}
export function reviewSource(source: TeachingSource, sample: TeachingSample) {
  const ranges = reviewRanges(source, sample);
  return { source_id: source.source_id, source_revision: source.source_revision,
    passages: ranges.map(r => ({ ...source.passages.find(p => p.lid === r.lid)!, text: source.passages.find(p => p.lid === r.lid)!.text.slice(r.start, r.end), range_utf16: { start: r.start, end: r.end } })) };
}
export function teachingReviewInput(source: TeachingSource, sample: TeachingSample, work: TeachingReviewWork) {
  const ranges = reviewRanges(source, sample);
  const remaining = ranges.filter(r => !cognitiveRangeWasRead(work.reading, r.lid, r));
  let next_range: { lid: string; start: number; end: number } | undefined;
  if (remaining.length) {
    const first = remaining[0];
    let start = first.start;
    for (const read of work.reading.readings.filter(r => r.lid === first.lid).sort((a, b) => (a.start ?? 0) - (b.start ?? 0))) {
      if ((read.start ?? 0) > start) break;
      start = Math.max(start, read.end ?? read.text.length);
    }
    next_range = { lid: first.lid, start, end: Math.min(first.end, start + 2000) };
  }
  return { mode: "source_review_step", samples: [sample], source_id: source.source_id, source_revision: source.source_revision,
    required_ranges: ranges, next_range, remaining_ranges: remaining.length, used: work.reading.used,
    notes: work.reading.notes, readings: work.reading.readings.slice(-1), searches: work.reading.searches.slice(-1),
    read_ranges: work.reading.readings.map(r => ({ lid: r.lid, start: r.start ?? 0, end: r.end ?? r.text.length })) };
}
export function acceptTeachingReview(source: TeachingSource, samples: TeachingSample[], candidate: unknown): TeachingReview {
  const review = TeachingReviewZ.parse(candidate);
  if (review.samples.length !== samples.length || new Set(review.samples.map(s => s.sample_id)).size !== review.samples.length) throw new Error("teaching review coverage incomplete");
  for (const sample of review.samples) {
    const expected = samples.find(s => s.sample_id === sample.sample_id);
    if (!expected) throw new Error("teaching review sample mismatch");
    checkTeachingBindings(sample.source_bindings, source);
    if (expected.evidence_lids.some(lid => !sample.source_bindings.some(b => b.lid === lid))) throw new Error("teaching review omits sampled source");
  }
  return review;
}
export function advanceTeachingReview(source: TeachingSource, sample: TeachingSample, work: TeachingReviewWork, candidate: unknown): TeachingReviewWork {
  const action = candidate as { kind?: string; review?: unknown };
  if (work.review) throw new Error("teaching review already complete");
  if (action.kind === "finish") {
    if (reviewRanges(source, sample).some(r => !cognitiveRangeWasRead(work.reading, r.lid, r))) throw new Error("teaching review source coverage incomplete");
    return { ...work, review: acceptTeachingReview(source, [sample], action.review) };
  }
  if (!["read", "search", "preview", "retain"].includes(action.kind ?? "")) throw new Error("invalid teaching review action");
  // The caller's BuildPlan still owns the aggregate token/time limits. A review
  // can inspect all of its cited original ranges without a cumulative context cap.
  const budget = { searches: 12, preview_chars: 4000, read_chars: source.passages.reduce((n, p) => n + p.text.length, 0) + 4000, context_chars: 2000 };
  const objects = { source_revision: source.source_revision } as FormalObjects;
  return { ...work, reading: advanceCognitiveWork({ work: work.reading, action: candidate, source, target: target(sample), objects, discourse: [], budget }) };
}
