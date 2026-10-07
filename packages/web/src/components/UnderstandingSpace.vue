<script setup lang="ts">
import { onMounted, onBeforeUnmount, nextTick, ref } from 'vue';
import { api as sharedApi, ApiError, type UnderstandingView, type UnderstandingRow, type LearningEvidenceView } from '../api';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
const view = ref<UnderstandingView | null>(null);
const selected = ref<LearningEvidenceView | null>(null);
const error = ref(''); const busy = ref(false); const correction = ref('');
const history = ref<{ row:UnderstandingRow; refs:string[]; next:number|null } | null>(null);
let pending: Parameters<typeof api.tutorCorrect>[0] | null = null;
const feedback = ref<HTMLElement>();
let observer: IntersectionObserver | undefined;
let failedFeedback: string | null = null;
const capability: Record<string,string> = { recognition:'识别',recall:'回忆',explanation:'解释',relation:'关系理解',application:'应用',evaluation:'来源辨析',production:'构造',transfer:'迁移' };
async function load(rebuild = false, more = false) {
  busy.value = true; error.value = '';
  try {
    const result = await api.tutorUnderstanding(more ? view.value?.next ?? 0 : 0, rebuild);
    view.value = more && view.value ? { ...result, rows:[...view.value.rows,...result.rows] } : result;
  } catch (e) { error.value = `理解记录暂不可用：${e instanceof Error ? e.message : String(e)}`; }
  finally { busy.value = false; }
}
async function inspect(id: string) {
  busy.value = true; error.value = '';
  try { selected.value = await api.tutorEvidence(id); correction.value = ''; await nextTick(); observer?.disconnect(); if (feedback.value) observer?.observe(feedback.value); }
  catch (e) { error.value = `依据读取失败：${e instanceof Error ? e.message : String(e)}`; }
  finally { busy.value = false; }
}
async function older(row: UnderstandingRow, more = false) {
  busy.value = true; error.value = '';
  try {
    const result = await api.tutorEvidenceList(row, more ? history.value?.next ?? undefined : undefined);
    history.value = { row, refs:more ? [...history.value!.refs,...result.refs] : result.refs, next:result.next };
  } catch (e) { error.value = `历史依据读取失败：${e instanceof Error ? e.message : String(e)}`; }
  finally { busy.value = false; }
}
async function correct() {
  if (!pending && (!selected.value || !correction.value.trim())) return;
  pending ??= { evidence_ref:selected.value!.evidence_id, operation_id:crypto.randomUUID(), text:correction.value.trim() };
  busy.value = true; error.value = '';
  try {
    const result = await api.tutorCorrect(pending);
    pending = null;
    await inspect(result.evidence_ref);
    await load();
    window.dispatchEvent(new Event('tutor-state-changed'));
  } catch (e) {
    if (e instanceof ApiError && e.status < 500) pending = null;
    error.value = `纠正尚未确认保存：${e instanceof Error ? e.message : String(e)}`;
  } finally { busy.value = false; }
}
async function retry() {
  if (pending) return correct();
  if (!failedFeedback) return load();
  try { await api.tutorFeedbackDisplayed(failedFeedback); failedFeedback = null; error.value = ''; }
  catch (e) { error.value = `反馈展示记录未保存：${String(e)}`; }
}
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    const entry = entries.find(e => e.isIntersecting);
    const action = (entry?.target as HTMLElement | undefined)?.dataset.feedbackRef;
    if (action) void api.tutorFeedbackDisplayed(action).then(() => { if (entry) observer?.unobserve(entry.target); }).catch(e => { failedFeedback = action; error.value = `反馈展示记录未保存：${String(e)}`; });
  });
  void load();
});
onBeforeUnmount(() => observer?.disconnect());
</script>
<template>
  <section aria-label="我的理解" class="understanding-space">
    <h3>我的理解</h3>
    <p>这里区分实际表现、你的原话和系统解释；没有依据的能力保持未知。</p>
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="retry">重试</button></p>
    <p v-if="view?.stale" role="status">有新的依据尚未合入此视图。<button :disabled="busy" @click="load(true)">更新理解视图</button></p>
    <p v-if="view && !view.rows.length">尚无可展示的学习对象；材料就绪并产生表现后会在这里显示。</p>
    <article v-for="row in view?.rows" :key="`${row.object_id}:${row.object_revision}:${row.capability}:${row.historical}`">
      <h4>{{ row.label }} <small v-if="row.historical">历史版本</small></h4>
      <p>{{ capability[row.capability] ?? (row.capability || '尚未观察能力') }}：{{ row.state === 'unknown' ? '未知' : '已有表现依据' }}</p>
      <p v-if="row.evidence_count">独立符合标准 {{ row.independent_support }} 次 · 帮助后符合 {{ row.assisted_support }} 次 · 改答后符合 {{ row.revised_support }} 次</p>
      <p v-if="row.partial || row.difficulty">部分符合 {{ row.partial }} 次 · 尚未符合 {{ row.difficulty }} 次</p>
      <p v-if="row.uncertain">有 {{ row.uncertain }} 条尚不能形成能力判断的解释。</p>
      <button v-for="(id,index) in row.evidence_refs" :key="id" :disabled="busy || !!pending" @click="inspect(id)">查看依据 {{ index + 1 }}</button>
      <button v-if="row.evidence_count" :disabled="busy || !!pending" @click="older(row)">查看完整解释历史</button>
    </article>
    <button v-if="view?.next != null" :disabled="busy" @click="load(false,true)">查看更多对象</button>
    <section v-if="history" aria-label="解释历史"><h4>{{ history.row.label }}的解释历史</h4><button v-for="(id,index) in history.refs" :key="id" :disabled="busy || !!pending" @click="inspect(id)">查看历史依据 {{ index + 1 }}</button><button v-if="history.next != null" :disabled="busy" @click="older(history.row,true)">更早的解释</button></section>
    <section v-if="selected" aria-label="表现依据">
      <h4>{{ selected.label }}的表现依据</h4>
      <p>当时的活动：{{ selected.prompt }}</p>
      <p>你的原话：</p><blockquote>{{ selected.learner_quote }}</blockquote>
      <p>系统解释：{{ selected.interpretation }}</p>
      <p>第 {{ selected.attempt }} 次作答；{{ selected.assistance_count ? '回答前已展示帮助' : '未记录到事先展示的帮助' }}。</p>
      <p v-if="selected.correction">你的纠正：{{ selected.correction }}</p>
      <p v-if="selected.feedback_hidden">详细判定依据会在本活动请求直接讲解后揭示。</p>
      <ul v-else-if="selected.source_quotes?.length" ref="feedback" :data-feedback-ref="selected.assessment_ref ?? undefined"><li v-for="(item,index) in selected.source_quotes" :key="index">{{ item.reason }} <q v-if="item.quote">{{ item.quote }}</q><q v-for="(source,i) in item.sources" :key="i">{{ source.quote }}</q></li></ul>
      <label>纠正这条解释<textarea v-model="correction" :disabled="!!pending" rows="2" /></label>
      <button :disabled="busy || (!pending && !correction.trim())" @click="correct">保存纠正</button>
    </section>
  </section>
</template>
<style scoped>
.understanding-space { border-top:1px solid var(--line); margin-top:20px; }
article, section[aria-label='表现依据'] { border-top:1px solid var(--line); padding:12px 0; }
button { font:inherit; min-height:44px; margin:4px; }
textarea { display:block; width:100%; box-sizing:border-box; font:inherit; }
blockquote { margin:8px 0; padding-left:12px; border-left:3px solid var(--line); white-space:pre-wrap; }
q { display:block; margin:8px; }
</style>
