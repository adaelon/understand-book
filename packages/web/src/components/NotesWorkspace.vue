<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowLeft, ArrowUpRight, BookOpen, Check, ChevronRight, FilePenLine, LayoutDashboard, Link2, LoaderCircle, MessageSquareText, NotebookPen, Pencil, Plus, Search, Trash2, X } from '@lucide/vue';
import { api as sharedApi } from '../api';
import { bindSceneApi, readerKey } from '../reader-context';
import { t as uiText } from '../i18n';
import { boardDate } from '../board-workspace';
import { renderMarkdown } from '../md';
import type { WorkspaceNote } from '../generated/WorkspaceNote';
import type { WorkspaceNoteRelation } from '../generated/WorkspaceNoteRelation';

const props = defineProps<{ initialNoteId?: string | null; learningWorkspaceId?: string | null; readonly?: boolean }>();
const emit = defineEmits<{
  (event: 'close'): void;
  (event: 'open-board', request: { noteId: string }): void;
  (event: 'ask', request: { noteId: string; title: string; content: string }): void;
  (event: 'source', request: { note_id: string; source_ref_id: string }, anchor: HTMLElement): void;
  (event: 'changed'): void;
}>();
const api = bindSceneApi(sharedApi, readerKey);
type Draft = { title: string; content: string; revision: number | null; error: string };
type RelationDraft = { relationId?: string; revision?: number; fromNoteId: string; toNoteId: string; label: string; reason: string; error: string };
type Confirmation = { kind: 'delete-note' | 'delete-relation' | 'discard'; id: string };
const NEW_NOTE = '__new_note__';
const root = ref<HTMLElement>();
const notes = ref<WorkspaceNote[]>([]), relations = ref<WorkspaceNoteRelation[]>([]);
const drafts = ref<Record<string, Draft>>({}), relationDrafts = ref<Record<string, RelationDraft>>({});
const deletedDraftIds = new Set<string>();
const selectedId = ref<string | null>(null), query = ref(''), preview = ref(!!props.readonly), mobileList = ref(true);
const loading = ref(false), loaded = ref(false), loadError = ref(''), notice = ref('');
const savingId = ref<string | null>(null), mutatingRelation = ref(false), deleting = ref(false);
const confirmation = ref<Confirmation | null>(null), confirmationError = ref('');
const composing = ref(false);
let loadSequence = 0, disposed = false;
let saveTask: Promise<WorkspaceNote | null> | null = null;
let confirmationTrigger: HTMLElement | null = null;

const selected = computed(() => notes.value.find(note => note.note_id === selectedId.value) ?? null);
const draft = computed(() => selectedId.value ? drafts.value[selectedId.value] ?? null : null);
const relationDraft = computed(() => selectedId.value ? relationDrafts.value[selectedId.value] ?? null : null);
const newDraft = computed(() => drafts.value[NEW_NOTE]);
const busy = computed(() => !!savingId.value || deleting.value || mutatingRelation.value);
const dirty = computed(() => !!selectedId.value && isDirty(selectedId.value));
const canSave = computed(() => !props.readonly && !!draft.value?.title.trim() && !!draft.value.content.trim() && dirty.value && !busy.value);
const outgoing = computed(() => relations.value.filter(relation => relation.from_note_id === selectedId.value));
const incoming = computed(() => relations.value.filter(relation => relation.to_note_id === selectedId.value));
const candidates = computed(() => notes.value.filter(note => note.note_id !== selectedId.value));
const filteredNotes = computed(() => {
  const term = query.value.trim().toLocaleLowerCase();
  return [...notes.value].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at)).filter(note => {
    const local = drafts.value[note.note_id];
    return !term || `${local?.title ?? note.title}\n${local?.content ?? note.content}`.toLocaleLowerCase().includes(term);
  });
});
const renderedContent = computed(() => preview.value ? renderMarkdown(draft.value?.content ?? '') : '');
const relationGroups = computed(() => [
  { title: uiText('从这条笔记出发'), incoming: false, relations: outgoing.value },
  { title: uiText('指向这条笔记'), incoming: true, relations: incoming.value },
]);

