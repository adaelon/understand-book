import { z } from "zod";
import { planStructureThemes, structureThemePlanningInput, type StructureThemeContext, type StructureThemePlan } from "./book-structure-themes";

export interface StructureThemePlanPageContext {
  theme: StructureThemeContext;
  chapter_page_size: number;
  seed_page_size: number;
}

export interface StructureThemePlanPageWork {
  chapter_offset: number;
  seed_offset: number;
  themes: StructureThemePlan[];
  complete: boolean;
}

export function structureThemePlanPageInput(work: StructureThemePlanPageWork, context: StructureThemePlanPageContext) {
  const full = structureThemePlanningInput(context.theme);
  const chapters = full.chapters.slice(work.chapter_offset, work.chapter_offset + context.chapter_page_size);
  const seeds = full.candidate_index.slice(work.seed_offset, work.seed_offset + context.seed_page_size);
  const chapterEnd = work.chapter_offset + chapters.length;
  const seedEnd = work.seed_offset + seeds.length;
  return {
    chapters: { offset: work.chapter_offset, total: full.chapters.length, items: chapters },
    seeds: { offset: work.seed_offset, total: full.candidate_index.length, items: seeds },
    directory: work.themes,
    last_page: chapterEnd === full.chapters.length && seedEnd === full.candidate_index.length,
  };
}

const pageAction = z.object({
  kind: z.literal("advance"),
  remove: z.array(z.string().min(1)).max(16).optional(),
  themes: z.array(z.object({
    ref: z.string().min(1).max(120),
    question: z.string().min(1).max(600).optional(),
    distinction: z.string().min(1).max(600).optional(),
    unit_lids: z.array(z.string().min(1)),
    member_refs: z.array(z.string().min(1)).max(6).optional(),
  }).strict()).max(16),
}).strict();

export function applyStructureThemePlanPageAction(
  work: StructureThemePlanPageWork, value: unknown, context: StructureThemePlanPageContext,
): StructureThemePlanPageWork {
  if (work.complete) throw new Error("theme planning is already complete");
  const action = pageAction.parse(value);
  const delivery = structureThemePlanPageInput(work, context);
  const seedUnits = new Map(delivery.seeds.items.map(seed => [seed.ref, seed.unit_lid]));
  const visibleUnits = new Set([...delivery.chapters.items.map(c => c.unit_lid), ...seedUnits.values()]);
  const themes = new Map(work.themes.map(theme => [theme.ref, theme]));
  const touched = new Set<string>();
  for (const ref of action.remove ?? []) {
    if (!themes.delete(ref)) throw new Error("remove requires an existing theme");
    touched.add(ref);
  }
  for (const patch of action.themes) {
    if (touched.has(patch.ref)) throw new Error("duplicate theme update on a planning page");
    touched.add(patch.ref);
    const prior = themes.get(patch.ref);
    if (!prior && (!patch.question || !patch.distinction)) throw new Error("new theme needs a question and distinction");
    if (patch.unit_lids.some(lid => !visibleUnits.has(lid) && !prior?.unit_lids.includes(lid))) {
      throw new Error("theme planning can add only a chapter from the current delivery");
    }
    const unit_lids = [...new Set([...(prior?.unit_lids ?? []), ...patch.unit_lids])];
    if (!unit_lids.length) throw new Error("theme planning needs a chapter");
    if (patch.member_refs?.some(ref => !unit_lids.includes(seedUnits.get(ref) ?? "") && !prior?.member_refs.includes(ref))) {
      throw new Error("theme planning seed must be delivered for one of the theme's chapters");
    }
    themes.set(patch.ref, { ref: patch.ref, question: patch.question ?? prior!.question,
      distinction: patch.distinction ?? prior!.distinction, unit_lids,
      member_refs: patch.member_refs ? [...new Set(patch.member_refs)] : prior?.member_refs ?? [] });
  }
  if (themes.size > 16) throw new Error("theme directory supports at most 16 questions");
  const next = { themes: [...themes.values()], chapter_offset: work.chapter_offset + delivery.chapters.items.length,
    seed_offset: work.seed_offset + delivery.seeds.items.length, complete: delivery.last_page };
  if (next.complete) finishStructureThemePlanPages(next, context.theme);
  return next;
}

export function finishStructureThemePlanPages(work: StructureThemePlanPageWork, context: StructureThemeContext) {
  if (!work.complete) throw new Error("theme planning still has undelivered pages");
  return planStructureThemes({ themes: work.themes.filter(theme => theme.unit_lids.length >= 2) }, context);
}
