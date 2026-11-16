<script setup lang="ts">
import { t as uiText } from '../i18n';
defineEmits<{ (event: 'accept'): void }>();
</script>

<template>
  <div class="input-suggestion-hint">
    <span>{{ uiText('灰字是建议，也可以直接写自己的想法') }}</span>
    <button type="button" @click="$emit('accept')"><kbd>Tab</kbd>{{ uiText('采用建议') }}</button>
  </div>
</template>

<style scoped>
.input-suggestion-hint { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 4px 12px; color: var(--steel); font-size: 11px; line-height: 1.6; }
.input-suggestion-hint button { display: inline-flex; align-items: center; gap: 6px; min-height: 28px; padding: 2px 0; border: 0; background: transparent; color: var(--brand-green-deep); font: inherit; cursor: pointer; }
kbd { padding: 0 4px; border: 1px solid var(--line); border-radius: 4px; color: var(--steel); font: inherit; }
button:focus-visible { outline: 2px solid var(--brand-green-deep); outline-offset: 3px; }
@media (pointer: coarse) { kbd { display: none; } .input-suggestion-hint button { min-height: 44px; } }
</style>
