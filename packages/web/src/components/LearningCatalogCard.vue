<script setup lang="ts">
import { ArrowRight } from '@lucide/vue';
import type { BookCoverSource } from '../api';
import MaterialCover from './MaterialCover.vue';

defineProps<{ title: string; kind: string; description: string; action: string; cover?: BookCoverSource | null; disabled?: boolean }>();
defineEmits<{ open: [] }>();
</script>

<template>
  <article class="collection-card">
    <div class="collection-cover"><MaterialCover :title="title" :cover="cover" /></div>
    <div class="collection-copy">
      <span class="eyebrow">{{ kind }}</span>
      <h3>{{ title }}</h3>
      <p>{{ description }}</p>
      <button type="button" class="primary" :disabled="disabled" @click="$emit('open')">{{ action }}<ArrowRight :size="16" aria-hidden="true" /></button>
    </div>
  </article>
</template>

<style scoped>
.collection-card { display:flex; gap:22px; min-width:0; padding:24px; border:1px solid var(--line); border-radius:12px; background:linear-gradient(130deg,var(--surface-soft),var(--canvas) 75%); }
.collection-cover { width:72px; flex:none; align-self:flex-start; }
.collection-copy { display:flex; flex-direction:column; align-items:flex-start; min-width:0; }
.eyebrow { color:var(--brand-accent-deep); font-size:12px; margin-bottom:7px; }
h3 { font-size:19px; line-height:1.5; font-weight:500; margin:0; overflow-wrap:anywhere; }
p { margin:10px 0 18px; color:var(--muted); font-size:13px; line-height:1.8; overflow-wrap:anywhere; }
button { display:inline-flex; align-items:center; gap:8px; margin-top:auto; min-height:42px; padding:10px 14px; font:inherit; font-size:13px; border:1px solid var(--brand-accent); border-radius:8px; background:var(--brand-accent); color:var(--brand-on-accent); cursor:pointer; }
button:disabled { opacity:.5; cursor:default; }
@media (max-width:620px) { .collection-card { padding:20px; gap:17px; } .collection-cover { width:58px; } }
</style>
