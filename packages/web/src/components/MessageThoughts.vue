<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { activeNoteMessage, registerNoteDestinations, startThoughtHere, thoughtSurfaceVisible, useNoteAttachments, type NoteDestination } from '../note-placements';
import type { NoteMessageRole } from '../generated/NoteMessageRole';
import AttachedThought from './AttachedThought.vue';
import { registerThoughtContext } from '../personal-notes';
const props = defineProps<{ sessionId: string; turnId: string; role: NoteMessageRole; text: string }>();
const content = ref<HTMLElement>(), quote = ref<string | null>(null), occurrence = ref(0);
const key = computed(() => `${props.sessionId}:${props.turnId}:${props.role}`);
const { items, error, retry } = useNoteAttachments(() => props.sessionId);
const attached = computed(() => items.value.filter(item => item.attachment.target.kind === 'message' && item.attachment.target.turn_id === props.turnId && item.attachment.target.role === props.role));
const mounts = ref<Record<string, HTMLElement>>({});
function visible() { return thoughtSurfaceVisible(content.value); }
function destination(): NoteDestination {
  return { kind: 'attachment', label: quote.value ? `对话 · ${quote.value.slice(0, 50)}` : props.role === 'user' ? '对话 · 我的表达' : '对话 · 助手回答',
    target: { kind: 'message', session_id: props.sessionId, turn_id: props.turnId, role: props.role, quote: quote.value, occurrence: occurrence.value } };
}
const unregister = registerNoteDestinations(() => {
  if (!props.sessionId || !props.turnId || activeNoteMessage.value !== key.value || !visible()) return [];
  const frozen = destination();
  return [{ key: key.value, label: quote.value ? '选中的这句话下面' : '这条消息下面', detail: quote.value || props.text.slice(0, 70), resolve: () => frozen }];
});
const unregisterContext = registerThoughtContext(() => {
  if (!props.sessionId || !props.turnId || activeNoteMessage.value !== key.value || !visible()) return null;
  const location = destination();
  return {session_id:props.sessionId,turn_id:props.turnId,label:location.label,origin:location.kind === 'attachment' ? location.target : undefined};
}, 40);
function select(event: Event) {
  if ((event.target as HTMLElement | null)?.closest('.agent-presentation,[data-note-attachment]')) return;
  activeNoteMessage.value = key.value;
  const selection = window.getSelection();
  if (!selection?.rangeCount || selection.isCollapsed || !content.value) return;
  const range = selection.getRangeAt(0);
  if (!content.value.contains(range.startContainer) || !content.value.contains(range.endContainer)) return;
  const selected = range.cloneContents(); selected.querySelectorAll('[data-note-attachment],.thought-inline-mount').forEach(node => node.remove());
  const text = (selected.textContent ?? '').trim(); if (!text) return;
  const before = document.createRange(); before.selectNodeContents(content.value); before.setEnd(range.startContainer, range.startOffset);
  const fragment = before.cloneContents(); fragment.querySelectorAll('[data-note-attachment]').forEach(node => node.remove());
  quote.value = text; occurrence.value = (fragment.textContent ?? '').split(text).length - 1;
}
function here() { startThoughtHere(destination()); }
function clearMounts() { Object.values(mounts.value).forEach(node => node.remove()); mounts.value = {}; }
async function position() {
  clearMounts(); await nextTick(); if (!content.value) return;
  const nodes: Text[] = []; const walker = document.createTreeWalker(content.value, NodeFilter.SHOW_TEXT, { acceptNode: node => (node.parentElement?.closest('[data-note-attachment],.thought-inline-mount,button,iframe,script,style') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT) });
  while (walker.nextNode()) nodes.push(walker.currentNode as Text);
  const full = nodes.map(node => node.textContent).join('');
  for (const item of attached.value) {
    const target = item.attachment.target; if (target.kind !== 'message' || !target.quote) continue;
    let index = -1;
    for (let n = 0; n <= target.occurrence; n++) { index = full.indexOf(target.quote, index + 1); if (index < 0) break; }
    if (index < 0) continue;
    let end = index + target.quote.length, node: Text | undefined;
    for (const candidate of nodes) { if (end <= candidate.length) { node = candidate; break; } end -= candidate.length; }
    const block = node?.parentElement?.closest('p,li,blockquote,pre,h1,h2,h3,h4') ?? node?.parentElement;
    if (!block || !content.value.contains(block)) continue;
    const mount = document.createElement('div'); mount.className = 'thought-inline-mount'; mount.setAttribute('data-reader-selection-ignore',''); block.after(mount);
    mounts.value[item.attachment.attachment_id] = mount;
  }
}
watch([attached, () => props.text], position, { immediate: true, flush: 'post' });
onBeforeUnmount(() => { unregister(); unregisterContext(); clearMounts(); if (activeNoteMessage.value === key.value) activeNoteMessage.value = ''; });
</script>
<template>
  <div class="message-thoughts" :data-note-message="key">
    <div ref="content" class="message-content" @pointerup="select" @keyup="select"><slot /></div>
    <div v-if="sessionId && turnId" class="message-thought-tools"><button type="button" @click="here">{{ quote ? '在这句话下记想法' : '在这里记想法' }}</button><button v-if="quote" aria-label="取消选中的原句" @click="quote=null;occurrence=0">×</button></div>
    <template v-for="item in attached" :key="item.attachment.attachment_id"><Teleport v-if="mounts[item.attachment.attachment_id]" :to="mounts[item.attachment.attachment_id]!"><AttachedThought :item="item" /></Teleport><AttachedThought v-else :item="item" :missing-quote="item.attachment.target.kind === 'message' && !!item.attachment.target.quote" /></template>
    <p v-if="error" class="attachment-error">想法位置暂未读取。<button @click="retry">重试</button></p>
  </div>
</template>
<style scoped>
.message-thoughts{min-width:0}.message-thought-tools{display:flex;gap:5px;margin:3px 0 8px}.message-thought-tools button{border:0;background:none;color:#9b805c;padding:3px 0;font:11px/1.6 inherit;cursor:pointer}.message-thought-tools button:hover{color:#785535;text-decoration:underline}.attachment-error{font-size:11px;color:#99764e}
</style>
