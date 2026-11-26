import { readFileSync } from 'node:fs';
import { z } from 'zod';

const requiredText = z.string().trim().min(1);

/** The public authoring brief selected after a trial. Personal responses stay with the trial. */
export const AcceptedCourseRouteZ = z.object({
  status: z.literal('accepted'),
  source_id: requiredText,
  source_revision: requiredText,
  title: requiredText,
  question: requiredText,
  audience: requiredText,
  final_task: requiredText,
  unit_lids: z.array(requiredText).min(1).refine(lids => new Set(lids).size === lids.length, '章节范围重复'),
  example_lessons: z.array(requiredText).min(1).max(2),
  trial_revision: z.string().regex(/^[A-Za-z0-9_-]+$/).optional(),
}).strict();

export type AcceptedCourseRoute = z.infer<typeof AcceptedCourseRouteZ>;

export function readAcceptedCourseRoute(file: string): AcceptedCourseRoute {
  return AcceptedCourseRouteZ.parse(JSON.parse(readFileSync(file, 'utf8')));
}
