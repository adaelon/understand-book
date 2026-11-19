<script setup lang="ts">
import { computed, ref } from 'vue';
import { X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import type { FollowUpNode, FollowUpStart } from '../follow-up-conversations';

const props = defineProps<{
  nodes: FollowUpNode[];
  current: string;
  selected: string;
  drafts: FollowUpStart[];
  busy?: boolean;
  error?: string;
  layout: 'side' | 'tray' | 'sheet';
}>();
const emit = defineEmits<{
  select: [id: string];
  enter: [id: string];
  start: [start: FollowUpStart];
  'update-question': [text: string];
  cancel: [];
  close: [];
  retry: [];
}>();

interface Row { id: string; parent?: string; label: string; pending?: boolean; awaitingAnswer?:boolean }
const rows = computed(() => {
  const nodes: Row[] = props.nodes.map(n => ({ id:n.session_id, parent:n.origin?.parent_session_id, awaitingAnswer:n.status === 'pending',
    label:n.origin?.anchor.excerpt || n.title || n.question || uiText('原问题') }));
  for (const draft of props.drafts) nodes.push({ id:'draft:' + draft.client_request_id, parent:draft.parent_session_id,
    label:draft.anchor.excerpt, pending:true });
  const ids = new Set(nodes.map(n => n.id));
  const children = new Map<string, Row[]>();
  for (const n of nodes) {
    const parent = n.parent && ids.has(n.parent) ? n.parent : '';
    const siblings = children.get(parent);
    if (siblings) siblings.push(n); else children.set(parent,[n]);
  }
  const ordered: { row:Row; guides:boolean[]; last:boolean }[] = [];
  function visit(parent: string, guides:boolean[]) {
    const siblings = children.get(parent) || [];
    siblings.forEach((row,i) => {
      const last = i === siblings.length - 1;
      ordered.push({row,guides,last});
      visit(row.id,[...guides,!last]);
    });
  }
  visit('',[]);
  return ordered;
});
const selectedNode = computed(() => props.nodes.find(n => n.session_id === props.selected));
const editing = computed(() => props.drafts.find(draft => 'draft:' + draft.client_request_id === props.selected));
function selectRow(row:Row) {
  emit('select',row.id);
  if (row.awaitingAnswer) emit('retry');
}
const navigation = ref<HTMLElement>();
function focusNode(id:string) {
  const buttons = Array.from(navigation.value?.querySelectorAll<HTMLButtonElement>('button[data-session-id]') || []);
  (buttons.find(button => button.dataset.sessionId === id) || buttons[0])?.focus({preventScroll:true});
}
defineExpose({focusNode});
</script>

<template>
  <section class="follow-up-path-panel" :class="'path-'+layout" :aria-label="uiText('追问路径')">
    <header v-if="layout !== 'sheet'"><h3>{{ uiText('追问路径') }}</h3>
      <button v-if="layout === 'tray'" class="path-close" type="button" :aria-label="uiText('收起追问路径')" @click="emit('close')"><X :size="16" aria-hidden="true" /></button>
    </header>
    <nav ref="navigation" :aria-label="uiText('从回答展开的问题')">
      <ul><li v-for="{row,guides,last} in rows" :key="row.id">
        <span aria-hidden="true" class="indent"><i v-for="(continues,i) in guides.slice(-6)" :key="i"
          :class="{ vertical:continues && i<Math.min(6,guides.length)-1, joint:i===Math.min(6,guides.length)-1, last, proposed:row.pending }" /></span>
        <button type="button" :data-session-id="row.id" :aria-current="row.id === current || undefined"
          :class="{ 'is-preview':row.id === selected, 'is-pending':row.pending }" :title="row.label" @click="selectRow(row)">
          <span class="point" aria-hidden="true" /><span class="node-name">{{ row.label }}</span>
          <small v-if="row.id === current">{{ uiText('当前') }}</small><small v-else-if="row.pending">{{ uiText('待追问') }}</small>
        </button>
      </li></ul>
    </nav>
    <div v-if="editing" class="follow-up-preview">
      <small>{{ uiText('待追问') }}</small>
      <p class="source-quote">{{ uiText('原句「{0}」', [editing.anchor.excerpt]) }}</p>
      <label>{{ uiText('我想问') }}<textarea rows="2" :value="editing.question" :disabled="busy"
        @input="emit('update-question',($event.target as HTMLTextAreaElement).value)" /></label>
      <div class="actions"><button type="button" class="primary" :disabled="busy || !editing.question.trim()" @click="emit('start',editing)">{{ uiText('开始这条追问') }}</button>
        <button type="button" class="discard" :disabled="busy" @click="emit('cancel')">{{ uiText('取消') }}</button></div>
    </div>
    <div v-else-if="selectedNode && (selected !== current || layout !== 'side' || selectedNode.status === 'unsent')" class="follow-up-preview" aria-live="polite">
      <small>{{ selected === current ? uiText('当前对话') : uiText('对话预览') }}</small>
      <p class="preview-question">{{ selectedNode.question || selectedNode.title }}</p>
      <p v-if="selectedNode.origin" class="source-quote">{{ uiText('原句「{0}」',[selectedNode.origin.anchor.excerpt]) }}</p>
      <p v-if="selectedNode.answer" class="preview-answer">{{ selectedNode.answer }}</p>
      <p v-else-if="selectedNode.status === 'pending'" class="source-quote">{{ uiText('等待回答') }}</p>
      <p v-else-if="selectedNode.status === 'failed'" class="source-quote">{{ uiText('这次回答未完成') }}</p>
      <p v-else-if="selectedNode.status === 'unsent'" class="source-quote">{{ uiText('首问尚未发送') }}</p>
      <div v-if="selected !== current || selectedNode.status === 'unsent'" class="actions">
        <button v-if="selected !== current" type="button" class="primary" :disabled="busy" @click="emit('enter',selected)">{{ uiText('进入这条对话') }}</button>
        <button v-else-if="selectedNode.origin && selectedNode.status === 'unsent'" type="button" class="primary" :disabled="busy" @click="emit('start',selectedNode.origin)">{{ uiText('发送首问') }}</button>
      </div>
    </div>
    <p v-if="error" class="path-error" role="status">{{ error }} <button type="button" @click="emit('retry')">{{ uiText('刷新') }}</button></p>
  </section>
</template>

<style scoped>
.follow-up-path-panel { --path-wash:color-mix(in srgb,var(--brand-accent) 10%,var(--canvas)); min-width:0; display:flex; flex-direction:column; gap:8px; color:var(--ink); font:inherit; text-align:start; }
header,.actions,li,li button { display:flex; align-items:center; }
header { justify-content:space-between; }
h3 { margin:0; font-size:.92rem; font-weight:600; }
ul { margin:0; padding:0; list-style:none; }
li { min-width:0; }
.indent { display:inline-flex; align-self:stretch; flex:none; }
.indent i { position:relative; width:15px; }
.indent .vertical::before,.indent .joint::before,.indent .joint::after { content:""; position:absolute; left:8px; top:0; bottom:0; border-left:1px solid var(--line); }
.indent .joint.last::before { bottom:50%; }
.indent .joint::after { top:50%; bottom:auto; width:12px; border:0; border-top:1px solid var(--line); }
.indent .joint.proposed::after { border-top-style:dashed; }
button { font:inherit; cursor:pointer; }
li button { flex:1; position:relative; min-width:0; min-height:30px; gap:7px; border:0; border-radius:6px; padding:4px; background:transparent; color:var(--ink); text-align:left; font-size:.82rem; }
li button:is(.is-preview,:hover) { background:var(--path-wash); }
.point { flex:none; display:block; width:8px; height:8px; border:1px solid var(--steel); border-radius:50%; background:var(--canvas); }
[aria-current="true"] .point { border-color:var(--brand-accent); background:var(--brand-accent); }
.is-pending .point { border-style:dashed; }
.node-name { min-width:0; overflow:hidden; white-space:nowrap; text-overflow:ellipsis; }
.follow-up-path-panel small,.source-quote { color:var(--steel); font-size:.75rem; }
li small { flex:none; margin-inline-start:auto; }
.follow-up-preview { display:grid; gap:4px; border-top:1px solid var(--line); padding-top:9px; overflow-wrap:anywhere; }
.follow-up-preview p,.path-error { margin:0; }
.preview-question { font-size:.92rem; font-weight:550; line-height:1.45; }
.preview-answer { font-size:.84rem; line-height:1.6; white-space:pre-line; }
.source-quote { line-height:1.5; }
label { display:grid; gap:3px; font-size:.8rem; }
textarea { box-sizing:border-box; resize:vertical; width:100%; min-height:48px; padding:6px 8px; font:inherit; color:var(--ink); border:1px solid var(--line); background:var(--canvas); border-radius:7px; }
.actions { gap:8px; margin-top:4px; flex-wrap:wrap; }
.actions button,.path-error button { border:0; padding:7px 10px; border-radius:7px; font-size:.82rem; }
.primary { background:var(--brand-accent-deep); color:var(--brand-on-accent); }
.discard,.path-close,.path-error button { color:var(--steel); background:transparent; }
.path-close { display:grid; place-items:center; width:32px; height:32px; border:0; border-radius:6px; }
button:disabled { cursor:default; opacity:.6; }
button:focus-visible,textarea:focus-visible { outline:2px solid var(--brand-accent); outline-offset:1px; }
.path-sheet li button,.path-sheet .actions button { min-height:44px; }
.path-sheet textarea { font-size:16px; }
.path-sheet .indent i { width:20px; }
.path-error { font-size:.84rem; }
</style>
