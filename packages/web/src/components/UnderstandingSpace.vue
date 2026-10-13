<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, nextTick, ref, watch } from 'vue';
import { ChevronRight, CornerDownRight, Lightbulb, MessageSquareText, PencilLine, Shapes } from '@lucide/vue';
import { api as sharedApi, ApiError, type UnderstandingView, type UnderstandingRow, type LearningEvidenceView } from '../api';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
const view = ref<UnderstandingView | null>(null);
const active = ref<{ kind: 'interpretation'; id: string } | { kind: 'object'; row: UnderstandingRow } | null>(null);
const selected = ref<LearningEvidenceView | null>(null);
const historicalEvidence = ref(false);
const error = ref('');
const feedbackFailure = ref<{ action: string; message: string } | null>(null);
const feedbackBusy = ref(false);
const busy = ref(false);
const drafts = ref<Record<string, string>>({});
const correction = computed({
  get: () => selected.value ? drafts.value[selected.value.evidence_id] ?? '' : '',
  set: text => { if (selected.value) drafts.value[selected.value.evidence_id] = text; },
});
const history = ref<{ refs: string[]; next: number | null } | null>(null);
const pending = ref<Parameters<typeof api.tutorCorrect>[0] | null>(null);
const detailsOpen = ref(false);
const feedback = ref<HTMLElement>();
const reading = ref<HTMLElement>();
const listing = ref<HTMLElement>();
let origin: HTMLElement | null = null;
let observer: IntersectionObserver | undefined;
let retryAction: (() => Promise<void>) | null = null;
let disposed = false;
const capability: Record<string, string> = { recognition:'识别', recall:'回忆', explanation:'解释', relation:'关系理解', application:'应用', evaluation:'来源辨析', production:'构造', transfer:'迁移' };
const status: Record<string, string> = { correct:'符合当时标准', partial:'部分符合当时标准', incorrect:'尚未符合当时标准', uncertain:'尚不能形成能力判断', unassessed:'尚未判定' };
const row = computed(() => active.value?.kind === 'object' ? active.value.row : null);
const locked = computed(() => busy.value || !!pending.value);
const explanationTitle = computed(() => historicalEvidence.value || row.value?.historical ? '历史解释' : row.value ? '最近解释' : '当前解释');
function rowKey(item: UnderstandingRow) { return `${item.object_id}:${item.object_revision}:${item.capability}:${item.historical}`; }
async function run(action: () => Promise<void>, message: string) {
  busy.value = true; error.value = ''; retryAction = null;
  try { await action(); }
  catch (e) {
    if (!disposed) {
      error.value = `${message}：${e instanceof Error ? e.message : String(e)}`;
      retryAction = () => run(action, message);
    }
  } finally { busy.value = false; }
}
async function refreshView(rebuild = false, more = false) {
  let result = await api.tutorUnderstanding(more ? view.value?.next ?? 0 : 0, rebuild);
  if (more && view.value) result = { ...result, rows: [...view.value.rows, ...result.rows],
    interpretations: [...view.value.interpretations ?? [], ...result.interpretations ?? []] };
  else {
    // Refresh loaded pages too: a correction on a later page must keep its object visible.
    const count = view.value?.rows.length ?? 0;
    const interpretationCount = view.value?.interpretations?.length ?? 0;
    while (result.next != null && (result.rows.length < count || (result.interpretations?.length ?? 0) < interpretationCount)) {
      const page = await api.tutorUnderstanding(result.next);
      result = { ...page, rows: [...result.rows, ...page.rows],
        interpretations: [...result.interpretations ?? [], ...page.interpretations ?? []] };
    }
  }
  if (disposed) return;
  view.value = result;
  if (active.value?.kind === 'object') {
    const updated = result.rows.find(item => rowKey(item) === rowKey(row.value!));
    if (updated) active.value = { kind: 'object', row: updated };
  }
}
function load(rebuild = false, more = false) { return run(() => refreshView(rebuild, more), '理解记录暂不可用'); }
async function focusReading() { await nextTick(); reading.value?.focus(); }
function showEvidence(evidence: LearningEvidenceView, historical = false) {
  selected.value = evidence; historicalEvidence.value = historical; detailsOpen.value = false;
}
function openItem(item: NonNullable<typeof active.value>, event: Event) {
  origin = event.currentTarget as HTMLElement;
  return run(async () => {
    const id = item.kind === 'interpretation' ? item.id : item.row.evidence_refs[0];
    const evidence = id ? await api.tutorEvidence(id) : null;
    if (disposed) return;
    active.value = item; selected.value = evidence; historicalEvidence.value = false;
    history.value = null; detailsOpen.value = false;
    await focusReading();
  }, '依据读取失败');
}
async function back() {
  active.value = null; selected.value = null; history.value = null; detailsOpen.value = false;
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
  }, '依据读取失败');
}
function older(more = false) {
  const target = row.value;
  if (!target) return;
  const before = more ? history.value?.next ?? undefined : undefined;
  return run(async () => {
    const result = await api.tutorEvidenceList(target, before);
    if (!disposed) history.value = { refs: more ? [...history.value!.refs, ...result.refs] : result.refs, next: result.next };
  }, '历史依据读取失败');
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
    history.value = null; delete drafts.value[request.evidence_ref]; pending.value = null;
    window.dispatchEvent(new Event('tutor-state-changed'));
    await focusReading();
  }, '纠正提交或刷新未完成');
}
async function recordFeedback(action: string) {
  feedbackBusy.value = true;
  try {
    await api.tutorFeedbackDisplayed(action);
    if (feedbackFailure.value?.action === action) feedbackFailure.value = null;
  }
  catch (e) {
    if (!disposed) {
      feedbackFailure.value = { action, message: `反馈展示记录未保存：${e instanceof Error ? e.message : String(e)}` };
    }
  } finally { feedbackBusy.value = false; }
}
watch(feedback, element => {
  observer?.disconnect();
  if (element?.dataset.feedbackRef) observer?.observe(element);
}, { flush: 'post' });
function onUnderstandingChanged() { if (!pending.value && !busy.value) void load(); }
onMounted(() => {
  window.addEventListener('tutor-state-changed', onUnderstandingChanged);
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting || entry.target !== feedback.value) continue;
      const action = (entry.target as HTMLElement).dataset.feedbackRef;
      if (action) { observer?.unobserve(entry.target); void recordFeedback(action); }
    }
  });
  void load();
});
onBeforeUnmount(() => { disposed = true; observer?.disconnect(); window.removeEventListener('tutor-state-changed', onUnderstandingChanged); });
</script>

