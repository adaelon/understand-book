<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { MessageCircle } from '@lucide/vue';
import { api as sharedApi, ApiError, type TutorActivity } from '../api';
import { answerDisplayParts, sourceMarkedParts } from '../answer-markdown';
import type { AgentAnswerSource } from '../generated/AgentAnswerSource';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
import type { PresentationRef } from '../generated/PresentationRef';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const props = withDefaults(defineProps<{ sessionId: string; turnId: string; reference?: PresentationRef;
  snapshot?: () => Promise<PresentationFollowUp>; ready?: boolean; sources?: AgentAnswerSource[];
  answerText?: string; answerTarget?: () => HTMLElement | undefined }>(), { ready: true, sources: () => [] });
const emit = defineEmits<{ (e: 'source', id: string, anchor: HTMLElement): void; (e: 'response-available', available: boolean): void }>();
const root = ref<HTMLElement>();
const activities = ref<TutorActivity[]>([]);
// A delivered follow-up may repeat the answer verbatim. Observe that answer instead of rendering it twice.
function usesAnswer(activity: TutorActivity) {
  const withoutSources = activity.move.prompt.replace(/\[\[source:([^\]]*)\]\]/g, (marker, id: string) =>
    props.sources.some(source => source.source_ref_id === id.trim()) ? '' : marker).replace(/[ \t]+$/gm, '').trim();
  return !!props.answerTarget && !!props.answerText?.trim() && activity.move.actions.length === 0
    && withoutSources === props.answerText.replace(/[ \t]+$/gm, '').trim();
}
const visibleActivities = computed(() => activities.value.filter(activity => !usesAnswer(activity)));
const responseActivity = computed(() => visibleActivities.value.find(activity => activity.status === 'active'
  && activity.move.actions.some(action => action === 'self_report' || action === (activity.attempted ? 'revise' : 'submit'))));
