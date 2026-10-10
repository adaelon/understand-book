<script setup lang="ts">
import { inject } from 'vue';
import { shareHighlightKey } from '../reading-share';
const shareHighlight = inject(shareHighlightKey, null);
import NoteDetail from './NoteDetail.vue';
import type { MemoryRecord } from '../api';
const props = defineProps<{
  records: MemoryRecord[];
  active: MemoryRecord;
  sourceText: string;
  renderMarkdown: (source: string) => string;
}>();
const emit = defineEmits<{
  (event: "open-note", record: MemoryRecord, restore?: boolean): void;
  (event: 'select', id: string): void;
  (event: 'close'): void;
  (event: 'edit' | 'delete' | 'show-notes' | 'place', record: MemoryRecord): void;
  (event: 'source', source: { lid: string; quote: string | null }): void;
}>();
function quote() {
  if (props.active.type === 'highlight') return props.active.content;
  return props.active.selection_context?.resolved_quote
    || props.active.content.match(/^(?:>.*\n?)+/)?.[0].replace(/^>\s?/gm, '').trim()
    || null;
}
</script>

<template>
  <section class="annotation-preview" role="dialog" aria-label="正文批注" tabindex="-1"
    data-reader-selection-ignore data-note-placement-ignore @keydown.esc.stop.prevent="emit('close')">
    <header>
      <strong>正文批注 · {{ props.records.length }} 条</strong>
      <button aria-label="关闭批注" @click="emit('close')">×</button>
    </header>
    <div class="annotation-records" v-if="props.records.length > 1" role="group" aria-label="此处的批注">
      <button v-for="(record, index) in props.records" :key="record.mem_id"
        :aria-pressed="record.mem_id === props.active.mem_id" @click="emit('select', record.mem_id)">
        {{ record.type === 'highlight' ? '高亮' : '笔记' }} {{ index + 1 }}
      </button>
    </div>
    <NoteDetail v-if="props.active.type === 'note'" :note="props.active" embedded @answer="emit('open-note', props.active)" @locate="emit('open-note', props.active)" @restore="emit('open-note', props.active, true)" />
    <div v-else class="annotation-content">
      <blockquote class="annotation-source">{{ quote() || props.sourceText }}</blockquote>
      <div class="md" v-html="props.renderMarkdown(props.active.content)"></div>
    </div>
    <footer>
      <button v-if="props.active.type === 'highlight' && shareHighlight" @click="shareHighlight(props.active)">生成分享图</button>
      <button @click="emit('edit', props.active)">编辑</button>
      <button @click="emit('delete', props.active)">删除</button>
      <button v-if="props.active.type === 'note' && props.active.note_placement?.kind === 'lid_block' && !props.active.selection_context"
        @click="emit('place', props.active)">移动</button>
      <button v-if="props.active.anchor.lid" @click="emit('source', { lid: props.active.anchor.lid, quote: quote() })">查看来源</button>
      <button @click="emit('show-notes', props.active)">在笔记中查看</button>
    </footer>
  </section>
</template>
