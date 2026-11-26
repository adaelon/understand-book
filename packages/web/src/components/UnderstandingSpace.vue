<script setup lang="ts">
import UnderstandingQuote from './UnderstandingQuote.vue';
import { t as uiText, formatLocale } from '../i18n';

import { computed, onMounted, onBeforeUnmount, nextTick, ref, watch } from 'vue';
import { ChevronRight, CornerDownRight, LayoutDashboard, Lightbulb, MessageSquareText, PencilLine, Shapes } from '@lucide/vue';
import { api as sharedApi, ApiError, type UnderstandingView, type UnderstandingRow, type LearningEvidenceView, type UnderstandingTimeline } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import LearningRecordPreview from './LearningRecordPreview.vue';
import UnderstandingSummaryCard from './UnderstandingSummary.vue';
import { openPersonalNotes } from '../personal-notes';
import { continueUnderstanding, type UnderstandingSummary } from '../note-thinking';
const props = withDefaults(defineProps<{ visible?: boolean; evidenceRef?: string }>(), { visible: true });
const api = bindSceneApi(sharedApi);
const view = ref<UnderstandingView | null>(null);
const importedPreview = ref<{ sessionId: string; throughSeq: number } | null>(null);
const active = ref<{ kind: 'interpretation'; id: string } | { kind: 'object'; row: UnderstandingRow } | null>(null);
const selected = ref<LearningEvidenceView | null>(null);
const historicalEvidence = ref(false);
const error = ref('');
const conversationNotice = ref('');
type RelatedBoard = Awaited<ReturnType<typeof sharedApi.boardRelated>>['items'][number];
const relatedBoards = ref<RelatedBoard[]>([]);
const relatedBoardsLoading = ref(false);
const relatedBoardsError = ref('');
let relatedBoardsRequest = 0;
const refreshNotice = ref('');
const feedbackFailure = ref<{ action: string; message: string } | null>(null);
const feedbackBusy = ref(false);
const busy = ref(false);
const drafts = ref<Record<string, string>>({});
const correction = computed({
  get: () => selected.value ? drafts.value[selected.value.evidence_id] ?? '' : '',
  set: text => { if (selected.value) drafts.value[selected.value.evidence_id] = text; },
});
const history = ref<{ refs: string[]; next: number | null } | null>(null);
const timeline = ref<UnderstandingTimeline | null>(null);
const timelineOpen = ref(false);
const timelineRef = ref<string | null>(null);
const timelineStale = ref(false);
const pending = ref<Parameters<typeof api.tutorCorrect>[0] | null>(null);
const detailsOpen = ref(false);
const feedback = ref<HTMLElement>();
const reading = ref<HTMLElement>();
const listing = ref<HTMLElement>();
let origin: HTMLElement | null = null;
let observer: IntersectionObserver | undefined;
let retryAction: (() => Promise<void>) | null = null;
let disposed = false;
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
let refreshController: AbortController | null = null;
let refreshPending = false;
let notifyingChange = false;
const capability: Record<string, string> = { recognition:uiText('识别'), recall:uiText('回忆'), explanation:uiText('解释'), relation:uiText('关系理解'), application:uiText('应用'), evaluation:uiText('来源辨析'), production:uiText('构造'), transfer:uiText('迁移') };
const status: Record<string, string> = { correct:uiText('符合当时标准'), partial:uiText('部分符合当时标准'), incorrect:uiText('尚未符合当时标准'), uncertain:uiText('尚不能形成能力判断'), unassessed:uiText('尚未判定') };
async function resumeUnderstanding(item:UnderstandingSummary,mode:'discuss'|'practice',message?:string){try{await continueUnderstanding(item,undefined,mode,message);}catch(failure){error.value=(failure as Error).message;}}
const row = computed(() => active.value?.kind === 'object' ? active.value.row : null);
const locked = computed(() => busy.value || !!pending.value);
const explanationTitle = computed(() => historicalEvidence.value || row.value?.historical ? uiText('历史解释') : row.value ? uiText('最近解释') : uiText('当前解释'));
function rowKey(item: UnderstandingRow) { return `${item.object_id}:${item.object_revision}:${item.capability}:${item.historical}`; }
function occurrence(value?: string | null) {
  if (!value) return uiText('原话时间未保留');
  const date = /^\d+$/.test(value) ? new Date(Number(value) * (value.length <= 10 ? 1000 : 1)) : new Date(value);
  if (Number.isNaN(date.getTime())) return uiText('原话时间未保留');
  return new Intl.DateTimeFormat(formatLocale(), { year: 'numeric', month: 'long', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }).format(date);
}
function openConversation(evidence: LearningEvidenceView) {
  conversationNotice.value = '';
  if (evidence.source_session_id && evidence.source_turn_id) window.dispatchEvent(new CustomEvent('understanding-open-chat', {
    detail: { session_id: evidence.source_session_id, turn_id: evidence.source_turn_id, reader_key: readerKey(),
      onUnavailable: (message: string) => { if (!disposed) conversationNotice.value = message; } },
  }));
}
function addToBoard() {
  if (!selected.value || locked.value) return;
  window.dispatchEvent(new CustomEvent('understanding-add-to-board', {
    detail: { reader_key: readerKey(), source: { kind: 'understanding', evidence_id: selected.value.evidence_id } },
  }));
}
async function loadRelatedBoards() {
  const evidenceId = selected.value?.evidence_id;
  const request = ++relatedBoardsRequest;
  relatedBoards.value = []; relatedBoardsError.value = '';
  relatedBoardsLoading.value = !!evidenceId;
  if (!evidenceId) return;
  const current = () => !disposed && request === relatedBoardsRequest && selected.value?.evidence_id === evidenceId;
  try {
    const result = await api.boardRelated({ kind: 'understanding', evidence_id: evidenceId });
    if (current()) relatedBoards.value = result.items;
  } catch (failure) {
    if (current()) relatedBoardsError.value = uiText('相关画板暂时无法读取：{0}', [failure instanceof Error ? failure.message : String(failure)]);
  } finally { if (current()) relatedBoardsLoading.value = false; }
}
function openRelatedBoard(item: RelatedBoard) {
  window.dispatchEvent(new CustomEvent('understanding-open-board', {
    detail: { reader_key: readerKey(), board_id: item.board_id,
      ...(item.item_id ? { item_id: item.item_id } : {}), ...(item.block_id ? { block_id: item.block_id } : {}) },
  }));
}
watch(() => selected.value?.evidence_id, () => { void loadRelatedBoards(); });
function resetTimeline() { timeline.value = null; timelineOpen.value = false; timelineRef.value = null; timelineStale.value = false; }
function readTimeline(more = false) {
  const id = more ? timelineRef.value : selected.value?.evidence_id;
  if (!id) return;
  return run(async () => {
    let result = await api.tutorTimeline(id, more ? timeline.value?.next ?? 0 : 0);
    const loadedCount = timeline.value?.entries.length ?? 0;
    while (!more && result.next != null && result.entries.length < loadedCount) {
      const page = await api.tutorTimeline(id, result.next);
      result = { ...page, entries: [...result.entries, ...page.entries] };
    }
    if (disposed) return;
    timelineRef.value = id;
    timeline.value = { ...result, entries: more ? [...timeline.value!.entries, ...result.entries] : result.entries };
    timelineStale.value = false;
  }, uiText('理解变化读取失败'));
}
function toggleTimeline(event: Event) {
  timelineOpen.value = (event.target as HTMLDetailsElement).open;
  if (timelineOpen.value && !timeline.value && !busy.value) void readTimeline();
}
async function run(action: () => Promise<void>, message: string) {
  stopRefresh();
  busy.value = true; error.value = ''; retryAction = null;
  try { await action(); }
  catch (e) {
    if (!disposed) {
      error.value = `${message}：${e instanceof Error ? e.message : String(e)}`;
      retryAction = () => run(action, message);
    }
  } finally { busy.value = false; scheduleRefresh(refreshPending ? 0 : 10_000); }
}
async function refreshView(rebuild = false, more = false, firstPage?: UnderstandingView, current = () => !disposed, signal?: AbortSignal) {
  let result = firstPage ?? await api.tutorUnderstanding(more ? view.value?.next ?? 0 : 0, rebuild, signal);
  if (more && view.value) result = { ...result, rows: [...view.value.rows, ...result.rows],
    interpretations: [...view.value.interpretations ?? [], ...result.interpretations ?? []], current_understanding:[...view.value.current_understanding??[],...result.current_understanding??[]] };
  else {
    // Refresh loaded pages too: a correction on a later page must keep its object visible.
    const count = view.value?.rows.length ?? 0;
    const interpretationCount = view.value?.interpretations?.length ?? 0;
    const summaryCount = view.value?.current_understanding?.length ?? 0;
    while (result.next != null && (result.rows.length < count || (result.interpretations?.length ?? 0) < interpretationCount || (result.current_understanding?.length ?? 0) < summaryCount)) {
      const page = await api.tutorUnderstanding(result.next, false, signal);
      result = { ...page, rows: [...result.rows, ...page.rows],
        interpretations: [...result.interpretations ?? [], ...page.interpretations ?? []], current_understanding:[...result.current_understanding??[],...page.current_understanding??[]] };
    }
  }
  if (disposed || !current()) return;
  view.value = result;
  refreshNotice.value = '';
  if (active.value?.kind === 'object') {
    const updated = result.rows.find(item => rowKey(item) === rowKey(row.value!));
    if (updated) active.value = { kind: 'object', row: updated };
  }
}
function load(rebuild = false, more = false) { return run(() => refreshView(rebuild, more), uiText('理解记录暂不可用')); }
function stopRefresh() {
  clearTimeout(refreshTimer); refreshTimer = undefined;
  refreshController?.abort(); refreshController = null;
}
function canRefresh() { return !disposed && props.visible && document.visibilityState !== 'hidden' && !busy.value && !pending.value; }
function scheduleRefresh(delay = 10_000) {
  clearTimeout(refreshTimer);
  if (canRefresh()) refreshTimer = setTimeout(() => { void refreshInBackground(); }, delay);
}
async function refreshInBackground() {
  if (!canRefresh() || refreshController) return;
  refreshPending = false;
  const controller = new AbortController(); refreshController = controller;
  const current = () => !disposed && !controller.signal.aborted && props.visible && refreshController === controller;
  try {
    const result = await api.tutorUnderstanding(0, false, controller.signal);
    if (!current()) return;
    const changed = !view.value || result.evidence_watermark !== view.value.evidence_watermark || result.projection_watermark !== view.value.projection_watermark;
    if (changed) {
      await refreshView(false, false, result, current, controller.signal);
      if (current() && timeline.value) timelineStale.value = true;
    }
    if (current()) refreshNotice.value = '';
  } catch {
    if (current()) refreshNotice.value = uiText('最新理解暂未更新，已显示的记录仍可阅读。');
  } finally {
    if (refreshController === controller) { refreshController = null; scheduleRefresh(refreshPending ? 0 : 10_000); }
  }
}
function refreshWhenVisible() {
  stopRefresh();
  if (!canRefresh()) return;
  if (view.value) void refreshInBackground(); else void load();
}
async function focusReading() { await nextTick(); reading.value?.focus(); }
function showEvidence(evidence: LearningEvidenceView, historical = false) {
  selected.value = evidence; historicalEvidence.value = historical; detailsOpen.value = false;
}
function openItem(item: NonNullable<typeof active.value>, event?: Event) {
  origin = event?.currentTarget as HTMLElement ?? null;
  return run(async () => {
    const id = item.kind === 'interpretation' ? item.id : item.row.evidence_refs[0];
    const evidence = id ? await api.tutorEvidence(id) : null;
    if (disposed) return;
    active.value = item; selected.value = evidence; historicalEvidence.value = false;
    history.value = null; detailsOpen.value = false; resetTimeline();
    await focusReading();
  }, uiText('依据读取失败'));
}
async function back() {
  active.value = null; selected.value = null; history.value = null; detailsOpen.value = false; resetTimeline();
  error.value = ''; retryAction = null;
  await nextTick();
  if (origin?.isConnected) origin.focus();
  else listing.value?.focus();
}
function inspect(id: string, historical = false) {
  return run(async () => {
    const evidence = await api.tutorEvidence(id);
    if (disposed) return;
    showEvidence(evidence, historical);
    await focusReading();
  }, uiText('依据读取失败'));
}
function older(more = false) {
  const target = row.value;
  if (!target) return;
  const before = more ? history.value?.next ?? undefined : undefined;
  return run(async () => {
    const result = await api.tutorEvidenceList(target, before);
    if (!disposed) history.value = { refs: more ? [...history.value!.refs, ...result.refs] : result.refs, next: result.next };
  }, uiText('历史依据读取失败'));
}
async function correct() {
  if (!pending.value && (!selected.value || !correction.value.trim())) return;
  const request = pending.value ?? { evidence_ref: selected.value!.evidence_id, operation_id: crypto.randomUUID(), text: correction.value.trim() };
  pending.value = request;
  // After a confirmed write, retries only read its returned record.
  let savedRef: string | null = null;
  await run(async () => {
    if (!savedRef) {
      try { savedRef = (await api.tutorCorrect(request)).evidence_ref; }
      catch (e) { if (e instanceof ApiError && e.status < 500) pending.value = null; throw e; }
    }
    const evidence = await api.tutorEvidence(savedRef);
    await refreshView();
    if (disposed) return;
    showEvidence(evidence);
    if (active.value?.kind === 'interpretation') active.value = { kind: 'interpretation', id: savedRef };
    history.value = null; resetTimeline(); delete drafts.value[request.evidence_ref]; pending.value = null;
    notifyingChange = true; window.dispatchEvent(new Event('tutor-state-changed')); notifyingChange = false;
    await focusReading();
  }, uiText('纠正提交或刷新未完成'));
}
async function recordFeedback(action: string) {
  feedbackBusy.value = true;
  try {
    await api.tutorFeedbackDisplayed(action);
    if (feedbackFailure.value?.action === action) feedbackFailure.value = null;
  }
  catch (e) {
    if (!disposed) {
      feedbackFailure.value = { action, message: uiText('学习反馈的阅读进度未保存：{0}', [e instanceof Error ? e.message : String(e)]) };
    }
  } finally { feedbackBusy.value = false; }
}
watch(feedback, element => {
  observer?.disconnect();
  if (element?.dataset.feedbackRef) observer?.observe(element);
}, { flush: 'post' });
function onUnderstandingChanged() {
  if (notifyingChange) return;
  refreshPending = true;
  if (canRefresh()) void refreshInBackground();
}
watch(() => props.visible, refreshWhenVisible);
watch(() => props.evidenceRef, id => { if(id) void openItem({kind:'interpretation',id}); });
onMounted(() => {
  window.addEventListener('tutor-state-changed', onUnderstandingChanged);
  document.addEventListener('visibilitychange', refreshWhenVisible);
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting || entry.target !== feedback.value) continue;
      const action = (entry.target as HTMLElement).dataset.feedbackRef;
      if (action) { observer?.unobserve(entry.target); void recordFeedback(action); }
    }
  });
  if (props.visible) { if(props.evidenceRef) void openItem({kind:'interpretation',id:props.evidenceRef}); else void load(); }
});
onBeforeUnmount(() => { disposed = true; stopRefresh(); observer?.disconnect(); window.removeEventListener('tutor-state-changed', onUnderstandingChanged); document.removeEventListener('visibilitychange', refreshWhenVisible); });
</script>

