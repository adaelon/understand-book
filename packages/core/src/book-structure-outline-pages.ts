import { z } from "zod";
import {
  applyStructureOutlineThemeAction, outlineThemeActionSchema,
  type StructureOutline, type StructureOutlineInput, type StructureOutlineThemeWork,
} from "./book-structure-planning";

export interface StructureOutlineThemePageContext {
  source: StructureOutlineInput;
  chapters: StructureOutline["chapters"];
  chapter_page_size: number;
  theme_page_size: number;
}

export interface StructureOutlineThemePageWork extends StructureOutlineThemeWork {
  chapter_offset: number;
  theme_offset: number;
  covered_unit_lids: string[];
  reading: string[];
}

export function structureOutlineChapterBook(source: StructureOutlineInput, unitLids: string[]): StructureOutlineInput {
  const requested = new Set(unitLids);
  return {
    ...source,
    preface: source.preface.filter(excerpt => requested.has(excerpt.unit_lid)),
    chapters: source.chapters.map(chapter => requested.has(chapter.unit_lid) ? chapter : {
      unit_lid: chapter.unit_lid, title: chapter.title, overview: { text: "", evidence_lids: [] },
    }),
  };
}

export function structureOutlineThemePageInput(work: StructureOutlineThemePageWork, context: StructureOutlineThemePageContext) {
  const chapters = context.chapters.slice(work.chapter_offset, work.chapter_offset + context.chapter_page_size);
  const onPage = new Set(chapters.map(chapter => chapter.unit_lid));
  const byLid = new Map(context.chapters.map(chapter => [chapter.unit_lid, chapter]));
  const reading = work.reading.filter(lid => !onPage.has(lid)).map(lid => byLid.get(lid)!);
  const nextChapterOffset = work.chapter_offset + chapters.length;
  const themes = work.themes.slice(work.theme_offset, work.theme_offset + context.theme_page_size);
  const nextThemeOffset = work.theme_offset + themes.length;
  const covered = new Set([...work.covered_unit_lids, ...chapters.map(chapter => chapter.unit_lid), ...work.reading]);
  return {
    book: context.source.chapters.map(chapter => ({ unit_lid: chapter.unit_lid, title: chapter.title })),
    chapters: {
      offset: work.chapter_offset, total: context.chapters.length,
      next_offset: nextChapterOffset < context.chapters.length ? nextChapterOffset : null,
      covered_after_this_page: covered.size, items: chapters,
    },
    reading,
    themes: {
      offset: work.theme_offset, total: work.themes.length,
      next_offset: nextThemeOffset < work.themes.length ? nextThemeOffset : null,
      items: themes.map((theme, index) => ({
        theme_index: work.theme_offset + index, question: theme.question.text, unit_lids: theme.unit_lids,
      })),
    },
  };
}

const outlineThemePageActionSchema = z.union([
  outlineThemeActionSchema,
  z.object({ kind: z.literal("browse"), offset: z.number().int().nonnegative() }).strict(),
  z.object({ kind: z.literal("inspect"), unit_lids: z.array(z.string()).min(1).max(4) }).strict(),
  z.object({ kind: z.literal("browse_themes"), offset: z.number().int().nonnegative() }).strict(),
]);

export function applyStructureOutlineThemePageAction(
  work: StructureOutlineThemePageWork, value: unknown, context: StructureOutlineThemePageContext,
): StructureOutlineThemePageWork {
  if (work.complete) throw new Error("outline themes are already complete");
  const action = outlineThemePageActionSchema.parse(value);
  const delivered = structureOutlineThemePageInput(work, context);
  const visible = [...delivered.chapters.items, ...delivered.reading];
  const next = { ...work, covered_unit_lids: [...new Set([
    ...work.covered_unit_lids, ...visible.map(chapter => chapter.unit_lid),
  ])] };
  if (action.kind === "browse") {
    if (action.offset !== delivered.chapters.next_offset) throw new Error("outline browse must use chapters.next_offset");
    return { ...next, chapter_offset: action.offset, reading: [] };
  }
  if (action.kind === "inspect") {
    const known = new Set(context.chapters.map(chapter => chapter.unit_lid));
    if (action.unit_lids.some(lid => !known.has(lid))) throw new Error("outline inspection references an unknown unit");
    return { ...next, reading: [...new Set(action.unit_lids)] };
  }
  if (action.kind === "browse_themes") {
    if (action.offset >= work.themes.length) throw new Error("outline theme page offset is out of range");
    return { ...next, theme_offset: action.offset };
  }
  if (action.kind === "finish") {
    const covered = new Set(next.covered_unit_lids);
    if (context.chapters.some(chapter => !covered.has(chapter.unit_lid))) {
      throw new Error("outline themes cannot finish before every chapter has been delivered");
    }
    return { ...next, complete: true };
  }

  // New claims cite the anchored chapter cards in this delivery. Prior theme
  // members keep their accepted evidence when a later page extends the theme.
  const evidence: StructureOutlineInput = {
    version: "book_structure_outline_input.v1", preface: [],
    chapters: visible.map(chapter => ({
      unit_lid: chapter.unit_lid, title: chapter.unit_lid,
      overview: {
        text: chapter.question.text + "\n" + chapter.progression.text,
        evidence_lids: [...new Set([...chapter.question.evidence_lids, ...chapter.progression.evidence_lids])],
      },
    })),
  };
  if (action.kind === "append") {
    applyStructureOutlineThemeAction({ themes: [], complete: false }, action, evidence);
  } else {
    if (!delivered.themes.items.some(theme => theme.theme_index === action.theme_index)) {
      throw new Error("outline theme extension requires its delivered directory entry");
    }
    applyStructureOutlineThemeAction({ themes: [], complete: false }, {
      kind: "append", themes: [{
        question: { text: work.themes[action.theme_index].question.text, evidence_lids: action.evidence_lids },
        unit_lids: action.unit_lids,
      }],
    }, evidence);
  }
  const accepted = applyStructureOutlineThemeAction(work, action, context.source);
  return { ...next, ...accepted };
}
