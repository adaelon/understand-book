<script setup lang="ts">
import { getLocale, t as uiText } from '../i18n';
import QuestionWork from './QuestionWork.vue';

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { MessageCircle } from '@lucide/vue';
import { api as sharedApi, ApiError, type TutorActivity, type TutorHelp, type TeachingResponseOrigin } from '../api';
import { openPersonalNotes } from '../personal-notes';
import { answerDisplayParts, sourceMarkedParts } from '../answer-markdown';
import type { AgentAnswerSource } from '../generated/AgentAnswerSource';
import { bindSceneApi } from '../network-context';
const api = bindSceneApi(sharedApi);
import type { PresentationRef } from '../generated/PresentationRef';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const props = withDefaults(defineProps<{ sessionId: string; turnId: string; reference?: PresentationRef;
  snapshot?: () => Promise<PresentationFollowUp>; ready?: boolean; sources?: AgentAnswerSource[];
  answerText?: string; answerTarget?: () => HTMLElement | undefined; socraticTarget?: (prompt: string) => HTMLElement | undefined; responseActionsTarget?: HTMLElement | null;
  presentationTarget?: () => HTMLElement | undefined;
  hostedPresentations?: PresentationRef[]; busy?: boolean; beforeResponse?: () => Promise<void> }>(), { ready: true, sources: () => [], hostedPresentations: () => [] });
const emit = defineEmits<{ (e: 'source', id: string, anchor: HTMLElement): void; (e: 'response-available', available: boolean): void; (e: 'editing-response', editing: boolean): void;
  (e: 'guidance-available', available: boolean): void;
  (e: 'locate-presentation', reference: PresentationRef): void; (e: 'goto', lid: string): void }>();
const root = ref<HTMLElement>();
const activities = ref<TutorActivity[]>([]);
const activitiesReady = ref(false);
// Actionless moves are delivered through this turn's final answer. The saved
// candidate can differ in wording or formatting; observe the answer's exposure.
function usesAnswer(activity: TutorActivity) {
  return !activity.move.response_expectation && !!props.answerTarget && !!props.answerText?.trim() && activity.move.actions.length === 0;
}
function usesPresentation(activity: TutorActivity) {
  return !activity.move.response_expectation && activity.move.actions.length === 0 && !!props.presentationTarget
    && !!props.reference && activity.move.presentation?.presentation_id === props.reference.presentation_id
    && activity.move.presentation.revision === props.reference.revision;
}
const visibleActivities = computed(() => activities.value.filter(activity => !usesAnswer(activity)
  && !usesPresentation(activity)
  && !(!props.reference && activity.move.presentation && props.hostedPresentations.some(reference =>
    reference.presentation_id === activity.move.presentation!.presentation_id && reference.revision === activity.move.presentation!.revision))));
function invitationOpen(activity: TutorActivity) {
  return !!activity.move.response_expectation && (!activity.response_invitation_status || activity.response_invitation_status === 'open');
}
const editingDelivery = ref<string | null>(null);
function usesChatComposer(activity: TutorActivity) {
  const invitation = activity.move.response_expectation;
  return invitation?.socratic && invitationOpen(activity) && activity.status === 'active'
    && editingDelivery.value !== activity.delivery_ref && !!props.socraticTarget?.(invitation.public_prompt);
}
const responseActivity = computed(() => visibleActivities.value.find(activity => activity.status === 'active'
  && !usesChatComposer(activity) && (invitationOpen(activity) || activity.move.actions.some(action => action === 'self_report' || action === (activity.attempted ? 'revise' : 'submit')))));
watch(responseActivity, activity => emit('response-available', !!activity), { immediate: true });
// Wait for the actual activity data; an initial empty array is not an empty task.
const guidanceAvailable = computed(() => activitiesReady.value && !activities.value.some(activity =>
  (activity.status === 'active' && (invitationOpen(activity)
    || activity.move.actions.some(action => ['submit', 'revise', 'self_report'].includes(action))))
  || ['eligible', 'offered'].includes(activity.continuation?.availability.status ?? '')));
