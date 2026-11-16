<script setup lang="ts">
import { onBeforeUnmount } from 'vue';
import { X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { applicationOverlayTarget } from '../application-overlay';
import LearningPurposeEntry from './LearningPurposeEntry.vue';
import type { LearningPurpose } from '../generated/LearningPurpose';
import type { LearningPurposeStart, LearningScopeKind, LearningScopeOption } from '../learning-purpose';

defineProps<{ available: boolean; ready: boolean; enabled: boolean; busy: boolean; reason?: string; error?: string; initialScope?: LearningScopeKind; scopeOptions: LearningScopeOption[] }>();
const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'choose', purpose: LearningPurpose, scope?: LearningScopeKind, request?: LearningPurposeStart): void;
}>();
const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null;
onBeforeUnmount(() => { if (origin?.isConnected) origin.focus({ preventScroll: true }); });
function trapFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('button:not(:disabled), textarea:not(:disabled), select:not(:disabled)')];
  const first = controls[0], last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
</script>

<template>
  <Teleport :to="applicationOverlayTarget()">
    <div class="purpose-backdrop" @click.self="emit('close')">
      <section class="purpose-dialog" role="dialog" aria-modal="true" :aria-label="uiText('开始一次学习')" @keydown.esc.stop.prevent="emit('close')" @keydown="trapFocus">
        <header><h2>{{ uiText('开始一次学习') }}</h2><button type="button" :aria-label="uiText('关闭')" @click="emit('close')"><X :size="20" /></button></header>
        <LearningPurposeEntry :available="available" :ready="ready" :enabled="enabled" :busy="busy" :reason="reason" :scope-options="scopeOptions"
          initial-purpose="understand" :initial-scope="initialScope" :show-custom="false" @choose="(purpose, scope, request) => emit('choose', purpose, scope, request)" />
        <p v-if="error" role="alert">{{ error }}</p>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.purpose-backdrop { position: fixed; inset: 0; z-index: 120; display: grid; place-items: center; padding: 20px; background: #211b184f; }
.purpose-dialog { box-sizing: border-box; width: min(480px, 100%); max-height: calc(var(--app-height, 100dvh) - 40px); overflow-y: auto; padding: 20px; border: 1px solid var(--line); border-radius: 16px; background: var(--canvas); color: var(--ink); box-shadow: 0 20px 70px #211b1826; }
header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 20px; }
h2 { margin: 0; font-size: 1.15rem; }
header button { display: grid; place-items: center; width: 36px; height: 36px; border: 0; border-radius: 8px; background: transparent; color: var(--steel); cursor: pointer; }
header button:focus-visible { outline: 2px solid var(--brand-green-deep); }
@media (max-width: 600px) { .purpose-backdrop { padding: 12px; } .purpose-dialog { padding: 16px; max-height: calc(var(--app-height, 100dvh) - 24px); } header button { width: 44px; height: 44px; } }
</style>
