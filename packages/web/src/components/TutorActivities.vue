<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api as sharedApi, ApiError, type TutorActivity } from '../api';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
import type { PresentationRef } from '../generated/PresentationRef';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const props = defineProps<{ sessionId: string; turnId: string; reference?: PresentationRef;
  snapshot?: () => Promise<PresentationFollowUp>; ready?: boolean }>();
const root = ref<HTMLElement>();
const activities = ref<TutorActivity[]>([]);
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
defineExpose({ focusActivity });
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
  visible.clear(); visibleHelp.clear(); displayed.clear(); observer?.disconnect();
  pending = null; activities.value = []; helps.value = {}; responses.value = {}; void load();
}, { immediate: true });
watch(() => props.ready, () => { void markDisplayed(); });
async function observeVisibleContent() {
  await nextTick();
  root.value?.querySelectorAll<HTMLElement>('[data-delivery-ref], [data-help-for], [data-feedback-ref]').forEach(node => observer?.observe(node));
}
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const node = entry.target as HTMLElement;
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
    <article v-for="activity in activities" :key="activity.delivery_ref" :data-delivery-ref="activity.delivery_ref">
      <p>{{ activity.move.prompt }}</p>
      <p v-if="activity.status !== 'active'" role="status">{{ activity.status === 'paused' ? '学习已暂停，可查看已交付内容和帮助。' : '原教学来源不可用，暂不能提交。' }}</p>
      <textarea v-if="activity.move.actions.some(a => ['submit', 'revise', 'self_report'].includes(a))" v-model="responses[activity.delivery_ref]" aria-label="活动回答" rows="2" />
      <template v-for="action in activity.move.actions" :key="action">
        <button v-if="!(action === 'submit' && activity.attempted) && !(action === 'revise' && !activity.attempted)"
          :disabled="busy || !!pending || (activity.status !== 'active' && ['submit', 'revise', 'skip'].includes(action))" @click="act(activity, action)">{{ labels[action] }}</button>
      </template>
      <p v-if="helps[activity.delivery_ref]" class="help" :data-help-for="activity.delivery_ref" role="status">{{ helps[activity.delivery_ref].text }}</p>
      <small v-if="activity.help_seen.length">此活动已展示过帮助；重开页面会保留这项记录。</small>
      <small v-if="activity.attempted" role="status">{{ assessmentLabels[activity.assessment?.status ?? 'unassessed'] }}</small>
      <small v-if="activity.assessment?.assistance_count">本次回答前已展示帮助。</small>
      <small v-if="(activity.assessment?.attempt ?? 0) > 1">这是第 {{ activity.assessment?.attempt }} 次作答。</small>
      <ul v-if="activity.assessment?.items?.length" :data-feedback-ref="activity.assessment.action_ref"><li v-for="item in activity.assessment.items" :key="item.id">{{ item.reason }}</li></ul>
    </article>
  </section>
</template>
<style scoped>
.tutor-activities:empty { display: none; }
article { padding: 12px 16px; border-top: 1px solid var(--line); }
textarea { display: block; width: 100%; box-sizing: border-box; font: inherit; }
button { min-height: 44px; margin: 4px; font: inherit; }
small { display: block; margin-top: 8px; }
.help { white-space: pre-wrap; }
</style>