watch(responseActivity, activity => emit('response-available', !!activity), { immediate: true });
function focusResponse() {
  const activity = responseActivity.value;
  if (!activity) return;
  const article = Array.from(root.value?.querySelectorAll<HTMLElement>('[data-delivery-ref]') ?? [])
    .find(node => node.dataset.deliveryRef === activity.delivery_ref);
  const input = article?.querySelector('textarea');
  input?.scrollIntoView({ block: 'center' });
  input?.focus({ preventScroll: true });
}
const answerDeliveries = new Map<Element, string[]>();
function renderText(text: string) {
  return answerDisplayParts(sourceMarkedParts(text, props.sources), props.sources)
    .map(part => part.kind === 'html' ? part.html : '').join('');
}
function openSource(text: string, event: MouseEvent) {
  const button = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('button[data-answer-source-index]') : null;
  if (!button || !(event.currentTarget as Element).contains(button)) return;
  const part = sourceMarkedParts(text, props.sources)[Number(button.dataset.answerSourceIndex)];
  if (part?.kind !== 'sources') return;
  event.stopPropagation();
  emit('source', part.source_ref_ids[0], button);
}
const responses = ref<Record<string, string>>({});
const helps = ref<Record<string, { event_id: string; text: string }>>({});
const error = ref(''); const busy = ref(false);
const visible = new Set<string>();
const visibleHelp = new Set<string>();
const feedbackSeen = new Set<string>();
const failedFeedback = new Set<string>();
let observer: IntersectionObserver | undefined;
let generation = 0;
let marking: Promise<void> | undefined;
const displayed = new Set<string>();
let pending: Parameters<typeof api.tutorAction>[0] | null = null;
const labels: Record<string, string> = { submit: '提交回答', revise: '改答', request_hint: '提示', reveal: '直接讲解', skip: '跳过', self_report: '记录自评' };
const assessmentLabels: Record<string,string> = { correct: '本次回答符合标准', partial: '本次回答部分符合标准', incorrect: '本次回答未符合标准', uncertain: '目前依据不足，暂不能判断', unassessed: '回答已记录，尚未评分。' };
async function load() {
  const current = ++generation;
  try {
    const result = await api.tutorActivities(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    activities.value = result.activities;
    await observeVisibleContent();
    await markDisplayed();
  } catch (failure) { error.value = `教学活动读取失败：${failure instanceof Error ? failure.message : String(failure)}`; }
}
async function markDisplayed() {
  if (props.ready === false || !activities.value.length) return;
  if (marking) return marking;
  const waiting = activities.value.filter(a => visible.has(a.delivery_ref) && !displayed.has(a.delivery_ref));
  if (!waiting.length) return;
  marking = (async () => { try {
    const scene = await props.snapshot?.();
    for (const activity of waiting) {
      await api.tutorDisplay(activity.delivery_ref, scene);
      displayed.add(activity.delivery_ref);
    }
  } catch (failure) { error.value = `教学展示记录未保存：${failure instanceof Error ? failure.message : String(failure)}`; }
  })();
  try { await marking; } finally { marking = undefined; }
}
function focusActivity(moveId: string, response?: string) {
  const activity = activities.value.find(a => a.move.move_id === moveId);
  if (!activity) return;
  if (typeof response === 'string') responses.value[activity.delivery_ref] = response;
  root.value?.scrollIntoView({ block: 'nearest' });
}
defineExpose({ focusActivity, focusResponse });
function continueLearning(eventId: string, text: string) {
  root.value?.dispatchEvent(new CustomEvent('tutor-response', { bubbles: true, detail: { eventId, text } }));
}
async function acknowledgeHelp(id: string) {
  const help = helps.value[id];
  if (!help || !visibleHelp.has(id)) return;
  await nextTick();
  await api.tutorHelpDisplayed(help.event_id);
}
async function act(activity: TutorActivity, action: string) {
  if (busy.value || pending) return;
  busy.value = true; error.value = '';
  try {
    const scene = await props.snapshot?.();
    pending = { operation_id: crypto.randomUUID(), delivery_ref: activity.delivery_ref, action,
      response: ['submit', 'revise', 'self_report'].includes(action) ? responses.value[activity.delivery_ref] ?? '' : null, scene };
    await send();
  } catch (failure) { if (failure instanceof ApiError && failure.status < 500) pending = null; error.value = `操作未确认：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { busy.value = false; }
}
async function send() {
  if (!pending) return;
  const request = pending;
  const result = await api.tutorAction(request);
  pending = null;
  if (result.help) {
    helps.value[request.delivery_ref] = result.help;
    await observeVisibleContent();
    await acknowledgeHelp(request.delivery_ref);
  } else {
    continueLearning(result.event_id, request.response || '我跳过了这个活动，请调整接下来的讲法。');
  }
  await load();
}
async function retry() {
  busy.value = true; error.value = '';
  try {
    for (const id of failedFeedback) { await api.tutorFeedbackDisplayed(id); failedFeedback.delete(id); feedbackSeen.add(id); }
    if (pending) await send();
    else { for (const id of Object.keys(helps.value)) await acknowledgeHelp(id); await load(); }
  } catch (failure) { if (failure instanceof ApiError && failure.status < 500) pending = null; error.value = `操作未确认：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { busy.value = false; }
}
watch(() => [props.sessionId, props.turnId, props.reference?.presentation_id, props.reference?.revision], () => {
  visible.clear(); visibleHelp.clear(); displayed.clear(); answerDeliveries.clear(); observer?.disconnect();
  pending = null; activities.value = []; helps.value = {}; responses.value = {}; void load();
}, { immediate: true });
watch(() => props.ready, () => { void markDisplayed(); });
async function observeVisibleContent() {
  await nextTick();
  answerDeliveries.clear();
  const answer = props.answerTarget?.();
  const ids = activities.value.filter(usesAnswer).map(activity => activity.delivery_ref);
  if (answer && ids.length) { answerDeliveries.set(answer, ids); observer?.observe(answer); }
  root.value?.querySelectorAll<HTMLElement>('[data-delivery-ref], [data-help-for], [data-feedback-ref]').forEach(node => observer?.observe(node));
}
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const node = entry.target as HTMLElement;
      const deliveries = answerDeliveries.get(node);
      if (deliveries) {
        for (const id of deliveries) { if (entry.isIntersecting) visible.add(id); else visible.delete(id); }
        continue;
      }
      const feedback = node.dataset.feedbackRef;
      if (feedback && entry.isIntersecting && !feedbackSeen.has(feedback)) {
        feedbackSeen.add(feedback);
        void api.tutorFeedbackDisplayed(feedback).catch(failure => { feedbackSeen.delete(feedback); failedFeedback.add(feedback); error.value = `反馈展示记录未保存：${String(failure)}`; });
      }
      const helpFor = node.dataset.helpFor;
      const id = helpFor ?? node.dataset.deliveryRef;
      if (!id) continue;
      const targets = helpFor ? visibleHelp : visible;
      if (entry.isIntersecting) targets.add(id); else targets.delete(id);
      if (helpFor && entry.isIntersecting) void acknowledgeHelp(id).catch(failure => { error.value = `帮助展示记录未保存：${String(failure)}`; });
    }
    void markDisplayed();
  });
  void observeVisibleContent();
  window.addEventListener('tutor-state-changed', load);
});
onBeforeUnmount(() => { generation++; observer?.disconnect(); window.removeEventListener('tutor-state-changed', load); });
</script>
<template>
  <section ref="root" class="tutor-activities" aria-label="教学活动">
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="retry">重试</button></p>
    <article v-for="activity in visibleActivities" :key="activity.delivery_ref" :data-delivery-ref="activity.delivery_ref" :class="{ interactive: activity.move.actions.length }">
      <header v-if="activity.move.actions.length" class="activity-heading"><MessageCircle :size="17" aria-hidden="true" /><span>{{ activity.attempted ? '继续完善你的回答' : '轮到你了' }}</span><small>用自己的话试一试</small></header>
      <div class="activity-text md" v-html="renderText(activity.move.prompt)" @click="openSource(activity.move.prompt, $event)"></div>
      <p v-if="activity.status !== 'active'" role="status">{{ activity.status === 'paused' ? '学习已暂停，可查看已交付内容和帮助。' : '原教学来源不可用，暂不能提交。' }}</p>
      <textarea v-if="activity.move.actions.some(a => ['submit', 'revise', 'self_report'].includes(a))" v-model="responses[activity.delivery_ref]" aria-label="活动回答" placeholder="写下你的理解，或还不确定的地方…" rows="3" />
      <div v-if="activity.move.actions.length" class="activity-actions">
      <template v-for="action in activity.move.actions" :key="action">
        <button v-if="!(action === 'submit' && activity.attempted) && !(action === 'revise' && !activity.attempted)"
          :class="{ primary: ['submit', 'revise', 'self_report'].includes(action) }"
          :data-help-action="['request_hint', 'reveal'].includes(action) ? action : undefined" :disabled="busy || !!pending || (activity.status !== 'active' && ['submit', 'revise', 'skip'].includes(action))" @click="act(activity, action)">{{ labels[action] }}</button>
      </template>
      </div>
      <div v-if="helps[activity.delivery_ref]" class="help activity-text md" :data-help-for="activity.delivery_ref" role="status" v-html="renderText(helps[activity.delivery_ref].text)" @click="openSource(helps[activity.delivery_ref].text, $event)"></div>
      <small v-if="activity.move.actions.length && !activity.move.actions.some(a => ['request_hint', 'reveal'].includes(a))">当前题目未提供提示或直接讲解，可保留题目并返回原文阅读。</small>
      <small v-if="activity.help_seen.length">此活动已展示过帮助；重开页面会保留这项记录。</small>
      <small v-if="activity.attempted" role="status">{{ assessmentLabels[activity.assessment?.status ?? 'unassessed'] }}</small>
      <small v-if="activity.assessment?.assistance_count">本次回答前已展示帮助。</small>
      <small v-if="(activity.assessment?.attempt ?? 0) > 1">这是第 {{ activity.assessment?.attempt }} 次作答。</small>
      <ul v-if="activity.assessment?.items?.length" :data-feedback-ref="activity.assessment.action_ref"><li v-for="item in activity.assessment.items" :key="item.id"><div class="activity-text md" v-html="renderText(item.reason)" @click="openSource(item.reason, $event)"></div></li></ul>
    </article>
  </section>
