<script lang="ts">
import type { AppContext } from 'vue';

type DisplayReceipt = { support_delivery_refs?: string[] };
const displayReceipts = new WeakMap<AppContext, Map<string, Promise<DisplayReceipt>>>();
</script>

<script setup lang="ts">
import { computed, getCurrentInstance, nextTick, ref, watch } from 'vue';
import { ChevronRight } from '@lucide/vue';
import { api as sharedApi } from '../api';
import { comparisonGraphs, type StoredTeachBackComparison } from '../teach-back-history';
import { focusNumber, statusLabels } from '../teach-back-layout';
import TeachBackGraph from './TeachBackGraph.vue';
import TeachBackCitation from './TeachBackCitation.vue';
import type { TeachBackRow } from '../generated/TeachBackRow';
import { t as uiText } from '../i18n';
import { bindSceneApi, readerPreferenceOwner } from '../reader-context';

type CorrectionAction = 'dispute_interpretation' | 'revise_understanding';
type Round = { turnId: string; comparison: StoredTeachBackComparison; displayed?: boolean };

const props = defineProps<{
  comparison: StoredTeachBackComparison;
  rounds?: Round[];
  correctionEnabled?: boolean;
  alreadyDisplayed?: boolean;
  sessionId: string;
  turnId: string;
  currentSourceId?: string | null;
  currentSourceRevision?: string | null;
}>();
const emit = defineEmits<{
  jump: [lid: string, turnId: string];
  locate: [turnId: string];
  correct: [request: { action: CorrectionAction; text: string; turnId: string; roundId: string; rowId: string }];
  displayed: [turnId: string, roundId: string];
  'view-round': [roundId: string];
}>();
const api = bindSceneApi(sharedApi);
const appContext = getCurrentInstance()!.appContext;
const receipts = displayReceipts.get(appContext) ?? new Map<string, Promise<DisplayReceipt>>();
displayReceipts.set(appContext, receipts);
const root = ref<HTMLElement>();
const selected = ref('');
const viewing = ref('');
const round = computed(() => props.rounds?.find(item => item.comparison.round_id === viewing.value)
  ?? { turnId: props.turnId, comparison: props.comparison, displayed: props.alreadyDisplayed });
const comparison = computed(() => comparisonGraphs(round.value.comparison));
watch(() => props.comparison.round_id, () => { viewing.value = ''; });
const roundButtons = computed(() => {
  const counts: Record<string, number> = {};
  return props.rounds?.map(item => {
    const kind = item.comparison.continuation?.kind;
    const label = !kind ? uiText('首次表达') : kind === 'dispute_interpretation' ? uiText('重新解读') : uiText('澄清后');
    const count = counts[label] = (counts[label] ?? 0) + 1;
    return { id: item.comparison.round_id, label: count === 1 ? label : `${label} · ${count}` };
  }) ?? [];
});
const receiptSupport = ref<string[]>();
const supportText = computed(() => {
  const refs = comparison.value.support_delivery_refs ?? receiptSupport.value ?? [];
  const count = refs.filter(id => id.startsWith('tb:')).length;
  return count === 0 ? uiText('表达前已看过原讲解。') : uiText('表达前已看过原讲解和 {0} 轮三图对照。', [count]);
});
const sameSource = computed(() =>
  props.currentSourceId === comparison.value.source.source_id &&
  props.currentSourceRevision === comparison.value.source.source_revision);
const unavailableMessage = computed(() => props.currentSourceId === comparison.value.source.source_id && props.currentSourceRevision
  ? uiText('原书版本已变，此处保留当时的引句。')
  : uiText('原书版本当前不可定位，此处保留当时的引句。'));

