<script setup lang="ts">
import { t as uiText } from '@understand-book/web/reader/i18n';

import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { learningRecords } from '../network-client';
import { ApiError } from '@understand-book/web/reader/api';
import type { LearningWorkspace } from '@understand-book/web/reader/reader-context';
import type { ImportedLearningRecord, LearningRecordSummary } from '@understand-book/web/reader/learning-records';
import LearningRecordPreview from '@understand-book/web/reader/components/LearningRecordPreview.vue';
const props = defineProps<{ target?: LearningWorkspace | null }>();
const emit = defineEmits<{ (e: 'refresh-goal'): void }>();
const records = ref<LearningRecordSummary[]>([]), linked = ref<ImportedLearningRecord[]>([]);
const selected = ref<string[]>([]), loading = ref(false), saving = ref(false), error = ref(''), notice = ref('');
const preview = ref<{ sessionId: string; throughSeq: number } | null>(null);
const candidates = computed(() => records.value.filter(item => !props.target || item.source_learning_workspace_id !== props.target.learning_workspace_id));
function existing(record: LearningRecordSummary) { return linked.value.find(item => item.selection.chat_session_id === record.session_id && item.through_seq >= record.through_seq); }
function hasEarlierImport(record: LearningRecordSummary) { return linked.value.some(item => item.selection.chat_session_id === record.session_id) && !existing(record); }
function sourceName(record: LearningRecordSummary) { return record.source_goal || (record.source_learning_workspace_id ? uiText('原工作区') : uiText('旧单书记录')); }
let request = 0;
async function load() {
  const current = ++request; loading.value = true; error.value = '';
  try {
    const [available, imported] = await Promise.all([learningRecords.list(), props.target ? learningRecords.linked(props.target.learning_workspace_id) : Promise.resolve({ records: [] })]);
    if (current === request) { records.value = available.records; linked.value = imported.records; }
  } catch (failure) { if (current === request) error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { if (current === request) loading.value = false; }
}
watch(() => props.target?.learning_workspace_id, () => { records.value = []; linked.value = []; selected.value = []; notice.value = ''; preview.value = null; void load(); }, { immediate: true });
onBeforeUnmount(() => { request++; });
async function linkSelected() {
  const target = props.target;
  if (!target || saving.value || !selected.value.length) return;
  const current = request, chosen = candidates.value.filter(item => selected.value.includes(item.session_id) && !existing(item));
  saving.value = true; error.value = ''; notice.value = '';
  let completed = 0;
  try {
    for (const item of chosen) {
      await learningRecords.link(target.learning_workspace_id, item.session_id, item.through_seq, target.goal.revision);
      completed++;
      if (current !== request) return;
      selected.value = selected.value.filter(id => id !== item.session_id);
    }
    if (current === request) {
      linked.value = (await learningRecords.linked(target.learning_workspace_id)).records;
      notice.value = uiText('已带入 {0} 份学习记录，可在这个工作区继续提问。', [completed]);
      window.dispatchEvent(new Event('tutor-state-changed'));
    }
  } catch (failure) {
    if (current === request) {
      error.value = `${completed ? uiText('已带入 {0} 份；其余记录尚未完成。', [completed]) : ''}${failure instanceof Error ? failure.message : String(failure)}`;
      if (failure instanceof ApiError && failure.status === 409) emit('refresh-goal');
      try { linked.value = (await learningRecords.linked(target.learning_workspace_id)).records; } catch { /* Keep the write error and selected items for the user's retry. */ }
    }
  } finally { if (current === request) saving.value = false; }
}
</script>

<template>
  <section class="learning-records-panel" :aria-label="target ? uiText('带入已有学习记录') : uiText('已有聊天与学习记录')" :aria-busy="loading || saving">
    <h3>{{ target ? uiText('带入已有学习记录') : uiText('已有聊天与学习记录') }}</h3>
    <template v-if="target"><p class="record-target">{{ uiText('带入到：') }}<strong>{{ target.goal.text }}</strong> {{ uiText('· 第') }} {{ target.goal.revision }} {{ uiText('版') }}</p><p>{{ uiText('选择与这个目标有关的记录。原聊天、解释和来源保持原样，后续学习保存在这个工作区。') }}</p></template>
    <p v-else>{{ uiText('旧聊天、学习解释和来源保留在这里。查看原记录后，可在工作区中选择带入。') }}</p>
    <p v-if="loading" role="status">{{ uiText('正在读取已有记录…') }}</p>
    <p v-if="error" role="alert">{{ error }} <button :disabled="saving || loading" @click="load">{{ uiText('刷新记录') }}</button></p>
    <p v-if="notice" role="status">{{ notice }}</p>
    <div class="record-list">
      <article v-for="record in candidates" :key="record.session_id" class="record-choice">
        <label v-if="target" class="record-check"><input v-model="selected" type="checkbox" :value="record.session_id" :disabled="saving || loading || !!existing(record) || target.archived" :aria-label="uiText('带入 {0}', [record.title || uiText('未命名聊天')])" /><span class="sr-only">{{ uiText('选择记录') }}</span></label>
        <div class="record-copy"><strong>{{ record.title || uiText('未命名聊天') }}</strong><p>{{ sourceName(record) }} · {{ record.turn_count }} {{ uiText('个回合') }}</p><small>{{ record.published_book_refs.length }} {{ uiText('份来源资料') }}</small><span v-if="existing(record)" class="record-imported">{{ uiText('已带入') }}</span><span v-else-if="hasEarlierImport(record)" class="record-imported">{{ uiText('有新增记录，可再次选择带入') }}</span></div>
        <button :disabled="saving" @click="preview = { sessionId: record.session_id, throughSeq: record.through_seq }">{{ uiText('查看原记录') }}</button>
      </article>
    </div>
    <p v-if="!loading && !error && !candidates.length">{{ target ? uiText('暂无可带入的其他聊天与学习记录。') : uiText('还没有已保存的聊天与学习记录。') }}</p>
    <div v-if="target" class="record-actions"><button :disabled="saving || loading || !selected.length || target.archived" @click="linkSelected">{{ saving ? uiText('正在带入…') : uiText('确认带入 {0} 份记录', [selected.length]) }}</button><span>{{ uiText('重复带入不会添加重复记录。') }}</span></div>
    <LearningRecordPreview v-if="preview" :session-id="preview.sessionId" :through-seq="preview.throughSeq" @close="preview = null" />
  </section>
</template>

<style scoped>
.learning-records-panel { margin: 20px 0; border: 1px solid var(--line); border-radius: 12px; padding: 20px; color: var(--ink); font-size: 13px; line-height: 1.65; overflow-wrap: anywhere; }
h3, p { margin: 0 0 10px; } h3 { font-size: 18px; }.record-target { margin-top: 12px; }.record-list { margin-top: 18px; }.record-choice { display: flex; align-items: center; gap: 12px; padding: 16px 0; border-top: 1px solid var(--line); }.record-copy { min-width: 0; flex: 1; }.record-copy p { margin: 4px 0 0; color: var(--muted); }.record-copy small { display: block; color: var(--muted); }.record-imported { color: var(--brand-green-deep, #356044); display: block; margin-top: 4px; }.record-check { display: flex; padding: 8px; }.record-check input { width: 18px; height: 18px; accent-color: var(--brand-green-deep, #356044); }
button { border: 1px solid var(--line); border-radius: 8px; padding: 8px 12px; min-height: 40px; background: var(--canvas); color: inherit; font: inherit; cursor: pointer; } button:disabled { opacity: .5; cursor: default; }.record-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; margin-top: 20px; }.record-actions span { color: var(--muted); font-size: 12px; }.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); border: 0; }
@media(max-width: 600px) { .learning-records-panel { padding: 14px; }.record-choice { flex-wrap: wrap; gap: 8px; }.record-copy { flex-basis: calc(100% - 48px); }.record-choice > button { margin-left: 42px; } button { min-height: 44px; } }
</style>
