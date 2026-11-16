<script setup lang="ts">
import NoteThinking from './NoteThinking.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ArrowLeft, ArrowUpRight, Check, ChevronDown, ChevronRight, FileText, GripHorizontal, History, Link2, MapPin, Maximize2, Minimize2, Minus, PenLine, Plus, Search, Trash2 } from '@lucide/vue';
import { useNoteWindow, type NoteWindowAction } from '../note-window';
import { useNoteLauncher } from '../note-launcher';
import { renderMarkdown } from '../md';
import { openPersonalChat, personalChat } from '../personal-chat';
import { currentThoughtContext, emptyNoteContext, noteRequest, noteContentFromView, pendingNoteSource, noteSourceError, type NoteContext } from '../personal-notes';
import { ApiError } from '../api';
import { compareNoteText, discardNoteDraft, draftStorageError, editThought, keepNoteDraft, notesChanged, notesOwner, notesPanel,
  openPersonalNotes, openThoughtCapture, personalNotes, readNoteDrafts, saveThought, startThought, type NoteDraft, type NoteLink, type NoteRevision, type PersonalNoteView } from '../personal-notes';
import type { NoteDiscussion } from '../generated/NoteDiscussion';
import type { NoteSearchHit } from '../generated/NoteSearchHit';
import { availableNoteDestinations, placeThought, removeThoughtPlacement, pendingNotePlacementLocation, type NoteDestinationChoice, type BoardNoteLocation } from '../note-placements';
import type { NoteAttachmentView } from '../generated/NoteAttachmentView';

const items = ref<PersonalNoteView[]>([]), cursor = ref<string | null>(null), query = ref(''), loading = ref(false);
const searchHits=ref<NoteSearchHit[]>([]),searchState=ref(''),searching=ref(false);
let searchRequest=0,searchTimer:ReturnType<typeof setTimeout>|undefined;
watch(query, value=>{clearTimeout(searchTimer);const request=++searchRequest;searching.value=!!value.trim();if(!value.trim()){searchHits.value=[];searchState.value='';return;}
  searchTimer=setTimeout(async()=>{try{const result=await personalNotes.search(value);if(request!==searchRequest)return;searchHits.value=result.hits;searchState.value=result.semantic.status;}
    catch(failure){if(request===searchRequest)error.value=(failure as Error).message;}finally{if(request===searchRequest)searching.value=false;}},250);
});
const selected = ref<PersonalNoteView | null>(null), draft = ref<NoteDraft | null>(null), error = ref(''), notice = ref(''), saving = ref(false);
watch(notice, (message, _previous, onCleanup) => {
  if (!message) return;
  const timeout = setTimeout(() => { notice.value = ''; }, 3000);
  onCleanup(() => clearTimeout(timeout));
}, { flush: 'sync' });
const history = ref<NoteRevision[]>([]), discussions = ref<NoteDiscussion[]>([]), links = ref<NoteLink[]>([]);
const tab = ref<'write' | 'history' | 'links'>('write'), historyVersion = ref<NoteRevision | null>(null), comparing = ref(false);
const conflict = ref<PersonalNoteView | null>(null), linkTarget = ref(''), linkReason = ref(''), confirmDelete = ref(false);
const linkQuery = ref(''), linkCandidates = ref<NoteSearchHit[]>([]), editingLink = ref<NoteLink | null>(null), editedReason = ref('');
let linkSearchRequest = 0;
watch(linkQuery, async value => {
  const request = ++linkSearchRequest;
  if (!value.trim()) { linkCandidates.value = []; return; }
  try { const result = await personalNotes.search(value); if (request === linkSearchRequest) linkCandidates.value = result.hits; }
  catch (failure) { if (request === linkSearchRequest) error.value = (failure as Error).message; }
});
const linkOptions = computed(() => linkQuery.value.trim()
  ? linkCandidates.value.filter(hit => hit.reference.note_id !== selected.value?.note_id).map(hit => ({ id: hit.reference.note_id, title: hit.title || hit.excerpt.slice(0, 70) }))
  : items.value.filter(note => note.note_id !== selected.value?.note_id).map(note => ({ id: note.note_id, title: title(note) })));