<template>
  <section aria-label="我的理解" class="understanding-space" :aria-busy="busy">
    <header v-show="!active" class="space-heading"><h3>看见理解的变化</h3><p>从你的原话出发，让每一次理解都有迹可循。</p></header>
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="retryAction?.()">重试</button></p>
    <p v-if="feedbackFailure" role="alert">{{ feedbackFailure.message }} <button :disabled="locked || feedbackBusy" @click="recordFeedback(feedbackFailure.action)">重试反馈记录</button></p>
    <p v-if="busy" role="status">正在读取或保存…</p>
    <p v-if="view?.stale" role="status">有新的依据尚未合入此视图。<button :disabled="locked" @click="load(true)">更新理解视图</button></p>
    <div v-show="!active" ref="listing" tabindex="-1" class="listing">
      <p v-if="view && !view.rows.length && !view.interpretations?.length" class="empty">尚无理解记录；相关提问、回应和活动表现会成为后续讲解的依据。</p>
      <section v-if="view?.interpretations?.length" aria-label="当前理解解释" class="concept-list">
        <h4>当前解释</h4>
        <button v-for="item in view.interpretations" :key="item.evidence_id" class="concept" :disabled="locked" @click="openItem({ kind: 'interpretation', id: item.evidence_id }, $event)">
          <span class="concept-icon"><Lightbulb aria-hidden="true" /></span><span class="concept-copy">
          <span class="concept-heading"><strong>{{ item.label }}</strong><span class="nature">{{ item.nature === 'hypothesis' ? '暂定理解' : '活动表现' }}</span></span>
          <span class="preview">{{ item.interpretation }}</span>
          <span v-if="item.correction" class="revision-label">有你的纠正</span>
          </span><ChevronRight class="concept-chevron" aria-hidden="true" />
        </button>
      </section>
      <section v-if="view?.rows.length" aria-label="概念与能力" class="concept-list">
        <h4>概念与能力</h4>
        <button v-for="item in view.rows" :key="rowKey(item)" class="concept" :disabled="locked" @click="openItem({ kind: 'object', row: item }, $event)">
          <span class="concept-icon"><Shapes aria-hidden="true" /></span><span class="concept-copy">
          <span class="concept-heading"><strong>{{ item.label }}</strong><span v-if="item.historical" class="nature">历史版本</span></span>
          <span class="preview">{{ capability[item.capability] ?? (item.capability || '尚未观察能力') }} · {{ item.state === 'unknown' ? '未知' : '已有表现依据' }}</span>
          </span><ChevronRight class="concept-chevron" aria-hidden="true" />
        </button>
      </section>
      <button v-if="view?.next != null" :disabled="locked" @click="load(false, true)">查看更多对象</button>
      <p v-if="view?.rows.length || view?.interpretations?.length" class="list-note"><MessageSquareText aria-hidden="true" />点开一项，查看原话与解释，也可以补充你的理解。</p>
    </div>
    <section v-if="active" ref="reading" tabindex="-1" aria-label="单项理解" class="reading">
      <button class="back" :disabled="locked" @click="back">← 返回概念列表</button>
      <div class="reading-heading"><h4 class="reading-title">{{ row?.label ?? selected?.label }}</h4><span v-if="selected" class="nature">{{ selected.nature === 'hypothesis' ? '暂定理解' : '活动表现' }}<span v-if="historicalEvidence"> · 历史记录</span></span></div>
      <p v-if="row?.historical" class="nature">历史版本 · 对象版本 {{ row.object_revision }}</p>
      <section v-if="row" aria-label="能力依据概况" class="summary">
        <p>{{ capability[row.capability] ?? (row.capability || '尚未观察能力') }}：{{ row.state === 'unknown' ? '未知' : '已有表现依据' }}</p>
        <p v-if="row.evidence_count">独立符合标准 {{ row.independent_support }} 次 · 帮助后符合 {{ row.assisted_support }} 次 · 改答后符合 {{ row.revised_support }} 次</p>
        <p v-if="row.partial || row.difficulty">部分符合 {{ row.partial }} 次 · 尚未符合 {{ row.difficulty }} 次</p>
        <p v-if="row.uncertain">有 {{ row.uncertain }} 条尚不能形成能力判断的解释。</p>
        <p v-if="!row.evidence_count">尚无这项能力的表现依据。</p>
      </section>
      <article v-if="selected" aria-label="表现依据" class="evidence">
        <section class="explanation"><h5>{{ explanationTitle }} · 系统解释</h5><p>{{ selected.interpretation }}</p></section>
        <section v-if="selected.nature === 'hypothesis'" class="context" aria-label="适用目标与帮助条件">
          <h5>适用目标与帮助条件</h5><p>{{ selected.prompt || '此条记录未保留适用目标。' }}</p>
          <p v-if="selected.assistance_count" class="conditions">这次交流前已展示 {{ selected.assistance_count }} 条活动帮助，理解需结合当时的帮助来看。</p>
        </section>
        <section class="quote"><h5>你的原话</h5><blockquote>{{ selected.learner_quote || '此条记录未保留原话。' }}</blockquote></section>
        <p v-if="selected.nature === 'hypothesis'" class="conditions">这是普通交流中的暂定理解，未评分，可随你的纠正和新表现修订。</p>
        <template v-else>
          <p class="conditions">第 {{ selected.attempt }} 次作答；{{ selected.assistance_count ? '帮助后作答，回答前已展示帮助' : '未记录到事先展示的帮助' }}<span v-if="selected.attempt > 1">；改答后记录</span>。</p>
          <p v-if="status[selected.status]">{{ status[selected.status] }}</p>
        </template>
        <p v-if="selected.correction" class="correction">你的纠正：{{ selected.correction }}</p>
        <section v-if="selected.teaching_implication" class="implication"><CornerDownRight aria-hidden="true" /><div><h5>接下来怎样帮助你</h5><p>{{ selected.teaching_implication }}</p></div></section>
        <details :key="selected.evidence_id" :open="detailsOpen" class="facts" @toggle="detailsOpen = ($event.target as HTMLDetailsElement).open">
          <summary>查看详细依据</summary>
          <div v-if="detailsOpen">
            <template v-if="selected.nature !== 'hypothesis'"><h5>当时的活动</h5><p>{{ selected.prompt || '此条记录未保留活动说明。' }}</p></template>
            <p v-if="selected.feedback_hidden">详细判定依据会在本活动请求直接讲解后揭示。</p>
            <ul v-else-if="selected.source_quotes?.length" ref="feedback" :data-feedback-ref="selected.assessment_ref ?? undefined">
              <li v-for="(item, index) in selected.source_quotes" :key="index"><p v-if="item.reason">{{ item.reason }}</p><blockquote v-if="item.response_quote">作答片段：{{ item.response_quote }}</blockquote><blockquote v-if="item.quote">{{ item.quote }}</blockquote><blockquote v-for="(source, i) in item.sources" :key="i">{{ source.quote }}</blockquote></li>
            </ul>
            <p v-else>此条记录未附详细引用。</p>
          </div>
        </details>
        <details class="correction-editor">
          <summary><PencilLine aria-hidden="true" />纠正这条解释</summary>
          <form class="correction-form" @submit.prevent="correct">
          <label>纠正这条解释<textarea v-model="correction" :disabled="!!pending" rows="3" placeholder="写下你的实际理解或需要更正的地方" /></label>
          <button class="save" :disabled="locked || !correction.trim()">保存纠正</button>
          </form>
        </details>
      </article>
      <details v-if="row?.evidence_count" class="history">
        <summary>依据与解释历史</summary>
        <div class="evidence-links"><button v-for="(id, index) in row.evidence_refs" :key="id" :disabled="locked" :aria-pressed="selected?.evidence_id === id" @click="inspect(id, index > 0)">查看依据 {{ index + 1 }}</button></div>
        <button :disabled="locked" @click="older()">查看完整解释历史</button>
        <section v-if="history" aria-label="解释历史"><h5>{{ row.label }}的解释历史</h5><div class="evidence-links"><button v-for="(id, index) in history.refs" :key="id" :disabled="locked" :aria-pressed="selected?.evidence_id === id" @click="inspect(id, true)">查看历史依据 {{ index + 1 }}</button></div><button v-if="history.next != null" :disabled="locked" @click="older(true)">更早的解释</button></section>
      </details>
    </section>
  </section>
