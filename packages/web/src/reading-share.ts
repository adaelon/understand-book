import type { InjectionKey } from 'vue';
import type { MemoryRecord, OuterOutcome } from './api';

export const sharePresentationKey: InjectionKey<(load: () => Promise<ShareSource>) => Promise<void>> = Symbol('share-presentation');
export const shareHighlightKey: InjectionKey<(highlight: MemoryRecord) => void> = Symbol('share-highlight');
export const shareNoteKey: InjectionKey<(note: MemoryRecord) => void> = Symbol('share-note');
export type SharePart = { id: 'body' | 'excerpt' | `recap:${string}`; label: string; text: string; format?: 'plain' | 'markdown' };
export type ShareSource = {
  parts: SharePart[]; sources: string[]; association: string;
  diagram?: { png: string; width: number; height: number; title: string };
  recap?: { sessionId: string; throughSeq: number; throughAt: string };
  answer?: { sessionId: string; turnId: string; fullText: string };
};
export type ShareDraft = {
  source: ShareSource; selected: SharePart['id'][]; title: string; reflection: string;
  orientation?: 'portrait' | 'landscape';
  layout: 'excerpt' | 'understanding' | 'recap' | 'diagram'; palette: 'paper' | 'blue';
};
// Only human-readable projections cross into the image. The record itself stays in memory.
export function noteShareSource(record: MemoryRecord, sources: string[]): ShareSource {
  const parts: SharePart[] = [];
  if (record.content.trim()) parts.push({ id: 'body', label: record.note ? '我的笔记' : '阅读笔记', text: record.content });
  const excerpt = record.note?.retained_excerpt;
  if (excerpt) parts.push({ id: 'excerpt', label: excerpt.kind === 'original' ? '原文摘录' : '助手解释', text: excerpt.text });
  const association = record.note?.association;
  return { parts, sources: [...sources], association: association?.kind === 'presentation'
    ? `记录于演示「${association.title}」` : association?.kind === 'answer' ? '记录于助手回答' : '' };
}

export { renderShare, paginateShare, shareSections } from './reading-share-layout';

export function excerptShareSource(text: string, sources: string[], format: 'plain' | 'markdown' = 'markdown'): ShareSource {
  return { parts: [{ id: 'excerpt', label: '原文摘录', text, format }], sources, association: '' };
}

export function deliveredAnswerText(turn: { turnId: string | null; pending: boolean; error?: string; runStatus?: string; outcome: OuterOutcome | null }): string {
  if (!turn.turnId || turn.pending || turn.error || turn.runStatus === '已停止' || !turn.outcome || turn.outcome.incomplete) return '';
  const parts = turn.outcome.answer_view?.parts;
  return (parts?.length ? parts.map(part => part.kind === 'markdown' ? part.text : part.kind === 'presentation' ? '\n\n' : '').join('') : turn.outcome.answer ?? '').trim();
}

export function answerShareSource(fullText: string, text: string, sessionId: string, turnId: string, sources: string[]): ShareSource {
  return { parts: [{ id: 'body', label: '助手解释', text, format: 'markdown' }],
    sources: sources.length ? [...sources] : ['无已记录出处'],
    association: text === fullText ? '回答文字全文 · 本回答来源' : '回答节选 · 本回答来源',
    answer: { sessionId, turnId, fullText } };
}

export async function saveShare(blob: Blob, url: string, current: () => boolean, page?: number): Promise<boolean> {
  const filename = page === undefined ? '阅读随记.png' : `阅读随记-${page}.png`;
  if ('__TAURI_INTERNALS__' in window) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    if (!current()) return false;
    const path = await save({ defaultPath: filename, filters: [{ name: 'PNG 图片', extensions: ['png'] }] });
    if (!path || !current()) return false;
    const { invoke } = await import('@tauri-apps/api/core');
    const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
    if (!current()) return false;
    await invoke('save_reading_share_image', { path, bytes });
  } else {
    if (!current()) return false;
    const link = document.createElement('a'); link.href = url; link.download = filename;
    document.body.append(link); link.click(); link.remove();
  }
  return true;
}
