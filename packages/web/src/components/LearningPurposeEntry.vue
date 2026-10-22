<script setup lang="ts">
import { computed, ref } from 'vue';
import { learningPurposes, musicLearningPurposes, type LearningScopeKind, type LearningScopeOption } from '../learning-purpose';
import type { LearningPurpose } from '../generated/LearningPurpose';

const props = withDefaults(defineProps<{
  available: boolean;
  ready: boolean;
  enabled: boolean;
  busy: boolean;
  musicMode?: boolean;
  scopeOptions?: LearningScopeOption[];
  showCustom?: boolean;
}>(), { scopeOptions: () => [], showCustom: true });
const emit = defineEmits<{
  (event: 'choose', purpose: LearningPurpose, scope?: LearningScopeKind): void;
  (event: 'custom'): void;
}>();
const selectedScope = ref<LearningScopeKind>();
const purposes = computed(() => props.musicMode ? musicLearningPurposes : learningPurposes);
const scope = computed({
  get: () => props.scopeOptions.some(option => option.value === selectedScope.value) ? selectedScope.value : props.scopeOptions[0]?.value,
  set: (value: LearningScopeKind | undefined) => { selectedScope.value = value; },
});
function choose(purpose: LearningPurpose) {
  if (props.busy || !props.available || (purpose !== 'discover' && !props.ready)) return;
  emit('choose', purpose, scope.value);
}
</script>

<template>
  <section class="learning-purpose-entry" aria-label="选择学习目的">
    <label v-if="scopeOptions.length" class="purpose-scope">学习范围
      <select v-if="scopeOptions.length > 1" v-model="scope" :disabled="busy" aria-label="学习范围">
        <option v-for="option in scopeOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
      </select>
      <span v-else>{{ scopeOptions[0].label }}</span>
    </label>
    <div class="purpose-options">
      <button v-for="option in purposes" :key="option.purpose" type="button" :data-purpose="option.purpose"
        :disabled="busy || !available || (option.purpose !== 'discover' && !ready)" @click="choose(option.purpose)">
        <strong>{{ option.label }}</strong><span>{{ option.description }}</span>
      </button>
    </div>
    <p class="purpose-notice">{{ !ready ? '连续教学准备中，可以先了解重点。' : !enabled ? musicMode ? '选择听懂、备考或探索，会开启连续教学并开始。' : '选择读懂、备考或探索，会开启连续教学并开始。' : '按你的目的开始，学习中可以随时调整。' }}</p>
    <div class="purpose-other">
      <button type="button" :disabled="busy || !available" @click="choose('discover')">{{ musicMode ? '还没想好，先带我听听' : '还没想好，先带我看看' }}</button>
      <button v-if="showCustom" type="button" :disabled="busy" @click="emit('custom')">自己描述目标…</button>
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
.purpose-notice { margin: 10px 0 4px; color: var(--steel); font-size: .72rem; line-height: 1.6; }
.purpose-other { display: flex; flex-wrap: wrap; gap: 2px 12px; }
.purpose-other button { min-height: 36px; padding: 3px 0; border: 0; background: transparent; color: var(--brand-green-deep); font-size: .75rem; }
button:disabled { opacity: .5; cursor: default; }
button:focus-visible, select:focus-visible { outline: 2px solid var(--brand-green-deep); outline-offset: 3px; }
@media (pointer: coarse) { button, select { min-height: 44px; } }
@media (max-width: 360px) { .purpose-options { grid-template-columns: 1fr; } }
</style>
