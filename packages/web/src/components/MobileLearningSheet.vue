<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { X } from '@lucide/vue';

const props = defineProps<{ open: boolean; title: string }>();
const emit = defineEmits<{ (e: 'close'): void }>();
const panel = ref<HTMLElement>();
let trigger: HTMLElement | null = null;
watch(() => props.open, async open => {
  if (open) {
    trigger = document.activeElement as HTMLElement;
    await nextTick();
    panel.value?.querySelector<HTMLButtonElement>('.sheet-close')?.focus({ preventScroll: true });
  } else {
    await nextTick();
    // A menu action can open another dialog; keep the focus it just acquired.
    const active = document.activeElement;
    if (active && active !== document.body && !panel.value?.contains(active)) return;
    if (trigger?.isConnected && trigger.getClientRects().length) trigger.focus({ preventScroll: true });
  }
});
function onKeydown(event: KeyboardEvent) {
  if (event.defaultPrevented || (event.target as Element).closest('[role="dialog"]') !== panel.value) return;
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); emit('close'); }
  if (event.key !== 'Tab') return;
  const controls = Array.from(panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), [href], [tabindex="0"]') ?? [])
    .filter(node => node.getClientRects().length);
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
</script>

<template>
  <Teleport to="body">
    <div v-show="open" class="learning-sheet-backdrop" @click.self="emit('close')">
      <section ref="panel" class="learning-sheet" role="dialog" aria-modal="true" :aria-label="title" @keydown="onKeydown">
        <header><div><span class="sheet-handle" aria-hidden="true" /><h2>{{ title }}</h2></div><button class="sheet-close" type="button" :aria-label="`关闭${title}`" @click="emit('close')"><X :size="20" aria-hidden="true" /></button></header>
        <div class="learning-sheet-content"><slot /></div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.learning-sheet-backdrop { position: fixed; z-index: 110; inset: var(--app-input-viewport-top, 0px) 0 auto; height: var(--app-input-viewport-height, 100dvh); display: flex; align-items: flex-end; background: #24211e40; }
.learning-sheet { position: relative; display: flex; flex-direction: column; width: 100%; max-height: 88%; border-radius: 22px 22px 0 0; background: var(--canvas); box-shadow: 0 -12px 48px #24211e14; overflow: hidden; }
header { position: relative; display: flex; align-items: center; justify-content: space-between; flex: 0 0 auto; padding: 22px 16px 10px 20px; }
.sheet-handle { position: absolute; width: 32px; height: 4px; top: 8px; left: calc(50% - 16px); border-radius: 8px; background: var(--line); }
h2 { margin: 0; font-size: 1.05rem; font-weight: 600; }
.sheet-close { display: grid; place-items: center; width: 44px; height: 44px; padding: 0; border: 0; border-radius: 12px; background: transparent; color: var(--steel); }
.learning-sheet-content { min-height: 0; overflow-y: auto; padding: 4px 20px max(20px, env(safe-area-inset-bottom)); }
.learning-sheet :deep(.conversation-learning-goal) { width: 100%; margin: 0; padding: 14px 0; border: 0; }
.learning-sheet :deep(.goal-heading) { grid-template-columns: 32px minmax(0, 1fr); gap: 10px; }
.learning-sheet :deep(.goal-icon) { display: grid; grid-column: 1; grid-row: 1; width: 32px; height: 32px; }
.learning-sheet :deep(.goal-label) { grid-column: 2; grid-row: 1; font-size: .76rem; }
.learning-sheet :deep(.goal-text) { grid-column: 1 / -1; grid-row: 2; margin: 0; font-size: 1rem; line-height: 1.7; white-space: normal; }
.learning-sheet :deep(.goal-heading .goal-actions) { grid-column: 1 / -1; grid-row: 3; margin-left: -6px; }
.learning-sheet :deep(.goal-manage), .learning-sheet :deep(.desktop-adjust) { display: inline; }
.learning-sheet :deep(.mobile-adjust) { display: none; }
.learning-sheet :deep(.teaching-hint.routine) { position: static; width: auto; height: auto; margin: 0; clip-path: none; white-space: normal; }
</style>