function message(error: unknown) { return error instanceof Error ? error.message : String(error); }
function noteTitle(id: string) { return drafts.value[id]?.title.trim() || notes.value.find(note => note.note_id === id)?.title || uiText('未命名笔记'); }
function excerpt(content: string) { return content.replace(/[#*`_>\[\]]/g, '').replace(/\s+/g, ' ').trim().slice(0, 130); }
function isDirty(id: string) {
  const local = drafts.value[id];
  if (!local) return false;
  const saved = notes.value.find(note => note.note_id === id);
  return saved ? local.title !== saved.title || local.content !== saved.content : !!local.title.trim() || !!local.content.trim();
}
function ensureDraft(note: WorkspaceNote) {
  if (!drafts.value[note.note_id]) drafts.value[note.note_id] = { title: note.title, content: note.content, revision: note.revision, error: '' };
}
function authorLabel(author: WorkspaceNote['author']) {
  return author === 'assistant' ? uiText('Agent 写作') : author === 'user' ? uiText('我写的') : uiText('原有笔记');
}
function selectNote(id: string) {
  const note = notes.value.find(item => item.note_id === id);
  if (note) ensureDraft(note);
  if (!note && id !== NEW_NOTE) return;
  selectedId.value = id;
  mobileList.value = false;
  notice.value = '';
}
async function newNote() {
  if (props.readonly) return;
  drafts.value[NEW_NOTE] ??= { title: '', content: '', revision: null, error: '' };
  selectNote(NEW_NOTE); preview.value = false;
  await nextTick(); root.value?.querySelector<HTMLTextAreaElement>('.notes-title-input')?.focus();
}
async function refresh() {
  const sequence = ++loadSequence;
  loading.value = true; loadError.value = '';
  try {
    const result = await api.workspaceNotes(props.learningWorkspaceId);
    if (disposed || sequence !== loadSequence) return;
    const savedIds = new Set(result.notes.map(note => note.note_id));
    for (const note of notes.value) if (!savedIds.has(note.note_id) && isDirty(note.note_id)) {
      // A second tab or Agent can delete a note while this editor has a draft.
      // Keep that draft reachable until it is copied into a new note or discarded.
      deletedDraftIds.add(note.note_id);
      drafts.value[note.note_id].error = uiText('这条笔记已被删除，草稿已保留。可以另存为新笔记。');
      result.notes.push(note);
    }
    // Refresh untouched editors, while keeping each unsaved draft and its base revision.
    for (const note of result.notes) {
      if (drafts.value[note.note_id] && !isDirty(note.note_id)) {
        drafts.value[note.note_id] = { title: note.title, content: note.content, revision: note.revision, error: '' };
      }
    }
    notes.value = result.notes; relations.value = result.relations;
    if (selectedId.value && selectedId.value !== NEW_NOTE && !notes.value.some(note => note.note_id === selectedId.value)) selectedId.value = null;
    loaded.value = true;
    const requested = props.initialNoteId;
    if (!selectedId.value && requested && notes.value.some(note => note.note_id === requested)) selectNote(requested);
    else if (!selectedId.value && notes.value.length) {
      selectNote([...notes.value].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))[0].note_id);
      mobileList.value = true;
    }
    if (requested && !notes.value.some(note => note.note_id === requested)) loadError.value = uiText('这条笔记已不存在，可以打开其他笔记。');
  } catch (error) {
    if (!disposed && sequence === loadSequence) loadError.value = uiText('笔记暂时没有读到：{0}', [message(error)]);
  } finally { if (sequence === loadSequence) loading.value = false; }
}
function markEdited() { if (draft.value) draft.value.error = ''; notice.value = ''; }
async function resizeTitle() {
  await nextTick();
  const title = root.value?.querySelector<HTMLTextAreaElement>('.notes-title-input');
  if (title) { title.style.height = 'auto'; title.style.height = `${title.scrollHeight}px`; }
}
function editTitle() {
  if (draft.value) draft.value.title = draft.value.title.replace(/[\r\n]+/g, ' ');
  markEdited(); void resizeTitle();
}
async function saveNote(id = selectedId.value): Promise<WorkspaceNote | null> {
  if (!id) return null;
  if (saveTask) { await saveTask; return saveNote(id === NEW_NOTE && selectedId.value !== NEW_NOTE ? selectedId.value : id); }
  const local = drafts.value[id], existing = notes.value.find(note => note.note_id === id);
  if (!local) return existing ?? null;
  if (!isDirty(id)) return existing ?? null;
  if (props.readonly) { local.error = uiText('这个学习空间已归档，笔记仅供阅读。'); return null; }
  if (!local.title.trim() || !local.content.trim()) {
    local.error = uiText('请写下标题和正文，再保存笔记。');
    selectNote(id); return null;
  }
  savingId.value = id; local.error = ''; notice.value = '';
  const title = local.title.trim(), content = local.content;
  const task = (async () => {
    try {
      const note = id === NEW_NOTE
        ? await api.workspaceNoteCreate({ title, content }, props.learningWorkspaceId)
        : await api.workspaceNoteUpdate({ note_id: id, expected_revision: local.revision!, title, content }, props.learningWorkspaceId);
      if (disposed) return note;
      const index = notes.value.findIndex(item => item.note_id === note.note_id);
      if (index === -1) notes.value.unshift(note); else notes.value[index] = note;
      drafts.value[note.note_id] = { title: note.title, content: note.content, revision: note.revision, error: '' };
      if (id === NEW_NOTE) {
        delete drafts.value[NEW_NOTE];
        if (selectedId.value === NEW_NOTE) selectedId.value = note.note_id;
      }
      notice.value = uiText('笔记已保存');
      emit('changed');
      return note;
    } catch (error) {
      if (!disposed) local.error = uiText('保存失败，草稿已保留：{0}', [message(error)]);
      return null;
    } finally { savingId.value = null; saveTask = null; }
  })();
  saveTask = task;
  return task;
}
async function saveAsNew() {
  const originalId = selectedId.value;
  if (!originalId || !draft.value || busy.value) return;
  if (newDraft.value && isDirty(NEW_NOTE)) { draft.value.error = uiText('请先保存列表中的新笔记草稿，再另存这条笔记。'); return; }
  drafts.value[NEW_NOTE] = { ...draft.value, revision: null, error: '' };
  const note = await saveNote(NEW_NOTE);
  if (note) {
    delete drafts.value[originalId];
    if (deletedDraftIds.has(originalId)) {
      notes.value = notes.value.filter(item => item.note_id !== originalId);
      deletedDraftIds.delete(originalId);
    }
    selectNote(note.note_id);
  }
}
async function beforeLeave() {
  if (deleting.value || mutatingRelation.value) return false;
  if (saveTask && !await saveTask) return false;
  for (const id of Object.keys(drafts.value)) {
    if (isDirty(id) && !await saveNote(id)) { selectNote(id); return false; }
  }
  for (const [id, pending] of Object.entries(relationDrafts.value)) {
    if (!pending.label.trim() && !pending.reason.trim() && !pending.toNoteId) { delete relationDrafts.value[id]; continue; }
    if (!await saveRelation(id)) { selectNote(id); return false; }
  }
  return true;
}
async function close() { if (await beforeLeave()) emit('close'); }
async function openBoard() { if (await beforeLeave() && selected.value) emit('open-board', { noteId: selected.value.note_id }); }
async function ask() {
  if (await beforeLeave() && selected.value) emit('ask', { noteId: selected.value.note_id, title: selected.value.title, content: selected.value.content });
}
async function startRelation(relation?: WorkspaceNoteRelation) {
  if (!selected.value || props.readonly) return;
  const id = selected.value.note_id;
  if (relationDrafts.value[id]) return;
  relationDrafts.value[id] = relation ? {
    relationId: relation.relation_id, revision: relation.revision, fromNoteId: relation.from_note_id,
    toNoteId: relation.to_note_id, label: relation.label, reason: relation.reason, error: '',
  } : { fromNoteId: id, toNoteId: '', label: '', reason: '', error: '' };
  await nextTick(); root.value?.querySelector<HTMLElement>('.notes-relation-form select, .notes-relation-form input')?.focus();
}
function cancelRelation() { if (selectedId.value && !mutatingRelation.value) delete relationDrafts.value[selectedId.value]; }
async function saveRelation(id = selectedId.value): Promise<boolean> {
  const pending = id ? relationDrafts.value[id] : null;
  if (!pending || mutatingRelation.value) return !pending;
  if (props.readonly) { pending.error = uiText('这个学习空间已归档，笔记仅供阅读。'); return false; }
  if (!pending.toNoteId || !pending.label.trim() || !pending.reason.trim()) {
    pending.error = uiText('请选择笔记，并写清关联名称和原因；也可以取消这次关联。'); return false;
  }
  mutatingRelation.value = true; pending.error = '';
  try {
    const relation = await api.workspaceNoteRelationSave({
      ...(pending.relationId ? { relation_id: pending.relationId, expected_revision: pending.revision } : {}),
      from_note_id: pending.fromNoteId, to_note_id: pending.toNoteId, label: pending.label.trim(), reason: pending.reason.trim(),
    }, props.learningWorkspaceId);
    if (disposed) return true;
    const index = relations.value.findIndex(item => item.relation_id === relation.relation_id);
    if (index < 0) relations.value.push(relation); else relations.value[index] = relation;
    delete relationDrafts.value[id!]; notice.value = uiText('关联已保存');
    emit('changed');
    return true;
  } catch (error) { if (!disposed) pending.error = uiText('关联未保存：{0}', [message(error)]); return false; }
  finally { mutatingRelation.value = false; }
}
async function confirmAction(kind: Confirmation['kind'], id: string) {
  confirmationTrigger = document.activeElement as HTMLElement;
  confirmation.value = { kind, id }; confirmationError.value = '';
  await nextTick(); root.value?.querySelector<HTMLElement>('.notes-confirmation .cancel')?.focus();
}
function dismissConfirmation() {
  if (deleting.value) return;
  confirmation.value = null; confirmationError.value = '';
  confirmationTrigger?.focus();
}
async function performConfirmed() {
  const action = confirmation.value;
  if (!action || busy.value) return;
  if (props.readonly && action.kind !== 'discard') return;
  deleting.value = true; confirmationError.value = '';
  try {
    if (action.kind === 'delete-note') {
      const note = notes.value.find(item => item.note_id === action.id);
      if (!note) return;
      await api.workspaceNoteDelete({ note_id: note.note_id, expected_revision: note.revision }, props.learningWorkspaceId);
      notes.value = notes.value.filter(item => item.note_id !== action.id);
      relations.value = relations.value.filter(item => item.from_note_id !== action.id && item.to_note_id !== action.id);
      delete drafts.value[action.id]; delete relationDrafts.value[action.id];
      selectedId.value = null;
      if (notes.value.length) selectNote(notes.value[0].note_id); else mobileList.value = true;
      notice.value = uiText('笔记已删除');
      emit('changed');
    } else if (action.kind === 'delete-relation') {
      const relation = relations.value.find(item => item.relation_id === action.id);
      if (!relation) return;
      await api.workspaceNoteRelationDelete({ relation_id: relation.relation_id, expected_revision: relation.revision }, props.learningWorkspaceId);
      relations.value = relations.value.filter(item => item.relation_id !== action.id);
      for (const [id, pending] of Object.entries(relationDrafts.value)) if (pending.relationId === action.id) delete relationDrafts.value[id];
      notice.value = uiText('关联已移除');
      emit('changed');
    } else {
      if (action.id === NEW_NOTE || deletedDraftIds.has(action.id)) {
        delete drafts.value[action.id]; selectedId.value = null;
        if (deletedDraftIds.delete(action.id)) notes.value = notes.value.filter(item => item.note_id !== action.id);
        if (notes.value.length) selectNote(notes.value[0].note_id); else mobileList.value = true;
      } else {
        // Fetch the latest saved version so a conflicted draft can actually recover.
        const note = await api.workspaceNote(action.id, props.learningWorkspaceId);
        const index = notes.value.findIndex(item => item.note_id === action.id);
        if (index < 0) notes.value.unshift(note); else notes.value[index] = note;
        drafts.value[action.id] = { title: note.title, content: note.content, revision: note.revision, error: '' };
      }
      notice.value = '';
    }
    confirmation.value = null;
  } catch (error) { confirmationError.value = message(error); }
  finally { deleting.value = false; }
}
function keyboard(event: KeyboardEvent) {
  if (confirmation.value) {
    if (event.key === 'Escape') { event.preventDefault(); dismissConfirmation(); }
    if (event.key === 'Tab') {
      const controls = [...(root.value?.querySelectorAll<HTMLElement>('.notes-confirmation button:not(:disabled)') ?? [])];
      const first = controls[0], last = controls[controls.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
  } else if ((event.ctrlKey || event.metaKey) && event.key === 'Enter' && !event.isComposing && !composing.value && event.keyCode !== 229 && canSave.value) {
    event.preventDefault(); void saveNote();
  }
}
function beforeUnload(event: BeforeUnloadEvent) {
  if (Object.keys(drafts.value).some(isDirty) || Object.keys(relationDrafts.value).length) { event.preventDefault(); event.returnValue = ''; }
}
watch(() => props.initialNoteId, id => { if (id && notes.value.some(note => note.note_id === id)) selectNote(id); });
watch(() => draft.value?.title, resizeTitle);
window.addEventListener('beforeunload', beforeUnload);
window.addEventListener('resize', resizeTitle);
onBeforeUnmount(() => { disposed = true; loadSequence++; window.removeEventListener('beforeunload', beforeUnload); window.removeEventListener('resize', resizeTitle); });
void refresh();
defineExpose({ beforeLeave, refresh });
</script>

<template>
  <section ref="root" class="notes-workspace" :aria-label="uiText('学习空间笔记')" @keydown="keyboard">
    <header class="notes-topbar">
      <button type="button" class="notes-icon-button" :aria-label="uiText('关闭笔记，回到学习')" :disabled="busy" @click="close"><ArrowLeft :size="19" aria-hidden="true" /></button>
      <div class="notes-brand"><NotebookPen :size="21" aria-hidden="true" /><h1>{{ uiText('笔记') }}</h1></div>
      <span class="notes-workspace-label">{{ uiText('学习空间') }}<ChevronRight :size="12" aria-hidden="true" />{{ uiText('共同积累的想法') }}</span>
      <button type="button" class="notes-button notes-primary" :disabled="readonly || busy || !loaded" @click="newNote"><Plus :size="16" aria-hidden="true" />{{ uiText('新建笔记') }}</button>
    </header>
    <div v-if="loadError" class="notes-load-error" role="alert"><span>{{ loadError }}</span><button type="button" :disabled="loading || busy" @click="refresh">{{ uiText('重新载入') }}</button></div>
    <p v-if="readonly" class="notes-notice" role="status">{{ uiText('这个学习空间已归档，笔记仅供阅读。') }}</p>
    <div class="notes-layout" :class="{ 'mobile-list': mobileList }">
      <aside class="notes-library" :aria-label="uiText('笔记列表')">
        <div class="notes-library-head"><span>{{ uiText('全部笔记') }}</span><span class="notes-count">{{ notes.length }}</span></div>
        <label class="notes-search"><Search :size="15" aria-hidden="true" /><input v-model="query" type="search" :aria-label="uiText('搜索笔记标题和正文')" :placeholder="uiText('搜索笔记…')" /></label>
        <div class="notes-list">
          <p v-if="loading && !loaded" class="notes-list-message" role="status"><LoaderCircle :size="16" class="notes-spinner" aria-hidden="true" />{{ uiText('正在读取笔记…') }}</p>
          <button v-if="newDraft" type="button" class="notes-list-item notes-new-draft" :class="{ selected: selectedId === NEW_NOTE }" @click="selectNote(NEW_NOTE)"><span class="notes-list-meta"><FilePenLine :size="13" aria-hidden="true" />{{ uiText('新笔记草稿') }}</span><strong>{{ newDraft.title.trim() || uiText('未命名笔记') }}</strong><span class="notes-list-excerpt">{{ excerpt(newDraft.content) || uiText('写下一个想法，让它继续生长。') }}</span></button>
          <button v-for="note in filteredNotes" :key="note.note_id" type="button" class="notes-list-item" :class="{ selected: selectedId === note.note_id }" :aria-current="selectedId === note.note_id ? 'true' : undefined" :data-note-id="note.note_id" @click="selectNote(note.note_id)">
            <span class="notes-list-meta"><span>{{ authorLabel(note.author) }}</span><span>{{ boardDate(note.updated_at) }}</span></span>
            <strong>{{ drafts[note.note_id]?.title || note.title }}<i v-if="isDirty(note.note_id)" class="notes-dirty-dot" :title="uiText('有未保存的修改')"></i></strong>
            <span class="notes-list-excerpt">{{ excerpt(drafts[note.note_id]?.content ?? note.content) }}</span>
          </button>
          <p v-if="loaded && !filteredNotes.length && query" class="notes-list-message">{{ uiText('没有找到相关笔记，试试其他关键词。') }}</p>
          <p v-else-if="loaded && !notes.length && !newDraft" class="notes-list-message">{{ uiText('你的想法会留在这里。') }}</p>
        </div>
        <footer class="notes-library-footer"><Link2 :size="13" aria-hidden="true" /><span>{{ uiText('同一份笔记，在书籍、对话和画板间复用。') }}</span></footer>
      </aside>

      <main v-if="draft" class="notes-main">
        <div class="notes-editor-toolbar">
          <button type="button" class="notes-mobile-back" @click="mobileList = true"><ArrowLeft :size="16" aria-hidden="true" />{{ uiText('全部笔记') }}</button>
          <div class="notes-editor-tabs" role="group" :aria-label="uiText('笔记编辑与预览')"><button type="button" :aria-pressed="!preview" :disabled="readonly" @click="preview = false">{{ uiText('编辑') }}</button><button type="button" :aria-pressed="preview" @click="preview = true">{{ uiText('预览') }}</button></div>
          <span class="notes-save-status" :class="{ unsaved: dirty }" role="status"><LoaderCircle v-if="savingId === selectedId" :size="13" class="notes-spinner" aria-hidden="true" /><Check v-else-if="!dirty && selected" :size="13" aria-hidden="true" />{{ savingId === selectedId ? uiText('正在保存…') : dirty ? uiText('有未保存的修改') : selected ? uiText('已保存') : uiText('草稿') }}</span>
          <button type="button" class="notes-button notes-save-button" :disabled="!canSave" @click="saveNote()">{{ uiText('保存笔记') }}</button>
        </div>
        <div v-if="draft.error" class="notes-editor-error" role="alert"><p>{{ draft.error }}</p><button v-if="selected" type="button" :disabled="busy" @click="saveAsNew">{{ uiText('另存为新笔记') }}</button></div>
        <p v-else-if="notice" class="notes-notice" role="status"><Check :size="13" aria-hidden="true" />{{ notice }}</p>
        <div class="notes-document-scroll">
          <div class="notes-document">
            <div class="notes-document-meta"><span><FilePenLine :size="13" aria-hidden="true" />{{ selected ? authorLabel(selected.author) : uiText('我的新想法') }}</span><span v-if="selected">{{ uiText('更新于 {0}', [boardDate(selected.updated_at)]) }}</span></div>
            <textarea v-model="draft.title" rows="1" class="notes-title-input" :aria-label="uiText('笔记标题')" :placeholder="uiText('给这个想法一个标题')" :readonly="readonly || busy" @input="editTitle" @compositionstart="composing = true" @compositionend="composing = false"></textarea>
            <div class="notes-document-rule"></div>
            <div v-if="preview" class="notes-prose md" v-html="renderedContent"></div>
            <textarea v-else v-model="draft.content" class="notes-content-input" :aria-label="uiText('笔记正文')" :placeholder="uiText('用自己的话，写下一个值得留下的想法。\n\n它说明了什么？为什么重要？还让你想到了什么？')" :readonly="readonly || busy" @input="markEdited" @compositionstart="composing = true" @compositionend="composing = false"></textarea>
            <div class="notes-editor-footnote"><span>{{ preview ? uiText('阅读全文，检查你的表达。') : uiText('支持 Markdown 和公式 · Ctrl / ⌘ + Enter 保存') }}</span><button v-if="dirty" type="button" :disabled="busy" @click="confirmAction('discard', selectedId!)">{{ uiText('放弃修改') }}</button></div>
          </div>
          <div class="notes-next-actions">
            <button type="button" class="notes-action" :disabled="readonly || busy || (!selected && !canSave)" @click="openBoard"><LayoutDashboard :size="18" aria-hidden="true" /><span><strong>{{ uiText('放到画板') }}</strong><small>{{ uiText('和其他想法一起展开') }}</small></span><ArrowUpRight :size="15" aria-hidden="true" /></button>
            <button type="button" class="notes-action" :disabled="readonly || busy || (!selected && !canSave)" @click="ask"><MessageSquareText :size="18" aria-hidden="true" /><span><strong>{{ uiText('和 Agent 继续想') }}</strong><small>{{ uiText('解释、追问，或寻找联系') }}</small></span><ArrowUpRight :size="15" aria-hidden="true" /></button>
          </div>
        </div>
      </main>
      <main v-else class="notes-empty">
        <div class="notes-empty-illustration" aria-hidden="true"><div class="notes-empty-sheet back"></div><div class="notes-empty-sheet"><NotebookPen :size="30" /><span></span><span></span><span></span></div><span class="notes-empty-link"><Link2 :size="19" /></span></div>
        <span class="notes-empty-kicker">{{ uiText('从一个想法开始') }}</span>
        <h2>{{ uiText('让理解留下来，也连接起来。') }}</h2>
        <p>{{ uiText('记下自己的解释，保存值得回来的回答。把相关的想法连接起来，在画板上继续思考。') }}</p>
        <button type="button" class="notes-button notes-primary" :disabled="readonly || !loaded || busy" @click="newNote"><Plus :size="16" aria-hidden="true" />{{ uiText('写下第一条笔记') }}</button>
        <span class="notes-empty-hint">{{ uiText('书籍和对话中的内容，也可以保存到这里。') }}</span>
      </main>

      <aside v-if="draft" class="notes-context" :aria-label="uiText('笔记的关联与来源')">
        <section class="notes-relations">
          <header class="notes-section-heading"><h2><Link2 :size="16" aria-hidden="true" />{{ uiText('关联的想法') }}</h2><button type="button" class="notes-icon-button" :aria-label="uiText('添加笔记关联')" :disabled="readonly || !selected || busy || !!relationDraft || !candidates.length" @click="startRelation()"><Plus :size="16" aria-hidden="true" /></button></header>
          <p v-if="!selected" class="notes-context-hint">{{ uiText('保存这条笔记后，就可以连接其他想法。') }}</p>
          <template v-else>
            <p v-if="!incoming.length && !outgoing.length && !relationDraft" class="notes-context-hint">{{ candidates.length ? uiText('哪个想法支持、补充或挑战了它？连接一条笔记，并写下原因。') : uiText('再写一条笔记，就可以把两个想法连接起来。') }}</p>
            <div v-for="group in relationGroups" :key="group.title" class="notes-relation-group">
              <h3 v-if="group.relations.length">{{ group.title }}</h3>
              <article v-for="relation in group.relations" :key="relation.relation_id" class="notes-relation-card">
                <div class="notes-relation-label"><span>{{ relation.label }}</span><span v-if="relation.author === 'assistant'">{{ uiText('Agent') }}</span></div>
                <button type="button" class="notes-related-title" @click="selectNote(group.incoming ? relation.from_note_id : relation.to_note_id)">{{ noteTitle(group.incoming ? relation.from_note_id : relation.to_note_id) }}<ArrowUpRight :size="14" aria-hidden="true" /></button>
                <p>{{ relation.reason }}</p>
                <div class="notes-relation-actions"><button type="button" :aria-label="uiText('编辑关联：{0}', [relation.label])" :disabled="readonly || busy || !!relationDraft" @click="startRelation(relation)"><Pencil :size="12" aria-hidden="true" />{{ uiText('编辑') }}</button><button type="button" :aria-label="uiText('移除关联：{0}', [relation.label])" :disabled="readonly || busy" @click="confirmAction('delete-relation', relation.relation_id)"><X :size="13" aria-hidden="true" />{{ uiText('移除') }}</button></div>
              </article>
            </div>
            <form v-if="relationDraft" class="notes-relation-form" @submit.prevent="saveRelation()">
              <h3>{{ relationDraft.relationId ? uiText('编辑关联') : uiText('连接另一个想法') }}</h3>
              <p v-if="relationDraft.relationId" class="notes-relation-direction">{{ noteTitle(relationDraft.fromNoteId) }} → {{ noteTitle(relationDraft.toNoteId) }}</p>
              <label v-else>{{ uiText('关联到') }}<select v-model="relationDraft.toNoteId" :disabled="busy"><option value="" disabled>{{ uiText('选择一条笔记') }}</option><option v-for="note in candidates" :key="note.note_id" :value="note.note_id">{{ note.title }}</option></select></label>
              <label>{{ uiText('关系是什么') }}<input v-model="relationDraft.label" :placeholder="uiText('例如：支持、补充、反例')" :disabled="busy" /></label>
              <label>{{ uiText('为什么有关联') }}<textarea v-model="relationDraft.reason" rows="3" :placeholder="uiText('写清两个想法之间的联系…')" :disabled="busy"></textarea></label>
              <p v-if="relationDraft.error" class="notes-inline-error" role="alert">{{ relationDraft.error }}</p>
              <div class="notes-form-actions"><button type="button" :disabled="busy" @click="cancelRelation">{{ uiText('取消') }}</button><button type="submit" class="notes-button notes-primary" :disabled="busy">{{ mutatingRelation ? uiText('正在保存…') : uiText('保存关联') }}</button></div>
            </form>
            <button v-else-if="candidates.length && !incoming.length && !outgoing.length" type="button" class="notes-add-relation" :disabled="readonly || busy" @click="startRelation()"><Plus :size="14" aria-hidden="true" />{{ uiText('添加关联') }}</button>
          </template>
        </section>
        <section v-if="selected" class="notes-source-section">
          <header class="notes-section-heading"><h2><BookOpen :size="15" aria-hidden="true" />{{ uiText('来源与写作') }}</h2></header>
          <dl class="notes-provenance"><div><dt>{{ uiText('最初内容') }}</dt><dd>{{ authorLabel(selected.author) }}</dd></div><div v-if="selected.updated_by !== selected.author"><dt>{{ uiText('最近编辑') }}</dt><dd>{{ authorLabel(selected.updated_by) }}</dd></div><div v-if="selected.origin"><dt>{{ uiText('保存自') }}</dt><dd>{{ selected.origin.kind === 'answer' ? uiText('对话回答') : uiText('阅读笔记') }}</dd></div></dl>
          <button v-for="binding in selected.source_bindings" :key="binding.source_ref_id" type="button" class="notes-source-link" @click="emit('source', { note_id: selected.note_id, source_ref_id: binding.source_ref_id }, $event.currentTarget as HTMLElement)"><BookOpen :size="13" aria-hidden="true" /><span><strong>{{ binding.label_snapshot }}</strong><small v-if="binding.preview_snapshot">{{ binding.preview_snapshot }}</small></span><ArrowUpRight :size="13" aria-hidden="true" /></button>
          <button type="button" class="notes-delete-note" :disabled="readonly || busy" @click="confirmAction('delete-note', selected.note_id)"><Trash2 :size="13" aria-hidden="true" />{{ uiText('删除这条笔记') }}</button>
        </section>
      </aside>
    </div>
    <div v-if="confirmation" class="notes-dialog-scrim" @click.self="dismissConfirmation">
      <section class="notes-confirmation" role="dialog" aria-modal="true" aria-labelledby="notes-confirm-title">
        <h2 id="notes-confirm-title">{{ confirmation.kind === 'delete-note' ? uiText('删除这条笔记？') : confirmation.kind === 'delete-relation' ? uiText('移除这条关联？') : uiText('放弃未保存的修改？') }}</h2>
        <p>{{ confirmation.kind === 'delete-note' ? uiText('笔记及其关联会被删除，画板中的引用会显示为已删除。原始书籍和对话仍然保留。') : confirmation.kind === 'delete-relation' ? uiText('只移除两个想法之间的这条关联，两条笔记都会保留。') : uiText('当前草稿会被放弃；已保存的笔记将重新载入最新版本。') }}</p>
        <p v-if="confirmationError" class="notes-inline-error" role="alert">{{ confirmationError }}</p>
        <div><button type="button" class="notes-button cancel" :disabled="deleting" @click="dismissConfirmation">{{ uiText('取消') }}</button><button type="button" class="notes-button notes-destructive" :disabled="deleting" @click="performConfirmed">{{ deleting ? uiText('正在处理…') : confirmation.kind === 'discard' ? uiText('放弃修改') : confirmation.kind === 'delete-note' ? uiText('删除笔记') : uiText('移除关联') }}</button></div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.notes-workspace{display:flex;flex-direction:column;position:relative;flex:1;min-height:0;min-width:0;height:100%;background:var(--reader-canvas);color:var(--ink);font-family:'Noto Sans SC Variable',var(--sans);font-size:13px}
.notes-workspace button,.notes-workspace input,.notes-workspace textarea,.notes-workspace select{font:inherit}.notes-workspace button{cursor:pointer}.notes-workspace button:disabled{opacity:.45;cursor:default}.notes-workspace :is(button,input,textarea,select):focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}.notes-workspace button{transition:background 120ms ease,color 120ms ease}
.notes-topbar{display:flex;align-items:center;gap:17px;min-height:68px;padding:14px 25px;border-bottom:1px solid var(--hairline);background:var(--reader-card);flex-shrink:0}.notes-icon-button{display:grid;place-items:center;width:31px;height:31px;flex-shrink:0;border:0;border-radius:6px;background:transparent;color:var(--steel)}.notes-icon-button:hover{background:var(--surface-soft);color:var(--ink)}.notes-brand{display:flex;align-items:center;gap:9px;white-space:nowrap}.notes-brand>svg{color:var(--brand-accent)}.notes-brand h1{font-size:17px;font-weight:600;margin:0}.notes-workspace-label{display:flex;align-items:center;gap:9px;padding-left:17px;border-left:1px solid var(--hairline);color:var(--stone);font-size:11px}.notes-topbar>.notes-button{margin-left:auto}.notes-button{display:inline-flex;align-items:center;justify-content:center;gap:6px;min-height:35px;padding:8px 13px;border:1px solid var(--hairline);border-radius:7px;color:var(--slate);background:var(--reader-card);white-space:nowrap;font-size:12px!important}.notes-button:hover{background:var(--surface-soft)}.notes-button.notes-primary{border-color:var(--brand-accent);background:var(--brand-accent);color:var(--brand-on-accent)}.notes-primary:hover{background:var(--brand-accent-deep);border-color:var(--brand-accent-deep)}
.notes-layout{display:grid;grid-template-columns:260px minmax(0,1fr) 280px;flex:1;min-height:0;overflow:hidden}.notes-library{display:flex;flex-direction:column;min-height:0;background:var(--surface-soft);border-right:1px solid var(--hairline)}.notes-library-head{display:flex;align-items:center;justify-content:space-between;padding:23px 22px 15px;color:var(--slate);font-size:12px;font-weight:550}.notes-count{min-width:22px;padding:2px 5px;text-align:center;border-radius:4px;background:var(--hairline-soft);font-size:10px;font-variant-numeric:tabular-nums;color:var(--stone)}.notes-search{display:flex;align-items:center;gap:8px;margin:0 16px 15px;padding:9px 10px;border:1px solid var(--hairline);border-radius:7px;background:var(--reader-card);color:var(--stone)}.notes-search input{min-width:0;flex:1;width:100%;border:0;outline:none;background:transparent;color:var(--ink);font-size:12px}.notes-search:focus-within{border-color:var(--brand-accent-line)}.notes-search input:focus-visible{outline:none}.notes-search input::placeholder{color:var(--stone)}.notes-list{overflow:auto;flex:1;min-height:0;padding:0 10px 20px;overscroll-behavior:contain}.notes-list-item{display:flex;flex-direction:column;gap:9px;text-align:left;width:100%;padding:17px 14px;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--ink);margin-bottom:5px}.notes-list-item:hover{background:color-mix(in srgb,var(--reader-card) 65%,transparent)}.notes-list-item.selected{background:var(--reader-card);border-color:var(--hairline);box-shadow:0 2px 6px #25232005}.notes-list-item.selected strong{color:var(--brand-accent-deep)}.notes-list-meta{display:flex;align-items:center;justify-content:space-between;gap:7px;font-size:10px;color:var(--stone)}.notes-list-item strong{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-weight:550;font-size:13px;line-height:1.75;overflow-wrap:anywhere}.notes-list-excerpt{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-size:11px;line-height:1.85;color:var(--steel);overflow-wrap:anywhere}.notes-dirty-dot{display:inline-block;margin-left:6px;width:5px;height:5px;background:var(--brand-accent);border-radius:50%;vertical-align:middle}.notes-new-draft .notes-list-meta{justify-content:flex-start;color:var(--brand-accent)}.notes-list-message{display:flex;align-items:center;justify-content:center;gap:8px;text-align:center;padding:24px 15px;color:var(--stone);font-size:12px;line-height:1.9}.notes-library-footer{display:flex;gap:8px;align-items:flex-start;padding:18px 22px;border-top:1px solid var(--hairline);font-size:10px;line-height:1.85;color:var(--stone)}.notes-library-footer svg{flex-shrink:0;margin-top:3px}
.notes-main{display:flex;flex-direction:column;min-width:0;min-height:0;overflow:hidden}.notes-editor-toolbar{display:flex;align-items:center;gap:12px;flex-shrink:0;min-height:64px;padding:13px 27px;border-bottom:1px solid var(--hairline-soft);background:var(--reader-card)}.notes-editor-tabs{display:flex;align-items:center;gap:3px;border-radius:6px;background:var(--surface-soft);padding:3px}.notes-editor-tabs button{border:0;background:transparent;border-radius:4px;padding:5px 12px;color:var(--stone);font-size:11px}.notes-editor-tabs button[aria-pressed=true]{background:var(--reader-card);color:var(--ink);box-shadow:0 1px 3px #2523200c}.notes-save-status{display:flex;align-items:center;gap:5px;margin-left:auto;color:var(--stone);font-size:10px;white-space:nowrap}.notes-save-status.unsaved{color:var(--brand-accent-deep)}.notes-save-button{border-color:var(--brand-accent-line);color:var(--brand-accent-deep);font-size:11px!important;padding:7px 11px;min-height:32px}.notes-document-scroll{flex:1;min-height:0;overflow:auto;overscroll-behavior:contain}.notes-document{display:flex;flex-direction:column;max-width:840px;min-height:calc(100% - 116px);margin:auto;padding:38px 38px 24px}.notes-document-meta{display:flex;align-items:center;flex-wrap:wrap;gap:15px;color:var(--stone);font-size:10px;margin-bottom:15px}.notes-document-meta>span{display:flex;align-items:center;gap:5px}.notes-title-input{resize:none;overflow:hidden;display:block;width:100%;min-width:0;padding:4px 0;border:0;background:transparent;color:var(--ink);font-family:var(--reader-font),serif!important;font-size:25px!important;font-weight:600;line-height:1.8}.notes-title-input::placeholder{color:var(--muted);font-weight:450}.notes-document-rule{height:1px;background:var(--hairline-soft);margin:21px 0 24px}.notes-content-input{flex:1;resize:vertical;width:100%;min-height:280px;padding:0;border:0;background:transparent;color:var(--reader-body);font-size:14px!important;line-height:2;overflow-wrap:anywhere}.notes-content-input::placeholder{color:var(--muted)}.notes-content-input:focus-visible,.notes-title-input:focus-visible{outline:none!important}.notes-editor-footnote{display:flex;align-items:center;justify-content:space-between;gap:15px;padding-top:30px;color:var(--stone);font-size:10px}.notes-editor-footnote button{padding:0;border:0;background:transparent;color:var(--steel);font-size:10px;white-space:nowrap}.notes-editor-footnote button:hover{color:var(--brand-error)}.notes-next-actions{display:flex;gap:12px;max-width:840px;margin:0 auto;padding:9px 38px 28px}.notes-action{display:flex;align-items:center;gap:10px;flex:1;min-width:0;padding:16px 14px;border:1px solid var(--hairline);border-radius:8px;background:var(--reader-card);color:var(--brand-accent-deep);text-align:left}.notes-action:hover{border-color:var(--brand-accent-line);background:var(--surface-soft)}.notes-action>svg{flex-shrink:0}.notes-action>span{display:flex;flex-direction:column;gap:6px;flex:1;min-width:0}.notes-action strong{font-weight:550;font-size:12px;color:var(--slate)}.notes-action small{font-size:10px;color:var(--stone);line-height:1.65}.notes-action>svg:last-child{color:var(--stone)}.notes-mobile-back{display:none}.notes-prose{flex:1;color:var(--reader-body);font-family:var(--reader-font),serif;font-size:15px;line-height:2;overflow-wrap:anywhere;min-height:280px}.notes-prose :deep(:first-child){margin-top:0}.notes-prose :deep(img){max-width:100%;height:auto}.notes-prose :deep(pre){white-space:pre-wrap;overflow-wrap:anywhere}.notes-prose :deep(table){display:block;max-width:100%;overflow:auto}.notes-prose :deep(blockquote){margin-left:0;padding-left:18px;border-left:2px solid var(--brand-accent-line);color:var(--steel)}
.notes-context{display:flex;flex-direction:column;gap:27px;padding:23px 23px 27px;border-left:1px solid var(--hairline);background:var(--reader-card);overflow:auto;min-width:0;min-height:0;overscroll-behavior:contain}.notes-section-heading{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:16px}.notes-section-heading h2{display:flex;align-items:center;gap:8px;font-size:12px;font-weight:550;margin:0;color:var(--slate)}.notes-section-heading h2>svg{color:var(--stone)}.notes-section-heading .notes-icon-button{width:26px;height:26px;border:1px solid var(--hairline);border-radius:5px}.notes-context-hint{color:var(--stone);font-size:11px;line-height:1.9;margin:0 0 13px}.notes-relation-group:empty{display:none}.notes-relation-group h3{font-size:10px;font-weight:400;color:var(--stone);margin:21px 0 9px}.notes-relation-card{border:1px solid var(--hairline);border-radius:8px;margin-bottom:10px;padding:13px;background:var(--reader-canvas)}.notes-relation-label{display:flex;justify-content:space-between;gap:8px;font-size:10px;color:var(--brand-accent-deep);margin-bottom:8px}.notes-relation-label>span:last-child:not(:first-child){color:var(--stone);font-size:9px}.notes-related-title{display:flex;gap:8px;align-items:flex-start;justify-content:space-between;width:100%;text-align:left;border:0;background:transparent;color:var(--slate);font-size:12px!important;font-weight:550!important;line-height:1.8;overflow-wrap:anywhere;padding:0}.notes-related-title svg{flex-shrink:0;color:var(--stone);margin-top:3px}.notes-related-title:hover{color:var(--brand-accent-deep)}.notes-relation-card p{margin:8px 0 0;font-size:11px;line-height:1.85;color:var(--steel);white-space:pre-wrap;overflow-wrap:anywhere}.notes-relation-actions{display:flex;justify-content:flex-end;gap:12px;margin-top:12px}.notes-relation-actions button{display:flex;align-items:center;gap:4px;border:0;background:transparent;padding:0;color:var(--stone);font-size:10px}.notes-relation-actions button:hover{color:var(--brand-accent-deep)}.notes-add-relation{display:flex;align-items:center;gap:5px;border:0;padding:0;background:transparent;color:var(--brand-accent-deep);font-size:11px!important}.notes-relation-form{border:1px solid var(--brand-accent-line);border-radius:8px;padding:13px;background:var(--surface-soft);margin-top:15px}.notes-relation-form h3{font-size:12px;margin:0 0 15px;font-weight:550}.notes-relation-form label{display:flex;flex-direction:column;gap:7px;margin:0 0 13px;color:var(--steel);font-size:10px}.notes-relation-form :is(input,textarea,select){min-width:0;width:100%;padding:8px 9px;border:1px solid var(--hairline);border-radius:5px;background:var(--reader-card);color:var(--ink);font-size:11px;line-height:1.7}.notes-relation-form textarea{resize:vertical;min-height:80px}.notes-relation-direction{font-size:10px;color:var(--steel);line-height:1.8;overflow-wrap:anywhere}.notes-form-actions{display:flex;gap:10px;justify-content:flex-end;align-items:center}.notes-form-actions>button:not(.notes-button){border:0;background:transparent;color:var(--steel);font-size:11px}.notes-form-actions .notes-button{min-height:30px;padding:6px 9px;font-size:10px!important}.notes-source-section{padding-top:22px;border-top:1px solid var(--hairline)}.notes-provenance{display:flex;flex-direction:column;gap:11px;margin:0 0 18px;font-size:10px}.notes-provenance>div{display:flex;justify-content:space-between;gap:12px}.notes-provenance dt{color:var(--stone)}.notes-provenance dd{margin:0;color:var(--steel);text-align:right}.notes-source-link{display:flex;gap:8px;align-items:flex-start;width:100%;padding:10px 0;text-align:left;border:0;border-top:1px solid var(--hairline-soft);background:transparent;color:var(--steel)}.notes-source-link>svg{flex-shrink:0;margin-top:3px;color:var(--stone)}.notes-source-link>span{min-width:0;flex:1}.notes-source-link strong{font-weight:500;font-size:11px;line-height:1.8}.notes-source-link small{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-size:10px;line-height:1.8;color:var(--stone);margin-top:4px}.notes-delete-note{display:flex;align-items:center;gap:6px;margin-top:25px;padding:0;border:0;background:transparent;color:var(--stone);font-size:10px!important}.notes-delete-note:hover{color:var(--brand-error)}
.notes-empty{grid-column:2/-1;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:40px;text-align:center;min-width:0;overflow:auto}.notes-empty-illustration{position:relative;width:130px;height:130px;margin-bottom:22px}.notes-empty-sheet{position:absolute;top:10px;left:21px;display:flex;flex-direction:column;gap:10px;width:78px;height:100px;padding:13px;border:1px solid var(--brand-accent-line);border-radius:8px;background:var(--reader-card);color:var(--brand-accent)}.notes-empty-sheet>span{width:100%;height:2px;border-radius:2px;background:var(--hairline)}.notes-empty-sheet>span:last-child{width:65%}.notes-empty-sheet.back{transform:rotate(-12deg);left:10px;top:13px;border-color:var(--hairline);background:var(--surface-soft)}.notes-empty-link{display:grid;place-items:center;position:absolute;right:3px;bottom:8px;width:39px;height:39px;border:1px solid var(--hairline);border-radius:50%;background:var(--reader-card);color:var(--brand-accent)}.notes-empty-kicker{font-size:11px;color:var(--brand-accent-deep);letter-spacing:.07em}.notes-empty h2{font-family:var(--reader-font),serif;font-size:27px;font-weight:550;line-height:1.7;margin:15px 0}.notes-empty p{max-width:370px;font-size:13px;color:var(--steel);line-height:2;margin:0 0 27px}.notes-empty-hint{font-size:10px;color:var(--stone);margin-top:18px}
.notes-load-error,.notes-editor-error{padding:12px 25px;background:color-mix(in srgb,var(--brand-error) 6%,var(--reader-card));color:var(--brand-error);font-size:12px;line-height:1.8;flex-shrink:0;overflow-wrap:anywhere}.notes-load-error{display:flex;align-items:center;justify-content:space-between;gap:12px}.notes-editor-error p{margin:0}.notes-load-error button,.notes-editor-error button{border:0;border-bottom:1px solid currentColor;padding:0;background:transparent;color:inherit;font-size:11px;white-space:nowrap}.notes-notice{display:flex;align-items:center;gap:6px;margin:0;padding:10px 28px;border-bottom:1px solid var(--hairline-soft);font-size:11px;color:var(--brand-accent-deep);background:var(--surface-soft)}.notes-inline-error{font-size:11px!important;line-height:1.8;color:var(--brand-error)!important}.notes-dialog-scrim{position:absolute;z-index:30;inset:0;display:flex;align-items:center;justify-content:center;padding:24px;background:#25232040;backdrop-filter:blur(3px)}.notes-confirmation{width:min(440px,100%);max-height:100%;overflow:auto;padding:28px;border:1px solid var(--hairline);border-radius:14px;background:var(--reader-card);box-shadow:0 18px 70px #25232024}.notes-confirmation h2{margin:0 0 15px;font-size:20px;font-weight:550}.notes-confirmation p{font-size:13px;line-height:1.9;color:var(--steel)}.notes-confirmation>div{display:flex;gap:10px;justify-content:flex-end;margin-top:24px}.notes-destructive{color:white;background:var(--brand-error);border-color:var(--brand-error)}.notes-destructive:hover{background:color-mix(in srgb,var(--brand-error) 85%,black)}.notes-spinner{animation:notes-spin 1.2s linear infinite}@keyframes notes-spin{to{transform:rotate(360deg)}}
@media(min-width:1600px){.notes-layout{grid-template-columns:290px minmax(0,1fr) 310px}.notes-context{padding:27px}.notes-document{padding-top:48px}}
@media(max-width:1150px){.notes-layout{grid-template-columns:235px minmax(0,1fr);grid-template-rows:minmax(0,1fr);overflow:auto}.notes-library{grid-column:1;grid-row:1;position:sticky;top:0;height:100%;max-height:100%}.notes-main{grid-column:2;grid-row:1;overflow:visible}.notes-document-scroll{overflow:visible}.notes-context{grid-column:2;grid-row:2;border-top:1px solid var(--hairline);border-left:0;display:grid;grid-template-columns:1fr 1fr;gap:27px;padding:25px 30px;overflow:visible}.notes-source-section{border-top:0;border-left:1px solid var(--hairline);padding-top:0;padding-left:23px}.notes-document{padding:30px;min-height:calc(100% - 116px)}.notes-next-actions{padding:9px 30px 25px}.notes-layout:has(.notes-main){grid-template-rows:auto auto}.notes-layout:has(.notes-main) .notes-library{grid-row:1 / 3}.notes-main{min-height:620px}.notes-context-hint{max-width:300px}.notes-editor-toolbar{padding:13px 22px}.notes-save-status{font-size:9px}.notes-workspace-label{font-size:10px}}
@media(max-width:700px){.notes-topbar{padding:11px 13px;gap:11px;min-height:60px}.notes-brand h1{font-size:16px}.notes-workspace-label{display:none}.notes-topbar .notes-button{min-height:36px;padding:8px 10px}.notes-layout{display:block;overflow:auto}.notes-library{position:static;width:100%;height:100%;max-height:none;border-right:0;display:none}.notes-layout.mobile-list .notes-library{display:flex}.notes-layout.mobile-list .notes-main,.notes-layout.mobile-list .notes-context{display:none}.notes-layout.mobile-list:has(.notes-empty) .notes-library{display:none}.notes-layout .notes-empty{min-height:100%;padding:30px 25px}.notes-empty h2{font-size:23px}.notes-main{display:flex;min-height:calc(100% - 30px);overflow:visible}.notes-editor-toolbar{position:sticky;top:0;z-index:2;padding:11px 15px;gap:8px;min-height:58px}.notes-mobile-back{display:flex;align-items:center;gap:5px;max-width:110px;padding:5px 0;border:0;background:transparent;color:var(--steel);font-size:11px!important;margin-right:auto}.notes-save-status{display:none}.notes-editor-tabs button{padding:5px 8px;font-size:10px}.notes-save-button{margin-left:auto;min-height:34px}.notes-document{padding:27px 22px 20px;min-height:440px}.notes-title-input{font-size:23px!important}.notes-content-input{font-size:16px!important;min-height:280px}.notes-document-meta{font-size:10px}.notes-next-actions{padding:5px 22px 24px;gap:8px}.notes-action{padding:13px 10px;gap:8px}.notes-action>svg:last-child{display:none}.notes-action strong{font-size:11px}.notes-action small{font-size:9px}.notes-context{display:block;padding:24px 22px;border-left:0}.notes-source-section{padding:22px 0 0;border-left:0;border-top:1px solid var(--hairline);margin-top:25px}.notes-relation-group{display:grid;grid-template-columns:1fr 1fr;gap:9px}.notes-relation-group h3{grid-column:1/-1;margin:12px 0 2px}.notes-relation-card{margin-bottom:0}.notes-relation-form{max-width:none}.notes-relation-form :is(input,textarea,select){font-size:16px}.notes-context-hint{max-width:none}.notes-search input{font-size:16px}.notes-load-error,.notes-editor-error{padding:10px 18px}.notes-notice{padding:9px 22px}.notes-list{padding:0 14px 20px}.notes-library-head{padding:21px 28px 15px}.notes-search{margin:0 22px 15px}.notes-list-item{padding:17px 16px}.notes-list-item strong{font-size:14px}.notes-list-excerpt{font-size:12px}.notes-list-meta{font-size:11px}.notes-list-item.selected{border-color:transparent;background:transparent;box-shadow:none}.notes-list-item.selected:hover{background:var(--reader-card)}.notes-dialog-scrim{padding:18px}.notes-confirmation{padding:24px}.notes-confirmation h2{font-size:18px}.notes-confirmation .notes-button{min-height:40px}}
@media(prefers-reduced-motion:reduce){.notes-workspace button{transition:none}.notes-spinner{animation:none}}
</style>
