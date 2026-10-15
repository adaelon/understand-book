<script setup lang="ts">
import { notePreview } from '../reading-notes';
import NoteDetail from "./NoteDetail.vue";
import type { MemoryRecord } from "../api";

const props = defineProps<{
  note: MemoryRecord;
  renderMarkdown: (source: string) => string;
}>();

const emit = defineEmits<{
  (event: "open-note", record: MemoryRecord, restore?: boolean): void;
  (event: "focus-source", source: { lid: string; quote: string | null }): void;
  (event: "show-notes", note: MemoryRecord): void;
  (event: "edit", note: MemoryRecord): void;
  (event: "delete", note: MemoryRecord): void;
}>();

function leadingQuote(content: string): string | null {
  const lines = content.split("\n");
  const quoteLines: string[] = [];
  for (const line of lines) {
    if (line.startsWith(">")) quoteLines.push(line.replace(/^>\s?/, ""));
    else if (quoteLines.length > 0 && line.trim() === "") break;
    else if (quoteLines.length > 0) break;
  }
  const quote = quoteLines.join(" ").replace(/\s+/g, " ").trim();
  return quote || null;
}

function noteSourceLabel(note: MemoryRecord): string {
  const quote = note.selection_context?.resolved_quote;
  if (quote) return "引用来源";
  return note.anchor.lid ? "跳到来源" : "无来源";
}

function isLongNote(note: MemoryRecord): boolean {
  return note.content.length > 360 || note.content.split("\n").length > 8;
}

function focusSource() {
  const lid = props.note.anchor.lid;
  if (!lid) return;
  emit("focus-source", { lid, quote: props.note.selection_context?.resolved_quote ?? null });
}
</script>

<template>
  <details class="note-card" :open="!isLongNote(props.note)">
    <summary class="note-summary">
      <span class="note-kind">笔记</span>
      <button
        v-if="props.note.anchor.lid"
        class="note-source"
        @click.prevent.stop="focusSource"
      >
        {{ noteSourceLabel(props.note) }}
      </button>
      <span v-else class="note-source">无来源</span>
      <span v-if="isLongNote(props.note)" class="note-fold">展开/收起</span>
      <div
        v-if="isLongNote(props.note)"
        class="note-preview note-summary-preview"
      >{{ notePreview(props.note) }}</div>
    </summary>
    <NoteDetail :note="props.note" embedded @answer="emit('open-note', props.note)" @locate="emit('open-note', props.note)" @restore="emit('open-note', props.note, true)" />
    <div class="note-actions">
      <button class="note-btn" @click="emit('show-notes', props.note)">在笔记中查看</button>
      <button class="note-btn" title="编辑" @click="emit('edit', props.note)">编辑</button>
      <button class="note-btn del" title="删除" @click="emit('delete', props.note)">删除</button>
    </div>
  </details>
</template>

<style scoped>
.note-summary-preview { overflow: hidden; max-height: 4.5em; overflow-wrap: anywhere; }
.note-actions { flex-wrap: wrap; }
</style>
