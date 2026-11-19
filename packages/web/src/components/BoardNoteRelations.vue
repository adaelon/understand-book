<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowRight, Link2, LoaderCircle, Pencil, Plus, Trash2 } from '@lucide/vue';
import { api as sharedApi } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import { t as uiText } from '../i18n';
import type { BoardItem } from '../generated/BoardItem';

const props = defineProps<{ noteId: string; items: BoardItem[]; notesRevision?: number; selectedRelationId?: string | null; disabled?: boolean }>();
const emit = defineEmits<{ (event: 'changed'): void; (event: 'open-note', noteId: string): void }>();
const api = bindSceneApi(sharedApi, readerKey);
type Notebook = Awaited<ReturnType<typeof api.workspaceNotes>>;
type Relation = Notebook['relations'][number];
const notebook = ref<Notebook | null>(null);
const loading = ref(false), saving = ref(false), error = ref('');
const editorOpen = ref(false), editing = ref<Relation | null>(null);
const targetId = ref(''), label = ref(''), reason = ref('');
let sequence = 0;
const relations = computed(() => notebook.value?.relations.filter(relation => relation.from_note_id === props.noteId || relation.to_note_id === props.noteId) ?? []);
const targets = computed(() => {
  const seen = new Set<string>();
  return props.items.flatMap(item => {
    if (item.target.kind !== 'note' || item.target.missing || item.target.note_id === props.noteId || seen.has(item.target.note_id)) return [];
    seen.add(item.target.note_id);
    return [{ id: item.target.note_id, title: item.title }];
  });
});
const blocked = computed(() => !!props.disabled || saving.value);
function title(id: string) { return notebook.value?.notes.find(note => note.note_id === id)?.title ?? uiText('笔记已删除'); }
function message(failure: unknown) { return failure instanceof Error ? failure.message : String(failure); }
function edit(relation: Relation) {
  editing.value = relation; targetId.value = relation.to_note_id; label.value = relation.label; reason.value = relation.reason;
  editorOpen.value = true; error.value = '';
}
function start() { editing.value = null; targetId.value = targets.value[0]?.id ?? ''; label.value = ''; reason.value = ''; editorOpen.value = true; error.value = ''; }
function selected() { const relation = relations.value.find(relation => relation.relation_id === props.selectedRelationId); if (relation) edit(relation); }
async function load() {
  const current = ++sequence; loading.value = true; error.value = '';
  try { const result = await api.workspaceNotes(); if (current === sequence) { notebook.value = result; if (!editorOpen.value) selected(); } }
  catch (failure) { if (current === sequence) error.value = message(failure); }
  finally { if (current === sequence) loading.value = false; }
}
async function save() {
  if (blocked.value || !label.value.trim() || !reason.value.trim() || (!editing.value && !targetId.value)) return;
  saving.value = true; error.value = '';
  try {
    const relation = await api.workspaceNoteRelationSave({
      ...(editing.value ? { relation_id: editing.value.relation_id, expected_revision: editing.value.revision } : {}),
      from_note_id: editing.value?.from_note_id ?? props.noteId, to_note_id: editing.value?.to_note_id ?? targetId.value,
      label: label.value.trim(), reason: reason.value.trim(),
    });
    if (notebook.value) notebook.value.relations = [...notebook.value.relations.filter(item => item.relation_id !== relation.relation_id), relation];
    editorOpen.value = false; editing.value = null; emit('changed');
  } catch (failure) { error.value = message(failure); }
  finally { saving.value = false; }
}
async function remove(relation: Relation) {
  if (blocked.value) return;
  saving.value = true; error.value = '';
  try {
    await api.workspaceNoteRelationDelete({ relation_id: relation.relation_id, expected_revision: relation.revision });
    if (notebook.value) notebook.value.relations = notebook.value.relations.filter(item => item.relation_id !== relation.relation_id);
    if (editing.value?.relation_id === relation.relation_id) { editorOpen.value = false; editing.value = null; }
    emit('changed');
  } catch (failure) { error.value = message(failure); }
  finally { saving.value = false; }
}
watch(() => props.noteId, () => { notebook.value = null; editorOpen.value = false; void load(); }, { immediate: true });
watch(() => props.notesRevision, () => { void load(); });
watch(() => props.selectedRelationId, selected);
onBeforeUnmount(() => { sequence++; });
</script>