<template>
  <section :aria-label="uiText('我的理解')" class="understanding-space" :aria-busy="busy">
    <header v-show="!active" class="space-heading"><h3>{{ uiText('看见理解的变化') }}</h3><p>{{ uiText('从你的原话出发，让每一次理解都有迹可循。') }}</p></header>
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="retryAction?.()">{{ uiText('重试') }}</button></p>
    <p v-if="conversationNotice" role="status">{{ conversationNotice }}</p>
    <p v-if="refreshNotice" role="status">{{ refreshNotice }} <button :disabled="locked" @click="load()">{{ uiText('刷新理解记录') }}</button></p>
    <p v-if="feedbackFailure" role="alert">{{ feedbackFailure.message }} <button :disabled="locked || feedbackBusy" @click="recordFeedback(feedbackFailure.action)">{{ uiText('重试反馈记录') }}</button></p>
    <p v-if="busy" role="status">{{ uiText('正在读取或保存…') }}</p>
    <p v-if="view?.stale" role="status">{{ uiText('有新的依据尚未合入此视图。') }}<button :disabled="locked" @click="load(true)">{{ uiText('更新理解视图') }}</button></p>
    <div v-show="!active" ref="listing" tabindex="-1" class="listing">
      <section v-if="view?.current_understanding?.length" class="current-understanding" aria-label="当前理解与卡点"><h4>当前理解与卡点</h4><UnderstandingSummaryCard v-for="item in view.current_understanding" :key="`${item.evidence_ref}`" :item="item" :busy="locked"  @evidence="openItem({kind:'interpretation',id:item.evidence_ref},$event)" @correct="openItem({kind:'interpretation',id:item.evidence_ref},$event)" @note="openPersonalNotes" @discuss="resumeUnderstanding(item,'discuss',$event)" @practice="resumeUnderstanding(item,'practice')" /></section>
      <p v-if="view && !view.rows.length && !view.interpretations?.length && !view.current_understanding?.length" class="empty">{{ view.imported_records?.length ? uiText('当前工作区尚无新的理解记录') : uiText('尚无理解记录') }}{{ uiText('；相关提问、回应和活动表现会成为后续讲解的依据。') }}</p>
      <section v-if="view?.imported_records?.length" :aria-label="uiText('带入的原记录')" class="imported-records">
        <h4>{{ uiText('带入的原记录') }}</h4><p>{{ uiText('保留原聊天、解释与来源，作为这个目标的历史参考。新的理解会从当前工作区的交流中形成。') }}</p>
        <article v-for="record in view.imported_records" :key="`${record.selection.source_learning_workspace_id}:${record.selection.chat_session_id}`">
          <strong>{{ record.title || uiText('未命名聊天') }}</strong><span>{{ record.source_goal || (record.selection.source_learning_workspace_id ? uiText('其他工作区') : uiText('旧单书记录')) }}</span>
          <button :disabled="!record.available || locked" @click="importedPreview = { sessionId: record.selection.chat_session_id, throughSeq: record.through_seq }">{{ uiText('查看原记录') }}</button><small v-if="!record.available">{{ uiText('原记录暂不可读') }}</small>
        </article>
      </section>
      <section v-if="view?.interpretations?.length" :aria-label="uiText('当前理解解释')" class="concept-list">
        <h4>{{ uiText('最近的理解 · 按原话时间排列') }}</h4>
        <button v-for="item in view.interpretations" :key="item.evidence_id" class="concept" :disabled="locked" @click="openItem({ kind: 'interpretation', id: item.evidence_id }, $event)">
          <span class="concept-icon"><Lightbulb aria-hidden="true" /></span><span class="concept-copy">
          <span class="concept-heading"><strong>{{ item.label }}</strong><span class="nature">{{ item.nature === 'hypothesis' ? uiText('暂定理解') : uiText('活动表现') }}</span></span>
          <span class="preview">{{ item.interpretation }}</span>
          <span class="record-meta"><span>{{ occurrence(item.occurred_at) }}</span><span v-if="item.source_session_title">{{ uiText('关联对话') }} · {{ item.source_session_title }}</span></span>
          <span v-if="item.correction" class="revision-label">{{ uiText('有你的纠正') }}</span>
          </span><ChevronRight class="concept-chevron" aria-hidden="true" />
        </button>
      </section>
      <section v-if="view?.rows.length" :aria-label="uiText('概念与能力')" class="concept-list">
        <h4>{{ uiText('概念与能力') }}</h4>
        <button v-for="item in view.rows" :key="rowKey(item)" class="concept" :disabled="locked" @click="openItem({ kind: 'object', row: item }, $event)">
          <span class="concept-icon"><Shapes aria-hidden="true" /></span><span class="concept-copy">
          <span class="concept-heading"><strong>{{ item.label }}</strong><span v-if="item.historical" class="nature">{{ uiText('历史版本') }}</span></span>
          <span class="preview">{{ capability[item.capability] ?? (item.capability || uiText('尚未观察能力')) }} · {{ item.state === 'unknown' ? uiText('未知') : uiText('已有表现依据') }}</span>
          </span><ChevronRight class="concept-chevron" aria-hidden="true" />
        </button>
      </section>
      <button v-if="view?.next != null" :disabled="locked" @click="load(false, true)">{{ uiText('查看更多对象') }}</button>
      <p v-if="view?.rows.length || view?.interpretations?.length" class="list-note"><MessageSquareText aria-hidden="true" />{{ uiText('点开一项，查看原话与解释，也可以补充你的理解。') }}</p>
    </div>
    <section v-if="active" ref="reading" tabindex="-1" :aria-label="uiText('单项理解')" class="reading">
      <button class="back" :disabled="locked" @click="back">{{ uiText('← 返回概念列表') }}</button>
      <div class="reading-heading"><h4 class="reading-title">{{ row?.label ?? selected?.label }}</h4><span v-if="selected" class="nature">{{ selected.nature === 'hypothesis' ? uiText('暂定理解') : uiText('活动表现') }}<span v-if="historicalEvidence"> {{ uiText('· 历史记录') }}</span></span></div>
      <p v-if="row?.historical" class="nature">{{ uiText('历史版本 · 对象版本') }} {{ row.object_revision }}</p>
      <section v-if="row" :aria-label="uiText('能力依据概况')" class="summary">
        <p>{{ capability[row.capability] ?? (row.capability || uiText('尚未观察能力')) }}：{{ row.state === 'unknown' ? uiText('未知') : uiText('已有表现依据') }}</p>
        <p v-if="row.evidence_count">{{ uiText('独立符合标准') }} {{ row.independent_support }} {{ uiText('次 · 帮助后符合') }} {{ row.assisted_support }} {{ uiText('次 · 改答后符合') }} {{ row.revised_support }} {{ uiText('次') }}</p>
        <p v-if="row.partial || row.difficulty">{{ uiText('部分符合') }} {{ row.partial }} {{ uiText('次 · 尚未符合') }} {{ row.difficulty }} {{ uiText('次') }}</p>
        <p v-if="row.uncertain">{{ uiText('有') }} {{ row.uncertain }} {{ uiText('条尚不能形成能力判断的解释。') }}</p>
        <p v-if="!row.evidence_count">{{ uiText('尚无这项能力的表现依据。') }}</p>
      </section>
      <article v-if="selected" :aria-label="uiText('表现依据')" class="evidence">
        <button type="button" :disabled="locked" @click="addToBoard">{{ uiText('加入画板') }}</button>
        <div class="record-origin"><span>{{ occurrence(selected.occurred_at) }}</span><span v-if="selected.source_session_title">{{ uiText('关联对话') }}</span><button v-if="selected.source_session_id && selected.source_turn_id" class="conversation-link" :disabled="locked" @click="openConversation(selected)"><MessageSquareText aria-hidden="true" />{{ selected.source_session_title || uiText('查看对应对话') }}</button><span v-else-if="selected.source_session_title">{{ selected.source_session_title }}</span></div>
        <section class="related-boards" :aria-label="uiText('相关画板')" :aria-busy="relatedBoardsLoading">
          <h5><LayoutDashboard :size="14" aria-hidden="true" />{{ uiText('相关画板') }}</h5>
          <p v-if="relatedBoardsLoading" role="status">{{ uiText('正在查找相关画板…') }}</p>
          <p v-else-if="relatedBoardsError" role="alert">{{ relatedBoardsError }} <button type="button" @click="loadRelatedBoards">{{ uiText('重试查找画板') }}</button></p>
          <ul v-else-if="relatedBoards.length">
            <li v-for="item in relatedBoards" :key="`${item.board_id}:${item.item_id ?? ''}:${item.block_id ?? ''}`">
              <button type="button" :disabled="locked" @click="openRelatedBoard(item)"><span><strong>{{ item.board_title }}</strong><small v-if="item.title !== item.board_title">{{ item.title }}</small></span><span class="related-board-action">{{ uiText('在画板中查看') }}<ChevronRight :size="14" aria-hidden="true" /></span></button>
            </li>
          </ul>
          <p v-else>{{ uiText('还没有关联画板，可以把这条理解加入画板。') }}</p>
        </section>
        <p v-if="selected.superseded" class="revision-notice">{{ uiText('这条解释后来已被更正，原话和当时的记录仍然保留。') }}</p>
        <section class="explanation"><h5>{{ explanationTitle }} {{ uiText('· 系统解释') }}</h5><p>{{ selected.interpretation }}</p></section>
        <section v-if="selected.nature === 'hypothesis'" class="context" :aria-label="uiText('适用目标与帮助条件')">
          <h5>{{ uiText('适用目标与帮助条件') }}</h5><p>{{ selected.prompt || uiText('此条记录未保留适用目标。') }}</p>
          <p v-if="selected.assistance_count" class="conditions">{{ uiText('这次交流前已展示') }} {{ selected.assistance_count }} {{ uiText('条讲解或帮助，理解需结合当时的帮助来看。') }}</p>
        </section>
        <section v-if="selected.nature !== 'hypothesis'" class="context">
          <template v-if="selected.activity_turn_request"><h5>{{ uiText('本轮请求') }}</h5><p>{{ selected.activity_turn_request }}</p></template>
          <h5>{{ uiText('当时的活动') }}</h5><p>{{ selected.prompt || uiText('此条记录未保留活动说明。') }}</p>
        </section>
        <section class="quote"><h5>{{ uiText('你的原话') }}</h5><UnderstandingQuote :quote="selected.learner_quote" :choice="selected.choice_answer" /></section>
        <p v-if="selected.nature === 'hypothesis'" class="conditions">{{ uiText('这是普通交流中的暂定理解，未评分，可随你的纠正和新表现修订。') }}</p>
        <template v-else>
          <p class="conditions">{{ uiText('第') }} {{ selected.attempt }} {{ uiText('次作答；') }}{{ selected.assistance_count ? uiText('帮助后作答，回答前已展示帮助') : uiText('未记录到事先展示的帮助') }}<span v-if="selected.attempt > 1">{{ uiText('；改答后记录') }}</span>。</p>
          <p v-if="status[selected.status]">{{ status[selected.status] }}</p>
        </template>
        <p v-if="selected.correction" class="correction">{{ uiText('你的纠正：') }}{{ selected.correction }}</p>
        <section v-if="selected.teaching_implication" class="implication"><CornerDownRight aria-hidden="true" /><div><h5>{{ uiText('接下来怎样帮助你') }}</h5><p>{{ selected.teaching_implication }}</p></div></section>
        <details :key="selected.evidence_id" :open="detailsOpen" class="facts" @toggle="detailsOpen = ($event.target as HTMLDetailsElement).open">
          <summary>{{ uiText('查看详细依据') }}</summary>
          <div v-if="detailsOpen">
            <p v-if="selected.feedback_hidden">{{ uiText('详细判定依据会在本活动请求直接讲解后揭示。') }}</p>
            <ul v-else-if="selected.source_quotes?.length" ref="feedback" :data-feedback-ref="selected.assessment_ref ?? undefined">
              <li v-for="(item, index) in selected.source_quotes" :key="index"><p v-if="item.reason">{{ item.reason }}</p><blockquote v-if="item.response_quote">{{ uiText('作答片段：') }}{{ item.response_quote }}</blockquote><blockquote v-if="item.quote">{{ item.quote }}</blockquote><blockquote v-for="(source, i) in item.sources" :key="i">{{ source.quote ?? source.observation }}</blockquote></li>
            </ul>
            <p v-else>{{ uiText('此条记录未附详细引用。') }}</p>
          </div>
        </details>
        <details class="correction-editor">
          <summary><PencilLine aria-hidden="true" />{{ uiText('纠正这条解释') }}</summary>
          <form class="correction-form" @submit.prevent="correct">
          <label>{{ uiText('纠正这条解释') }}<textarea v-model="correction" :disabled="!!pending" rows="3" :placeholder="uiText('写下你的实际理解或需要更正的地方')" /></label>
          <button class="save" :disabled="locked || !correction.trim()">{{ uiText('保存纠正') }}</button>
          </form>
        </details>
      </article>
      <details v-if="selected" class="understanding-timeline" :open="timelineOpen" @toggle="toggleTimeline">
        <summary>{{ uiText('理解变化时间线') }}<span>{{ uiText('从早到晚，保留每次表达和当时的帮助') }}</span></summary>
        <section v-if="timeline" :aria-label="uiText('理解变化时间线')">
          <button v-if="timelineStale" :disabled="locked" @click="readTimeline()">{{ uiText('理解记录已更新，刷新时间线') }}</button>
          <p class="timeline-note">{{ uiText('时间对应你的原话；后来的表达不会抹去此前的理解。解释有误时，会单独标明更正。') }}</p>
          <ol v-if="timeline.entries.length" class="timeline-list">
            <li v-for="entry in timeline.entries" :key="entry.evidence_id" :class="{ 'is-corrected': entry.superseded }">
              <div class="timeline-date"><span>{{ occurrence(entry.occurred_at) }}</span><span v-if="entry.superseded" class="revision-label">{{ uiText('解释已被更正') }}</span><span v-else-if="entry.supersedes" class="revision-label">{{ uiText('对原解释的更正') }}</span></div>
              <p v-if="entry.source_session_title" class="timeline-conversation">{{ uiText('关联对话') }} · {{ entry.source_session_title }}</p>
              <div class="timeline-content">
                <h5>{{ uiText('你的原话') }}</h5>
                <UnderstandingQuote v-if="entry.choice_answer" :quote="entry.learner_quote" :choice="entry.choice_answer" />
                <template v-else-if="entry.expressions?.length"><blockquote v-for="expression in entry.expressions" :key="expression.fact_ref">{{ expression.text }}</blockquote></template>
                <blockquote v-else>{{ entry.learner_quote || uiText('此条记录未保留原话。') }}</blockquote>
                <div v-if="entry.assistance?.length" class="timeline-assistance"><h5>{{ uiText('当时得到的帮助') }}</h5><p v-for="help in entry.assistance" :key="help.fact_ref">{{ help.text }}</p></div>
                <p v-else class="conditions">{{ entry.assistance_count ? uiText('当时已展示 {0} 条讲解或帮助，未保留帮助正文。', [entry.assistance_count]) : uiText('此条记录未关联具体帮助；不能据此判定为独立完成。') }}</p>
                <h5>{{ entry.superseded ? uiText('当时的解释 · 后来已更正') : entry.supersedes ? uiText('更正后的解释') : uiText('当时的系统解释') }}</h5><p class="timeline-interpretation">{{ entry.interpretation }}</p>
                <p v-if="entry.correction" class="correction">{{ uiText('你的纠正：') }}{{ entry.correction }}</p>
                <p v-if="entry.teaching_implication" class="timeline-next">{{ uiText('当时的后续建议：') }}{{ entry.teaching_implication }}</p>
                <div class="timeline-actions"><button :disabled="locked" @click="inspect(entry.evidence_id, true)">{{ uiText('查看这次依据') }}</button><button v-if="entry.source_session_id && entry.source_turn_id" class="conversation-link" :disabled="locked" @click="openConversation(entry)">{{ uiText('查看对应对话') }}</button></div>
              </div>
            </li>
          </ol>
          <p v-else class="empty">{{ uiText('这项理解暂时没有可读取的变化记录。') }}</p>
          <p v-if="timeline.entries.length === 1 && timeline.next == null" class="timeline-note">{{ uiText('这是目前唯一一次记录。以后与这项理解有关的新表达会继续保留在这里。') }}</p>
          <button v-if="timeline.next != null" :disabled="locked" @click="readTimeline(true)">{{ uiText('继续查看后续记录') }}</button>
        </section>
      </details>
      <details v-if="row?.evidence_count" class="history">
        <summary>{{ uiText('依据与解释历史') }}</summary>
        <div class="evidence-links"><button v-for="(id, index) in row.evidence_refs" :key="id" :disabled="locked" :aria-pressed="selected?.evidence_id === id" @click="inspect(id, index > 0)">{{ uiText('查看依据') }} {{ index + 1 }}</button></div>
        <button :disabled="locked" @click="older()">{{ uiText('查看完整解释历史') }}</button>
        <section v-if="history" :aria-label="uiText('解释历史')"><h5>{{ row.label }}{{ uiText('的解释历史') }}</h5><div class="evidence-links"><button v-for="(id, index) in history.refs" :key="id" :disabled="locked" :aria-pressed="selected?.evidence_id === id" @click="inspect(id, true)">{{ uiText('查看历史依据') }} {{ index + 1 }}</button></div><button v-if="history.next != null" :disabled="locked" @click="older(true)">{{ uiText('更早的解释') }}</button></section>
      </details>
    </section>
    <LearningRecordPreview v-if="importedPreview" :session-id="importedPreview.sessionId" :through-seq="importedPreview.throughSeq" @close="importedPreview = null" />
  </section>
