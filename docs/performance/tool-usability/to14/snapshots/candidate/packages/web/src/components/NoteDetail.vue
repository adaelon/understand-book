<script setup lang="ts">
import { inject } from 'vue';
import { shareNoteKey } from '../reading-share';
import type { MemoryRecord } from '../api';
import { renderMarkdown } from '../md';
import PresentationSceneCard from './PresentationSceneCard.vue';
defineProps<{ note: MemoryRecord; embedded?: boolean }>();
const emit = defineEmits<{ (e: 'locate' | 'restore' | 'answer' | 'edit' | 'delete'): void }>();
const shareNote = inject(shareNoteKey, null);
</script>
<template>
  <section class="note-detail" aria-label="笔记详情">
    <template v-if="note.content"><p>{{ note.note ? '自己的文字' : '阅读笔记' }}</p><div class="md" v-html="renderMarkdown(note.content)"></div></template>
    <template v-if="note.note?.retained_excerpt"><p>{{ note.note.retained_excerpt.kind === 'original' ? '原文摘录' : '助手摘录' }}</p><blockquote class="md" v-html="renderMarkdown(note.note.retained_excerpt.text)"></blockquote></template>
    <template v-if="note.note?.association.kind === 'presentation'">
      <p>{{ note.note.association.title }}</p>
      <PresentationSceneCard :receipt="note.note.association.receipt" :mem-id="note.mem_id" @locate="emit('locate')" @restore="emit('restore')" />
    </template>
    <p v-if="note.note?.association.kind === 'answer'">助手回答 <button @click="emit('answer')">返回原回答</button></p>
    <button v-if="shareNote" data-share-note @click="shareNote(note)">生成分享图</button>
    <footer v-if="!embedded"><button @click="emit('edit')">编辑</button><button @click="emit('delete')">删除</button></footer>
  </section>
</template>
<style scoped>
.note-detail { min-width: 0; max-width: 100%; line-height: 1.7; overflow-wrap: anywhere; padding: 8px 0; }
.note-detail :deep(pre), .note-detail :deep(.katex-display), .note-detail :deep(table) { max-width: 100%; overflow-x: auto; }
.note-detail :deep(table) { display: block; }
.note-detail blockquote { margin: 12px 0; padding-left: 12px; border-left: 2px solid var(--hairline); }
.note-detail button { min-height: 40px; }
footer { display: flex; gap: 12px; margin-top: 16px; }
</style>