const rows = computed(() => comparison.value.rows);
type GraphSide = 'book' | 'student';
const picked = ref<{ side: GraphSide; kind: 'node' | 'edge'; id: string }>();
function selectFocus(rowId: string, side?: GraphSide, kind?: 'node' | 'edge', id?: string) {
  selected.value = rowId;
  picked.value = side && kind && id ? { side, kind, id } : undefined;
}
// Title each focus with a proposition actually made in this round. A standalone
// assertion has its own label; for a chain use the conclusion of the selected steps.
function proposition(row: TeachBackRow, side: GraphSide) {
  const graph = comparison.value[`${side}_graph`];
  const isolated = graph.nodes.find(node => row[`${side}_node_ids`].includes(node.id));
  const edges = graph.edges.filter(edge => row[`${side}_edge_ids`].includes(edge.id));
  const end = edges.find(edge => !edges.some(next => next.from === edge.to)) ?? edges[0];
  const node = end && graph.nodes.find(node => node.id === end.to);
  return node?.label ?? isolated?.label;
}
const topics = computed(() => new Map(rows.value.map(row =>
  [row.row_id, proposition(row, 'student') || proposition(row, 'book') || row.summary])));
const focus = computed(() => {
  const row = rows.value.find(r => r.row_id === selected.value);
  if (!row) return;
  const { book_graph, student_graph } = comparison.value;
  function focusItems<N extends { id: string }, E extends { id: string; from: string; to: string }>(
    graph: { nodes: N[]; edges: E[] }, nodeIds: string[], edgeIds: string[], side: GraphSide,
  ) {
    const edges = graph.edges.filter(e => edgeIds.includes(e.id));
    const ids = new Set([...nodeIds, ...edges.flatMap(e => [e.from, e.to])]);
    const items = [...edges, ...graph.nodes.filter(n => ids.has(n.id))];
    const item = picked.value;
    if (item?.side !== side) return items;
    const first = items.find(entry => entry.id === item.id);
    if (!first) return items;
    const related = item.kind === 'node' ? edges.filter(edge => edge.to === item.id || edge.from === item.id) : [];
    const ordered = new Set<N | E>([first, ...related, ...items]);
    return [...ordered];
  }
  const book = focusItems(book_graph, row.book_node_ids, row.book_edge_ids, 'book');
  const student = focusItems(student_graph, row.student_node_ids, row.student_edge_ids, 'student');
  const evidence = book.flatMap(item => item.evidence);
  const quotes = student.flatMap(item => item.quotes);
  const sources = [
    { side: 'book' as const, title: uiText('书中原句'), empty: uiText('这条关系没有书中引句。'),
      conditions: [...new Set(book.flatMap(item => item.condition ? [item.condition] : []))],
      entries: evidence.filter((q, i) => evidence.findIndex(o => o.lid === q.lid && o.quote === q.quote) === i)
        .map(q => ({ id: q.lid, text: q.quote, action: uiText('回到原段 {0}', [q.lid]) })) },
    { side: 'student' as const, title: uiText('本次原话'), empty: uiText('这条关系没有对应的原话。'),
      conditions: [...new Set(student.flatMap(item => item.condition ? [item.condition] : []))],
      entries: quotes.filter((q, i) => quotes.findIndex(o => o.turn_id === q.turn_id && o.text === q.text) === i)
        .map(q => ({ id: q.turn_id, text: q.text, action: uiText('回到我的表达') })) },
  ];
  return { ...row, sources };
});
function locateQuote(side: GraphSide, id: string) {
  if (side === 'book') emit('jump', id, round.value.turnId);
  else emit('locate', id);
}
const editor = ref<{ action: CorrectionAction; rowId: string }>();
const draft = ref('');
const input = ref<HTMLTextAreaElement>();
const displayConfirmed = ref(false);
const canCorrect = computed(() => props.correctionEnabled && sameSource.value);

watch(() => comparison.value.round_id, id => {
  picked.value = undefined;
  const correctedRow = props.rounds?.find(item => item.comparison.continuation?.parent_round_id === id)?.comparison.continuation?.row_id;
  selected.value = rows.value.find(row => row.row_id === correctedRow)?.row_id ?? comparison.value.follow_up?.row_id
    ?? rows.value.find(row => !['consistent', 'alternative_valid', 'clarified'].includes(row.status))?.row_id
    ?? rows.value[0]?.row_id ?? '';
  editor.value = undefined;
  draft.value = '';
}, { immediate: true });
watch(() => comparison.value.round_id, id => emit('view-round', id));

async function edit(action: CorrectionAction, rowId: string) {
  if (editor.value?.action !== action || editor.value.rowId !== rowId) draft.value = '';
  editor.value = { action, rowId };
  await nextTick();
  input.value?.focus();
}

