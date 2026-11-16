<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, nextTick, onMounted, ref, useId, watch } from 'vue';
import { learningPurposes, learningPurposeSetup, musicLearningPurposes, videoLearningPurposes, type LearningPurposeStart, type LearningScopeKind, type LearningScopeOption } from '../learning-purpose';
import type { LearningPurpose } from '../generated/LearningPurpose';
import { suggestionTab, useReadingGuidance } from '../reading-guidance';
import InputSuggestionHint from './InputSuggestionHint.vue';

const props = withDefaults(defineProps<{
  available: boolean;
  ready: boolean;
  busy: boolean;
  musicMode?: boolean;
  videoMode?: boolean;
  reason?: string;
  scopeOptions?: LearningScopeOption[];
  showCustom?: boolean;
  initialPurpose?: LearningPurpose;
  initialScope?: LearningScopeKind;
}>(), { scopeOptions: () => [], showCustom: true });
const emit = defineEmits<{
  (event: 'choose', purpose: LearningPurpose, scope?: LearningScopeKind, request?: LearningPurposeStart): void;
  (event: 'custom'): void;
  (event: 'editing', active: boolean): void;
}>();
const selectedScope = ref<LearningScopeKind | undefined>(props.initialScope);
const selectedPurpose = ref<LearningPurpose | null>(props.initialPurpose ?? null);
const practiceFormat = ref('');
const formatPrompts: Record<string,string> = {
  single_choice: uiText('请在所选范围内出一道单选题。'), multiple_choice: uiText('请在所选范围内出一道多选题。'),
  worked_solution: uiText('请在所选范围内出一道需要写出过程的计算题。'),
};
watch(selectedPurpose, value => emit('editing', value !== null), { immediate: true });
const drafts = ref<Partial<Record<LearningPurpose, string>>>({});
const input = ref<HTMLTextAreaElement>();
const entry = ref<HTMLElement>();
const inputId = useId();
const setup = computed(() => selectedPurpose.value ? learningPurposeSetup(selectedPurpose.value, props.musicMode, props.videoMode) : null);
const draft = computed({
  get: () => selectedPurpose.value ? drafts.value[selectedPurpose.value] ?? '' : '',
  set: (value: string) => { if (selectedPurpose.value) drafts.value[selectedPurpose.value] = value; },
});
const purposes = computed(() => props.musicMode ? musicLearningPurposes : props.videoMode ? videoLearningPurposes : learningPurposes);
const notice = computed(() => {
  if (!props.available) return props.reason || uiText('学习入口暂不可用，请等待资料加载。');
  if (!props.ready) return props.reason ? uiText('{0} 可以先了解重点，也可以先保存学习目标。', [props.reason]) : uiText('资料准备中，可以先了解重点。');
  return '';
});
const scope = computed({
  get: () => props.scopeOptions.some(option => option.value === selectedScope.value) ? selectedScope.value : props.scopeOptions[0]?.value,
  set: (value: LearningScopeKind | undefined) => { selectedScope.value = value; },
});
const composing = ref(false);
const guidance = useReadingGuidance({
  purpose: () => selectedPurpose.value === 'understand' || selectedPurpose.value === 'explore' ? selectedPurpose.value : null,
  scope: () => scope.value, active: () => props.available && !props.busy, text: () => draft.value,
});
const suggestionVisible = computed(() => !!guidance.value && !draft.value.length && !composing.value && props.available && !props.busy);
async function acceptSuggestion() {
  if (!suggestionVisible.value) return;
  draft.value = guidance.value!.draft;
  await nextTick(); input.value?.focus(); input.value?.setSelectionRange(draft.value.length, draft.value.length);
}
function keydown(event: KeyboardEvent) {
  if (suggestionVisible.value && suggestionTab(event, composing.value)) {
    event.preventDefault(); event.stopPropagation(); void acceptSuggestion(); return;
  }
  if (!composing.value && !event.isComposing && event.keyCode !== 229 && (event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault(); event.stopPropagation(); submit();
  }
}
function start(purpose: LearningPurpose, message?: string) {
  if (props.busy || !props.available || (purpose !== 'discover' && !props.ready)) return;
  if (message) emit('choose', purpose, scope.value, { message });
  else emit('choose', purpose, scope.value);
}
async function choose(purpose: LearningPurpose) {
  if (props.busy || !props.available || (purpose !== 'discover' && !props.ready)) return;
  if (purpose === 'discover') { start(purpose); return; }
  selectedScope.value = scope.value;
  selectedPurpose.value = purpose;
  await nextTick(); input.value?.focus();
}
function submit() {
  const format = selectedPurpose.value === 'practice' ? formatPrompts[practiceFormat.value] : '';
  const message = [draft.value.trim(), format].filter(Boolean).join('\n');
  if (selectedPurpose.value && message) start(selectedPurpose.value, message);
}
function fallback() {
  if (!selectedPurpose.value || !setup.value) return;
  if (selectedPurpose.value === 'practice') { void choose('understand'); return; }
  start(selectedPurpose.value, setup.value.fallbackMessage);
}
async function back() {
  const previous = selectedPurpose.value;
  selectedPurpose.value = null;
  await nextTick(); entry.value?.querySelector<HTMLButtonElement>(`[data-purpose="${previous}"]`)?.focus();
}
onMounted(() => { if (selectedPurpose.value) input.value?.focus(); });
</script>

<template>
  <section ref="entry" class="learning-purpose-entry" :aria-label="uiText('选择学习目的')">
    <label v-if="scopeOptions.length" class="purpose-scope">{{ uiText('学习范围') }} <select v-if="scopeOptions.length > 1" v-model="scope" :disabled="busy" :aria-label="uiText('学习范围')">
        <option v-for="option in scopeOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
      </select>
      <span v-else>{{ scopeOptions[0].label }}</span>
    </label>
    <div v-if="!setup" class="purpose-options">
      <button v-for="option in purposes" :key="option.purpose" type="button" :data-purpose="option.purpose"
        :disabled="busy || !available || (option.purpose !== 'discover' && !ready)" @click="choose(option.purpose)">
        <strong>{{ option.label }}</strong><span>{{ option.description }}</span>
      </button>
    </div>
    <div v-else class="purpose-setup" role="group" :aria-labelledby="`${inputId}-label`">
      <button type="button" class="purpose-back" :disabled="busy" @click="back">← {{ uiText('换个目的') }}</button>
      <label :id="`${inputId}-label`" :for="inputId">{{ guidance?.question || setup.question }}</label>
      <p :id="`${inputId}-help`" class="purpose-outcome">{{ setup.outcome }}</p>
      <textarea :id="inputId" ref="input" v-model="draft" rows="3" :disabled="busy || !available" :placeholder="suggestionVisible ? guidance!.draft : composing ? '' : setup.placeholder"
        :aria-describedby="`${inputId}-help${suggestionVisible ? ` ${inputId}-suggestion` : ''}`" @keydown="keydown"
        @compositionstart="composing = true" @compositionend="composing = false" />
      <InputSuggestionHint v-if="suggestionVisible" :id="`${inputId}-suggestion`" @accept="acceptSuggestion" />
      <label v-if="selectedPurpose === 'practice' && !musicMode && !videoMode" class="purpose-scope">{{ uiText('题型') }}
        <select v-model="practiceFormat" :disabled="busy"><option value="">{{ uiText('由助教选题') }}</option>
          <option value="single_choice">{{ uiText('单选题') }}</option><option value="multiple_choice">{{ uiText('多选题') }}</option>
          <option value="worked_solution">{{ uiText('计算题') }}</option></select>
      </label>
      <button type="button" class="purpose-submit" :disabled="busy || !available || !ready || (!draft.trim() && !(selectedPurpose === 'practice' && practiceFormat))" @click="submit">{{ setup.submit }} <span aria-hidden="true">→</span></button>
      <button type="button" class="purpose-fallback" :disabled="busy || !available || !ready" @click="fallback">{{ setup.fallback }}</button>
    </div>
    <p v-if="notice" class="purpose-notice" role="status">{{ notice }}</p>
    <div v-if="!setup" class="purpose-other">
      <button type="button" :disabled="busy || !available" @click="choose('discover')">{{ musicMode ? uiText('还没想好，先带我听听') : uiText('还没想好，先带我看看') }}</button>
      <button v-if="showCustom" type="button" :disabled="busy" @click="emit('custom')">{{ uiText('自己描述目标…') }}</button>
    </div>
  </section>
</template>

<style scoped>
.learning-purpose-entry { width: 100%; min-width: 0; text-align: left; }
.purpose-scope { display: flex; align-items: center; gap: 8px; color: var(--steel); font-size: .76rem; margin-bottom: 12px; }
.purpose-scope select { flex: 1; min-width: 0; max-width: 100%; padding: 6px 8px; border: 1px solid var(--line); border-radius: 7px; background: var(--canvas); color: var(--ink); font: inherit; }
.purpose-scope > span { color: var(--ink); overflow-wrap: anywhere; }
.purpose-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
button { font: inherit; cursor: pointer; }
.purpose-options button { display: flex; flex-direction: column; align-items: flex-start; gap: 5px; min-width: 0; padding: 13px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--reader-card, var(--canvas)); color: var(--ink); text-align: left; }
.purpose-options strong { font-size: .85rem; font-weight: 600; }
.purpose-options span { color: var(--steel); font-size: .73rem; line-height: 1.5; }
.purpose-options button:hover:not(:disabled) { border-color: var(--brand-green-deep); background: var(--reader-coral-soft); }
.purpose-setup { display: flex; flex-direction: column; align-items: stretch; gap: 12px; padding: 16px; border: 1px solid var(--line); border-radius: 12px; background: var(--reader-card, var(--canvas)); }
.purpose-back { align-self: flex-start; border: 0; padding: 0; background: transparent; color: var(--steel); font-size: .75rem; min-height: 28px; }
.purpose-setup textarea::placeholder { color: var(--steel); opacity: .8; }
.purpose-setup label { font-size: 1rem; font-weight: 600; line-height: 1.6; color: var(--ink); }
.purpose-outcome { margin: 0; color: var(--steel); font-size: .8rem; line-height: 1.65; }
.purpose-setup textarea { box-sizing: border-box; width: 100%; min-height: 96px; resize: vertical; border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; background: var(--canvas); color: var(--ink); font: inherit; font-size: .85rem; line-height: 1.6; }
.purpose-submit { display: flex; justify-content: space-between; align-items: center; gap: 12px; border: 1px solid var(--brand-green-deep); border-radius: 8px; padding: 10px 12px; background: var(--brand-green-deep); color: white; font-size: .85rem; }
.purpose-fallback { border: 0; border-radius: 6px; padding: 5px 0; background: transparent; color: var(--brand-green-deep); text-align: left; font-size: .78rem; line-height: 1.6; }
.purpose-fallback:hover:not(:disabled), .purpose-back:hover:not(:disabled) { text-decoration: underline; }
.purpose-notice { margin: 10px 0 4px; color: var(--steel); font-size: .72rem; line-height: 1.6; }
.purpose-other { display: flex; flex-wrap: wrap; gap: 2px 12px; }
.purpose-other button { min-height: 36px; padding: 3px 0; border: 0; background: transparent; color: var(--brand-green-deep); font-size: .75rem; }
button:disabled { opacity: .5; cursor: default; }
button:focus-visible, select:focus-visible, textarea:focus-visible { outline: 2px solid var(--brand-green-deep); outline-offset: 3px; }
@media (pointer: coarse) { button, select { min-height: 44px; } .purpose-setup textarea { font-size: 16px; } }
@media (max-width: 360px) { .purpose-options { grid-template-columns: 1fr; } }
</style>
