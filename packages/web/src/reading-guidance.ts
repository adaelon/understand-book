import { computed, inject, onBeforeUnmount, provide, shallowRef, watch, type InjectionKey, type Ref } from 'vue';
import { api } from './api';
import { bindSceneApi, readerKey } from './reader-context';
import { getLocale } from './i18n';
import type { LearningScopeKind, LearningScopeOption } from './learning-purpose';

export type GuidancePurpose = 'welcome' | 'understand' | 'explore' | 'selection' | 'followup';
export interface ReadingGuidance { question: string; draft: string }
export interface GuidanceTurn { turn_id: string; user: string; assistant: string; source_quote: string }
export interface ReadingGuidanceRequest {
  purpose: GuidancePurpose; scope: LearningScopeKind; lid?: string;
  quote: string; material_title: string; scope_label: string; known_intent: string;
  medium: 'text' | 'video' | 'music'; locale: 'zh-CN' | 'en';
  recent_turns?: GuidanceTurn[];
}
export interface GuidanceContext {
  key: string; title: string; scopes: LearningScopeOption[];
  selection: { lid: string; quote: string } | null;
  intent: string; medium: ReadingGuidanceRequest['medium'];
}
interface GuidanceProvider {
  context: Readonly<Ref<GuidanceContext | null>>;
  load: (request: ReadingGuidanceRequest) => Promise<ReadingGuidance | null>;
}
export const readingGuidanceKey: InjectionKey<GuidanceProvider> = Symbol('reading-guidance');

export function provideReadingGuidance(context: GuidanceProvider['context']) {
  const bound = bindSceneApi(api, readerKey);
  // Reopening a purpose reuses the same small result, including an in-flight call.
  // This cache belongs to the mounted reader, never to another account or book.
  const cache = new Map<string, Promise<ReadingGuidance | null>>();
  provide(readingGuidanceKey, { context, load: request => {
    const key = JSON.stringify([context.value?.key, request]);
    let task = cache.get(key);
    if (!task) { task = bound.readingGuidance(request); cache.set(key, task); }
    return task;
  } });
}

function conversationExcerpt(text: string, limit: number): string {
  if (text.length <= limit) return text;
  const tail = Math.floor(limit / 3);
  return `${text.slice(0, limit - tail - 5)}\n[…]\n${text.slice(-tail)}`;
}

export function useReadingGuidance(options: {
  purpose: () => GuidancePurpose | null; scope: () => LearningScopeKind | undefined;
  active: () => boolean; text: () => string;
  recentTurns?: () => GuidanceTurn[];
}) {
  const provider = inject(readingGuidanceKey, null);
  const guidance = shallowRef<ReadingGuidance | null>(null);
  let generation = 0;
  const request = computed(() => {
    const context = provider?.context.value, purpose = options.purpose();
    if (!context || !purpose || !options.active()) return null;
    const scope = context.scopes.find(item => item.value === options.scope()) ?? context.scopes[0];
    if (!scope) return null;
    const selection = scope.value === 'selection' ? context.selection : null;
    const recentTurns = options.recentTurns?.().slice(-2).map(turn => ({
      turn_id: turn.turn_id, user: conversationExcerpt(turn.user, 1000),
      assistant: conversationExcerpt(turn.assistant, 2400), source_quote: turn.source_quote.slice(0, 1000),
    }));
    return { key: context.key, body: {
      purpose, scope: scope.value, lid: selection?.lid ?? scope.lid,
      quote: selection?.quote.slice(0, 4000) ?? '', material_title: context.title,
      scope_label: scope.label, known_intent: context.intent, medium: context.medium, locale: getLocale(),
      ...(recentTurns?.length ? { recent_turns: recentTurns } : {}),
    } satisfies ReadingGuidanceRequest };
  });
  watch([() => JSON.stringify(request.value), () => !!options.text().length], async ([state, hasText], [previousState]) => {
    const current = ++generation, snapshot = request.value;
    // Typing hides the ghost but keeps an already displayed invitation stable.
    if (state !== previousState) guidance.value = null;
    if (!snapshot || !provider || hasText) return;
    try {
      const result = await provider.load(snapshot.body);
      // A purpose switch, closed input or newly typed draft keeps the UI untouched.
      if (current === generation && !options.text().length) guidance.value = typeof result?.question === 'string'
        && typeof result?.draft === 'string' && result.question.trim() && result.draft.trim() ? result : null;
    } catch { /* Existing instructions remain usable when the provider is unavailable. */ }
  }, { immediate: true });
  onBeforeUnmount(() => { generation++; });
  return guidance;
}

export function suggestionTab(event: KeyboardEvent, composing: boolean): boolean {
  return event.key === 'Tab' && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey
    && !event.isComposing && event.keyCode !== 229 && !composing;
}