async function cancel() {
  const action = editor.value?.action;
  editor.value = undefined;
  draft.value = '';
  await nextTick();
  root.value?.querySelector<HTMLButtonElement>(`.selected [data-action="${action}"]`)?.focus();
}

function submitCorrection() {
  if (!canCorrect.value || !editor.value || !displayConfirmed.value || !draft.value.trim()) return;
  emit('correct', { action: editor.value.action, rowId: editor.value.rowId, text: draft.value.trim(),
    turnId: round.value.turnId, roundId: comparison.value.round_id });
  editor.value = undefined;
  draft.value = '';
}

function navigate(event: KeyboardEvent, index: number) {
  const last = rows.value.length - 1;
  const positions: Record<string, number> = {
    ArrowDown: Math.min(last, index + 1), ArrowRight: Math.min(last, index + 1),
    ArrowUp: Math.max(0, index - 1), ArrowLeft: Math.max(0, index - 1),
    Home: 0, End: last,
  };
  const next = positions[event.key];
  if (next === undefined) return;
  event.preventDefault();
  selectFocus(rows.value[next].row_id);
  root.value?.querySelectorAll<HTMLButtonElement>('.teach-back-select')[next]?.focus();
}

const displayError = ref(false);
let retryDisplay: (() => void) | undefined;
watch([root, () => props.sessionId, () => round.value.turnId, () => comparison.value.round_id],
  ([element, sessionId, turnId, roundId], _previous, onCleanup) => {
    displayError.value = false;
    displayConfirmed.value = false;
    receiptSupport.value = undefined;
    if (!element || !sessionId || !turnId) return;
    let intersecting = false, active = true, writing = false, recorded = false;
    async function record() {
      if (!active || !intersecting || document.visibilityState !== 'visible' || writing || recorded) return;
      writing = true;
      try {
        const key = JSON.stringify([readerPreferenceOwner(), sessionId, turnId, roundId]);
        let request = receipts.get(key);
        if (!request) {
          request = round.value.displayed
            ? Promise.resolve({ support_delivery_refs: comparison.value.support_delivery_refs })
            : api.teachBackDisplayed(sessionId, turnId, roundId).catch(error => {
                receipts.delete(key);
                throw error;
              });
          receipts.set(key, request);
        }
        const receipt = await request;
        recorded = true;
        observer.disconnect();
        if (active) {
          receiptSupport.value = receipt.support_delivery_refs;
          displayError.value = false;
          displayConfirmed.value = true;
          emit('displayed', turnId, roundId);
        }
      } catch {
        if (active) displayError.value = true;
      } finally { writing = false; }
    }
    // A zero-area edge touch and the first visible pixels must produce separate callbacks.
    const observer = new IntersectionObserver(entries => {
      const entry = entries[entries.length - 1];
      intersecting = entry.isIntersecting && entry.intersectionRect.width > 0 && entry.intersectionRect.height > 0;
      void record();
    }, { threshold: Number.EPSILON });
    observer.observe(element);
    document.addEventListener('visibilitychange', record);
    retryDisplay = record;
    onCleanup(() => {
      active = false;
      observer.disconnect();
      document.removeEventListener('visibilitychange', record);
      retryDisplay = undefined;
    });
  }, { flush: 'post' });
</script>