const shownVersion = computed(() => tab.value === 'history' && historyVersion.value ? historyVersion.value : selected.value?.value);
const shownContext = computed(() => draft.value && tab.value === 'write' ? draft.value.value.context : shownVersion.value?.context);
const shownOrigins = computed(() => draft.value && tab.value === 'write' ? draft.value.value : shownVersion.value);
function removeOrigin(index: number) {
  if (!draft.value || draft.value.pending || saving.value) return;
  draft.value.value.source_refs.splice(index, 1); keepNoteDraft(draft.value);
}
function returnToSource() {
  const context = shownContext.value; if (!context) return; noteSourceError.value = '';
  if (context.origin) pendingNotePlacementLocation.value = {kind:'attachment',target:context.origin,label:context.label || '记录现场'};
  else if (context.book_id) pendingNoteSource.value = {...context};
  else if (context.session_id) void openPersonalChat(undefined,'discuss',undefined,context.session_id);
}
const comparisonRevision = ref(0);
const textInput = ref<HTMLTextAreaElement | null>(null), panel = ref<HTMLElement | null>(null);
const historicalReader = ref<HTMLElement | null>(null);
const composing=ref(false);
const noteWindow = useNoteWindow(notesOwner);
const launcherElement = ref<HTMLElement | null>(null);
const launcher = useNoteLauncher(launcherElement, notesOwner, () => !!notesOwner() && !notesPanel.open);
const resizeEdges: NoteWindowAction[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'sw'];
const contextMenuOpen = ref(false), contextCandidate = ref<Partial<NoteContext> | null>(null);
const placementMenuOpen = ref(false), placementChoices = ref<NoteDestinationChoice[]>([]), placing = ref(false);
const sourceCaptureId = ref<string | null>(null);
const capturingSource = computed(() => !!sourceCaptureId.value && sourceCaptureId.value === draft.value?.draft_id);
const placements = ref<NoteAttachmentView[]>([]);
const boardPlacements = ref<BoardNoteLocation[]>([]);
function openBoardPlacement(location: BoardNoteLocation) { pendingNoteSource.value = {...emptyNoteContext(),book_id:location.book_id,publication_id:location.publication_id,workspace_id:location.workspace_id,board_id:location.board_id,board_item_id:location.item_id}; }
async function removeBoardPlacement(location: BoardNoteLocation) { try { await noteRequest('unattach_board',{...location,note_id:selected.value?.note_id}); notesChanged.value++; } catch (failure) { error.value = (failure as Error).message; } }
function togglePlacementMenu() { placementChoices.value = availableNoteDestinations(); placementMenuOpen.value = !placementMenuOpen.value; contextMenuOpen.value = false; }
function onlySave() { if (draft.value) { delete draft.value.destination; keepNoteDraft(draft.value); } placementMenuOpen.value = false; }
async function choosePlacement(choice: NoteDestinationChoice) {
  if (placing.value) return;
  const request = selectionRequest, currentDraft = draft.value, currentNote = selected.value;
  placing.value = true; error.value = '';
  try {
    const destination = await choice.resolve();
    if (selectionRequest !== request || currentDraft?.draft_id !== draft.value?.draft_id) return;
    if (currentDraft) { currentDraft.destination = destination; keepNoteDraft(currentDraft); }
    else if (currentNote) { await placeThought(currentNote.note_id, destination); if (selectionRequest === request) notice.value = `已放到 · ${destination.label}`; }
    if (selectionRequest === request) placementMenuOpen.value = false;
  } catch (failure) { if (selectionRequest === request) error.value = (failure as Error).message; }
  finally { placing.value = false; }
}
let placementRead = 0;
watch([() => selected.value?.note_id, notesChanged], async ([id]) => {
  const request = ++placementRead; placements.value = []; boardPlacements.value = [];
  if (!id) return;
  try { const result = await noteRequest<{items:NoteAttachmentView[]}>('attachments', {note_id:id}); if (request === placementRead) placements.value = result.items; }
  catch (failure) { if (request === placementRead) error.value = (failure as Error).message; }
  try { const result = await noteRequest<{items:BoardNoteLocation[]}>('board_locations',{note_id:id}); if (request === placementRead) boardPlacements.value = result.items; }
  catch (failure) { if (request === placementRead) error.value = (failure as Error).message; }
});
async function removePlacement(id: string) { try { await removeThoughtPlacement(id); } catch (failure) { error.value = (failure as Error).message; } }
const canChooseContext = computed(() => !!draft.value && !draft.value.note_id && !draft.value.value.source && !draft.value.value.capture && !draft.value.value.based_on_turn && !draft.value.value.source_refs.length);
const contextLabel = computed(() => shownContext.value?.label || (shownContext.value?.book_id ? '阅读位置' : shownContext.value?.session_id ? '这段对话' : shownContext.value?.workspace_id ? '学习工作区' : '独立想法'));
function captureContext(): Partial<NoteContext> | null {
  const current = currentThoughtContext();
  return personalChat.open ? current?.session_id === personalChat.sessionId ? current : (personalChat.sessionId ? { session_id: personalChat.sessionId, label: personalChat.title || '这段对话' } : null) : current;
}
function prepareCaptureSource(value: NoteDraft) {
  const context = value.value.context;
  if (personalChat.open || !context.session_id || !context.turn_id || (context.origin && context.origin.kind !== 'presentation')) return;
  const choice = availableNoteDestinations().find(choice => choice.key.startsWith('presentation:'));
  if (!choice) return;
  sourceCaptureId.value = value.draft_id;
  void Promise.resolve().then(() => choice.resolve()).then(destination => {
    if (destination.kind !== 'attachment' || value.value.context !== context) return;
    value.value.context = {...context,origin:destination.target,label:destination.label};keepNoteDraft(value);
  }).catch(failure => { if (draft.value?.draft_id === value.draft_id) error.value = `记录现场暂未保存：${(failure as Error).message}`; })
    .finally(() => { if (sourceCaptureId.value === value.draft_id) sourceCaptureId.value = null; });
}
function capture() { const resume = readNoteDrafts().length > 0; const value = openThoughtCapture(captureContext()); if (!resume) prepareCaptureSource(value); nextTick(() => textInput.value?.focus({ preventScroll: true })); }
function toggleContextMenu() { contextCandidate.value = captureContext(); contextMenuOpen.value = !contextMenuOpen.value; }
function chooseContext(context: Partial<NoteContext> | null) {
  if (!draft.value || draft.value.pending || saving.value || !canChooseContext.value) return;
  draft.value.value.context = { ...emptyNoteContext(), ...context }; keepNoteDraft(draft.value); contextMenuOpen.value = false;
  sourceCaptureId.value = null;
  if (context) prepareCaptureSource(draft.value);
}
const drafts = computed(() => { notesChanged.value; return readNoteDrafts(); });
const filtered = computed(() => items.value.filter(note => `${note.value.title ?? ''}\n${note.value.record.content}\n${note.value.context.label ?? ''}`.toLocaleLowerCase().includes(query.value.toLocaleLowerCase())));
const currentText = computed(() => draft.value?.value.content ?? selected.value?.value.record.content ?? '');
const differences = computed(() => compareNoteText(historyVersion.value?.record.content ?? '', (comparisonRevision.value ? history.value.find(version=>version.revision===comparisonRevision.value)?.record.content : selected.value?.value.record.content) ?? ''));
const dirty = computed(() => {
  if (!draft.value) return false;
  if (!selected.value) return true;
  const original = noteContentFromView(selected.value);
  return Object.entries(original).some(([key,value]) => JSON.stringify(value) !== JSON.stringify(draft.value!.value[key as keyof typeof original]));
});
const title = (note: PersonalNoteView) => note.value.title?.trim() || note.value.record.content.trim().split('\n')[0]?.slice(0, 70) || '保留的素材';
const author = (revision: NoteRevision) => ({ user: '我的表述', assistant_draft: 'AI 草稿', adopted_assistant: '采用 AI 修改', material_excerpt: '资料摘录', unknown: '原记录' }[revision.authorship]);
const time = (value: string) => { const date = new Date(/^\d{10}$/.test(value) ? Number(value) * 1000 : value); return Number.isNaN(date.valueOf()) ? value : date.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }); };
let listRequest = 0;
let selectionRequest = 0, priorFocus: HTMLElement | null = null;
function close() { notesPanel.open = false; nextTick(() => { if (!notesPanel.open) priorFocus?.focus({ preventScroll: true }); }); }
function changed() { if (draft.value) { draft.value.value.authorship = draft.value.value.authorship === 'assistant_draft' || draft.value.value.authorship === 'adopted_assistant' ? 'adopted_assistant' : 'user'; keepNoteDraft(draft.value); } }
function create() { ++selectionRequest; const value = startThought(captureContext() ?? {}); draft.value = value; selected.value = null; history.value = []; links.value = []; tab.value = 'write'; error.value = ''; notice.value = ''; prepareCaptureSource(value); nextTick(() => textInput.value?.focus()); }
function resume(value: NoteDraft) { ++selectionRequest; draft.value = value; tab.value = 'write'; notesPanel.view = 'capture'; notesPanel.draftId = value.draft_id; error.value = ''; notice.value = ''; if (value.note_id) void select(value.note_id, value); else { selected.value = null; history.value = []; links.value = []; } nextTick(() => textInput.value?.focus({ preventScroll: true })); }
async function load(more = false) {
  if (!notesOwner()) return;
  const request=++listRequest; loading.value = true;
  try { const page = await personalNotes.list(more ? cursor.value ?? undefined : undefined); if(request!==listRequest)return; items.value = more ? [...items.value, ...page.notes] : page.notes; cursor.value = page.next_cursor; }
  catch (failure) { error.value = (failure as Error).message; }
  finally { if(request===listRequest)loading.value = false; }
}
async function select(id: string, existingDraft?: NoteDraft) {
  placementMenuOpen.value = false;
  notesPanel.view = 'detail';
  const request = ++selectionRequest; noteSourceError.value = ''; error.value = ''; notice.value = ''; confirmDelete.value = false; tab.value = 'write'; historyVersion.value = null; comparisonRevision.value=0; conflict.value = null; editingLink.value = null; linkTarget.value = ''; discussion.value = null;
  try {
    const note = await personalNotes.read({ note_id: id });
    if (request !== selectionRequest) return;
    selected.value = note; draft.value = existingDraft ?? readNoteDrafts().find(value => value.note_id === id) ?? null;
    const [timeline, related] = await Promise.all([personalNotes.history(id), personalNotes.links(id)]);
    if (request !== selectionRequest) return;
    history.value = timeline.revisions; discussions.value = timeline.discussions; links.value = related.links;
  } catch (failure) { if (request === selectionRequest) error.value = (failure as Error).message; }
}
function edit() { if (selected.value) { draft.value = editThought(selected.value); nextTick(() => textInput.value?.focus()); } }
async function save(): Promise<boolean> {
  if (!draft.value || saving.value || capturingSource.value) return false;
  const submitted = draft.value, request = selectionRequest;
  saving.value = true; error.value = ''; conflict.value = null;
  try {
    const note = await saveThought(submitted);
    window.dispatchEvent(new CustomEvent('personal-notes-changed', { detail: note.note_id }));
    if (draft.value?.draft_id !== submitted.draft_id || selectionRequest !== request) { await load(); return false; }
    draft.value = null; selected.value = note; notesPanel.draftId = null;
    if (notesPanel.view === 'capture') { if (notesPanel.open) close(); }
    else { await select(note.note_id); await load(); }
    notice.value = submitted.destination ? `已保存并放到 · ${submitted.destination.label}` : `已保存 · 第 ${note.current_revision} 版`;
    return true;
  } catch (failure) {
    if (draft.value?.draft_id !== submitted.draft_id || selectionRequest !== request) return false;
    error.value = (failure as Error).message;
    if (failure instanceof ApiError && failure.errorCode === 'NOTE_REVISION_CONFLICT' && draft.value?.note_id) {
      try { conflict.value = await personalNotes.read({ note_id: draft.value.note_id }); } catch { /* Keep the original draft and error. */ }
    }
    return false;
  } finally { saving.value = false; }
}
async function mergeAndSave() { if (!draft.value || !conflict.value) return; draft.value.base_revision = conflict.value.current_revision; keepNoteDraft(draft.value); await save(); }
function discard() { if (!draft.value || draft.value.pending) return; discardNoteDraft(draft.value); draft.value = null; notesPanel.draftId = null; notesChanged.value++; if (!selected.value) close(); }
async function moreHistory() { if (!selected.value || !history.value.length) return; try { const page = await personalNotes.history(selected.value.note_id, history.value.at(-1)!.revision); history.value.push(...page.revisions); } catch (failure) { error.value = (failure as Error).message; } }
async function restore(saveFirst = false) {
  if (!selected.value || !historyVersion.value || saving.value) return;
  const revision = historyVersion.value.revision, noteId = selected.value.note_id;
  if (saveFirst && !(await save())) return;
  const request = selectionRequest;
  const key = `understand-book:note-restore:${notesOwner()}:${noteId}`;
  let operation: {base:number;revision:number;operationId:string};
  try { operation = JSON.parse(localStorage.getItem(key) ?? 'null') ?? { base: selected.value!.current_revision, revision, operationId: crypto.randomUUID() }; localStorage.setItem(key, JSON.stringify(operation)); }
  catch { error.value='设备暂时不能保留这次恢复操作，请腾出存储空间后重试。'; return; }
  saving.value = true; error.value = '';
  try {
    const note = await personalNotes.restore(noteId, operation.base, operation.revision, operation.operationId);
    localStorage.removeItem(key); notesChanged.value++;window.dispatchEvent(new Event('personal-notes-changed'));
    if (selectionRequest !== request || selected.value?.note_id !== noteId) { await load(); return; }
    if (draft.value) discard(); await select(noteId); await load();
    notice.value = `已从第 ${operation.revision} 版恢复为第 ${note.current_revision} 版`; window.dispatchEvent(new Event('personal-notes-changed'));
  } catch (failure) { error.value = (failure as Error).message; if (failure instanceof ApiError && failure.status < 500 && ![408,429].includes(failure.status)) localStorage.removeItem(key); }
  finally { saving.value = false; }
}
async function remove() {
  if (!selected.value || saving.value) return; const id=selected.value.note_id; saving.value = true;
  try { await personalNotes.delete(id); if (selected.value?.note_id===id) { if (draft.value?.note_id===id) discard(); selected.value = null; confirmDelete.value = false; } notesChanged.value++; await load(); window.dispatchEvent(new Event('personal-notes-changed')); }
  catch (failure) { error.value = (failure as Error).message; } finally { saving.value = false; }
}
async function link() {
  if (!selected.value || !linkTarget.value) return;
  const from={ note_id: selected.value.note_id, revision: selected.value.current_revision }, target=linkTarget.value, reason=linkReason.value;
  try { await personalNotes.link(from, { note_id: target }, reason || undefined);
    const related=await personalNotes.links(from.note_id); if(selected.value?.note_id!==from.note_id)return; links.value=related.links; if(linkTarget.value===target && linkReason.value===reason){linkTarget.value = ''; linkReason.value = '';} }
  catch (failure) { error.value = (failure as Error).message; }
}
async function unlink(id: string) { try { await personalNotes.unlink(id); links.value = links.value.filter(link => link.link_id !== id); } catch (failure) { error.value = (failure as Error).message; } }
async function updateLink() {
  const item = editingLink.value; if (!item) return;
  try { const updated = await personalNotes.link(item.reason_refs?.[0] ?? item.from, item.reason_refs?.[1] ?? item.to, editedReason.value || undefined); links.value = links.value.map(link => link.link_id === updated.link_id ? updated : link); editingLink.value = null; }
  catch (failure) { error.value = (failure as Error).message; }
}
function linkedId(link: NoteLink) { return link.from.note_id === selected.value?.note_id ? link.to.note_id : link.from.note_id; }
async function discuss() {
  if (draft.value && !(await save())) return;
  if (!selected.value) return;
  await openPersonalChat([{ note_id: selected.value.note_id, revision: selected.value.current_revision }]);
}
// The open reader grounds the problem in its material and claims the event.
async function practice() {
  if (draft.value && !(await save())) return;
  if (!selected.value) return;
  const event = new CustomEvent('practice-note', { cancelable: true,
    detail: { note_id: selected.value.note_id, revision: selected.value.current_revision } });
  if (window.dispatchEvent(event)) notice.value = '打开一份材料后，就能用这条想法练一练。';
}
async function suggestLink() {
  if(!selected.value || !linkTarget.value)return;
  try {const other=await personalNotes.read({note_id:linkTarget.value});await openPersonalChat([{note_id:selected.value.note_id,revision:selected.value.current_revision},{note_id:other.note_id,revision:other.current_revision}],'link');}
  catch(failure){error.value=(failure as Error).message;}
}
const discussion=ref<{title:string;personal:boolean;session_id:string;turn:{user:string;outcome?:{answer?:string};error?:{message:string}}}|null>(null);
async function readDiscussion(sessionId:string,turnId:string){try{discussion.value=await noteRequest('discussion',{session_id:sessionId,turn_id:turnId});}catch(failure){error.value=(failure as Error).message;}}
async function readThinkingRevision(revision:number){if(!selected.value)return;const noteId=selected.value.note_id;try{const note=await personalNotes.read({note_id:noteId,revision});if(selected.value?.note_id!==noteId)return;historyVersion.value=note.value;tab.value='history';await nextTick();historicalReader.value?.scrollIntoView?.({block:'start'});}catch(failure){error.value=(failure as Error).message;}}
function keydown(event: KeyboardEvent) {
  if (event.isComposing || composing.value || event.keyCode === 229) return;
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); if (placementMenuOpen.value) placementMenuOpen.value = false; else if (contextMenuOpen.value) contextMenuOpen.value = false; else close(); }
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter' && tab.value === 'write') { event.preventDefault(); event.stopPropagation(); if (draft.value?.value.content.trim() || draft.value?.value.source?.retained_excerpt) void save(); }
}
function captureShortcut(event: KeyboardEvent) {
  if (!notesOwner() || event.isComposing || event.keyCode === 229) return;
  if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.code === 'KeyM') { event.preventDefault(); capture(); }
}
function beforeUnload(event: BeforeUnloadEvent) { if (draftStorageError.value && draft.value) { event.preventDefault(); event.returnValue = ''; } }
watch(() => [notesPanel.open, notesPanel.view, notesPanel.draftId] as const, ([open, view, id], previous) => {
  contextMenuOpen.value = false;
  placementMenuOpen.value = false;
  if (!open) { composing.value = false; noteWindow.finish(); return; }
  if (!previous?.[0]) priorFocus = document.activeElement as HTMLElement;
  if (view === 'library') { ++selectionRequest; void load(); nextTick(() => panel.value?.focus({ preventScroll: true })); }
  if (id && view !== 'library') { const value = readNoteDrafts().find(value => value.draft_id === id); if (value && value !== draft.value) resume(value); nextTick(() => textInput.value?.focus({ preventScroll: true })); }
});
watch(() => notesPanel.reference, async reference => { if (reference) {await select(reference.note_id);if(reference.revision){try{const historical=await personalNotes.read(reference);historyVersion.value=historical.value;tab.value='history';}catch(failure){error.value=(failure as Error).message;}}} });
watch(notesOwner, () => { ++selectionRequest; pendingNoteSource.value=null; noteSourceError.value=''; notesPanel.open = false; draft.value = null; selected.value = null; items.value = []; history.value = []; links.value = []; notice.value = ''; error.value = ''; });
onMounted(() => { window.addEventListener('keydown', captureShortcut); window.addEventListener('beforeunload', beforeUnload); });
onBeforeUnmount(() => { clearTimeout(searchTimer); window.removeEventListener('keydown', captureShortcut); window.removeEventListener('beforeunload', beforeUnload); });
</script>

