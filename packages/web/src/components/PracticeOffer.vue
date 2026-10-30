<script setup lang="ts">
import { t as uiText } from '../i18n';
import { computed } from 'vue';

// An offer, not a problem: nothing is generated until the reader accepts.
const props = defineProps<{ kind: 'margin' | 'warmup' | 'warmup-next' | 'exam-next'; step?: number; total?: number; busy?: boolean }>();
const emit = defineEmits<{ (e: 'accept'): void; (e: 'dismiss'): void }>();
const text = computed(() => props.kind === 'margin'
  ? uiText('读到这里，要不要用前面学过的内容练一道？')
  : props.kind === 'warmup'
    ? uiText('热身：用之前学过的内容练几道小题？')
    : props.kind === 'exam-next'
      ? uiText('今日练习：再来一题（第 {0} / {1} 题）？', [String(props.step ?? 2), String(props.total ?? 3)])
      : uiText('热身：再来一题（第 {0} / {1} 题）？', [String(props.step ?? 2), String(props.total ?? 3)]));
</script>
<template>
  <aside class="practice-offer" role="status" :aria-label="uiText('练习提议')">
    <span>{{ text }}</span>
    <button type="button" class="primary" :disabled="busy" @click="emit('accept')">{{ props.kind === 'margin' ? uiText('练一题') : uiText('开始') }}</button>
    <button type="button" :disabled="busy" @click="emit('dismiss')">{{ uiText('不用了') }}</button>
  </aside>
</template>
<style scoped>
.practice-offer { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 10px; margin: 8px 0; padding: 10px 12px; border: 1px solid var(--line); border-left: 2px solid var(--brand-green); border-radius: 8px; background: var(--surface-soft); color: var(--slate); font-size: .82rem; line-height: 1.6; }
.practice-offer span { flex: 1 1 14rem; min-width: 0; }
button { min-height: 34px; padding: 4px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--canvas); color: var(--slate); font: inherit; cursor: pointer; }
button.primary { background: var(--brand-green-deep); border-color: var(--brand-green-deep); color: #fff; }
button:disabled { opacity: .5; cursor: default; }
</style>