<template>
  <section ref="root" class="teach-back-comparison" :aria-label="uiText('讲后对照')">
    <div class="comparison-surface">
      <div v-if="roundButtons.length > 1" class="teach-back-rounds" role="group" :aria-label="uiText('回看对照')">
        <button v-for="item in roundButtons" :key="item.id" type="button" :data-round-id="item.id"
          :aria-pressed="item.id === comparison.round_id" @click="viewing = item.id">{{ item.label }}</button>
      </div>
      <p class="teach-back-support">{{ supportText }}
        <span v-if="comparison.continuation?.kind === 'dispute_interpretation'">{{ uiText('本轮按首次原话重新解读。') }}</span>
      </p>
      <div class="focus-bar" role="group" :aria-label="uiText('关系对照')">
        <span class="focus-label">{{ uiText('切换焦点') }}</span>
        <button v-for="(row, index) in rows" :key="row.row_id" type="button" class="teach-back-select"
          :data-row-id="row.row_id" :data-status="row.status" :aria-pressed="row.row_id === selected"
          :title="row.summary" @click="selectFocus(row.row_id)" @keydown="navigate($event, index)">
          <span class="focus-dot" aria-hidden="true"></span>
          <b class="focus-ordinal">{{ focusNumber(index) }}</b>
          <span class="focus-topic">{{ topics.get(row.row_id) }}</span>
          <span class="focus-state">{{ uiText(statusLabels[row.status]) }}</span>
        </button>
      </div>
      <div class="teach-back-graphs" @keydown="navigate($event, rows.findIndex(r => r.row_id === selected))">
        <TeachBackGraph side="book" :graph="comparison.book_graph" :rows="rows" :selected="selected"
          @select="(rowId, kind, id) => selectFocus(rowId, 'book', kind, id)" />
        <TeachBackGraph side="student" :graph="comparison.student_graph" :rows="rows" :selected="selected"
          @select="(rowId, kind, id) => selectFocus(rowId, 'student', kind, id)" />
      </div>
      <article v-if="focus" class="teach-back-row selected" :data-status="focus.status">
        <header class="focus-summary">
          <div class="focus-heading">
            <span class="comparison-status" :data-status="focus.status">{{ uiText(statusLabels[focus.status]) }}</span>
            <h4><span class="focus-ordinal">{{ focusNumber(rows.findIndex(row => row.row_id === selected)) }}</span>
              {{ topics.get(focus.row_id) }}</h4>
          </div>
          <p v-if="focus.summary !== topics.get(focus.row_id)">{{ focus.summary }}</p>
          <p v-if="focus.divergence_condition" class="divergence"><strong>{{ uiText('分歧出现的条件') }}</strong>{{ focus.divergence_condition }}</p>
        </header>
        <div :key="comparison.round_id + ':' + focus.row_id" class="teach-back-evidence">
          <section v-for="source in focus.sources" :key="source.side">
            <h4>{{ source.title }}</h4>
            <p v-for="condition in source.conditions" :key="condition" class="condition">{{ uiText('成立条件') }} · {{ condition }}</p>
            <template v-if="source.entries.length">
              <TeachBackCitation :text="source.entries[0].text" :action="source.entries[0].action" :disabled="source.side === 'book' && !sameSource"
                @navigate="locateQuote(source.side, source.entries[0].id)" />
              <details v-if="source.entries.length > 1" class="more-quotes">
                <summary><ChevronRight aria-hidden="true" />{{ uiText('其余引句（{0}）', [source.entries.length - 1]) }}</summary>
                <TeachBackCitation v-for="(entry, index) in source.entries.slice(1)" :key="index"
                  :text="entry.text" :action="entry.action" :disabled="source.side === 'book' && !sameSource"
                  @navigate="locateQuote(source.side, entry.id)" />
              </details>
              <p v-if="source.side === 'book' && !sameSource" class="empty">{{ unavailableMessage }}</p>
            </template>
            <p v-else class="empty">{{ source.empty }}</p>
          </section>
        </div>
        <aside v-if="comparison.follow_up?.row_id === focus.row_id" class="follow-up">
          <strong>{{ uiText('接着想这一点') }}</strong><p>{{ comparison.follow_up.question }}</p>
        </aside>
        <div v-if="canCorrect" class="teach-back-corrections">
          <div class="correction-actions">
            <button type="button" data-action="dispute_interpretation" :aria-pressed="editor?.rowId === selected && editor.action === 'dispute_interpretation'"
              @click="edit('dispute_interpretation', focus.row_id)">{{ uiText('你把我听反了') }}</button>
            <button type="button" data-action="revise_understanding" :aria-pressed="editor?.rowId === selected && editor.action === 'revise_understanding'"
              @click="edit('revise_understanding', focus.row_id)">{{ uiText('改一下我的想法') }}</button>
          </div>
          <form v-if="editor?.rowId === selected" @submit.prevent="submitCorrection">
            <label><span>{{ editor.action === 'dispute_interpretation' ? uiText('首次原话中，哪句话被理解错了？') : uiText('看过对照后，你现在怎么解释？') }}</span>
              <textarea ref="input" v-model="draft" rows="3" @keydown.esc.prevent="cancel" />
            </label>
            <div class="correction-actions">
              <button type="submit" :disabled="!draft.trim() || !displayConfirmed">{{ editor.action === 'dispute_interpretation' ? uiText('重新核对原话') : uiText('提交新表达') }}</button>
              <button type="button" @click="cancel">{{ uiText('取消') }}</button>
            </div>
          </form>
        </div>
      </article>
      <details v-for="(note, i) in comparison.side_notes" :key="i" class="teach-back-side-note">
        <summary><ChevronRight aria-hidden="true" />{{ note.label }}</summary>
        <div class="side-note-body">
          <p>{{ note.text }}</p>
          <TeachBackCitation v-for="(entry, j) in note.evidence" :key="j" :text="entry.quote"
            :action="uiText('回到原段 {0}', [entry.lid])" :disabled="!sameSource" @navigate="emit('jump', entry.lid, round.turnId)" />
          <span v-if="note.evidence.length && !sameSource" class="empty">{{ unavailableMessage }}</span>
        </div>
      </details>
      <p v-if="displayError" role="status" class="receipt-error">
        {{ uiText('这次对照的显示记录尚未保存。') }}
        <button type="button" class="quote-action" @click="retryDisplay?.()">{{ uiText('重试记录') }}</button>
      </p>
    </div>
  </section>