<template>
    <section v-if="notesPanel.open" id="personal-thought-window" data-tour="thought-window" ref="panel" class="thought-space" :class="{ 'is-moving': noteWindow.interacting.value }" :style="noteWindow.style.value" role="region" aria-label="我的想法" tabindex="-1" data-personal-notes @keydown="keydown">
      <header class="thought-heading" @pointerdown="noteWindow.begin($event, 'move')">
        <div class="thought-window-title"><span class="thought-pen"><PenLine :size="16" /></span><h1>{{ notesPanel.view === 'capture' ? '随手记' : '我的想法' }}</h1></div>
        <span class="thought-drag-grip" tabindex="0" role="button" aria-label="移动想法窗口" aria-description="拖动顶部，或使用方向键移动窗口" title="拖动顶部移动" @keydown="noteWindow.keyboard($event, 'move')"><GripHorizontal :size="18" /></span>
        <div class="thought-window-tools"><button v-if="notesPanel.view !== 'library'" class="icon-button" aria-label="我的想法" title="查看全部想法" @click="openPersonalNotes()"><FileText :size="16" /></button><button v-else class="icon-button" aria-label="记个想法" title="继续随手记" @click="capture"><PenLine :size="16" /></button><button class="icon-button" :aria-label="noteWindow.expanded.value ? '还原想法窗口' : '展开想法窗口'" :title="noteWindow.expanded.value ? '还原窗口' : '展开阅读思考过程'" @click="noteWindow.toggleExpanded"><Minimize2 v-if="noteWindow.expanded.value" :size="16" /><Maximize2 v-else :size="16" /></button><button class="icon-button" aria-label="收起我的想法" title="收起，草稿会保留" @click="close"><Minus :size="18" /></button></div>
      </header>
      <div class="thought-layout">
        <aside v-if="notesPanel.view === 'library'" class="thought-navigation">
          <button class="new-thought" @click="create"><Plus :size="18" />记下一个想法</button><button class="quiet" @click="openPersonalChat()">继续我的讨论</button>
          <label class="thought-search"><Search :size="16" /><input v-model="query" placeholder="寻找想法或关键词" aria-label="搜索我的想法" /></label>
          <div v-if="drafts.length" class="draft-list"><h2>还在写 <span>{{ drafts.length }}</span></h2><button v-for="value in drafts" :key="value.draft_id" :class="{ active: draft?.draft_id === value.draft_id }" @click="resume(value)"><span class="draft-dot" />{{ value.value.title || value.value.content.slice(0, 40) || '未命名的想法' }}<small>设备草稿 · {{ time(value.updated_at) }}</small></button></div>
          <div class="note-list"><h2>{{query.trim()?'找到的想法':'已保存的想法'}}</h2>
            <template v-if="query.trim()"><button v-for="hit in searchHits" :key="hit.reference.note_id" @click="select(hit.reference.note_id)"><strong>{{hit.title || hit.excerpt.slice(0,70)}}</strong><span>{{hit.excerpt}}</span><small>{{hit.context.label || '独立想法'}} · 第 {{hit.reference.revision}} 版</small></button><p class="muted" role="status">{{searching?'正在寻找…':searchState==='ready'?'按词语与意思寻找':searchState==='partial'?'部分想法仍在整理，词语查询已可用':'按词语寻找，语义检索暂未就绪'}}</p><p v-if="!searching && !searchHits.length" class="muted">没有找到相关想法，可以换个说法。</p></template>
            <template v-else><button v-for="note in filtered" :key="note.note_id" :class="{ active: selected?.note_id === note.note_id }" @click="select(note.note_id)"><strong>{{ title(note) }}</strong><span>{{ note.value.record.content.slice(0, 110) }}</span><small>{{ note.value.context.label || '独立想法' }} · 第 {{ note.current_revision }} 版</small></button><p v-if="!filtered.length && !loading" class="muted">写一句疑问、联想，或自己的解释。</p><button v-if="cursor" :disabled="loading" @click="load(true)">继续加载</button></template><p v-if="loading" class="muted" role="status">正在读取想法…</p>
          </div>
        </aside>
        <div v-else class="thought-detail" :class="{ 'is-writing': draft && tab === 'write' }">
          <template v-if="selected || draft">
            <button v-if="notesPanel.view === 'detail'" class="mobile-back" @click="openPersonalNotes()"><ArrowLeft :size="16" />全部想法</button>
            <div v-if="selected" class="thought-meta"><span>{{ draft?.value.context.label || selected?.value.context.label || '随时记录的想法' }}</span><span>第 {{ selected.current_revision }} 版 · {{ time(selected.value.saved_at) }}</span></div>
            <nav v-if="selected" class="thought-tabs" aria-label="笔记视图"><button :aria-pressed="tab === 'write'" @click="tab = 'write'"><FileText :size="15" />当前表述</button><button :aria-pressed="tab === 'history'" @click="tab = 'history'"><History :size="15" />思考过程</button><button :aria-pressed="tab === 'links'" @click="tab = 'links'"><Link2 :size="15" />联系 <span>{{ links.length || '' }}</span></button></nav>
            <NoteThinking v-if="selected && tab==='history'" :note-id="selected.note_id" :revision="selected.current_revision" @revision="readThinkingRevision" @discussion="readDiscussion" />
            <template v-if="tab === 'write'">
              <div v-if="draft" class="thought-compose" data-workspace-input><input v-model="draft.value.title" class="thought-title-input" placeholder="一个念头…" aria-label="想法标题" title="标题可以留空" :readonly="saving || !!draft.pending" @input="changed" /><textarea ref="textInput" v-model="draft.value.content" class="thought-writing" placeholder="想到了什么？先写下来。" aria-label="想法正文" @compositionstart="composing=true" @compositionend="composing=false" :readonly="saving || !!draft.pending" @input="changed" /><div class="draft-status-line"><p class="muted draft-status" :role="error || draftStorageError ? 'alert' : undefined" :class="{ 'storage-error': draftStorageError || error }">{{ draftStorageError ? '草稿暂未写入设备' : error || (draft.pending ? '保存结果待确认，可重试' : '草稿已保留') }}</p><div class="thought-context"><button v-if="canChooseContext" class="context-toggle" :disabled="saving || !!draft.pending" :aria-expanded="contextMenuOpen" aria-label="选择想法来源" :title="contextLabel" @click="toggleContextMenu"><MapPin :size="12" /><span>{{ contextLabel }}</span><ChevronDown :size="12" /></button><span v-else class="context-fixed" :title="contextLabel"><MapPin :size="12" />{{ contextLabel }}</span><div v-if="contextMenuOpen" class="context-options"><p>记录它从哪里产生</p><button v-if="contextCandidate" @click="chooseContext(contextCandidate)"><MapPin :size="14" /><span>关联当前位置<small>{{ contextCandidate.label || '当前阅读位置' }}</small></span></button><button @click="chooseContext(null)"><PenLine :size="14" /><span>独立想法<small>不关联页面位置</small></span></button></div></div></div></div>
              <template v-else-if="selected"><h2 class="thought-title">{{ title(selected) }}</h2><p class="muted">{{ author(selected.value) }}</p><div class="md thought-prose" v-html="renderMarkdown(currentText)"></div></template>
              <details v-if="(draft?.value.source ?? selected?.value.record.note)?.retained_excerpt" class="thought-excerpt"><summary>{{ (draft?.value.source ?? selected?.value.record.note)?.retained_excerpt?.kind === 'assistant' ? '助手摘录' : '原文摘录' }}</summary><div class="md" v-html="renderMarkdown((draft?.value.source ?? selected?.value.record.note)!.retained_excerpt!.text)"></div></details>
              <div v-if="conflict" class="thought-conflict"><h3>另一处已保存了第 {{ conflict.current_revision }} 版</h3><p>你的修改仍在上方。对照后可继续整合，再明确保存。</p><pre>{{ conflict.value.record.content }}</pre><button @click="mergeAndSave">将上方内容保存为下一版</button></div>
              <footer class="thought-actions">
                <button v-if="draft" class="discard-draft" aria-label="放弃这份草稿" title="放弃这份草稿" :disabled="saving || !!draft.pending" @click="discard"><Trash2 :size="15" /></button><button v-else @click="edit">继续写</button>
                <div class="thought-placement"><button class="placement-toggle" :title="draft?.destination?.label" :aria-expanded="placementMenuOpen" :disabled="saving || placing || (!!draft?.pending && !draft?.saved_note)" @click="togglePlacementMenu"><MapPin :size="13" /><span>{{ draft?.destination ? '已选位置' : '放到…' }}</span><ChevronDown :size="12" /></button>
                  <div v-if="placementMenuOpen" class="placement-options" aria-label="选择想法放置位置"><p>让想法留在相关内容旁</p><button v-if="draft" @click="onlySave"><span>只保存想法<small>默认 · 不放到页面里</small></span><Check v-if="!draft.destination" :size="14" /></button><button v-for="choice in placementChoices" :key="choice.key" :disabled="placing" @click="choosePlacement(choice)"><span>{{ choice.label }}<small>{{ choice.detail }}</small></span></button><p v-if="!placementChoices.length">打开画布、点选一条消息或打开演示，就能选择放置位置。</p><p v-if="placing" role="status">正在记录位置…</p><p v-if="draft?.destination" class="chosen-location">已选：{{ draft.destination.label }}</p></div>
                </div><span class="action-spacer" /><button v-if="selected" :disabled="saving" @click="discuss">一起推敲 <ArrowUpRight :size="15" /></button><button v-if="selected" :disabled="saving" @click="practice">练一练</button><button v-if="draft" class="primary" title="Ctrl / ⌘ + Enter 保存" :disabled="saving || placing || capturingSource || (!draft.value.content.trim() && !draft.value.source?.retained_excerpt)" @click="save">{{ saving ? '正在保存…' : capturingSource ? '正在记录现场…' : draft.saved_note ? '重试放置' : draft.pending ? '重试这次保存' : draft.destination ? '保存并放置' : '保存想法' }}<Check :size="15" /></button>
              </footer>
              <NoteThinking v-if="selected" :note-id="selected.note_id" :revision="selected.current_revision" compact @revision="readThinkingRevision" @discussion="readDiscussion" />
              <details v-if="selected && (placements.length || boardPlacements.length)" class="thought-locations"><summary>放在 {{ placements.length + boardPlacements.length }} 个位置</summary><div v-for="item in placements" :key="item.attachment.attachment_id"><button @click="pendingNotePlacementLocation={kind:'attachment',target:item.attachment.target,label:item.attachment.label}">{{ item.attachment.label }} ↗</button><button aria-label="移除这个放置位置" @click="removePlacement(item.attachment.attachment_id)">×</button></div><div v-for="location in boardPlacements" :key="location.board_id+location.item_id"><button @click="openBoardPlacement(location)">画布 · {{ location.label }} ↗</button><button aria-label="从这个画布移除" @click="removeBoardPlacement(location)">×</button></div></details>
            </template>
            <template v-if="tab === 'history' && selected">
              <article v-if="historyVersion" ref="historicalReader" class="historical-note"><header><h3>第 {{ historyVersion.revision }} 版 · 只读</h3><button @click="comparing = !comparing">{{ comparing ? '阅读原文' : '对照当前版' }}</button></header><label v-if="comparing" class="muted">对照版本<select v-model.number="comparisonRevision" aria-label="对照版本"><option :value="0">当前版</option><option v-for="version in history" :key="version.revision" :value="version.revision">第 {{version.revision}} 版</option></select></label><div v-if="comparing" class="note-diff"><div v-for="(line, index) in differences" :key="index" :class="line.kind"><span>{{ line.kind === 'added' ? '+ 新' : line.kind === 'removed' ? '− 旧' : '　' }}</span><pre>{{ line.text || ' ' }}</pre></div></div><div v-else class="md" v-html="renderMarkdown(historyVersion.record.content)"></div><blockquote v-if="historyVersion.record.note?.retained_excerpt">{{ historyVersion.record.note.retained_excerpt.text }}</blockquote><button :disabled="saving || !!draft?.pending" @click="restore(dirty)">{{ dirty ? '先保存当前修改，再恢复此版本' : '从此版本继续写（保存为新版本）' }}</button></article>
            </template>
            <template v-if="tab === 'links' && selected">
              <p class="muted">把互相启发的想法联系起来，也可以写下为什么。</p>
              <article v-for="item in links" :key="item.link_id" class="thought-link"><button @click="select(linkedId(item))">{{ items.find(note => note.note_id === linkedId(item)) ? title(items.find(note => note.note_id === linkedId(item))!) : '打开相连的想法' }}<ChevronRight :size="16" /></button><template v-if="editingLink?.link_id === item.link_id"><textarea v-model="editedReason" aria-label="修改联系说明" /><button @click="updateLink">保存联系说明</button><button @click="editingLink=null">取消修改</button></template><p v-else>{{ item.reason || '我建立的联系' }}</p><small>建立时：<button class="quiet" @click="openPersonalNotes(item.from)">第 {{item.from.revision}} 版</button> ↔ <button class="quiet" @click="openPersonalNotes(item.to)">第 {{item.to.revision}} 版</button></small><small v-if="item.reason_refs?.length">说明依据：<button v-for="(reference,index) in item.reason_refs" :key="reference.note_id" class="quiet" @click="openPersonalNotes(reference)">想法 {{index+1}} · 第 {{reference.revision}} 版</button></small><button class="quiet" @click="editingLink=item;editedReason=item.reason || ''">修改说明</button><button class="quiet" @click="unlink(item.link_id)">移除联系</button></article>
              <form class="thought-link-form" @submit.prevent="link"><label>寻找要连接的想法<input v-model="linkQuery" aria-label="寻找要连接的想法" placeholder="搜索全部已保存想法" /></label><label>连接另一条想法<select v-model="linkTarget" aria-label="连接另一条想法"><option value="">选择想法</option><option v-for="option in linkOptions" :key="option.id" :value="option.id">{{ option.title }}</option></select></label><label>联系说明（可选）<textarea v-model="linkReason" placeholder="它们在哪一点相互启发？" /></label><button :disabled="!linkTarget" class="primary">建立联系</button><button type="button" :disabled="!linkTarget" @click="suggestLink">请 Agent 建议联系</button></form>
            </template>
            <aside v-if="(selected && (shownContext?.book_id || shownContext?.session_id || shownContext?.origin)) || shownOrigins?.source_refs.length || shownOrigins?.based_on_turn" class="thought-provenance">
              <h3>从哪里生长出来</h3>
              <button v-if="shownContext?.book_id || shownContext?.session_id || shownContext?.origin" :disabled="!!pendingNoteSource || !!pendingNotePlacementLocation" @click="returnToSource">回到记录现场<span v-if="shownContext.video_time_ms != null"> · {{ Math.floor(shownContext.video_time_ms / 60000) }}:{{ String(Math.floor(shownContext.video_time_ms / 1000) % 60).padStart(2,'0') }}</span><span v-else-if="shownContext.lid"> · {{ shownContext.lid }}</span></button>
              <div v-for="(reference,index) in shownOrigins?.source_refs" :key="`${reference.note_id}:${reference.revision}`"><button @click="openPersonalNotes(reference)">查看来源想法 · 第 {{ reference.revision }} 版</button><button v-if="draft && tab === 'write'" class="quiet" :disabled="saving || !!draft.pending" @click="removeOrigin(index)">移除这条来源引用</button></div>
              <button v-if="shownOrigins?.based_on_turn" @click="readDiscussion(shownOrigins.based_on_turn.session_id,shownOrigins.based_on_turn.turn_id)">查看形成这版表述的讨论</button>
            </aside>
            <div v-if="selected" class="note-maintenance"><button v-if="!confirmDelete" class="quiet" @click="confirmDelete = true">删除想法…</button><template v-else><span>删除正文与版本，画板中的引用会显示已删除。</span><button :disabled="saving" @click="remove">确认删除</button><button @click="confirmDelete = false">保留</button></template></div>
          </template>
          <div v-else class="thought-empty"><span class="thought-mark">✳</span><h2>从你的一个想法开始。</h2><p>疑问、联想、还没想清楚的半句话，<br />都可以先留在这里。</p><button class="primary" @click="create"><Plus :size="17" />写下此刻的想法</button></div>
          <article v-if="discussion" class="historical-note"><header><h3>{{discussion.title}} · 当时的讨论</h3><button @click="discussion=null">收起</button></header><p>{{discussion.turn.user}}</p><div class="md" v-html="renderMarkdown(discussion.turn.outcome?.answer || discussion.turn.error?.message || '这轮讨论尚未完成')"></div><button v-if="discussion.personal" @click="openPersonalChat(undefined,'discuss',undefined,discussion.session_id)">继续这条讨论</button></article>
          <p v-if="noteSourceError" class="thought-error" role="alert">{{ noteSourceError }}</p><p v-if="notice" class="thought-notice" role="status">{{ notice }}</p><p v-if="(error || draftStorageError) && (!draft || tab !== 'write')" class="thought-error" role="alert">{{ error || draftStorageError }}</p>
        </div>
      </div>
      <span v-for="edge in resizeEdges" :key="edge" class="thought-resize" :class="`resize-${edge}`" aria-hidden="true" @pointerdown="noteWindow.begin($event, edge)" />
      <button class="thought-resize resize-se" aria-label="调整想法窗口大小" aria-description="拖动边角，或使用方向键调整窗口大小" title="拖动调整大小" @pointerdown="noteWindow.begin($event, 'se')" @keydown="noteWindow.keyboard($event, 'se')"><svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M3 9 9 3M7 9 9 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg></button>
    </section>
  <div v-if="notesOwner()" v-show="!notesPanel.open" ref="launcherElement" class="thought-toolbar" :class="{ 'is-dragging': launcher.dragging.value, 'keyboard-open': launcher.keyboardOpen.value }" :style="launcher.style.value" data-personal-notes>
    <span v-if="notice" class="toolbar-notice" role="status"><Check :size="14" />{{ notice }}</span>
    <button class="thought-entry" data-tour="thought-entry" aria-label="记一下" :title="drafts.length ? '继续草稿 · 可拖动 · Ctrl / ⌘ + Shift + M' : '记一下 · 可拖动 · Ctrl / ⌘ + Shift + M'" aria-description="点击直接记录；拖动或按 Alt 加方向键移动入口" aria-keyshortcuts="Control+Shift+M Meta+Shift+M" aria-controls="personal-thought-window" :aria-expanded="notesPanel.open && notesPanel.view === 'capture'" @pointerdown="launcher.begin" @keydown="launcher.keyboard" @click="launcher.click($event, capture)"><PenLine :size="17" /><span>记一下</span><span v-if="drafts.length" class="draft-dot" aria-label="有未完成的草稿" /></button>
  </div>