</template>

<style scoped>
.understanding-space { min-width: 0; overflow-wrap: anywhere; color: var(--reader-ink, var(--ink)); font: 14px/1.7 var(--sans); }
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
.list-note { display: flex; align-items: flex-start; gap: 8px; padding-top: 18px; border-top: 1px solid var(--line); color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; }.list-note svg { width: 16px; height: 16px; margin-top: 2px; }
.reading, .listing { min-width: 0; outline: none; }.back { border: 0; background: transparent; padding-left: 0; color: var(--tutor-muted, var(--reader-muted)); font-size: 12px; min-height: 32px; }
.reading-heading { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 12px; margin: 17px 0 24px; }.reading-title { font-size: 25px; line-height: 1.45; font-weight: 650; }
.summary { padding: 0 0 20px; font-size: 13px; }.summary p + p { margin-top: 8px; font-size: 12px; }
.evidence { padding: 0; }.evidence > * + * { margin-top: 22px; }
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
[role='alert'], [role='status'], .empty { margin: 12px 0; font-size: 13px; }[role='alert'] { color: var(--brand-error); }.empty { padding: 28px 0; color: var(--tutor-muted, var(--reader-muted)); }
.space-heading h3 { font-family: var(--font-display); font-weight: 400; font-synthesis: none; }
@media (max-width: 480px) { .space-heading h3 { font-size: 22px; }.reading-title { font-size: 23px; }.quote { padding: 16px; }.concept { gap: 10px; }.concept-icon { width: 32px; height: 36px; }textarea { font-size: 16px; } }
</style>
