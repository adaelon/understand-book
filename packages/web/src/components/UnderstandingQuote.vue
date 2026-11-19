<script setup lang="ts">
import { t as uiText } from '../i18n';
import { renderMarkdown } from '../md';
import type { UnderstandingChoiceAnswer } from '../note-thinking';
defineProps<{ quote: string; choice?: UnderstandingChoiceAnswer | null }>();
</script>
<template>
  <blockquote v-if="choice">
    <div v-if="!choice.options.length">{{ quote }}</div>
    <div v-for="(option, index) in choice.options" :key="index" class="option"><strong>{{ option.id }}</strong><div v-if="option.text" class="md" v-html="renderMarkdown(option.text)" /></div>
    <div v-if="choice.reasoning" class="reasoning"><small>{{ uiText('我的思路') }}</small>{{ choice.reasoning }}</div>
  </blockquote>
  <blockquote v-else>{{ quote || uiText('此条记录未保留原话。') }}</blockquote>
</template>
<style scoped>
.reasoning { white-space: pre-wrap; }
.option { display: flex; gap: .5em; align-items: baseline; }
.reasoning small { display: block; margin-top: .5em; color: var(--muted); }
.md :deep(p) { margin: 0; }
</style>
