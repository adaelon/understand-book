<script setup lang="ts">
import { ref } from 'vue';
import { openPersonalNotes } from '../personal-notes';
import { removeThoughtPlacement } from '../note-placements';
import { renderMarkdown } from '../md';
import type { NoteAttachmentView } from '../generated/NoteAttachmentView';
const props = defineProps<{ item: NoteAttachmentView; missingQuote?: boolean }>();
const error = ref(''), removing = ref(false);
async function remove() {
  removing.value = true; error.value = '';
  try { await removeThoughtPlacement(props.item.attachment.attachment_id); }
  catch (failure) { error.value = (failure as Error).message; }
  finally { removing.value = false; }
}
</script>
<template>
  <aside class="attached-thought" data-note-attachment :data-attachment-id="item.attachment.attachment_id">
    <div class="attached-heading"><span>我的想法</span><button @click="openPersonalNotes({note_id:item.note.note_id})">打开 · 第 {{ item.note.current_revision }} 版 ↗</button></div>
    <blockquote v-if="item.attachment.target.kind === 'message' && item.attachment.target.quote">{{ item.attachment.target.quote }}</blockquote>
    <p v-if="missingQuote" class="attachment-hint">原句位置已变化，想法仍保留在这条消息下。</p>
    <details><summary>{{ item.note.value.title || item.note.value.record.content.split('\n')[0]?.slice(0, 90) || '保留的想法' }}</summary><div class="md" v-html="renderMarkdown(item.note.value.record.content)"></div><button class="remove-placement" :disabled="removing" @click="remove">{{ removing ? '正在移除…' : '从这里移除' }}</button></details>
    <p v-if="error" role="alert">{{ error }}</p>
  </aside>
</template>
<style scoped>
.attached-thought{margin:10px 0 14px;padding:10px 13px;background:#fbf5e8;border:1px solid #e6d8be;border-left:3px solid #b88655;border-radius:4px 10px 10px 4px;color:#594b39;font:13px/1.7 var(--font-body,inherit);white-space:normal;overflow-wrap:anywhere}.attached-heading{display:flex;align-items:center;justify-content:space-between;gap:8px;font-size:10px;color:#9b7a54}.attached-thought button{border:0;background:none;color:#89643e;font:inherit;padding:2px;cursor:pointer}.attached-thought blockquote{margin:5px 0;font-size:11px;color:#97856e;border:0;padding:0}.attached-thought summary{cursor:pointer;margin-top:5px}.attached-thought .md{margin-top:10px}.attached-thought .md :deep(pre){overflow:auto}.attachment-hint{font-size:11px;color:#99764e}.attached-thought .remove-placement{font-size:11px;margin-top:5px}
</style>