</template>
<style scoped>
.tutor-activities:empty { display: none; }
article { margin-top: 24px; min-width: 0; scroll-margin-block: 20px; }
article.interactive { padding: 22px; border: 1px solid color-mix(in srgb, var(--brand-green) 28%, var(--line)); border-radius: 16px; background: var(--surface-soft); }
.activity-heading { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin-bottom: 16px; color: var(--brand-green-deep); font-size: .85rem; font-weight: 600; }
.activity-heading small { margin: 0 0 0 auto; font-weight: 400; }
textarea { display: block; width: 100%; box-sizing: border-box; min-height: 100px; margin: 18px 0 12px; padding: 12px 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--canvas); color: var(--ink); font: inherit; font-size: 16px; line-height: 1.65; resize: vertical; }
textarea::placeholder { color: var(--steel); }
textarea:focus { outline: 2px solid var(--reader-coral); outline-offset: 2px; }
.activity-actions { display: flex; flex-wrap: wrap; gap: 8px; }
button { min-height: 42px; padding: 8px 14px; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--slate); font: inherit; font-size: .82rem; cursor: pointer; }
button:hover:not(:disabled) { border-color: var(--brand-green); color: var(--brand-green-deep); }
button.primary { background: var(--brand-green-deep); border-color: var(--brand-green-deep); color: #fff; font-weight: 600; }
button.primary:hover:not(:disabled) { background: color-mix(in srgb, var(--brand-green-deep) 88%, var(--ink)); color: #fff; }
button:disabled { opacity: .5; cursor: default; }
small { display: block; margin-top: 10px; color: var(--steel); font-size: .75rem; line-height: 1.6; }
.activity-text { line-height: 1.85; overflow-wrap: anywhere; }
.help { margin-top: 18px; padding: 14px; border-left: 2px solid var(--brand-green); background: var(--canvas); border-radius: 0 8px 8px 0; }
@media (max-width: 600px) { article.interactive { padding: 16px; border-radius: 12px; } .activity-heading small { flex-basis: 100%; margin-left: 25px; } }
.activity-text :deep(.agent-source-button) { display: inline-flex; align-items: center; gap: 0.3rem; max-width: 100%; margin: 0 0.2rem; padding: 0.18rem 0.42rem; border: 1px solid var(--line); border-radius: 6px; color: var(--accent); background: var(--canvas); font: inherit; font-size: 0.8em; cursor: pointer; }
</style>
