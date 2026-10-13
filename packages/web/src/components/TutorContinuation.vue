<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowRight } from '@lucide/vue';
import { api as sharedApi } from '../api';
import { bindSceneApi } from '../network-context';
import type { TutorContinuationView } from '../generated/TutorContinuationView';
import type { ContinuationDisplayed } from '../generated/ContinuationDisplayed';
import type { PresentationRef } from '../generated/PresentationRef';
import type { PresentationFollowUp } from '../generated/PresentationFollowUp';
const api = bindSceneApi(sharedApi);
const props = defineProps<{ sessionId: string; turnId: string; head: string | null; busy?: boolean;
  enabled: boolean; reference?: PresentationRef; snapshot?: () => Promise<PresentationFollowUp> }>();
const emit = defineEmits<{
  (e: 'choose', view: TutorContinuationView, optionId: string, receipt?: PresentationFollowUp): void;
  (e: 'locate', turnId: string): void;
  (e: 'edit'): void;
  (e: 'read'): void;
  (e: 'available', available: boolean): void;
}>();
const root = ref<HTMLElement>();
const view = ref<TutorContinuationView>();
const error = ref('');
const choosing = ref(false);
const loading = ref(false);
const options = computed(() => [...(view.value?.continuation.options ?? [])].sort((a, b) =>
  Number(b.option_id === view.value?.continuation.recommended_option_id) - Number(a.option_id === view.value?.continuation.recommended_option_id)));
const actionable = computed(() => !!view.value && view.value.availability.status === 'eligible'
  && props.enabled && props.head === view.value.expected_chat_head && !props.busy && !loading.value && !choosing.value);
const visible = new Set<string>();
const recorded = new Set<string>();
const pending = new Map<string, ContinuationDisplayed>();
let observer: IntersectionObserver | undefined;
let generation = 0;
let writing = false;
async function load() {
  const current = ++generation;
  loading.value = true;
  try {
    const result = await api.tutorActivities(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    view.value = result.activities.find(a => a.continuation)?.continuation ?? undefined;
    emit('available', !!view.value);
    await nextTick();
    root.value?.querySelectorAll('[data-option-id]').forEach(node => observer?.observe(node));
  } catch (failure) { if (current === generation) error.value = `建议读取未完成：${String(failure)}`; }
  finally { if (current === generation) loading.value = false; }
}
async function writeDisplayed() {
  if (writing) return;
  writing = true;
  const current = generation;
  try {
    for (const [key, input] of pending) {
      await api.tutorContinuationDisplayed(input);
      if (current !== generation) return;
      recorded.add(key); pending.delete(key);
    }
  } catch { error.value = '展示记录未保存，仍可选择建议。'; }
  finally { writing = false; }
}
function recordVisible() {
  const original = view.value;
  const ids = options.value.filter(o => visible.has(o.option_id)).map(o => o.option_id);
  if (!original || !ids.length) return;
  const key = JSON.stringify([original.delivery_ref, ids]);
  if (!recorded.has(key)) pending.set(key, { delivery_ref: original.delivery_ref, option_ids: ids,
    recommended_option_id: original.continuation.recommended_option_id });
  void writeDisplayed();
}
async function choose(optionId: string) {
  if (!actionable.value || !view.value) return;
  const original = JSON.parse(JSON.stringify(view.value)) as TutorContinuationView;
  const current = generation;
  choosing.value = true; error.value = '';
  try {
    const receipt = await props.snapshot?.();
    if (current !== generation || props.busy || !props.enabled || props.head !== original.expected_chat_head) return;
    emit('choose', original, optionId, receipt);
    await nextTick();
  } catch (failure) { error.value = `现场未保存，选择未发送：${String(failure)}`; }
  finally { choosing.value = false; }
}
async function retry() { error.value = ''; await writeDisplayed(); await load(); }
watch(() => [props.sessionId, props.turnId, props.reference?.presentation_id, props.reference?.revision], () => {
  view.value = undefined; visible.clear(); recorded.clear(); pending.clear(); observer?.disconnect(); void load();
}, { immediate: true });
watch(() => [props.head, props.enabled, props.busy], () => { void load(); });
onMounted(() => {
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const id = (entry.target as HTMLElement).dataset.optionId!;
      if (entry.isIntersecting) visible.add(id); else visible.delete(id);
    }
    recordVisible();
  });
  root.value?.querySelectorAll('[data-option-id]').forEach(node => observer?.observe(node));
  window.addEventListener('tutor-state-changed', load);
});
onBeforeUnmount(() => { generation++; observer?.disconnect(); window.removeEventListener('tutor-state-changed', load); });
</script>
<template>
  <section ref="root" class="tutor-continuation" aria-label="接着理解">
    <template v-if="view">
      <div class="continuation-heading"><span>接着理解</span><span class="continuation-hint">选一个方向，往前走一步</span></div>
      <p class="focus">{{ view.target_summary }}</p>
      <div class="continuation-options">
      <div v-for="option in options" :key="option.option_id" :data-option-id="option.option_id" class="option">
        <button v-if="view.availability.status === 'eligible' && enabled && head === view.expected_chat_head"
          :class="{ recommended: option.option_id === view.continuation.recommended_option_id }" :aria-label="option.label" :disabled="!actionable" @click="choose(option.option_id)">
          <span class="option-top"><span v-if="option.option_id === view.continuation.recommended_option_id" class="recommendation">建议先试</span><ArrowRight :size="17" aria-hidden="true" /></span>
          <span class="option-label">{{ option.label }}</span><small>{{ option.purpose }}</small>
        </button>
        <div v-else class="past-option"><span class="option-label">{{ option.label }}</span><small>{{ option.purpose }}</small></div>
      </div>
      </div>
      <button v-if="view.availability.status === 'consumed'" @click="emit('locate', view.availability.accepted_turn_id)">查看已接纳的任务</button>
      <small v-else-if="view.availability.status !== 'eligible' || !enabled || head !== view.expected_chat_head">这是之前的建议，可对原回答继续提问。</small>
      <div class="exits"><button @click="emit('edit')">换个问题</button><button @click="emit('read')">返回原文</button></div>
    </template>
    <p v-if="error" role="status">{{ error }} <button @click="retry">重试记录</button></p>
  </section>
