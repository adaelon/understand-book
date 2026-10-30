<script setup lang="ts">
import { applicationOverlayTarget } from "../application-overlay";
import { t as uiText } from '../i18n';

import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { learningRecords } from '../network-client';
import { recapStatus, recapTime } from '../session-recap';
import type { LearningRecordDetail } from '../learning-records';
import SourceExcerpt from './SourceExcerpt.vue';
const props = defineProps<{ sessionId: string; throughSeq: number }>();
const emit = defineEmits<{ (e: 'close'): void }>();
const record = ref<LearningRecordDetail | null>(null), loading = ref(false), error = ref('');
const closeButton = ref<HTMLButtonElement>();
const previousFocus = document.activeElement as HTMLElement | null;
let request = 0;
async function load() {
  const current = ++request; loading.value = true; error.value = '';
  try { const value = await learningRecords.read(props.sessionId, props.throughSeq); if (current === request) record.value = value; }
  catch (failure) { if (current === request) error.value = failure instanceof Error ? failure.message : String(failure); }
  finally { if (current === request) loading.value = false; }
}
watch(() => [props.sessionId, props.throughSeq], () => { record.value = null; void load(); }, { immediate: true });
void nextTick(() => closeButton.value?.focus());
onBeforeUnmount(() => { request++; if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true }); });
function trapFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = Array.from((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled), a, summary'));
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
</script>

<template>
  <Teleport :to="applicationOverlayTarget()">
    <div class="record-backdrop" @click.self="emit('close')">
      <section class="record-preview" role="dialog" aria-modal="true" aria-labelledby="learning-record-title" @keydown.esc.stop.prevent="emit('close')" @keydown="trapFocus">
        <header><div><p>{{ uiText('原记录 · 只读查看') }}</p><h2 id="learning-record-title">{{ record?.title || uiText('聊天与学习记录') }}</h2></div><button ref="closeButton" @click="emit('close')">{{ uiText('关闭原记录') }}</button></header>
        <p class="record-note">{{ uiText('这里保留原聊天、学习解释与引用。关闭后可在当前工作区继续学习。') }}</p>
        <p v-if="loading" role="status">{{ uiText('正在读取原记录…') }}</p>
        <p v-if="error" role="alert">{{ error }} <button :disabled="loading" @click="load">{{ uiText('重试读取') }}</button></p>
        <template v-if="record">
          <p class="record-time">{{ uiText('截至') }} {{ recapTime(record.recap.through_at) }}</p>
          <section :aria-label="uiText('原聊天')" class="record-section">
            <h3>{{ uiText('原聊天') }}</h3>
            <article v-for="turn in record.turns" :key="turn.turn_id" class="record-turn">
              <section v-if="turn.workspace_goal" class="record-goal" :aria-label="uiText('当时的大目标')"><h4>{{ uiText('当时的大目标 · 第') }} {{ turn.workspace_goal.revision }} {{ uiText('版') }}</h4><p>{{ turn.workspace_goal.text }}</p><p v-if="turn.workspace_goal.constraints.length" class="record-note">{{ turn.workspace_goal.constraints.join('；') }}</p></section>
              <h4>{{ uiText('你的问题') }}</h4><p>{{ turn.user }}</p>
              <blockquote v-if="turn.question_quote?.quote">{{ turn.question_quote.quote }}</blockquote>
              <section v-if="turn.accepted_workspace_goal" class="record-goal" :aria-label="uiText('本回合确认的新目标')"><h4>{{ uiText('本回合确认的新目标 · 第') }} {{ turn.accepted_workspace_goal.revision }} {{ uiText('版') }}</h4><p>{{ turn.accepted_workspace_goal.text }}</p><p v-if="turn.accepted_workspace_goal.constraints.length" class="record-note">{{ turn.accepted_workspace_goal.constraints.join('；') }}</p></section>
              <template v-if="turn.outcome?.answer"><h4>{{ uiText('当时的回答') }}</h4><p>{{ turn.outcome.answer }}</p></template>
              <p v-else class="record-note">{{ recapStatus(turn.status) }}</p>
            </article>
            <p v-if="!record.turns.length">{{ uiText('这份记录尚无聊天回合。') }}</p>
          </section>
          <section :aria-label="uiText('原引用来源')" class="record-section">
            <h3>{{ uiText('原引用来源') }}</h3>
            <article v-for="source in record.sources" :key="`${source.turn_id}:${source.source_ref_id}`" class="record-source">
              <h4>{{ source.label }}</h4>
              <SourceExcerpt v-if="source.view" :source="source.view" />
              <blockquote v-else>{{ source.quote }}</blockquote>
              <p v-if="source.unavailable_reason" class="record-note">{{ source.unavailable_reason }}</p>
            </article>
            <p v-if="!record.sources.length">{{ uiText('这份记录未保留原文引用。') }}</p>
          </section>
          <section :aria-label="uiText('原学习记录')" class="record-section">
            <h3>{{ uiText('原学习记录') }}</h3>
            <p class="record-note">{{ uiText('这些解释保留当时的目标与帮助条件；带入后作为历史参考。') }}</p>
            <article v-for="evidence in record.learning.evidence" :key="evidence.evidence_id" class="record-evidence">
              <h4>{{ evidence.label }}</h4><p>{{ evidence.interpretation }}</p>
              <blockquote v-if="evidence.learner_quote">{{ evidence.learner_quote }}</blockquote>
              <p v-if="evidence.correction">{{ uiText('你的纠正：') }}{{ evidence.correction }}</p>
              <p v-if="evidence.teaching_implication">{{ uiText('当时的后续建议：') }}{{ evidence.teaching_implication }}</p>
              <p v-if="evidence.supersedes" class="record-note">{{ uiText('这是对较早解释的修订；原解释一并保留。') }}</p>
            </article>
            <p v-if="!record.learning.evidence.length">{{ uiText('这份记录尚无已保存的学习解释。') }}</p>
            <p v-if="record.learning.facts.length" class="record-note">{{ uiText('保留了') }} {{ record.learning.facts.length }} {{ uiText('条原始学习事实及其关联。') }}</p>
          </section>
        </template>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.record-backdrop { position: fixed; inset: 0; z-index: 1000; padding: 24px; background: #1a211d66; display: flex; justify-content: center; align-items: center; }
.record-preview { width: min(840px, 100%); max-height: 92dvh; overflow: auto; padding: 28px; border-radius: 18px; background: var(--canvas, #fffdf8); color: var(--ink, #262b27); font: 14px/1.75 var(--sans, sans-serif); overflow-wrap: anywhere; }
header { display: flex; align-items: start; justify-content: space-between; gap: 16px; } h2, h3, h4, p { margin: 0; } h2 { font-size: 23px; } h3 { font-size: 18px; margin-bottom: 12px; } h4 { font-size: 13px; margin: 12px 0 6px; } header > div > p, .record-note, .record-time { color: var(--muted, #68716a); font-size: 12px; }
button { min-height: 40px; padding: 8px 12px; border: 1px solid var(--line, #ddd); border-radius: 8px; background: transparent; color: inherit; font: inherit; cursor: pointer; flex-shrink: 0; }
.record-note, .record-time { margin-top: 12px; } .record-section { margin-top: 28px; } .record-turn, .record-source, .record-evidence { border-top: 1px solid var(--line, #ddd); padding: 6px 0 18px; } .record-turn > p, .record-evidence > p { white-space: pre-wrap; } blockquote { margin: 12px 0; padding: 8px 14px; border-left: 3px solid var(--line, #ddd); white-space: pre-wrap; background: var(--surface-soft, #f6f4ef); }
.record-goal { margin: 12px 0; padding: 10px 14px; border-radius: 8px; background: var(--surface-soft, #f6f4ef); }.record-goal h4 { margin-top: 0; }.record-goal p { white-space: pre-wrap; }
@media(max-width: 600px) { .record-backdrop { padding: 8px; } .record-preview { padding: 18px; max-height: 96dvh; } h2 { font-size: 20px; } button { min-height: 44px; } }
</style>
