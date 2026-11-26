<script setup lang="ts">
import { ArrowUpRight } from '@lucide/vue';

defineProps<{ text: string; action: string; disabled?: boolean }>();
defineEmits<{ navigate: [] }>();
</script>

<template>
  <div class="citation">
    <blockquote>{{ text }}</blockquote>
    <button type="button" class="quote-action" :disabled="disabled" @click="$emit('navigate')">
      {{ action }}<ArrowUpRight aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.citation { min-width: 0; }
.citation + .citation { margin-top: 10px; }
.citation blockquote { margin: 0; padding: 0 0 0 10px; border: 0; border-inline-start: 2px solid #d5c9b9; color: var(--reader-body); background: transparent; white-space: pre-line; font: 400 13px/1.65 var(--sans); }
.quote-action { display: inline-flex; align-items: center; gap: 3px; padding: 3px 0; margin: 2px 0 0 12px; border: 0; background: transparent; color: var(--brand-accent-deep); font: 550 12px/1.45 var(--sans); cursor: pointer; }
.quote-action svg { width: 12px; height: 12px; }
.quote-action:disabled { color: var(--reader-muted); cursor: default; }
.quote-action:not(:disabled):hover { text-decoration: underline; }
.quote-action:focus-visible { outline: 2px solid var(--brand-accent-deep); outline-offset: 2px; }
</style>
