<script setup lang="ts">
import { BookOpen, GraduationCap, LayoutDashboard, Route } from '@lucide/vue';
import { t as uiText } from '../i18n';

defineProps<{ current?: 'courses' | 'materials' | 'boards' | 'understanding'; busy: boolean }>();
const emit = defineEmits<{ courses: []; materials: []; boards: []; understanding: [] }>();
</script>

<template>
  <nav class="network-primary-nav" :aria-label="uiText('主导航')">
    <button type="button" data-tour="courses-entry" :disabled="busy" :aria-label="uiText('学习')" :aria-current="current === 'courses' ? 'page' : undefined" @click="emit('courses')"><GraduationCap :size="16" aria-hidden="true" /><span>{{ uiText('学习') }}</span></button>
    <button type="button" :disabled="busy" :aria-label="uiText('我的理解')" :aria-current="current === 'understanding' ? 'page' : undefined" @click="emit('understanding')"><Route :size="16" aria-hidden="true"/><span class="full-name">{{ uiText('我的理解') }}</span><span class="short-name">{{ uiText('理解') }}</span></button>
    <button type="button" data-tour="materials-entry" :disabled="busy" :aria-label="uiText('学习资料')" :aria-current="current === 'materials' ? 'page' : undefined" @click="emit('materials')">
      <BookOpen :size="16" aria-hidden="true" /><span class="full-name">{{ uiText('学习资料') }}</span><span class="short-name">{{ uiText('资料') }}</span>
    </button>
    <button type="button" data-tour="boards-entry" :disabled="busy" :aria-label="uiText('学习画板')" :aria-current="current === 'boards' ? 'page' : undefined" @click="emit('boards')">
      <LayoutDashboard :size="16" aria-hidden="true" /><span class="full-name">{{ uiText('学习画板') }}</span><span class="short-name">{{ uiText('画板') }}</span>
    </button>
  </nav>
</template>

<style scoped>
.network-primary-nav { display: flex; align-items: stretch; height: 100%; gap: 12px; }
button { position: relative; display: inline-flex; align-items: center; justify-content: center; gap: 7px; min-height: 0; padding: 0 6px; border: 0; border-radius: 0; background: transparent; color: var(--steel); font: 500 13px/1.2 var(--sans); white-space: nowrap; }
button:hover, button[aria-current] { color: var(--brand-accent-deep); }
button[aria-current]::after { content: ''; position: absolute; bottom: 0; inset-inline: 6px; height: 2px; background: var(--brand-accent); }
button:focus-visible { outline-offset: -3px; }
.short-name { display: none; }
@media (max-width: 640px) {
  .network-primary-nav { gap: 2px; }
  button { padding-inline: 7px; }
  .full-name, svg { display: none; }
  .short-name { display: inline; }
}
</style>