</template>

<style scoped>
.thought-space { position: fixed; z-index: 10000; box-sizing: border-box; min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden; background: #fffdf7; color: #39352e; border: 1px solid #ddd2bf; border-radius: 18px; box-shadow: 0 16px 48px #3f2e1b24, 0 3px 10px #3f2e1b12, inset 0 1px #ffffff; outline: none; font-size: 14px; }
.thought-space.is-moving { user-select: none; box-shadow: 0 22px 56px #3f2e1b30, 0 4px 12px #3f2e1b14; }
.thought-heading { position: relative; flex: none; display: flex; justify-content: space-between; align-items: center; min-height: 54px; padding: 0 12px 0 18px; cursor: grab; touch-action: none; user-select: none; background: linear-gradient(180deg, #f9f3e7, #fcf8ef); border-bottom: 1px solid #eadfcb80; }
.is-moving .thought-heading { cursor: grabbing; }
.thought-window-title, .thought-window-tools { display: flex; align-items: center; gap: 8px; }
.thought-pen { color: #b57549; display: flex; }
.thought-drag-grip { position: absolute; display: grid; place-items: center; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 36px; height: 32px; color: #c5b79e; border-radius: 6px; }
.thought-drag-grip:focus-visible { outline: 2px solid #b57549; outline-offset: 0; }
h1 { font-size: 14px; font-weight: 600; letter-spacing: .06em; margin: 0; color: #77634d; }
h2, h3, p { margin-top: 0; }
button, input, textarea, select { font: inherit; }
button { cursor: pointer; min-height: 34px; }
button:disabled { opacity: .55; cursor: default; }
button:focus-visible, input:focus-visible, textarea:focus-visible, select:focus-visible, summary:focus-visible { outline: 2px solid #b57549; outline-offset: 2px; }
.icon-button { display: grid; place-items: center; width: 32px; min-height: 32px; border: 0; border-radius: 8px; background: none; padding: 6px; color: #96856f; }
.icon-button:hover { background: #e9ddc960; color: #70573d; }
.thought-layout { min-height: 0; flex: 1; display: flex; flex-direction: column; }
.thought-navigation { flex: 1; overflow: auto; padding: 16px; overscroll-behavior: contain; }
.new-thought { display: flex; gap: 9px; align-items: center; width: 100%; border: 1px solid #dfc5ad; background: #f4e9d9; color: #865b3e; border-radius: 10px; padding: 10px 13px; text-align: left; }
.thought-search { display: flex; align-items: center; gap: 8px; margin: 16px 0; color: var(--steel, #687066); }
.thought-search input { border: 0; border-bottom: 1px solid var(--line, #dedfd6); background: transparent; min-width: 0; width: 100%; padding: 9px 0; }
.note-list h2, .draft-list h2 { font-size: 11px; font-weight: 500; letter-spacing: .04em; color: var(--steel, #687066); margin: 20px 6px 8px; }
.note-list > button, .draft-list > button { width: 100%; text-align: left; border: 1px solid transparent; background: transparent; padding: 12px 10px; display: flex; flex-direction: column; gap: 6px; border-radius: 8px; color: inherit; margin-bottom: 5px; overflow-wrap: anywhere; }
.note-list > button.active, .draft-list > button.active { border-color: #b9ccbd; background: var(--surface, #f3f1e9); }
.note-list > button:hover, .draft-list > button:hover { background: var(--surface, #f3f1e9); }
.note-list strong { font-weight: 550; font-size: 14px; line-height: 1.6; }
.note-list button > span { font-size: 12px; color: var(--steel, #687066); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.note-list small, .draft-list small { font-size: 10px; color: var(--steel, #687066); }
.draft-dot { width: 5px; height: 5px; border-radius: 50%; background: #bc8c39; }
.thought-detail { flex: 1; overflow: auto; padding: 16px 22px 16px; min-width: 0; overscroll-behavior: contain; scrollbar-width: thin; scrollbar-color: #ded2bd transparent; }
.thought-detail.is-writing { display: flex; flex-direction: column; }
.thought-meta { display: flex; justify-content: space-between; gap: 8px; flex-wrap: wrap; flex: none; font-size: 11px; color: var(--steel, #687066); margin-bottom: 12px; overflow-wrap: anywhere; }
.thought-tabs { display: flex; flex: none; gap: 14px; border-bottom: 1px solid var(--line, #dedfd6); margin-bottom: 18px; }
.thought-tabs button { display: flex; gap: 5px; align-items: center; border: 0; border-bottom: 2px solid transparent; background: none; color: var(--steel, #687066); padding: 0 0 8px; font-size: 12px; }
.thought-tabs button[aria-pressed=true] { color: #335c46; border-color: #335c46; }
.thought-compose { display: flex; flex-direction: column; flex: 1 0 112px; min-height: 0; }
.thought-title-input { flex: none; border: 0; border-radius: 0; background: transparent; font-size: 18px; font-weight: 600; line-height: 1.6; width: 100%; box-sizing: border-box; color: #4b4033; margin: 0 0 8px; padding: 0; box-shadow: none; }
.thought-title-input::placeholder { color: #b7a88e; font-weight: 500; }
.thought-writing { flex: 1; border: 0; border-radius: 0; background: transparent; box-sizing: border-box; width: 100%; min-height: 32px; resize: none; font-size: 16px; line-height: 1.9; letter-spacing: .01em; color: inherit; padding: 0; box-shadow: none; scrollbar-width: thin; scrollbar-color: #ded2bd transparent; }
.thought-writing::placeholder { color: #a89c89; }
.thought-writing:focus, .thought-title-input:focus { outline: none; border: 0; background: transparent; box-shadow: none; }
.thought-title { font-size: 19px; line-height: 1.6; overflow-wrap: anywhere; }
.thought-prose { line-height: 1.9; overflow-wrap: anywhere; }
.muted { font-size: 11px; line-height: 1.7; color: var(--steel, #687066); }
.draft-status-line { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex: none; margin-top: 8px; }
.draft-status { margin: 0; min-width: 0; font-size: 10px; color: #a2937d; }
.thought-context { position: relative; min-width: 0; max-width: 65%; flex: none; }
.context-toggle, .context-fixed { display: flex; align-items: center; gap: 4px; min-height: 24px; padding: 3px 0 3px 5px; border: 0; background: transparent; font-size: 10px; color: #97866e; }
.context-toggle span, .context-fixed { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.context-toggle { max-width: 100%; }
.context-toggle svg, .context-fixed svg { flex: none; }
.context-toggle:hover { color: #8b5f3f; }
.context-options { position: absolute; right: -6px; bottom: calc(100% + 6px); z-index: 3; width: 225px; padding: 7px; background: #fffdf7; border: 1px solid #dfd2bc; border-radius: 12px; box-shadow: 0 8px 28px #4a341d24; }
.context-options p { font-size: 10px; color: #a18f76; margin: 3px 7px 5px; }
.context-options button { display: flex; align-items: flex-start; gap: 8px; width: 100%; text-align: left; background: transparent; border: 0; border-radius: 7px; padding: 8px; color: #705a40; font-size: 12px; }
.context-options button:hover { background: #f7efe1; }
.context-options button > span { min-width: 0; }
.context-options small { display: block; font-size: 10px; color: #a18f76; margin-top: 4px; overflow-wrap: anywhere; }
.storage-error { color: #8b5026; }
.thought-actions { display: flex; flex: none; align-items: center; flex-wrap: wrap; gap: 6px; padding: 10px 0 0; border-top: 1px solid #eee5d5; margin-top: 10px; }
.thought-placement{position:relative;min-width:0}.thought-actions .placement-toggle{border:0;color:#9b805c;padding:7px 2px;gap:4px;font-size:11px}.placement-options{position:absolute;left:-28px;bottom:calc(100% + 8px);z-index:5;box-sizing:border-box;width:252px;max-width:calc(100vw - 60px);max-height:230px;overflow:auto;padding:8px;background:#fffdf7;border:1px solid #dfd2bc;border-radius:12px;box-shadow:0 8px 28px #4a341d24}.placement-options p{font-size:10px;line-height:1.7;color:#a18f76;margin:4px 7px}.thought-actions .placement-options button{display:flex;justify-content:space-between;align-items:center;text-align:left;width:100%;padding:8px;border:0;background:none;color:#705a40;font-size:12px}.placement-options button:hover{background:#f7efe1}.placement-options small{display:block;font-size:10px;line-height:1.6;color:#a18f76;margin-top:4px;overflow-wrap:anywhere}.thought-locations{margin-top:12px;font-size:11px;color:#927958}.thought-locations summary{cursor:pointer}.thought-locations div{display:flex;gap:8px;margin:6px 0}.thought-locations button{border:0;background:none;text-align:left;font:inherit;color:inherit;overflow-wrap:anywhere}.thought-locations div>button:first-child{flex:1}
.thought-actions button, .primary, .historical-note > button, .thought-conflict button, .thought-link-form button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; border: 1px solid var(--line, #dedfd6); border-radius: 7px; background: transparent; color: inherit; padding: 7px 10px; font-size: 12px; }
.primary, .thought-actions .primary, .thought-link-form .primary { background: #ad7049; color: #fffdf7; border-color: #ad7049; border-radius: 9px; padding: 8px 13px; box-shadow: 0 1px 2px #70432314; }
.primary:hover:not(:disabled), .new-thought:hover { filter: brightness(.96); }
.thought-actions .discard-draft { border: 0; background: none; box-shadow: none; color: #ab9c87; padding: 8px; margin-left: -8px; }
.thought-actions .discard-draft:hover { color: #9f593b; background: #f3eadd; }
.action-spacer { flex: 1; }
.thought-excerpt { flex: none; margin: 12px 0 0; padding: 10px 12px; border-left: 2px solid #a9baab; background: var(--surface, #f6f6ee); font-size: 12px; overflow-wrap: anywhere; }
.thought-excerpt summary { cursor: pointer; font-size: 11px; color: var(--steel, #687066); }
.thought-excerpt .md { margin-top: 10px; max-height: 180px; overflow: auto; }
.thought-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 30px 0; }
.thought-empty h2 { font-size: 19px; font-weight: 500; }
.thought-empty p { font-size: 13px; line-height: 1.9; color: var(--steel, #687066); }
.thought-mark { font-size: 30px; color: #739a71; margin-bottom: 15px; }
.thought-timeline { display: flex; flex-wrap: wrap; gap: 8px; margin: 16px 0; }
.thought-timeline button { display: flex; flex-direction: column; gap: 6px; text-align: left; border: 1px solid var(--line, #dedfd6); border-radius: 8px; background: transparent; padding: 10px; min-width: 125px; font-size: 12px; }
.thought-timeline button[aria-pressed=true] { border-color: #335c46; background: #335c460c; }
.thought-timeline small { font-size: 10px; color: var(--steel, #687066); }
.historical-note { margin: 18px 0; padding: 12px; border: 1px solid var(--line, #dedfd6); border-radius: 8px; overflow-wrap: anywhere; line-height: 1.8; }
.historical-note header { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.historical-note h3 { font-size: 13px; margin: 0; }
.historical-note header button { border: 0; background: none; font-size: 11px; color: #335c46; }
.note-diff { margin: 12px 0; font-size: 12px; }
.note-diff > div { display: flex; gap: 5px; }
.note-diff span { flex-shrink: 0; font-size: 10px; min-width: 30px; padding: 4px; }
.note-diff pre { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; min-width: 0; padding: 4px; }
.note-diff .added { background: #e8f1e6; }
.note-diff .removed { background: #f9ebe3; }
.thought-link { border-bottom: 1px solid var(--line, #dedfd6); padding: 12px 0; }
.thought-link > button:first-child { display: flex; align-items: center; gap: 8px; border: 0; background: none; padding: 0; font-weight: 550; color: inherit; }
.thought-link p { font-size: 13px; margin: 8px 0; }
.thought-link small { display: block; font-size: 11px; color: var(--steel, #687066); }
.quiet { border: 0; background: none; color: var(--steel, #687066); font-size: 11px; }
.thought-link-form { display: flex; flex-direction: column; gap: 12px; align-items: flex-start; margin-top: 20px; }
.thought-link-form label { display: flex; flex-direction: column; gap: 8px; width: 100%; font-size: 12px; }
.thought-link-form input, .thought-link-form select, .thought-link-form textarea { border: 1px solid var(--line, #dedfd6); border-radius: 7px; background: transparent; color: inherit; padding: 9px; min-width: 0; }
.note-maintenance { margin-top: 24px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; font-size: 12px; }
.note-maintenance button { border: 0; background: transparent; color: var(--steel, #687066); }
.thought-error { flex: none; padding: 10px; background: #fff1e2; color: #794619; border-radius: 8px; font-size: 12px; line-height: 1.7; margin: 10px 0 0; }
.thought-notice { color: #335c46; font-size: 12px; padding: 10px 0; }
.thought-conflict { padding: 12px; border: 1px solid #bd9d67; border-radius: 8px; margin: 14px 0; font-size: 12px; }
.thought-conflict pre { white-space: pre-wrap; overflow-wrap: anywhere; background: var(--surface, #f6f6ee); padding: 10px; }
.related-discussions { margin-top: 20px; font-size: 12px; line-height: 1.8; }
.related-discussions small { color: var(--steel, #687066); }
.mobile-back { display: flex; flex: none; align-items: center; gap: 6px; border: 0; background: none; padding: 0; font-size: 12px; color: var(--steel, #687066); margin-bottom: 12px; }
.md :deep(pre), .md :deep(table), .md :deep(.katex-display) { max-width: 100%; overflow-x: auto; }
.thought-provenance { flex: none; border-top: 1px solid var(--line, #dedfd6); margin-top: 18px; padding-top: 14px; display: flex; gap: 8px; flex-wrap: wrap; }
.thought-provenance h3 { width: 100%; font-size: 12px; margin-bottom: 0; }
.thought-provenance button { background: transparent; border: 1px solid var(--line, #dedfd6); border-radius: 7px; padding: 7px 9px; color: var(--steel, #687066); font-size: 11px; }
.thought-link textarea { width: 100%; box-sizing: border-box; min-height: 90px; }
.thought-toolbar { position: fixed; z-index: 10000; width: max-content; pointer-events: none; }
.thought-toolbar .thought-entry { pointer-events: auto; }
/* Keep the receipt and measured size, while yielding all input controls to the keyboard. */
.thought-toolbar.keyboard-open .thought-entry { visibility: hidden; pointer-events: none; }
.thought-toolbar .thought-entry { display: inline-flex; align-items: center; justify-content: center; gap: 8px; box-sizing: border-box; height: 44px; padding: 0 15px; border: 1px solid #dfd2be; border-radius: 24px; background: #fffaf0; color: #896243; font: inherit; font-size: 13px; white-space: nowrap; cursor: pointer; touch-action: none; user-select: none; box-shadow: 0 3px 12px #49341b14, 0 1px 3px #49341b09; }
.thought-toolbar .thought-entry:active { transform: none; }
.thought-toolbar .thought-entry:hover { background: #f7eddd; border-color: #c9b18e; }
.thought-toolbar .thought-entry:focus-visible { outline: 2px solid var(--brand-accent); outline-offset: 3px; }
.thought-toolbar.is-dragging .thought-entry { cursor: grabbing; box-shadow: 0 8px 24px #49341b26; }
.thought-toolbar .draft-dot { position: absolute; top: 7px; right: 8px; width: 5px; height: 5px; border-radius: 50%; background: #b68055; }
.toolbar-notice { position: absolute; right: 0; bottom: calc(100% + 8px); display: flex; align-items: center; gap: 5px; width: max-content; max-width: 240px; padding: 6px 10px; border: 1px solid #e6dfd0; border-radius: 9px; background: #fffdf7; color: #786b56; font-size: 11px; pointer-events: none; }
.toolbar-notice svg { flex: none; }
.thought-resize { position: absolute; z-index: 2; touch-action: none; }
.resize-n, .resize-s { left: 18px; right: 18px; height: 6px; cursor: ns-resize; }
.resize-n { top: 0; } .resize-s { bottom: 0; }
.resize-e, .resize-w { top: 18px; bottom: 18px; width: 6px; cursor: ew-resize; }
.resize-e { right: 0; } .resize-w { left: 0; }
.resize-ne, .resize-nw, .resize-se, .resize-sw { width: 18px; height: 18px; }
.resize-ne { top: 0; right: 0; cursor: nesw-resize; } .resize-nw { top: 0; left: 0; cursor: nwse-resize; }
.resize-sw { bottom: 0; left: 0; cursor: nesw-resize; }
.thought-space .resize-se { right: 1px; bottom: 1px; display: grid; place-items: center; width: 20px; height: 20px; min-height: 0; padding: 0; color: #bbae96; cursor: nwse-resize; background: transparent; border: 0; border-radius: 0; }
@media (max-width: 599px) {
  .thought-detail { padding: 12px 18px 14px; }
  .thought-heading { min-height: 50px; }
  .thought-meta { margin-bottom: 8px; }
}
</style>