</template>

<style scoped>
.teach-back-comparison {
  box-sizing: border-box;
  container: teach-back / inline-size;
  min-width: 0;
  color: var(--reader-ink);
  color-scheme: light;
  font: 14px/1.55 var(--sans);
  overflow-wrap: anywhere;
}
.comparison-surface { padding: 12px; border: 1px solid #ddd5cb; border-radius: 13px; background: var(--reader-canvas); }
[data-status] { --status-ink: #4e6a58; }
[data-status="divergent"] { --status-ink: #964b44; }
:is([data-status="omitted"], [data-status="unverified_against_passage"], [data-status="insufficient_book_basis"]) { --status-ink: #826220; }
.teach-back-rounds, .correction-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 7px; }
.teach-back-rounds { margin-bottom: 8px; }
.teach-back-rounds button, .correction-actions button {
  min-height: 30px; max-width: 100%; padding: 4px 10px; border: 1px solid #cec4b7; border-radius: 5px;
  color: var(--reader-body); background: transparent; font: 550 12px/1.5 var(--sans); cursor: pointer;
}
.teach-back-rounds button[aria-pressed="true"], .correction-actions button[aria-pressed="true"], .correction-actions button[type="submit"] { border-color: var(--brand-accent-deep); color: var(--brand-accent-deep); background: var(--brand-accent-soft); }
.teach-back-comparison :is(button, textarea, summary):focus-visible { outline: 2px solid var(--brand-accent-deep); outline-offset: 2px; }
.focus-bar { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin: 0 0 10px; padding-bottom: 10px; border-bottom: 1px solid var(--hairline); }
.focus-label { color: var(--reader-muted); margin-inline-end: 3px; font-size: 12px; }
.teach-back-select {
  display: inline-flex; align-items: center; flex-wrap: wrap; gap: 4px; min-width: 0; max-width: 100%; padding: 5px 8px;
  border: 1px solid var(--hairline); border-radius: 5px; background: var(--reader-card); color: var(--reader-ink);
  font: 550 12px/1.5 var(--sans); text-align: start; cursor: pointer;
}
.teach-back-select:hover { border-color: #937f6d; }
.teach-back-select[aria-pressed="true"] { border-color: var(--brand-accent); background: #f6e9dc; }
.focus-dot { width: 7px; height: 7px; flex: none; margin-inline-end: 2px; border-radius: 50%; background: var(--status-ink); }
.focus-ordinal { font-family: system-ui, sans-serif; font-weight: 600; line-height: 1; }
.focus-topic { font-weight: 650; }
.focus-state { color: var(--status-ink); font-weight: 450; }
.focus-state::before { content: "· "; }
.teach-back-select[aria-pressed="true"] .focus-ordinal { color: var(--brand-accent-deep); }
.teach-back-graphs { display: grid; align-items: stretch; gap: 10px; }
.teach-back-row { min-width: 0; margin-top: 10px; padding: 13px 15px; border: 1px solid var(--hairline); border-radius: 8px; background: var(--reader-card); }
.focus-heading { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; }
.focus-heading h4 { display: inline; margin: 0; color: var(--reader-ink); font: 650 15px/1.5 var(--sans); }
.focus-heading .focus-ordinal { margin-inline-end: 2px; color: var(--brand-accent-deep); }
.comparison-status { display: inline-block; padding: 1px 7px; border: 1px solid color-mix(in srgb, var(--status-ink) 45%, var(--reader-card)); border-radius: 4px; color: var(--status-ink); background: color-mix(in srgb, var(--status-ink) 7%, var(--reader-card)); font: 650 12px/1.55 var(--sans); }
.focus-summary > p { margin: 7px 0 0; color: var(--reader-body); font: 400 14px/1.65 var(--sans); }
.focus-summary .divergence { padding-inline-start: 9px; border-inline-start: 2px solid var(--brand-accent-line); color: var(--reader-muted); font-size: 13px; }
.divergence strong { margin-inline-end: .5em; color: var(--reader-ink); font-weight: 580; }
.teach-back-evidence { display: grid; gap: 12px; padding-top: 10px; margin-top: 10px; border-top: 1px solid var(--hairline); }
.teach-back-evidence section { min-width: 0; }
.teach-back-evidence h4, .follow-up strong { margin: 0 0 4px; color: var(--reader-ink); font: 650 13px/1.5 var(--sans); }
.condition, .empty { margin: 3px 0 6px; color: var(--reader-muted); font-size: 12px; }
.more-quotes { margin-top: 4px; }
.more-quotes summary, .teach-back-side-note summary { display: flex; align-items: baseline; gap: 4px; min-height: 24px; color: var(--reader-muted); cursor: pointer; list-style: none; }
.more-quotes summary { font: 500 12px/1.65 var(--sans); }
summary::-webkit-details-marker { display: none; }
summary svg { align-self: start; margin-top: 4px; width: 13px; height: 13px; flex: none; }
details[open] > summary svg { transform: rotate(90deg); }
summary:hover, .quote-action:hover { color: var(--reader-ink); text-decoration: underline; }
.more-quotes[open] > summary { margin-bottom: 4px; }
.follow-up { margin: 12px 0 0; padding: 8px 10px; border-inline-start: 2px solid var(--brand-accent-line); background: var(--reader-note); }
.follow-up p { margin: 3px 0 0; color: var(--reader-body); }
.teach-back-corrections { margin-top: 12px; }
.teach-back-corrections form { margin-top: 10px; }
.teach-back-corrections label { display: grid; gap: 5px; color: var(--reader-body); font-weight: 550; }
.teach-back-corrections textarea { box-sizing: border-box; width: 100%; min-width: 0; margin-bottom: 7px; padding: 8px; border: 1px solid #afa293; border-radius: 5px; background: var(--reader-canvas); color: var(--reader-ink); resize: vertical; font: 400 14px/1.5 var(--sans); }
.correction-actions button:disabled { background: var(--reader-card); border-color: var(--hairline); color: var(--reader-muted); cursor: default; }
.teach-back-side-note { padding: 9px 2px 0; font-size: 13px; color: var(--reader-body); }
.side-note-body { padding-inline-start: 17px; }
.side-note-body p { margin: 3px 0 6px; }
.teach-back-support, .receipt-error { margin: 8px 2px 0; color: var(--reader-muted); font: 400 12px/1.5 var(--sans); }
.teach-back-support { margin: 0 0 7px; }
.receipt-error button { border: 0; background: transparent; color: var(--brand-accent-deep); cursor: pointer; }
@container teach-back (min-width: 560px) {
  .teach-back-graphs, .teach-back-evidence { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .teach-back-evidence { column-gap: 20px; }
}
@container teach-back (max-width: 430px) {
  .focus-bar { align-items: flex-start; }
  .focus-label { width: 100%; }
  .teach-back-select { padding: 5px 7px; }
  .teach-back-row { padding: 11px; }
}
</style>
