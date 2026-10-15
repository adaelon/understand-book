<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { renderMarkdown } from '../md';
const props = defineProps<{ content: string; title: string; revision?: number; summary?: string; saving: boolean; error?: string;
  disabled?: boolean; legacy?: boolean; excerpt?: import('../generated/NoteExcerpt').NoteExcerpt | null }>();
const emit = defineEmits<{ (e: 'update:content', value: string): void; (e: 'save' | 'collapse' | 'discard'): void }>();
const preview = ref(false);
const composing = ref(false);
const canSave = computed(() => !props.saving && !props.disabled && (!!props.content.trim() || !!props.excerpt));
const viewport = ref({ top: 0, height: window.innerHeight });
function resize() { viewport.value = { top: window.visualViewport?.offsetTop ?? 0, height: window.visualViewport?.height ?? window.innerHeight }; }
onMounted(() => { resize(); window.visualViewport?.addEventListener('resize', resize); window.visualViewport?.addEventListener('scroll', resize); window.addEventListener('resize', resize); });
onBeforeUnmount(() => { window.visualViewport?.removeEventListener('resize', resize); window.visualViewport?.removeEventListener('scroll', resize); window.removeEventListener('resize', resize); });
function keydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && !e.isComposing && !composing.value && e.keyCode !== 229 && canSave.value) { e.preventDefault(); emit('save'); }
}
</script>
<template>
  <section class="note-editor-panel" role="region" aria-label="笔记编辑" :style="{ '--note-top': `${viewport.top}px`, '--note-height': `${viewport.height}px` }">
    <header><strong>{{ legacy ? '阅读笔记' : '记下你的想法' }}</strong><button @click="emit('collapse')">收起</button></header>
    <p class="note-context">{{ title }}<span v-if="revision"> · 版本 {{ revision }}</span></p>
    <p v-if="summary" class="note-context">{{ summary }}</p>
    <div v-if="excerpt" class="note-excerpt"><strong>{{ excerpt.kind === 'original' ? '原文摘录' : '助手摘录' }}</strong><div class="md" v-html="renderMarkdown(excerpt.text)"></div></div>
    <textarea :value="content" :readonly="saving || disabled" :aria-label="legacy ? '阅读笔记' : '记下你的想法'" placeholder="支持 Markdown 和公式" @input="emit('update:content', ($event.target as HTMLTextAreaElement).value)" @compositionstart="composing = true" @compositionend="composing = false" @keydown="keydown" />
    <p v-if="error" role="alert">{{ error }}</p>
    <button @click="preview = !preview">{{ preview ? '收起预览' : '预览' }}</button>
    <div v-if="preview" class="md" v-html="renderMarkdown(content)"></div>
    <footer><button :disabled="saving || disabled" @click="emit('discard')">放弃</button><button class="primary" :disabled="!canSave" @click="emit('save')">{{ saving ? '正在保存…' : '保存笔记' }}</button></footer>
  </section>
</template>
<style scoped>
.note-editor-panel { position: fixed; z-index: 120; right: 16px; top: 92px; bottom: 24px; width: min(380px, calc(100vw - 32px)); box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; padding: 20px; background: var(--canvas); color: var(--ink); border: 1px solid var(--line); border-radius: 14px; box-shadow: 0 8px 32px #0002; overflow: auto; }
header, footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.note-context { margin: 0; color: var(--steel); overflow-wrap: anywhere; }
textarea { flex: 1 0 140px; min-height: 140px; width: 100%; box-sizing: border-box; resize: vertical; font: inherit; line-height: 1.7; color: inherit; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 12px; }
button { min-height: 36px; cursor: pointer; }
.md { overflow-wrap: anywhere; }
.note-excerpt { flex: 0 1 auto; min-height: 48px; max-height: 25%; overflow: auto; }
header, footer { flex-shrink: 0; }
@media(max-width:600px) { .note-editor-panel { top: calc(var(--note-top) + 8px); height: calc(var(--note-height) - 16px); bottom: auto; right: 8px; width: calc(100vw - 16px); padding: 14px; gap: 8px; } textarea { flex: 1 1 100px; min-height: 60px; resize: none; font-size: 16px; } footer { position: sticky; bottom: 0; background: var(--canvas); } button { min-height: 44px; } }
</style>
