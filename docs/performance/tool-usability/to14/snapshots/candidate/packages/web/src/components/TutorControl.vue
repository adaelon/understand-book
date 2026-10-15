<script setup lang="ts">
import { computed, useId } from 'vue';
const props = defineProps<{ enabled: boolean; label: string; busy: boolean; unavailable: boolean; error: string }>();
const routineHint = computed(() => ['可以开始或继续学习', '围绕你的目标逐步学'].includes(props.label));
const emit = defineEmits<{ (e: 'toggle'): void; (e: 'retry'): void }>();
const descriptionId = useId();
</script>
<template>
  <div class="tutor-control">
    <div class="teaching-control-row">
      <button type="button" class="teaching-switch" role="switch" aria-label="连续教学" title="连续教学开关对所有书籍生效" :aria-describedby="descriptionId" :aria-checked="enabled" :disabled="busy || unavailable" @click="emit('toggle')">
        <span class="switch-track" aria-hidden="true"><span /></span>连续教学
      </button>
      <span class="teaching-hint" :class="{ routine: routineHint }" role="status">{{ label }}</span>
    </div>
    <p :id="descriptionId" class="teaching-scope">连续教学开关对所有书籍生效</p>
    <p v-if="error" class="teaching-error" role="alert">{{ error }} <button type="button" :disabled="busy" @click="emit('retry')">重试</button></p>
  </div>
</template>
<style scoped>
.tutor-control { min-width: 0; font-size: .78rem; }
.teaching-control-row { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 10px; }
button { font: inherit; color: var(--ink); cursor: pointer; }
.teaching-switch { display: inline-flex; flex-shrink: 0; align-items: center; gap: 7px; min-height: 36px; padding: 3px 0; border: 0; background: transparent; }
.switch-track { display: flex; align-items: center; width: 28px; height: 17px; padding: 3px; border-radius: 12px; background: var(--line); }
.switch-track > span { width: 11px; height: 11px; border-radius: 50%; background: var(--canvas); }
[aria-checked=true] .switch-track { justify-content: flex-end; background: var(--brand-green-deep); }
.teaching-hint, .teaching-scope { color: var(--steel); font-size: .73rem; overflow-wrap: anywhere; }
.teaching-scope { margin: 0; }
.teaching-error { margin: 6px 0 0; color: var(--ink); overflow-wrap: anywhere; }
.teaching-error button { min-height: 36px; padding: 3px 6px; border: 0; background: transparent; color: var(--brand-green-deep); }
button:disabled { opacity: .5; cursor: default; }
@media (pointer: coarse) { .teaching-switch { min-height: 44px; } }
@media (max-width: 1023px) {
  .teaching-switch { min-height: 44px; }
  .teaching-hint:not(.routine) { flex-basis: 100%; padding-bottom: 6px; }
  .teaching-scope, .teaching-hint.routine { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
}
</style>
