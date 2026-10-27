import type { BoardItem } from './generated/BoardItem';
import type { BoardView } from './generated/BoardView';
import type { BoardFollowUp } from './generated/BoardFollowUp';
import type { BoardDelivery } from './generated/BoardDelivery';
import type { BoardDocument } from './generated/BoardDocument';
import type { BoardRoute } from './generated/BoardRoute';
import type { BoardVisualTarget } from './generated/BoardVisualTarget';
import { t as uiText, formatLocale } from './i18n';
import type { BoardDiscussionTurn } from './board-conversation';

export type BoardKind = 'prebuild' | 'answer' | 'presentation' | 'understanding';
export interface BoardSourceRequest { board_id: string; item_id: string; source_ref_id: string }
export interface BoardQuestionRequest { client_request_id: string; message: string; context: BoardFollowUp }
export interface BoardRunView {
  request_id: string;
  board_id: string;
  item_id?: string | null;
  question: string;
  status_text: string;
  accepted: boolean;
  pending: boolean;
  error?: string | null;
  can_retry: boolean;
  can_stop: boolean;
  turn_id?: string;
  delivery?: BoardDelivery | null;
  commits?: BoardDelivery[];
  incomplete?: boolean;
  discussion?: BoardDiscussionTurn;
}
export interface BoardLocation { board_id: string; item_id?: string | null; block_id?: string | null; query?: string }

export function boardDeliveryKey(delivery: BoardDelivery): string {
  return `${delivery.board_id}:${delivery.commit_id ?? `revision:${delivery.after_revision}`}`;
}

/** Durable parts survive reconnects, draft replacement, and terminal snapshots. */
export function mergeBoardDeliveries(...groups: (readonly BoardDelivery[] | null | undefined)[]): BoardDelivery[] {
  const deliveries = new Map<string, BoardDelivery>();
  for (const group of groups) for (const delivery of group ?? []) {
    const key = boardDeliveryKey(delivery);
    const sameRevision = [...deliveries].find(([, previous]) => previous.board_id === delivery.board_id && previous.after_revision === delivery.after_revision);
    if (sameRevision && sameRevision[0] !== key) deliveries.delete(sameRevision[0]);
    deliveries.set(key, delivery);
  }
  return [...deliveries.values()].sort((a, b) => a.after_revision - b.after_revision);
}

export const boardKinds: BoardKind[] = ['prebuild', 'answer', 'presentation', 'understanding'];
export const boardKindLabels: Record<BoardItem['target']['kind'], string> = {
  prebuild: '材料知识', artifact: '材料知识', answer: '问答讲解', presentation: '互动演示', understanding: '我的理解', drawing: '板书图解',
};
export const boardArtifactLabels = { all: '全部材料知识', pass1: '概念与论断', discourse: '篇章脉络', book_structure: '全书结构', teaching: '教学知识点' };
export const boardKindDescriptions: Record<BoardKind, string> = {
  prebuild: '材料中的概念、论断、篇章脉络与全书结构，保留当时内容与原文。',
  answer: '一段完整回答，和它引用的原文一起保留。',
  presentation: '保存演示内容及当前状态，随时回到这个现场。',
  understanding: '把自己的原话与 Agent 对理解的解释并排留下。',
};
export const defaultBoardView = (): BoardView => ({ x: 36, y: 36, zoom: 1, selected_item_id: null, reading_item_id: null, selected_block_id: null, playback: null, presentation_states: {} });

export function boardVisualKey(target: BoardVisualTarget): string {
  return target.kind === 'link' ? `link:${target.link_id}` : target.kind === 'item' ? `item:${target.item_id}`
    : `anchor:${target.item_id}:${target.reference.presentation_id}:${target.reference.revision}:${target.anchor_id}`;
}

/** A seek derives the completed prefix; it never replays mutations to reach a step. */
export function projectBoardRoute(document: BoardDocument, route: BoardRoute | null, stepId: string | null, showAll = false) {
  const index = route?.steps.findIndex(step => step.step_id === stepId) ?? -1;
  const step = index >= 0 ? route!.steps[index] : null;
  if (!route || showAll) return { index, step, visibleItemIds: undefined, visibleLinkIds: undefined,
    highlightTargets: [] as BoardVisualTarget[], previousTargets: new Set<string>() };
  const targets = route.steps.slice(0, index + 1).flatMap(entry => entry.reveal_targets);
  const items = new Set<string>(), links = new Set<string>();
  for (const target of targets) {
    if (target.kind === 'link') links.add(target.link_id); else items.add(target.item_id);
  }
  // A revealed connection brings its endpoints into view; a highlighted item is readable at this step.
  for (const link of document.links ?? []) if (links.has(link.link_id)) {
    for (const end of [link.from, link.to]) if (end.kind !== 'link') items.add(end.item_id);
  }
  for (const target of step?.focus_targets ?? []) {
    if (target.kind === 'link') links.add(target.link_id); else items.add(target.item_id);
  }
  return { index, step, visibleItemIds: [...items], visibleLinkIds: [...links], highlightTargets: step?.focus_targets ?? [],
    previousTargets: new Set(route.steps.slice(0, Math.max(0, index)).flatMap(entry => entry.reveal_targets.map(boardVisualKey))) };
}

export function boardItemPreview(item: BoardItem): string {
  const text = item.target.kind === 'understanding' ? item.target.learner_quote : item.readable_content;
  return boardTextPreview(text);
}

export function boardTextPreview(text: string): string {
  return text.replace(/```[\s\S]*?```/g, ` ${uiText('[代码]')} `).replace(/\$\$[\s\S]*?\$\$/g, ` ${uiText('[公式]')} `)
    .replace(/\$[^$\n]+\$/g, uiText('[公式]')).replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[\[source:[^\]\n]+\]\]/g, '').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/[#*_`>]/g, '').replace(/\s+/g, ' ').trim();
}

export function clampBoardZoom(zoom: number): number { return Math.max(0.25, Math.min(1.75, zoom)); }

export function zoomBoardAt(view: BoardView, zoom: number, point: { x: number; y: number }): BoardView {
  const next = clampBoardZoom(zoom), ratio = next / view.zoom;
  return { ...view, zoom: next, x: point.x - (point.x - view.x) * ratio, y: point.y - (point.y - view.y) * ratio };
}

export function fitBoard(items: BoardItem[], width: number, height: number, current: BoardView): BoardView {
  if (!items.length || width <= 0 || height <= 0) return { ...current, x: 36, y: 36, zoom: 1 };
  const left = Math.min(...items.map(item => item.position.x));
  const top = Math.min(...items.map(item => item.position.y));
  const right = Math.max(...items.map(item => item.position.x + item.size.width));
  const bottom = Math.max(...items.map(item => item.position.y + item.size.height));
  const zoom = Math.min(1, clampBoardZoom(Math.min((width - 72) / Math.max(1, right - left), (height - 100) / Math.max(1, bottom - top))));
  return { ...current, zoom, x: (width - (right - left) * zoom) / 2 - left * zoom,
    y: (height - (bottom - top) * zoom) / 2 - top * zoom };
}

export function boardDate(value: number | string): string {
  const numeric = typeof value === 'number' ? value : /^\d+$/.test(value) ? Number(value) : null;
  const date = numeric !== null ? new Date(numeric < 1e12 ? numeric * 1000 : numeric) : new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString(formatLocale(), { month: 'short', day: 'numeric' });
}