</template>

<style scoped>
.understanding-space { min-width: 0; overflow-wrap: anywhere; color: var(--reader-ink, var(--ink)); font: 14px/1.7 var(--sans); }
.imported-records { margin: 20px 0 28px; padding: 16px; background: var(--surface-soft); border-radius: 10px; }.imported-records > p { margin: 8px 0 14px; color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.imported-records article { display: flex; align-items: center; flex-wrap: wrap; gap: 8px 12px; padding: 12px 0; border-top: 1px solid var(--line); }.imported-records article > span, .imported-records small { color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.imported-records article > strong { flex: 1; }
.space-heading h3, h4, h5, p { margin: 0; }
.space-heading { margin: 4px 0 25px; }.space-heading h3 { font-size: 24px; line-height: 1.45; font-weight: 650; margin-bottom: 7px; }.space-heading p, .preview, .conditions { color: var(--tutor-muted, var(--reader-muted)); }.space-heading p { font-size: 13px; }
svg { width: 18px; height: 18px; stroke-width: 1.7; flex-shrink: 0; }
button { font: inherit; font-size: 13px; min-height: 40px; max-width: 100%; padding: 8px 12px; border: 1px solid var(--line); border-radius: 9px; background: var(--reader-card); color: inherit; cursor: pointer; white-space: normal; overflow-wrap: anywhere; }
button:disabled { opacity: .55; cursor: default; }
button:focus-visible, summary:focus-visible, textarea:focus-visible { outline: 2px solid var(--focus-blue); outline-offset: 3px; }
.concept-list + .concept-list { margin-top: 25px; }.concept-list h4 { font-size: 11px; letter-spacing: .5px; font-weight: 500; color: var(--tutor-muted, var(--reader-muted)); margin-bottom: 8px; }
.concept { display: flex; align-items: center; gap: 14px; width: 100%; text-align: left; margin: 0; padding: 18px 0; border: 0; border-top: 1px solid var(--line); border-radius: 0; background: transparent; }
.concept:hover:not(:disabled) .concept-heading strong { color: var(--brand-green-deep); }.concept-icon { display: grid; place-items: center; width: 36px; height: 40px; border: 1px solid var(--line); background: var(--surface-soft); color: var(--brand-green-deep); border-radius: 9px; flex-shrink: 0; }.concept-copy { min-width: 0; flex: 1; }.concept-chevron { width: 15px; height: 15px; color: var(--tutor-muted, var(--reader-muted)); }
.concept-heading { display: flex; flex-wrap: wrap; gap: 4px 12px; align-items: center; }.concept-heading strong { font-size: 14px; font-weight: 600; }
.nature { display: inline-block; color: var(--tutor-green, #56725a); background: var(--tutor-green-soft, #edf2e9); padding: 2px 7px; border-radius: 5px; font-size: 11px; font-weight: 400; flex-shrink: 0; }
.preview { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; margin-top: 5px; font-size: 12px; }.revision-label { display: block; color: var(--tutor-muted, var(--reader-muted)); font-size: 11px; margin-top: 4px; }
.record-meta { display: flex; flex-wrap: wrap; gap: 2px 12px; margin-top: 8px; color: var(--tutor-muted, var(--reader-muted)); font-size: 11px; }.record-meta > span { min-width: 0; }
.record-origin { display: flex; align-items: center; flex-wrap: wrap; gap: 4px 12px; color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.conversation-link { display: inline-flex; align-items: center; gap: 6px; background: transparent; border: 0; padding: 6px 0; color: var(--brand-green-deep); text-align: left; }.conversation-link svg { width: 15px; height: 15px; }.revision-notice { padding: 10px 14px; background: var(--surface-soft); border-radius: 8px; font-size: 12px; color: var(--tutor-muted, var(--reader-muted)); }
.list-note { display: flex; align-items: flex-start; gap: 8px; padding-top: 18px; border-top: 1px solid var(--line); color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.list-note svg { width: 16px; height: 16px; margin-top: 2px; }
.reading, .listing { min-width: 0; outline: none; }.back { border: 0; background: transparent; padding-left: 0; color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; min-height: 32px; }
.reading-heading { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; margin: 17px 0 24px; }.reading-title { font-size: 25px; line-height: 1.45; font-weight: 650; }
.summary { padding: 0 0 20px; font-size: 13px; }.summary p + p { margin-top: 8px; font-size: 12px; }
.evidence { padding: 0; }.evidence > * + * { margin-top: 22px; }
.related-boards { padding: 16px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface-soft); }
.related-boards h5 { display: flex; align-items: center; gap: 7px; }
.related-boards p { margin: 0; font-size: 12px; line-height: 1.8; color: var(--tutor-muted, var(--reader-muted)); }
.related-boards ul { display: grid; gap: 7px; margin: 0; padding: 0; list-style: none; }
.related-boards li > button { display: flex; align-items: center; justify-content: space-between; gap: 12px; width: 100%; min-height: 44px; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--reader-card); text-align: left; }
.related-boards li > button > span:first-child { min-width: 0; }
.related-boards strong, .related-boards small { display: block; overflow-wrap: anywhere; }
.related-boards strong { font-size: 13px; font-weight: 550; }
.related-boards small { margin-top: 4px; font-size: 11px; color: var(--tutor-muted, var(--reader-muted)); }
.related-board-action { display: inline-flex; align-items: center; gap: 4px; flex-shrink: 0; font-size: 11px; color: var(--brand-green-deep); }
h5 { font-size: 11px; font-weight: 500; color: var(--tutor-muted, var(--reader-muted)); margin-bottom: 9px; letter-spacing: .5px; }
.explanation p { font-size: 15px; line-height: 1.85; }.evidence p, blockquote { white-space: pre-wrap; }
.quote { background: var(--surface-soft); border-radius: 11px; padding: 18px 20px; }blockquote { margin: 0; padding: 0; border: 0; font-size: 16px; line-height: 1.85; }
.correction { padding: 14px 16px; border-left: 2px solid var(--reader-coral); background: var(--reader-coral-soft); border-radius: 0 8px 8px 0; font-size: 13px; }
.conditions { font-size: 12px; }.implication { display: flex; align-items: flex-start; gap: 12px; font-size: 13px; }.implication > svg { color: var(--brand-green-deep); margin-top: 1px; }.implication > div { min-width: 0; }
details { border-top: 1px solid var(--line); padding-top: 8px; }summary { min-height: 44px; padding: 8px 0; cursor: pointer; font-size: 12px; color: var(--tutor-muted, var(--reader-muted)); }
.facts ul { padding-left: 20px; }.facts li + li, .facts blockquote { margin-top: 12px; }.facts blockquote { font-size: 13px; padding-left: 12px; border-left: 2px solid var(--line); }.facts p { font-size: 13px; }
.correction-editor > summary { display: flex; align-items: center; gap: 7px; color: var(--reader-ink); }.correction-editor > summary svg { width: 16px; height: 16px; }
.correction-form { padding-top: 8px; }.correction-form label { display: block; font-size: 13px; }
textarea { display: block; width: 100%; box-sizing: border-box; font: inherit; line-height: 1.7; padding: 14px; margin: 8px 0; resize: vertical; border: 1px solid var(--line); border-radius: 10px; background: var(--reader-card); color: inherit; }
.save { background: var(--brand-green-deep); color: var(--reader-card); border-color: var(--brand-green-deep); min-height: 44px; }
.history { margin-top: 24px; }.history h5 { margin-top: 16px; }.evidence-links { display: flex; flex-wrap: wrap; gap: 8px; margin: 8px 0; }[aria-pressed='true'] { border-color: var(--brand-green-deep); background: var(--reader-coral-soft); }
.understanding-timeline { margin-top: 28px; }.understanding-timeline > summary { color: var(--reader-ink); font-size: 14px; }.understanding-timeline > summary > span { display: block; padding-left: 17px; color: var(--tutor-muted, var(--reader-muted)); font-size: 11px; }.timeline-note { color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; margin: 8px 0 20px; }.timeline-list { list-style: none; padding: 0 0 0 9px; margin: 20px 0; }.timeline-list > li { position: relative; padding: 0 0 30px 22px; border-left: 1px solid var(--line); min-width: 0; }.timeline-list > li:last-child { padding-bottom: 0; }.timeline-list > li::before { content: ''; position: absolute; width: 7px; height: 7px; left: -4px; top: 7px; border-radius: 50%; background: var(--brand-green-deep); }.timeline-list > li.is-corrected::before { background: var(--reader-muted); }.timeline-date { display: flex; align-items: baseline; flex-wrap: wrap; gap: 4px 10px; font-size: 12px; font-weight: 600; }.timeline-date .revision-label { margin: 0; font-weight: 400; }.timeline-conversation { color: var(--tutor-muted, var(--reader-muted)); font-size: 11px; margin-top: 3px; }.timeline-content { margin-top: 14px; }.timeline-content blockquote { padding: 12px 15px; background: var(--surface-soft); border-radius: 8px; font-size: 14px; line-height: 1.85; white-space: pre-wrap; }.timeline-content blockquote + blockquote { margin-top: 8px; }.timeline-content > h5:not(:first-child), .timeline-assistance { margin-top: 18px; }.timeline-content > .conditions { margin-top: 12px; }.timeline-assistance { padding-left: 12px; border-left: 2px solid var(--line); color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.timeline-assistance p + p { margin-top: 8px; }.timeline-interpretation, .timeline-assistance p { white-space: pre-wrap; }.timeline-next { margin-top: 12px; font-size: 12px; color: var(--tutor-muted, var(--reader-muted)); }.timeline-content .correction { margin-top: 12px; }.timeline-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; margin-top: 12px; }
[role='alert'], [role='status'], .empty { margin: 12px 0; font-size: 13px; }[role='alert'] { color: var(--brand-error); }.empty { padding: 28px 0; color: var(--tutor-muted, var(--reader-muted)); }
.space-heading h3 { font-family: var(--font-display); font-weight: 400; font-synthesis: none; }
@media (max-width: 480px) { .space-heading h3 { font-size: 22px; }.reading-title { font-size: 23px; }.quote { padding: 16px; }.concept { gap: 10px; }.concept-icon { width: 32px; height: 36px; }textarea { font-size: 16px; } }
</style>