<template>
  <section class="note-relations" :aria-label="uiText('笔记关系')">
    <header><h3><Link2 :size="14" aria-hidden="true" />{{ uiText('笔记关系') }}</h3><button type="button" :disabled="blocked || loading || !targets.length" @click="start"><Plus :size="14" aria-hidden="true" />{{ uiText('建立关系') }}</button></header>
    <p class="note-relations-hint">{{ uiText('关系保存在学习空间，也会出现在其他使用这两条笔记的画板。') }}</p>
    <p v-if="loading && !notebook" class="note-relations-hint" role="status"><LoaderCircle :size="14" class="note-relations-spinner" aria-hidden="true" />{{ uiText('正在读取…') }}</p>
    <div v-for="relation in relations" :key="relation.relation_id" class="note-relation" :class="{ editing: editing?.relation_id === relation.relation_id && editorOpen }">
      <div class="note-relation-route"><button type="button" @click="emit('open-note', relation.from_note_id)">{{ title(relation.from_note_id) }}</button><ArrowRight :size="13" aria-hidden="true" /><button type="button" @click="emit('open-note', relation.to_note_id)">{{ title(relation.to_note_id) }}</button></div>
      <strong>{{ relation.label }}</strong><p>{{ relation.reason }}</p>
      <div class="note-relation-actions"><button type="button" :disabled="blocked" @click="edit(relation)"><Pencil :size="12" aria-hidden="true" />{{ uiText('编辑关系') }}</button><button type="button" :disabled="blocked" @click="remove(relation)"><Trash2 :size="12" aria-hidden="true" />{{ uiText('移除关系') }}</button></div>
    </div>
    <p v-if="!loading && notebook && !relations.length && !editorOpen" class="note-relations-empty">{{ targets.length ? uiText('选择另一条笔记，写下它们为什么相关。') : uiText('再向画板添加一条笔记，就可以建立关系。') }}</p>
    <form v-if="editorOpen" class="note-relation-form" @submit.prevent="save">
      <label v-if="!editing">{{ uiText('关联到') }}<select v-model="targetId" :disabled="blocked" required><option v-for="target in targets" :key="target.id" :value="target.id">{{ target.title }}</option></select></label>
      <p v-else class="note-relation-editing-title">{{ title(editing.from_note_id) }} → {{ title(editing.to_note_id) }}</p>
      <label>{{ uiText('关系名称') }}<input v-model="label" :disabled="blocked" :placeholder="uiText('例如：支持、反例、进一步解释')" maxlength="120" required /></label>
      <label>{{ uiText('关联理由') }}<textarea v-model="reason" :disabled="blocked" :placeholder="uiText('这两条笔记为什么相关？')" rows="3" required /></label>
      <div><button type="button" :disabled="saving" @click="editorOpen = false">{{ uiText('取消') }}</button><button class="primary" type="submit" :disabled="blocked || !targetId || !label.trim() || !reason.trim()">{{ saving ? uiText('正在保存…') : uiText('保存关系') }}</button></div>
    </form>
    <p v-if="error" class="note-relations-error" role="alert">{{ error }}<button type="button" :disabled="saving" @click="editorOpen = false; load()">{{ editorOpen ? uiText('放弃修改并重新读取') : uiText('重试') }}</button></p>
  </section>
</template>

<style scoped>
.note-relations{margin-top:28px;padding-top:20px;border-top:1px solid var(--hairline)}.note-relations header{display:flex;align-items:center;justify-content:space-between;gap:10px}.note-relations h3{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:550;margin:0;color:var(--steel)}.note-relations button{display:inline-flex;align-items:center;gap:5px;min-height:28px;border:0;padding:3px 0;border-radius:0;background:transparent;color:var(--brand-accent);font:inherit;font-size:11px;cursor:pointer}.note-relations button:disabled{opacity:.45;cursor:default}.note-relations-hint,.note-relations-empty{display:flex;align-items:center;gap:7px;font-size:11px;line-height:1.8;color:var(--stone);margin:10px 0 14px}.note-relation{padding:14px 0;border-bottom:1px solid var(--hairline-soft)}.note-relation.editing{border-bottom-color:var(--brand-accent)}.note-relation-route{display:flex;align-items:center;gap:7px;min-width:0;margin-bottom:5px;color:var(--stone)}.note-relation-route button{display:block;min-width:0;max-width:47%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--steel)}.note-relation-route svg{flex-shrink:0}.note-relation strong{font-size:12px;font-weight:550;color:var(--ink)}.note-relation p{font-size:12px;line-height:1.8;color:var(--steel);white-space:pre-wrap;overflow-wrap:anywhere;margin:6px 0}.note-relation-actions{display:flex;justify-content:flex-end;gap:15px}.note-relation-actions button{color:var(--stone)}.note-relation-form{display:flex;flex-direction:column;gap:12px;padding:15px;margin-top:12px;background:var(--surface-soft);border:1px solid var(--hairline);border-radius:8px}.note-relation-form label{display:flex;flex-direction:column;gap:6px;font-size:11px;color:var(--steel)}.note-relation-form input,.note-relation-form textarea,.note-relation-form select{box-sizing:border-box;min-width:0;width:100%;padding:9px;border:1px solid var(--hairline);border-radius:6px;background:var(--reader-card);color:var(--ink);font:inherit;font-size:12px;line-height:1.7}.note-relation-form textarea{resize:vertical;min-height:75px}.note-relation-form>div{display:flex;justify-content:flex-end;gap:12px}.note-relation-form .primary{padding:7px 11px;border-radius:6px;background:var(--brand-accent);color:white}.note-relation-editing-title{font-size:12px;line-height:1.7;overflow-wrap:anywhere;margin:0;color:var(--steel)}.note-relations-error{font-size:12px;line-height:1.8;color:var(--brand-error);overflow-wrap:anywhere}.note-relations-error button{margin-left:8px;color:inherit}.note-relations :is(button,input,textarea,select):focus-visible{outline:2px solid var(--brand-accent);outline-offset:2px}.note-relations-spinner{animation:note-relations-spin 1.2s linear infinite}@keyframes note-relations-spin{to{transform:rotate(360deg)}}@media(prefers-reduced-motion:reduce){.note-relations-spinner{animation:none}}
</style>
