<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { t as uiText } from '../i18n';

const props = defineProps<{ active: boolean; placementActive?: boolean }>();
const open = defineModel<boolean>('open', { required: true });
const emit = defineEmits<{ exit: [] }>();
const toggle = ref<HTMLButtonElement>();

function escape(event: KeyboardEvent) {
  if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing || !props.active || props.placementActive) return;
  const pane = toggle.value?.closest('.reader-pane, .pdf-reader-pane');
  const target = event.target;
  if (!pane || target instanceof Element && target !== document.body && (
    !pane.contains(target) || target.closest('input, textarea, select, [contenteditable="true"], [role="dialog"]')
  )) return;
  if (Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"], dialog[open]')).some(el => el.getClientRects().length)) return;
  const selection = window.getSelection();
  if (selection && !selection.isCollapsed) {
    if (pane.contains(selection.anchorNode)) selection.removeAllRanges();
    return;
  }
  event.preventDefault();
  if (open.value) {
    open.value = false;
    toggle.value?.focus({ preventScroll: true });
  } else {
    emit('exit');
  }
}
onMounted(() => window.addEventListener('keydown', escape));
onBeforeUnmount(() => window.removeEventListener('keydown', escape));
</script>

<template>
  <nav class="focus-controls" :aria-label="uiText('专注阅读工具')">
    <button class="focus-exit" type="button" aria-keyshortcuts="Escape" @click="emit('exit')">{{ uiText('退出专注') }}</button>
    <button ref="toggle" class="focus-tools-toggle" type="button" :aria-label="uiText('阅读工具')" :aria-expanded="open" @click="open = !open">···</button>
    <div v-if="open" class="focus-tools"><slot /></div>
  </nav>
</template>