</template>
<style scoped>
.tutor-continuation:empty { display: none; }
.tutor-continuation { container-type: inline-size; padding: 24px 0 4px; margin-top: 24px; border-top: 1px solid var(--line); min-width: 0; scroll-margin-top: 24px; }
.continuation-heading { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; color: var(--brand-green-deep); font-size: .76rem; font-weight: 600; }
.continuation-hint { color: var(--steel); font-size: .73rem; font-weight: 400; }
.focus { margin: 8px 0 16px; color: var(--ink); font-size: 1.06rem; font-weight: 600; line-height: 1.6; }
small { color: var(--steel); font-size: .78rem; line-height: 1.7; }
.continuation-options { display: grid; gap: 10px; }
.option { min-width: 0; overflow-wrap: anywhere; }
.option small { display: block; margin-top: 6px; font-weight: 400; }
button { font: inherit; color: inherit; cursor: pointer; min-height: 44px; border: 1px solid var(--line); border-radius: 12px; background: var(--canvas); padding: 12px 16px; text-align: left; max-width: 100%; overflow-wrap: anywhere; transition: border-color var(--motion-fast), background var(--motion-fast); }
.option > button { width: 100%; height: 100%; }
.option-top { display: flex; align-items: center; justify-content: flex-end; min-height: 22px; margin-bottom: 8px; color: var(--brand-green-deep); }
.recommendation { margin-right: auto; padding: 2px 7px; border-radius: 4px; background: var(--reader-coral-soft); font-size: .67rem; font-weight: 500; }
.option-label { display: block; color: var(--ink); font-size: .9rem; font-weight: 600; }
button.recommended { border-color: color-mix(in srgb, var(--brand-green) 50%, var(--line)); background: var(--surface-soft); }
.option button:hover:not(:disabled) { border-color: var(--brand-green); background: var(--surface-soft); }
.past-option { padding: 12px 0; border-bottom: 1px solid var(--hairline-soft); }
button:disabled { opacity: .55; cursor: default; }
.exits { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 10px; }
.exits button { border-color: transparent; background: transparent; padding-inline: 10px; color: var(--steel); font-size: .78rem; }
.exits button:hover { color: var(--brand-green-deep); background: var(--surface-soft); }
@container (min-width: 540px) { .continuation-options { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