watch(guidanceAvailable, available => emit('guidance-available', available), { immediate: true });
function focusResponse() {
  const activity = responseActivity.value;
  if (!activity) return false;
  const article = Array.from(root.value?.querySelectorAll<HTMLElement>('[data-delivery-ref]') ?? [])
    .find(node => node.dataset.deliveryRef === activity.delivery_ref);
  const input = article?.querySelector('textarea');
  input?.scrollIntoView({ block: 'center' });
  input?.focus({ preventScroll: true });
  return !!input;
}
const answerDeliveries = new Map<Element, string[]>();
const answerEnd = ref<HTMLElement>();
function renderText(text: string) {
  return answerDisplayParts(sourceMarkedParts(text, props.sources), props.sources)
    .map(part => part.kind === 'html' ? part.html : '').join('');
}
function invitationAnswerHost(activity: TutorActivity): HTMLElement | undefined {
  const expectation = activity.move.response_expectation;
  if (expectation?.socratic) {
    const question = props.socraticTarget?.(expectation.public_prompt);
    if (question) return question;
  }
  if (!activity.move.response_expectation || props.reference || activity.move.presentation || !props.answerText?.trim()) return;
  const answer = props.answerTarget?.();
  // The final answer is the delivered wording, even when it revises the saved
  // invitation. Do not try to recover semantic identity by matching strings.
  // A target container alone is not evidence that its answer has rendered.
  return answer?.textContent?.trim() ? answer : undefined;
}
function answerInvitationHost(): HTMLElement | undefined {
  return visibleActivities.value.map(invitationAnswerHost).find(Boolean);
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
const reasoning = ref<Record<string, string>>({});
const askingId = ref('');
const gradingId = ref('');
const exposedSteps = new Set<string>(), seenSteps = new Set<string>();
const stepReceipts = new Map<string, Promise<void>>();
function markStep(id: string) {
  if (seenSteps.has(id)) return Promise.resolve();
  const pending = stepReceipts.get(id); if (pending) return pending;
  const task = api.tutorHelpDisplayed(id).then(() => { seenSteps.add(id); exposedSteps.delete(id); }).finally(() => stepReceipts.delete(id));
  stepReceipts.set(id, task); return task;
}
function updateAnswer(activity: TutorActivity, answer: string, thought: string) {
  if (activity.move.answer_input) responses.value[activity.delivery_ref] = answer;
  reasoning.value[activity.delivery_ref] = thought;
}
async function ensureDisplayed(activity: TutorActivity) {
  // An explicit action inside the card is also evidence of display before the observer's first frame.
  visible.add(activity.delivery_ref); await markDisplayed();
  if (!displayed.has(activity.delivery_ref)) {
    await api.tutorDisplay(activity.delivery_ref, await props.snapshot?.());
    displayed.add(activity.delivery_ref);
  }
}
// Keep supplied wording even if the user edits it. Editing is not evidence that
// the learner independently constructed an explanation they were already shown.
const responseOrigins = ref<Record<string, TeachingResponseOrigin>>({});
function beginResponseEditing(deliveryRef: string) { editingDelivery.value = deliveryRef; emit('editing-response', true); }
function endResponseEditing() {
  if (root.value?.contains(document.activeElement)) (document.activeElement as HTMLElement)?.blur();
  editingDelivery.value = null; emit('editing-response', false);
}
const helps = ref<Record<string, TutorHelp>>({});
const concluded = new Set<string>();
function concludePractice(deliveryRef: string) {
  if (concluded.has(deliveryRef)) return;
  concluded.add(deliveryRef);
  window.dispatchEvent(new CustomEvent('practice-concluded', { detail: { delivery_ref: deliveryRef } }));
}
// Practice hints point back to sources; the label says where, without a level number.
const hintPlaces: Record<NonNullable<TutorHelp['hint_kind']>, string> = { in_problem: uiText('再看看题目'), structure: uiText('换个角度看题'), own_note: uiText('你自己的想法'),
  source: uiText('回到原文'), worked_step: uiText('关键一步') };
function hintsLeft(activity: TutorActivity) {
  const practice = activity.move.practice;
  if (!practice) return null;
  // Saved displays survive a reload; the current help may not be acknowledged yet.
  const seen = (activity.help_seen as Partial<TutorHelp>[]).map(help => help.hint_index ?? -1);
  const released = Math.max(-1, helps.value[activity.delivery_ref]?.hint_index ?? -1, ...seen) + 1;
  return Math.max(0, practice.hint_count - released);
}
// Card assembly answers with the chosen card ids, in the order they were picked.
const pickedCards = ref<Record<string, string[]>>({});
function toggleCard(activity: TutorActivity, id: string) {
  const picked = [...(pickedCards.value[activity.delivery_ref] ?? [])];
  const at = picked.indexOf(id);
  if (at >= 0) picked.splice(at, 1); else picked.push(id);
  pickedCards.value[activity.delivery_ref] = picked;
  responses.value[activity.delivery_ref] = picked.length ? JSON.stringify(picked) : '';
  responseOrigins.value[activity.delivery_ref] = { kind: 'user_text' };
}
function actionLabel(activity: TutorActivity, action: string) {
  if (activity.move.answer_input && action === 'submit') return activity.move.answer_input.kind === 'worked_solution' ? uiText('提交解答') : uiText('提交选项');
  if (activity.move.answer_input && action === 'revise') return uiText('提交改答');
  if (!activity.move.practice) return labels[action];
  if (action === 'reveal') return uiText('看解答');
  if (action === 'request_hint') return uiText('提示（还有 {0} 条）', [String(hintsLeft(activity))]);
  return labels[action];
}
const error = ref(''); const busy = ref(false);
const visible = new Set<string>();
const visibleHelp = new Set<string>();
const feedbackSeen = new Set<string>();
const failedFeedback = new Set<string>();
let observer: IntersectionObserver | undefined;
let generation = 0;
let marking: Promise<void> | undefined;
const displayed = new Set<string>();
const pendingPageHelp: Array<Omit<Parameters<typeof api.tutorPageHelp>[0], 'presentation_follow_up'> & { presentation_follow_up?: PresentationFollowUp }> = [];
let savingPageHelp: Promise<void> | undefined;
let pending: Parameters<typeof api.tutorAction>[0] | null = null;
const labels: Record<string, string> = { submit: uiText('提交回答'), revise: uiText('改答'), request_hint: uiText('提示'), reveal: uiText('直接讲解'), skip: uiText('跳过'), self_report: uiText('记录自评') };
const assessmentLabels: Record<string,string> = { correct: uiText('本次回答符合标准'), partial: uiText('本次回答部分符合标准'), incorrect: uiText('本次回答未符合标准'), uncertain: uiText('目前依据不足，暂不能判断'), unassessed: uiText('回答已记录，尚未评分。') };
async function load() {
  const current = ++generation;
  activitiesReady.value = false;
  try {
    const result = await api.tutorActivities(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    for (const activity of result.activities) {
      if (activity.help) helps.value[activity.delivery_ref] = activity.help;
      const previous = activities.value.find(saved => saved.delivery_ref === activity.delivery_ref);
      if (activity.move.practice && activity.assessment?.status === 'correct'
        && previous && previous.assessment?.status !== 'correct') concludePractice(activity.delivery_ref);
    }
    activities.value = result.activities;
    activitiesReady.value = true;
    await observeVisibleContent();
    await markDisplayed();
  } catch (failure) { error.value = uiText('学习内容读取失败：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
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
  } catch (failure) { error.value = uiText('学习进度未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  })();
  try { await marking; } finally { marking = undefined; }
}
function focusActivity(moveId: string, response?: string) {
  const activity = activities.value.find(a => a.move.move_id === moveId);
  if (!activity) return;
  // An explicit page action can still open its scene-bound editor and prefill.
  if (usesChatComposer(activity)) beginResponseEditing(activity.delivery_ref);
  if (typeof response === 'string') {
    responses.value[activity.delivery_ref] = response;
    responseOrigins.value[activity.delivery_ref] = { kind: 'presentation_prefill', prefill_text: response,
      ...(props.reference ? { presentation: props.reference } : {}) };
  }
  root.value?.scrollIntoView({ block: 'nearest' });
  void nextTick(focusResponse);
}
async function reportPageHelp(moveId: string, kind: 'hint' | 'explanation', text: string) {
  const activity = activities.value.find(item => item.move.move_id === moveId);
  if (!activity || !props.snapshot || !text.trim()) throw new Error(uiText('当前演示没有可关联的教学邀请。'));
  pendingPageHelp.push({ delivery_ref: activity.delivery_ref, kind, text });
  try {
    await waitForHelp();
  } catch (failure) { error.value = uiText('提示与讲解的阅读进度未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]); throw failure; }
}
async function waitForHelp() {
  if (savingPageHelp) return savingPageHelp;
  savingPageHelp = (async () => {
    while (pendingPageHelp.length) {
      const help = pendingPageHelp[0];
      help.presentation_follow_up ??= await props.snapshot?.();
      if (!help.presentation_follow_up) throw new Error(uiText('暂时无法保存提示对应的演示状态，请重试。'));
      await api.tutorPageHelp({ ...help, presentation_follow_up: help.presentation_follow_up });
      pendingPageHelp.shift();
    }
  })();
  try { await savingPageHelp; } finally { savingPageHelp = undefined; }
}
defineExpose({ focusActivity, focusResponse, endResponseEditing, reportPageHelp, waitForHelp });
function continueLearning(eventId: string, text: string, scene?: PresentationFollowUp, responseOrigin?: TeachingResponseOrigin) {
  root.value?.dispatchEvent(new CustomEvent('tutor-response', { bubbles: true, detail: { eventId, text, scene, responseOrigin } }));
}
async function respond(activity: TutorActivity, request?: 'help' | 'explain' | 'defer') {
  if (busy.value || props.busy || activity.status !== 'active' || !invitationOpen(activity)) return;
  const requests = { help: uiText('我不知道从哪里开始想，请帮我找一个起点。'), explain: uiText('请先完整讲解这一步。'), defer: uiText('这次先暂缓这个问题，我想继续阅读。') };
  const text = request ? requests[request] : responses.value[activity.delivery_ref] ?? '';
  if (!text.trim()) return;
  busy.value = true; error.value = '';
  try {
    await props.beforeResponse?.();
    await waitForHelp();
    const scene = await props.snapshot?.();
    const origin: TeachingResponseOrigin = request ? { kind: 'interface_request' }
      : responseOrigins.value[activity.delivery_ref] ?? { kind: 'user_text' };
    endResponseEditing();
    // This enters the ordinary chat admission path. Only the learner's actual
    // text is a possible understanding expression; requests retain their origin.
    continueLearning(activity.delivery_ref, text, scene, origin);
  } catch (failure) { error.value = uiText('消息未发送：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { busy.value = false; }
}
async function acknowledgeHelp(id: string) {
  const help = helps.value[id];
  if (!help || !visibleHelp.has(id)) return;
  await nextTick();
  await api.tutorHelpDisplayed(help.event_id);
}
async function act(activity: TutorActivity, action: string) {
  if (busy.value || props.busy || pending || askingId.value) return;
  busy.value = true; error.value = '';
  try {
    await props.beforeResponse?.();
    await waitForHelp();
    await ensureDisplayed(activity);
    await Promise.all([...exposedSteps].map(markStep));
    const scene = await props.snapshot?.();
    pending = { operation_id: crypto.randomUUID(), delivery_ref: activity.delivery_ref, action,
      response: ['submit', 'revise', 'self_report'].includes(action) ? responses.value[activity.delivery_ref] ?? '' : null,
      ...(activity.move.answer_input && activity.move.answer_input.kind !== 'worked_solution' && ['submit','revise'].includes(action)
        ? { reasoning: reasoning.value[activity.delivery_ref] ?? '' } : {}),
      response_origin: responseOrigins.value[activity.delivery_ref], scene };
    await send();
  } catch (failure) { if (failure instanceof ApiError && failure.status < 500) pending = null; error.value = uiText('操作未确认：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { busy.value = false; }
}
async function send() {
  if (!pending) return;
  const request = pending;
  const result = await api.tutorAction(request);
  pending = null;
  const activity = activities.value.find(a => a.delivery_ref === request.delivery_ref);
  const formal = ['submit', 'revise'].includes(request.action);
  // A concluded practice problem lets an active warm-up offer its next problem.
  if (activities.value.some(a => a.delivery_ref === request.delivery_ref && a.move.practice)
    && (result.assessment === 'correct' || ['skip', 'reveal'].includes(request.action))) {
    concludePractice(request.delivery_ref);
  }
  if (result.help) {
    helps.value[request.delivery_ref] = result.help;
    await observeVisibleContent();
    await acknowledgeHelp(request.delivery_ref);
  }
  // A correct practice answer also releases its solution; the answer still gets a reply.
  if (formal && activity?.move.answer_input) {
    await load();
    if (activity.move.answer_input.kind === 'worked_solution' && result.assessment === 'unassessed') {
      await grade(activity, result.event_id);
    }
    return;
  }
  if (!result.help || formal) {
    endResponseEditing();
    continueLearning(result.event_id, request.response || uiText('我跳过了这个活动，请调整接下来的讲法。'), request.scene, request.response_origin);
  }
  await load();
}
async function grade(activity: TutorActivity, actionRef: string) {
  gradingId.value = activity.delivery_ref; error.value = '';
  try { await api.tutorGrade(actionRef, getLocale()); await load(); }
  catch (failure) { error.value = uiText('答案已保存，可重新批改这次提交。{0}', [failure instanceof Error ? failure.message : '']); }
  finally { gradingId.value = ''; }
}
async function retry() {
  busy.value = true; error.value = '';
  try {
    await waitForHelp();
    for (const id of failedFeedback) { await api.tutorFeedbackDisplayed(id); failedFeedback.delete(id); feedbackSeen.add(id); }
    if (pending) await send();
    else { for (const id of Object.keys(helps.value)) await acknowledgeHelp(id); await load(); }
  } catch (failure) { if (failure instanceof ApiError && failure.status < 500) pending = null; error.value = uiText('操作未确认：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { busy.value = false; }
}
watch(() => [props.sessionId, props.turnId, props.reference?.presentation_id, props.reference?.revision], () => {
  visible.clear(); visibleHelp.clear(); displayed.clear(); answerDeliveries.clear(); observer?.disconnect();
  exposedSteps.clear(); seenSteps.clear(); stepReceipts.clear(); reasoning.value = {}; askingId.value = ''; gradingId.value = '';
  pending = null; activities.value = []; helps.value = {}; concluded.clear(); responses.value = {}; responseOrigins.value = {}; void load();
}, { immediate: true });
watch(() => props.ready, () => { void markDisplayed(); });
watch(visibleActivities, () => { void observeVisibleContent(); });
watch(() => props.answerText, () => { void observeVisibleContent(); });
async function observeVisibleContent() {
  await nextTick();
  answerDeliveries.clear();
  const answer = props.answerTarget?.();
  const ids = activities.value.filter(usesAnswer).map(activity => activity.delivery_ref);
  if (answer && ids.length) { answerDeliveries.set(answer, ids); observer?.observe(answer); }
  // An actionless page is the delivery itself; its private move prompt is not
  // another explanation. Record exposure against the actual rendered frame.
  const presentation = props.presentationTarget?.();
  const pageIds = activities.value.filter(usesPresentation).map(activity => activity.delivery_ref);
  if (presentation && pageIds.length) { answerDeliveries.set(presentation, pageIds); observer?.observe(presentation); }
  const invitations = visibleActivities.value.filter(invitationAnswerHost).map(activity => activity.delivery_ref);
  if (answerEnd.value && invitations.length) {
    // This marks reaching the delivered answer's end, not reading a particular
    // sentence or understanding it. The opening and response controls do not count.
    answerDeliveries.set(answerEnd.value, invitations);
    observer?.observe(answerEnd.value);
  }
  root.value?.querySelectorAll<HTMLElement>('[data-delivery-ref], [data-help-for], [data-feedback-ref], [data-step-help]').forEach(node => observer?.observe(node));
}
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const node = entry.target as HTMLElement;
      const step = node.dataset.stepHelp;
      if (step && entry.isIntersecting && !seenSteps.has(step)) {
        exposedSteps.add(step);
        void markStep(step).catch(() => { error.value = uiText('引导的阅读记录暂未保存。'); });
      }
      const deliveries = answerDeliveries.get(node);
      if (deliveries) {
        for (const id of deliveries) { if (entry.isIntersecting) visible.add(id); else visible.delete(id); }
        continue;
      }
      const feedback = node.dataset.feedbackRef;
      if (feedback && entry.isIntersecting && !feedbackSeen.has(feedback)) {
        feedbackSeen.add(feedback);
        void api.tutorFeedbackDisplayed(feedback).catch(failure => { feedbackSeen.delete(feedback); failedFeedback.add(feedback); error.value = uiText('学习反馈的阅读进度未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]); });
      }
      const helpFor = node.dataset.helpFor;
      const id = helpFor ?? node.dataset.deliveryRef;
      if (!id) continue;
      if (!helpFor && activities.value.some(activity => activity.delivery_ref === id && invitationAnswerHost(activity))) continue;
      const targets = helpFor ? visibleHelp : visible;
      if (entry.isIntersecting) targets.add(id); else targets.delete(id);
      if (helpFor && entry.isIntersecting) void acknowledgeHelp(id).catch(failure => { error.value = uiText('提示与讲解的阅读进度未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]); });
    }
    void markDisplayed();
  });
  void observeVisibleContent();
  window.addEventListener('tutor-state-changed', load);
});
onBeforeUnmount(() => { generation++; observer?.disconnect(); window.removeEventListener('tutor-state-changed', load); emit('response-available', false); emit('guidance-available', false); });
</script>
<template>
  <section ref="root" class="tutor-activities" :aria-label="uiText('教学活动')">
    <Teleport v-if="answerInvitationHost()" :to="answerInvitationHost()!">
      <span ref="answerEnd" class="answer-delivery-end" aria-hidden="true"></span>
    </Teleport>
    <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="retry">{{ uiText('重试') }}</button></p>
    <article v-for="activity in visibleActivities" v-show="!usesChatComposer(activity)" :key="activity.delivery_ref" :data-delivery-ref="activity.delivery_ref" :class="{ interactive: activity.move.actions.length && !activity.move.response_expectation, invitation: !!activity.move.response_expectation }">
      <template v-if="activity.move.response_expectation">
        <div v-if="!invitationAnswerHost(activity)" class="activity-text md" v-html="renderText(activity.move.response_expectation.public_prompt)" @click="openSource(activity.move.response_expectation.public_prompt, $event)"></div>
        <button v-if="activity.move.presentation && !reference" class="scene-link" @click="emit('locate-presentation', activity.move.presentation)">{{ uiText('回到这个演示') }}</button>
        <p v-if="activity.status !== 'active'" role="status">{{ activity.status === 'paused' ? uiText('学习已暂停，仍可回看这个问题。') : uiText('原教学来源不可用，仍可回看这个问题。') }}</p>
        <template v-else-if="invitationOpen(activity)">
          <textarea v-model="responses[activity.delivery_ref]" data-workspace-input="tutor" :aria-label="uiText('说说你的理解')" :placeholder="uiText('说说你的理解，或者还没想通的地方…')" rows="2" :disabled="busy || props.busy" @focus="beginResponseEditing(activity.delivery_ref)" />
          <small v-if="responseOrigins[activity.delivery_ref]?.kind === 'presentation_prefill'">{{ uiText('演示已提供一段文字，你可以修改或说明自己的想法。') }}</small>
          <Teleport :to="responseActionsTarget || 'body'" :disabled="!responseActionsTarget || editingDelivery !== activity.delivery_ref">
            <div class="activity-actions">
              <button class="primary" :disabled="busy || props.busy || !responses[activity.delivery_ref]?.trim()" @click="respond(activity)">{{ uiText('发送') }}</button>
              <details class="invitation-help"><summary>{{ uiText('换一种继续方式') }}</summary><div class="activity-actions">
                <button :disabled="busy || props.busy" @click="respond(activity, 'help')">{{ uiText('帮我找个起点') }}</button>
                <button :disabled="busy || props.busy" @click="respond(activity, 'explain')">{{ uiText('先完整讲给我') }}</button>
                <button :disabled="busy || props.busy" @click="respond(activity, 'defer')">{{ uiText('暂缓这个问题') }}</button>
              </div></details>
            </div>
          </Teleport>
        </template>
        <small v-else-if="activity.response_invitation_status === 'deferred'">{{ uiText('这个问题先留在这里。') }}</small>
      </template>
      <template v-else>
      <header v-if="activity.move.practice" class="activity-heading"><MessageCircle :size="17" aria-hidden="true" /><span>{{ activity.attempted ? uiText('继续完善你的回答') : uiText('练一练') }}</span><small>{{ uiText('想不起来时，提示会一步步指回你的想法和原文') }}</small></header>
      <header v-else-if="activity.move.actions.length" class="activity-heading"><MessageCircle :size="17" aria-hidden="true" /><span>{{ activity.attempted ? uiText('继续完善你的回答') : uiText('轮到你了') }}</span><small>{{ uiText('用自己的话试一试') }}</small></header>
      <div class="activity-text md" v-html="renderText(activity.move.prompt)" @click="openSource(activity.move.prompt, $event)"></div>
      <p v-if="activity.status !== 'active'" role="status">{{ activity.status === 'paused' ? uiText('学习已暂停，可查看已交付内容和帮助。') : uiText('原教学来源不可用，暂不能提交。') }}</p>
      <p v-if="activity.practice_check === 'questionable'" class="practice-check" role="status">{{ uiText('这道题的解答可能有误，本题结果不计入学习记录。') }}</p>
      <div v-if="activity.move.practice?.cards?.length" class="practice-cards" role="group" :aria-label="uiText('选出能一起推出结论的卡片')">
        <button v-for="card in activity.move.practice.cards" :key="card.id" type="button" class="practice-card"
          :aria-pressed="(pickedCards[activity.delivery_ref] ?? []).includes(card.id)" :disabled="busy || activity.status !== 'active'"
          @click="toggleCard(activity, card.id)">{{ card.text }}</button>
      </div>
      <textarea v-if="!activity.move.answer_input && !activity.move.practice?.cards?.length && activity.move.actions.some(a => ['submit', 'revise', 'self_report'].includes(a))" v-model="responses[activity.delivery_ref]" data-workspace-input="tutor" :aria-label="uiText('活动回答')" :placeholder="uiText('写下你的理解，或还不确定的地方…')" rows="3" @focus="beginResponseEditing(activity.delivery_ref)" />
      <QuestionWork v-if="activity.move.actions.includes('submit')"
        :activity="activity" :busy="busy || !!props.busy || !!gradingId" :ensure-displayed="() => ensureDisplayed(activity)"
        @asking="askingId = $event ? activity.delivery_ref : ''" @change="(answer, thought) => updateAnswer(activity, answer, thought)" @reload="load" />
      <Teleport :to="responseActionsTarget || 'body'" :disabled="!responseActionsTarget || editingDelivery !== activity.delivery_ref">
      <div v-if="activity.move.actions.length" class="activity-actions">
      <template v-for="action in activity.move.actions" :key="action">
        <button v-if="!(action === 'submit' && activity.attempted) && !(action === 'revise' && !activity.attempted)"
          :class="{ primary: ['submit', 'revise', 'self_report'].includes(action) }"
          :data-help-action="['request_hint', 'reveal'].includes(action) ? action : undefined"
          :disabled="busy || props.busy || !!askingId || !!gradingId || !!pending || (activity.status !== 'active' && ['submit', 'revise', 'skip'].includes(action)) || (action === 'request_hint' && hintsLeft(activity) === 0) || (!!activity.move.answer_input && ['submit','revise'].includes(action) && !responses[activity.delivery_ref]?.trim())"
          @click="act(activity, action)">{{ actionLabel(activity, action) }}</button>
      </template>
      </div>
      </Teleport>
      <div v-if="helps[activity.delivery_ref]" class="help" :data-help-for="activity.delivery_ref" role="status">
        <small v-if="helps[activity.delivery_ref].hint_kind" class="hint-place">{{ hintPlaces[helps[activity.delivery_ref].hint_kind!] }}</small>
        <small v-else-if="helps[activity.delivery_ref].reason === 'concluded'" class="hint-place">{{ uiText('完整解答') }}</small>
        <div class="activity-text md" v-html="renderText(helps[activity.delivery_ref].text)" @click="openSource(helps[activity.delivery_ref].text, $event)"></div>
        <button v-if="helps[activity.delivery_ref].note_ref" class="hint-link" @click="openPersonalNotes(helps[activity.delivery_ref].note_ref!)">{{ uiText('打开这条想法') }}</button>
        <button v-for="lid in helps[activity.delivery_ref].source_lids ?? []" :key="lid" class="hint-link" @click="emit('goto', lid)">{{ uiText('打开原文') }}</button>
      </div>
      <small v-if="activity.move.actions.length && !activity.move.actions.includes('submit') && !activity.move.actions.some(a => ['request_hint', 'reveal'].includes(a))">{{ uiText('当前题目未提供提示或直接讲解，可保留题目并返回原文阅读。') }}</small>
      <small v-if="activity.help_seen.length">{{ uiText('此活动已展示过帮助；重开页面会保留这项记录。') }}</small>
      <small v-if="activity.attempted" role="status">{{ gradingId === activity.delivery_ref ? uiText('正在逐项批改…') : assessmentLabels[activity.assessment?.status ?? 'unassessed'] }}</small>
      <button v-if="activity.move.answer_input?.kind === 'worked_solution' && activity.assessment?.status === 'unassessed' && activity.assessment.action_ref && !gradingId"
        :disabled="busy || props.busy || activity.status !== 'active'" @click="grade(activity, activity.assessment!.action_ref!)">{{ uiText('批改这次提交') }}</button>
      <small v-if="activity.assessment?.assistance_count">{{ uiText('本次回答前已展示帮助。') }}</small>
      <small v-if="(activity.assessment?.attempt ?? 0) > 1">{{ uiText('这是第') }} {{ activity.assessment?.attempt }} {{ uiText('次作答。') }}</small>
      <ul v-if="activity.assessment?.items?.length" :data-feedback-ref="activity.assessment.action_ref"><li v-for="item in activity.assessment.items" :key="item.id">
        <template v-if="item.retry && item.verdict === 'supported'">{{ uiText('这一步成立：{0}', [item.response_quote]) }}</template>
        <template v-else-if="item.retry && item.verdict === 'uncertain'">{{ uiText('这项依据还不充分，可以补充说明。') }}</template>
        <template v-else-if="item.retry && !item.reason">{{ uiText('请对照题目补全这项要求。') }}</template>
        <div v-else class="activity-text md" v-html="renderText(item.reason)" @click="openSource(item.reason, $event)"></div>
      </li></ul>
      </template>
    </article>
  </section>
</template>
<style scoped>
.tutor-activities:empty { display: none; }
.answer-delivery-end { display: block; width: 1px; height: 1px; pointer-events: none; }
article { margin-top: 24px; min-width: 0; scroll-margin-block: 20px; }
article.interactive { padding: 22px; border: 1px solid color-mix(in srgb, var(--brand-green) 28%, var(--line)); border-radius: 16px; background: var(--surface-soft); }
article.invitation { padding-block: 8px; }
.invitation textarea { min-height: 78px; }
.invitation-help { align-self: center; }
.invitation-help summary { padding: 8px; color: var(--steel); cursor: pointer; font-size: .82rem; }
.invitation-help .activity-actions { margin-top: 8px; }
.scene-link { margin-top: 8px; }
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
.help { margin-top: 18px; padding: 14px; border-left: 2px solid var(--brand-green); background: var(--canvas); border-radius: 0 8px 8px 0; line-height: 1.85; overflow-wrap: anywhere; }
.practice-cards { display: flex; flex-direction: column; gap: 8px; margin: 18px 0 12px; }
.practice-card { min-height: 42px; padding: 10px 12px; text-align: left; line-height: 1.6; }
.practice-card[aria-pressed=true] { border-color: var(--brand-green-deep); background: color-mix(in srgb, var(--brand-green) 12%, var(--canvas)); color: var(--ink); }
.practice-check { margin: 12px 0 0; padding: 10px 12px; border-radius: 8px; background: var(--canvas); color: var(--reader-coral); font-size: .82rem; }
.help .hint-place { margin: 0 0 6px; color: var(--brand-green-deep); font-weight: 600; }
.help .hint-link { margin: 10px 8px 0 0; min-height: 34px; }
@media (max-width: 600px) { article.interactive { padding: 16px; border-radius: 12px; } .activity-heading small { flex-basis: 100%; margin-left: 25px; } }
.activity-text :deep(.agent-source-button) { display: inline-flex; align-items: center; gap: 0.3rem; max-width: 100%; margin: 0 0.2rem; padding: 0.18rem 0.42rem; border: 1px solid var(--line); border-radius: 6px; color: var(--accent); background: var(--canvas); font: inherit; font-size: 0.8em; cursor: pointer; }
</style>
