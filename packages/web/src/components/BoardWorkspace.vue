<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { ArrowLeft, ArrowUp, BookOpen, Check, ChevronDown, CornerDownRight, LayoutDashboard, LoaderCircle, MessageSquareText, Minimize, PanelRightClose, PanelRightOpen, Pause, Play, Plus, RefreshCw, Search, Shapes, SkipBack, SkipForward, Square, Trash2, X } from '@lucide/vue';
import { api as sharedApi, type BoardCatalogItem, type BoardConversationSession, type BoardPrebuildCategory, type BoardDocument, type BoardImportSource, type BoardSummary, type BoardView } from '../api';
import { bindSceneApi, readerKey } from '../network-context';
import { requestWorkspaceRecovery } from '../network-client';
import { t as uiText } from '../i18n';
import { boardDeliveryKey, mergeBoardDeliveries, boardDate, boardKindDescriptions, boardKindLabels, boardKinds, boardTextPreview, boardVisualKey, projectBoardRoute, defaultBoardView, type BoardKind, type BoardSourceRequest, type BoardQuestionRequest, type BoardRunView, type BoardLocation } from '../board-workspace';
import type { BoardSearchHit } from '../generated/BoardSearchHit';
import type { BoardDelivery } from '../generated/BoardDelivery';
import { boardArtifactLabels } from '../board-workspace';
import type { BoardContentRef } from '../generated/BoardContentRef';
import type { BoardVisualTarget } from '../generated/BoardVisualTarget';
import BoardCanvas from './BoardCanvas.vue';
import BoardReadingPane from './BoardReadingPane.vue';
import BoardDiscussion from './BoardDiscussion.vue';
import { discussionTurnKey, mergeBoardConversation, type BoardDiscussionTurn } from '../board-conversation';

const props = defineProps<{ boardId?: string | null; pendingImport?: BoardImportSource | null; run?: BoardRunView | null; pendingLocate?: BoardLocation | null }>();
const emit = defineEmits<{
  (event: 'board-open', boardId: string): void;
  (event: 'board-deleted', boardId: string): void;
  (event: 'close'): void;
  (event: 'source', request: BoardSourceRequest, anchor: HTMLElement): void;
  (event: 'imported'): void;
  (event: 'follow-up', request: BoardQuestionRequest): void;
  (event: 'retry-follow-up', requestId: string): void;
  (event: 'cancel-follow-up'): void;
  (event: 'located'): void;
}>();
const api = bindSceneApi(sharedApi, readerKey);
const root = ref<HTMLElement>();
const board = ref<BoardDocument | null>(null);
const view = ref<BoardView>(defaultBoardView());
const boards = ref<BoardSummary[]>([]);
const loading = ref(false), saving = ref(false), mutating = ref(false);
const error = ref(''), savedNotice = ref('');
const galleryOpen = ref(!props.boardId), addOpen = ref(false), deleteOpen = ref(false);
const newTitle = ref('');
const searchQuery = ref(''), searchResults = ref<BoardSearchHit[]>([]), searchLoading = ref(false), searchError = ref('');
const drafts = ref<Record<string, string>>({}), preparingQuestion = ref(false), questionError = ref('');
const submitted = ref<BoardQuestionRequest | null>(null), submittedTitle = ref('');
const deliveryNotice = ref<BoardDelivery | null>(null), deliveryError = ref(''), refreshingDelivery = ref(false);
const acknowledgedRequests = new Set<string>(), restoredRequests = new Set<string>();
let searchSequence = 0;
let queuedDelivery: BoardDelivery | null = null;
let failedDelivery: BoardDelivery | null = null;
let deliveryRefreshTask: Promise<void> | null = null;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let openingBoard: { id: string; promise: Promise<void> } | null = null;
const mobileTab = ref<'canvas' | 'reading' | 'discussion'>('canvas');
const railTab = ref<'reading' | 'discussion'>('discussion');
const railCollapsed = ref(false), canvasFullscreen = ref(false), fullscreenQuestionOpen = ref(false);
const fullscreenNotice = ref('');
let previousLayout: { railCollapsed: boolean; railTab: 'reading' | 'discussion'; mobileTab: 'canvas' | 'reading' | 'discussion' } | null = null;
let ownsBrowserFullscreen = false;
let browserFullscreenActive = false;
const conversationSessions = ref<BoardConversationSession[]>([]), conversationLoading = ref(false), conversationError = ref('');
const retainedDiscussion = ref<Record<string, BoardDiscussionTurn[]>>({});
let conversationSequence = 0;
let conversationTimer: ReturnType<typeof setTimeout> | undefined;
const discussionTurns = computed(() => mergeBoardConversation(conversationSessions.value, retainedDiscussion.value[board.value?.board_id ?? ''] ?? []));
const catalogKind = ref<BoardKind>('prebuild');
const catalogCategory = ref<BoardPrebuildCategory>('all'), catalogTotal = ref<number | null>(null);
let catalogTimer: ReturnType<typeof setTimeout> | undefined;
const catalog = ref<BoardCatalogItem[]>([]), catalogLoading = ref(false), catalogQuery = ref(''), catalogError = ref('');
const catalogNext = ref<number | null>(null);
const relatedItem = ref<BoardCatalogItem | null>(null), relatedBoards = ref<BoardSearchHit[]>([]), relatedLoading = ref(false), relatedError = ref('');
let relatedSequence = 0;
const visited = ref<string[]>([]);
const boardMount = ref(0);
const canvas = ref<InstanceType<typeof BoardCanvas>>();
const panes = new Map<string, InstanceType<typeof BoardReadingPane>>();
const pendingPositions = new Map<string, { x: number; y: number }>();
let pendingTitle: string | undefined;
let savingTitle: string | undefined;
const savingPositions = new Map<string, { x: number; y: number }>();
let viewDirty = false;
let timer: ReturnType<typeof setTimeout> | undefined;
let savePromise: Promise<boolean> | null = null;
let loadSequence = 0, catalogSequence = 0;
let handledImport: BoardImportSource | null = null;
let disposed = false;
const dirty = ref(false);
const activeItem = computed(() => board.value?.items.find(item => item.item_id === view.value.selected_item_id) ?? null);
const readingItem = computed(() => board.value?.items.find(item => item.item_id === (view.value.reading_item_id ?? view.value.selected_item_id)) ?? null);
const questionItem = computed(() => view.value.selected_block_id ? readingItem.value : activeItem.value);
const playing = ref(false), animationProgress = ref(1);
const manualHighlights = ref<BoardVisualTarget[]>([]), mappingChoices = ref<BoardContentRef[]>([]);
let playbackTimer: ReturnType<typeof setTimeout> | undefined;
let playbackSequence = 0;
const routes = computed(() => board.value?.routes ?? []);
const currentRoute = computed(() => routes.value.find(route => route.route_id === view.value.playback?.route_id) ?? null);
const routeProjection = computed(() => board.value ? projectBoardRoute(board.value, currentRoute.value, view.value.playback?.step_id ?? null, view.value.playback?.show_all ?? false) : null);
const highlightedTargets = computed(() => {
  if (manualHighlights.value.length) return manualHighlights.value;
  if (view.value.playback && !view.value.playback.show_all) return routeProjection.value?.highlightTargets ?? [];
  return view.value.selected_block_id || view.value.selected_item_id === readingItem.value?.item_id
    ? (board.value?.mappings ?? []).filter(mapping => mapping.explanation.item_id === readingItem.value?.item_id && mapping.explanation.block_id === view.value.selected_block_id).map(mapping => mapping.visual) : [];
});
const focusedPresentationId = computed(() => highlightedTargets.value.find(target => target.kind === 'presentation_anchor')?.item_id ?? null);
const graphicProgress = computed(() => {
  const result: Record<string, number> = {}, projection = routeProjection.value;
  if ((playing.value || animationProgress.value < 1) && projection && !view.value.playback?.show_all) for (const target of projection.step?.reveal_targets ?? []) {
    if (!projection.previousTargets.has(boardVisualKey(target))) result[target.kind === 'link' ? target.link_id : target.item_id] = animationProgress.value;
  }
  return result;
});
const highlightedBlock = computed(() => view.value.selected_block_id ?? (view.value.playback?.show_all ? null : routeProjection.value?.step?.explanation?.block_id));
const question = computed({ get: () => board.value ? drafts.value[board.value.board_id] ?? '' : '', set: value => { if (board.value) drafts.value[board.value.board_id] = value; } });
const awaitingAcceptance = computed(() => !!submitted.value && !acknowledgedRequests.has(submitted.value.client_request_id) && props.run?.request_id !== submitted.value.client_request_id);
const runOnBoard = computed(() => {
  const run = props.run?.board_id === board.value?.board_id ? props.run : null;
  if (!run?.pending || !run.turn_id || !run.discussion) return run;
  const saved = conversationSessions.value.find(session => session.session_id === run.discussion?.sessionId)?.turns.find(turn => turn.turn_id === run.turn_id);
  return saved && saved.status !== 'pending_assistant' ? { ...run, pending: false, can_stop: false } : run;
});
const questionBusy = computed(() => preparingQuestion.value || awaitingAcceptance.value || !!(props.run?.board_id === board.value?.board_id ? runOnBoard.value?.pending : props.run?.pending));
const boardCommits = computed(() => mergeBoardDeliveries(runOnBoard.value?.commits));
const targetTitle = computed(() => runOnBoard.value?.item_id ? board.value?.items.find(item => item.item_id === runOnBoard.value?.item_id)?.title ?? submittedTitle.value : uiText('全画板'));
const visitedItems = computed(() => board.value?.items.filter(item => visited.value.includes(item.item_id)) ?? []);
const filteredCatalog = computed(() => { if (catalogKind.value === 'prebuild') return catalog.value; const query = catalogQuery.value.trim().toLocaleLowerCase(); return catalog.value.filter(item => !query || `${item.title}\n${item.summary}`.toLocaleLowerCase().includes(query)); });
const busy = computed(() => loading.value || mutating.value);
const saveLabel = computed(() => saving.value ? uiText('正在保存…') : error.value ? uiText('尚未保存') : dirty.value ? uiText('有未保存的修改') : uiText('已保存'));

function message(failure: unknown) { return failure instanceof Error ? failure.message : String(failure); }
function setPane(id: string, instance: unknown) { if (instance) panes.set(id, instance as InstanceType<typeof BoardReadingPane>); else panes.delete(id); }
function touch() { dirty.value = true; savedNotice.value = ''; clearTimeout(timer); timer = setTimeout(() => { void saveChanges(); }, 550); }
function rememberSelection(id: string | null) {
  if (id && !visited.value.includes(id)) visited.value.push(id);
}
function updateView(next: BoardView) { view.value = next; rememberSelection(next.selected_item_id); rememberSelection(next.reading_item_id); viewDirty = true; touch(); }
function updateCanvasView(next: BoardView) {
  if (!next.selected_item_id) manualHighlights.value = [];
  updateView({ ...view.value, x: next.x, y: next.y, zoom: next.zoom, selected_item_id: next.selected_item_id,
    ...(!next.selected_item_id ? { selected_block_id: null } : {}) });
}
function wholeBoard() { userControl(); manualHighlights.value = []; mappingChoices.value = []; updateView({ ...view.value, selected_item_id: null, selected_block_id: null }); }
async function showPanel(tab: 'reading' | 'discussion') {
  railCollapsed.value = false; railTab.value = tab; mobileTab.value = tab;
  if (tab === 'reading' && highlightedBlock.value && readingItem.value) {
    // A route can advance while this stack is hidden; locate only after it has a layout again.
    await nextTick();
    await panes.get(readingItem.value.item_id)?.locate(highlightedBlock.value, undefined, { focus: false });
  }
}
async function suggestQuestion(text: string) { if (!question.value.trim()) question.value = text; fullscreenQuestionOpen.value = true; showPanel('discussion'); await nextTick(); root.value?.querySelector('textarea')?.focus(); }
function collapseRail() { railCollapsed.value = true; mobileTab.value = 'canvas'; }
async function toggleQuestion() {
  fullscreenQuestionOpen.value = !fullscreenQuestionOpen.value;
  if (fullscreenQuestionOpen.value) { await nextTick(); root.value?.querySelector('textarea')?.focus(); }
}
function restoreFullscreenLayout() {
  canvasFullscreen.value = false; fullscreenNotice.value = ''; fullscreenQuestionOpen.value = false;
  if (previousLayout) {
    railCollapsed.value = previousLayout.railCollapsed; railTab.value = previousLayout.railTab; mobileTab.value = previousLayout.mobileTab;
    previousLayout = null;
  }
  browserFullscreenActive = false;
}
async function exitCanvasFullscreen() {
  restoreFullscreenLayout();
  if (ownsBrowserFullscreen && document.fullscreenElement) {
    try { await document.exitFullscreen(); } catch { /* The browser may already be exiting after Escape. */ }
  }
  ownsBrowserFullscreen = false;
}
async function toggleCanvasFullscreen() {
  if (canvasFullscreen.value) { await exitCanvasFullscreen(); return; }
  previousLayout = { railCollapsed: railCollapsed.value, railTab: railTab.value, mobileTab: mobileTab.value };
  canvasFullscreen.value = true; collapseRail(); fullscreenQuestionOpen.value = false; fullscreenNotice.value = '';
  if (document.fullscreenElement) { browserFullscreenActive = true; return; }
  if (!document.documentElement.requestFullscreen) {
    fullscreenNotice.value = uiText('当前环境不支持全屏，已展开到窗口。按 Esc 退出。'); return;
  }
  try {
    // Keep the app-level original-source reader inside the fullscreen element.
    await document.documentElement.requestFullscreen();
    ownsBrowserFullscreen = document.fullscreenElement === document.documentElement;
    browserFullscreenActive = !!document.fullscreenElement;
    if (!canvasFullscreen.value && ownsBrowserFullscreen) await exitCanvasFullscreen();
  } catch {
    if (canvasFullscreen.value) fullscreenNotice.value = uiText('浏览器未允许全屏，已展开到窗口。按 Esc 退出。');
  }
}
function fullscreenChanged() {
  if (canvasFullscreen.value && browserFullscreenActive && !document.fullscreenElement) {
    ownsBrowserFullscreen = false; restoreFullscreenLayout();
  }
  if (canvasFullscreen.value && document.fullscreenElement) browserFullscreenActive = true;
}
function fullscreenKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !canvasFullscreen.value || event.defaultPrevented || galleryOpen.value || addOpen.value || deleteOpen.value) return;
  if (event.target instanceof Node && event.target !== document && event.target !== document.body && !root.value?.contains(event.target)) return;
  event.preventDefault(); void exitCanvasFullscreen();
}
async function loadConversation() {
  const id = board.value?.board_id;
  if (!id) return;
  const sequence = ++conversationSequence; conversationLoading.value = true; conversationError.value = '';
  try {
    const result = await api.boardConversation(id);
    if (!disposed && sequence === conversationSequence && board.value?.board_id === id) conversationSessions.value = result.sessions;
  } catch (failure) {
    if (!disposed && sequence === conversationSequence) {
      conversationError.value = uiText('讨论暂时没有读到：{0}', [message(failure)]);
      requestWorkspaceRecovery(failure);
    }
  } finally {
    if (sequence === conversationSequence) {
      conversationLoading.value = false;
      scheduleConversationRefresh();
    }
  }
}
function scheduleConversationRefresh() {
  clearTimeout(conversationTimer);
  // A turn can finish after the reader selects another chat. Follow its saved
  // board history until it settles, without switching the reader back.
  if (!disposed && discussionTurns.value.some(turn => turn.pending) && !runOnBoard.value?.can_stop) {
    conversationTimer = setTimeout(() => { void loadConversation(); }, 3000);
  }
}
function pausePlayback(complete = true) { clearTimeout(playbackTimer); playbackSequence++; playing.value = false; if (complete) animationProgress.value = 1; }
function userControl() {
  pausePlayback(false);
  if (view.value.playback?.following) updateView({ ...view.value, playback: { ...view.value.playback, following: false } });
}
function mappedBlocks(itemId: string) { return [...new Set((board.value?.mappings ?? []).filter(mapping => mapping.explanation.item_id === itemId).map(mapping => mapping.explanation.block_id).filter((id): id is string => !!id))]; }
function mappedItem(itemId: string) { return (board.value?.mappings ?? []).some(mapping => mapping.explanation.item_id === itemId && mapping.explanation.block_id === null); }
function contentTitle(reference: BoardContentRef) {
  const item = board.value?.items.find(item => item.item_id === reference.item_id);
  const block = item?.explanation_blocks?.find(entry => entry.block_id === reference.block_id);
  return block ? `${item!.title} · ${boardTextPreview(block.markdown).slice(0, 52)}` : item?.title ?? reference.item_id;
}
async function readContent(reference: BoardContentRef, focus = false) {
  updateView({ ...view.value, reading_item_id: reference.item_id, selected_block_id: reference.block_id });
  mappingChoices.value = [];
  await nextTick(); await panes.get(reference.item_id)?.locate(reference.block_id, undefined, { focus });
}
function select(id: string) { void activateVisual({ kind: 'item', item_id: id }); }
async function activateVisual(target: BoardVisualTarget) {
  userControl(); manualHighlights.value = [target];
  const itemId = target.kind === 'link' ? null : target.item_id;
  const matches = (board.value?.mappings ?? []).filter(mapping => boardVisualKey(mapping.visual) === boardVisualKey(target));
  mappingChoices.value = [...new Map(matches.map(mapping => [`${mapping.explanation.item_id}:${mapping.explanation.block_id ?? ''}`, mapping.explanation])).values()];
  updateView({ ...view.value, selected_item_id: itemId, selected_block_id: null,
    reading_item_id: !matches.length && itemId ? itemId : view.value.reading_item_id ?? view.value.selected_item_id });
  if (mappingChoices.value.length === 1) await readContent(mappingChoices.value[0]);
  if (target.kind === 'presentation_anchor') { rememberSelection(target.item_id); await nextTick(); panes.get(target.item_id)?.locateAnchor(target); }
  showPanel('reading');
}
async function activateBlock(target: { item_id: string; block_id: string | null }) {
  userControl(); mappingChoices.value = [];
  manualHighlights.value = (board.value?.mappings ?? []).filter(mapping => mapping.explanation.item_id === target.item_id && mapping.explanation.block_id === target.block_id).map(mapping => mapping.visual);
  updateView({ ...view.value, selected_item_id: target.item_id, reading_item_id: target.item_id, selected_block_id: target.block_id,
    playback: view.value.playback ? { ...view.value.playback, show_all: true, following: false } : null });
  mobileTab.value = 'canvas';
  await focusRegions(manualHighlights.value);
  canvas.value?.focusTargets(manualHighlights.value);
}
async function focusRegions(targets: BoardVisualTarget[]) {
  for (const target of targets) if (target.kind === 'presentation_anchor') rememberSelection(target.item_id);
  await nextTick();
  for (const target of targets) if (target.kind === 'presentation_anchor') panes.get(target.item_id)?.locateAnchor(target);
}
async function showStep() {
  const projection = routeProjection.value;
  if (!projection?.step) return;
  if (projection.step.explanation) await readContent(projection.step.explanation);
  await focusRegions(projection.step.focus_targets);
  if (view.value.playback?.following) canvas.value?.focusTargets(projection.step.focus_targets.length ? projection.step.focus_targets : projection.step.reveal_targets);
}
async function seekStep(index: number, following = view.value.playback?.following ?? true) {
  pausePlayback(); const route = currentRoute.value;
  if (!route?.steps[index]) return;
  manualHighlights.value = []; mappingChoices.value = [];
  updateView({ ...view.value, playback: { route_id: route.route_id, step_id: route.steps[index].step_id, following, show_all: false } });
  await showStep();
}
async function chooseRoute(id: string) {
  pausePlayback(); const route = routes.value.find(entry => entry.route_id === id);
  if (!route) return;
  updateView({ ...view.value, playback: { route_id: id, step_id: route.steps[0]?.step_id ?? null, following: true, show_all: false } });
  await seekStep(0, true);
}
async function playRoute() {
  if (playing.value) { pausePlayback(false); return; }
  if (!currentRoute.value && routes.value[0]) await chooseRoute(routes.value[0].route_id);
  if (!currentRoute.value?.steps.length) return;
  const index = routeProjection.value?.index ?? -1;
  const resumeProgress = view.value.playback?.show_all ? 0 : animationProgress.value < 1 ? animationProgress.value : 0;
  const startSequence = playbackSequence + 1;
  await seekStep(index < 0 || (index === currentRoute.value.steps.length - 1 && !resumeProgress) ? 0 : index, true);
  if (playbackSequence !== startSequence || disposed) return;
  playing.value = true;
  const animate = (from = 0) => {
    const step = routeProjection.value?.step;
    if (!step || !playing.value) return;
    const duration = Math.max(300, step.duration_ms), started = Date.now() - duration * from;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
    animationProgress.value = reduced || step.transition === 'instant' ? 1 : from;
    const tick = () => {
      if (!playing.value || disposed) return;
      const elapsed = Date.now() - started;
      animationProgress.value = reduced || step.transition === 'instant' ? 1 : Math.min(1, elapsed / duration);
      if (elapsed < duration) { playbackTimer = setTimeout(tick, 32); return; }
      const next = (routeProjection.value?.index ?? -1) + 1;
      if (next >= (currentRoute.value?.steps.length ?? 0)) { pausePlayback(); return; }
      const nextSequence = playbackSequence + 1;
      void seekStep(next, true).then(() => { if (!disposed && playbackSequence === nextSequence) { playing.value = true; animate(); } });
    };
    playbackTimer = setTimeout(tick, 32);
  };
  animate(resumeProgress);
}
async function showAll() { userControl(); manualHighlights.value = []; if (view.value.playback) updateView({ ...view.value, playback: { ...view.value.playback, show_all: true, following: false } }); await nextTick(); canvas.value?.fit(); }
async function resumeFollowing() { if (currentRoute.value) await seekStep(Math.max(0, routeProjection.value?.index ?? 0), true); }
function move(id: string, position: { x: number; y: number }) {
  const item = board.value?.items.find(item => item.item_id === id);
  if (!item) return;
  item.position = position; pendingPositions.set(id, position); touch();
}
function rename(event: Event) {
  if (!board.value) return;
  const title = (event.target as HTMLInputElement).value;
  board.value.title = title; pendingTitle = title; touch();
}
function mergeSaved(saved: BoardDocument) {
  if (board.value?.board_id === saved.board_id && board.value.revision > saved.revision) return;
  for (const item of saved.items) { const position = pendingPositions.get(item.item_id) ?? savingPositions.get(item.item_id); if (position) item.position = position; }
  const localTitle = pendingTitle ?? savingTitle;
  if (localTitle !== undefined) saved.title = localTitle;
  board.value = saved;
  const summary: BoardSummary = { ...saved, item_count: saved.items.length };
  boards.value = [summary, ...boards.value.filter(item => item.board_id !== saved.board_id)];
}
async function saveChanges(): Promise<boolean> {
  clearTimeout(timer);
  if (savePromise) return savePromise;
  if (!board.value || !dirty.value) return true;
  savePromise = Promise.resolve().then(async () => {
    saving.value = true; error.value = '';
    try {
      while (board.value && (pendingTitle !== undefined || pendingPositions.size || viewDirty)) {
        if (pendingTitle !== undefined || pendingPositions.size) {
          const title = pendingTitle, positions = [...pendingPositions].map(([item_id, position]) => ({ item_id, position }));
          if (title !== undefined && !title.trim()) throw new Error(uiText('请给画板起一个名字，再保存。'));
          savingTitle = title;
          for (const position of positions) savingPositions.set(position.item_id, position.position);
          pendingTitle = undefined; pendingPositions.clear();
          try {
            const saved = await api.boardUpdate({ board_id: board.value.board_id, expected_revision: board.value.revision, ...(title !== undefined ? { title: title.trim() } : {}), positions });
            savingTitle = undefined; savingPositions.clear();
            mergeSaved(saved);
          } catch (failure) {
            if (pendingTitle === undefined) pendingTitle = title;
            for (const position of positions) if (!pendingPositions.has(position.item_id)) pendingPositions.set(position.item_id, position.position);
            throw failure;
          } finally { savingTitle = undefined; savingPositions.clear(); }
        }
        if (viewDirty && board.value) {
          viewDirty = false;
          try { await api.boardViewSave(board.value.board_id, { ...view.value }); }
          catch (failure) { viewDirty = true; throw failure; }
        }
      }
      dirty.value = false;
      return true;
    } catch (failure) { error.value = uiText('画板尚未保存：{0}', [message(failure)]); return false; }
    finally { saving.value = false; }
  }).finally(() => { savePromise = null; });
  return savePromise;
}
async function flush(): Promise<boolean> {
  if (mutating.value) return false;
  if (!await saveChanges()) return false;
  try { for (const pane of panes.values()) await pane.flush(); error.value = ''; return true; }
  catch (failure) { error.value = uiText('演示现场尚未保存：{0}', [message(failure)]); return false; }
}
let failedBoardId: string | null = null;
let listLoadError: string | null = null;
async function refreshList() {
  try {
    boards.value = (await api.boardList()).items;
    if (error.value === listLoadError) error.value = '';
    listLoadError = null;
  }
  catch (failure) {
    if (disposed) return;
    listLoadError = uiText('画板列表读取失败：{0}', [message(failure)]); error.value = listLoadError;
    requestWorkspaceRecovery(failure);
  }
}
function install(document: BoardDocument, state: BoardView) {
  if (board.value?.board_id !== document.board_id) {
    deliveryNotice.value = null; deliveryError.value = ''; failedDelivery = null; queuedDelivery = null;
  }
  pausePlayback(); manualHighlights.value = []; mappingChoices.value = [];
  boardMount.value++;
  board.value = document; view.value = { ...defaultBoardView(), ...state, reading_item_id: state.reading_item_id ?? state.selected_item_id }; pendingPositions.clear(); pendingTitle = undefined; viewDirty = false; dirty.value = false;
  if (view.value.playback && !document.routes?.some(route => route.route_id === view.value.playback?.route_id)) view.value.playback = null;
  visited.value = []; rememberSelection(state.selected_item_id); rememberSelection(view.value.reading_item_id);
  for (const target of highlightedTargets.value) if (target.kind === 'presentation_anchor') rememberSelection(target.item_id);
  if (state.selected_item_id && !document.items.some(item => item.item_id === state.selected_item_id)) view.value.selected_item_id = null;
  galleryOpen.value = false; deleteOpen.value = false; error.value = ''; savedNotice.value = ''; mobileTab.value = 'canvas';
  railTab.value = document.items.length && readingItem.value ? 'reading' : 'discussion';
  conversationSessions.value = []; void loadConversation();
}
async function discardAndReload() {
  const id = board.value?.board_id;
  if (!id || busy.value || saving.value) return;
  clearTimeout(timer); loading.value = true;
  try { const saved = await api.boardRead(id); install(saved.document, saved.view); }
  catch (failure) { error.value = uiText('画板没有重新载入：{0}', [message(failure)]); }
  finally { loading.value = false; }
}
async function openBoard(id: string) {
  if (openingBoard?.id === id) return openingBoard.promise;
  const promise = loadBoard(id);
  openingBoard = { id, promise };
  try { await promise; } finally { if (openingBoard?.promise === promise) openingBoard = null; }
}
async function loadBoard(id: string) {
  failedBoardId = null;
  if (board.value?.board_id === id) { galleryOpen.value = false; await importPending(); return; }
  pausePlayback();
  if (!await flush()) return;
  const sequence = ++loadSequence; loading.value = true; error.value = '';
  try {
    const result = await api.boardRead(id);
    if (disposed || sequence !== loadSequence) return;
    install(result.document, result.view); emit('board-open', id);
    await importPending();
  } catch (failure) {
    if (!disposed && sequence === loadSequence) {
      failedBoardId = id; error.value = uiText('画板没有打开：{0}', [message(failure)]);
      requestWorkspaceRecovery(failure);
    }
  }
  finally { if (sequence === loadSequence) loading.value = false; }
}
async function createBoard() {
  if (busy.value || !await flush()) return;
  mutating.value = true; error.value = '';
  try {
    const created = await api.boardCreate(newTitle.value.trim() || uiText('新的知识画板'));
    install(created, defaultBoardView()); mergeSaved(created); newTitle.value = ''; emit('board-open', created.board_id);
  } catch (failure) { error.value = uiText('画板没有创建：{0}', [message(failure)]); }
  finally { mutating.value = false; }
  await importPending();
}
async function importSource(source: BoardImportSource) {
  if (!board.value || mutating.value || !await flush()) return false;
  mutating.value = true; error.value = '';
  try {
    const existing = new Set(board.value.items.map(item => item.item_id));
    const saved = await api.boardImport({ board_id: board.value.board_id, expected_revision: board.value.revision, source });
    mergeSaved(saved);
    const imported = saved.items.find(item => !existing.has(item.item_id));
    if (imported) { userControl(); updateView({ ...view.value, selected_item_id: imported.item_id, reading_item_id: imported.item_id, selected_block_id: null }); await nextTick(); canvas.value?.fit(); }
    addOpen.value = false; mobileTab.value = 'canvas'; savedNotice.value = uiText('已加入画板，全文和来源一并保留');
    await saveChanges();
    return true;
  } catch (failure) { error.value = uiText('内容没有加入画板：{0}', [message(failure)]); return false; }
  finally { mutating.value = false; }
}
async function importPending() {
  const source = props.pendingImport;
  if (!source || source === handledImport || !board.value || mutating.value) return;
  if (await importSource(source)) { handledImport = source; emit('imported'); }
}
async function removeItem(id: string) {
  if (!board.value || busy.value || protectedItem(id) || !await flush()) return;
  mutating.value = true; error.value = '';
  try {
    mergeSaved(await api.boardUpdate({ board_id: board.value.board_id, expected_revision: board.value.revision, remove_item_ids: [id] }));
    visited.value = visited.value.filter(item => item !== id);
    if (view.value.selected_item_id === id || view.value.reading_item_id === id) {
      userControl(); const nextId = board.value?.items[0]?.item_id ?? null;
      updateView({ ...view.value, selected_item_id: view.value.selected_item_id === id ? nextId : view.value.selected_item_id,
        reading_item_id: view.value.reading_item_id === id ? nextId : view.value.reading_item_id, selected_block_id: null });
    }
    await saveChanges(); savedNotice.value = uiText('已从画板移除');
  } catch (failure) { error.value = uiText('内容没有移除：{0}', [message(failure)]); }
  finally { mutating.value = false; }
}
async function deleteBoard() {
  if (!board.value || busy.value || (runOnBoard.value?.pending || preparingQuestion.value) || !await flush()) return;
  mutating.value = true; error.value = '';
  try {
    const id = board.value.board_id; await api.boardDelete(id, board.value.revision);
    board.value = null; visited.value = []; boards.value = boards.value.filter(item => item.board_id !== id); deleteOpen.value = false; galleryOpen.value = true;
    emit('board-deleted', id);
  } catch (failure) { error.value = uiText('画板没有删除：{0}', [message(failure)]); }
  finally { mutating.value = false; }
}
async function close() { pausePlayback(); if (!busy.value && await flush()) emit('close'); }
async function source(itemId: string, sourceId: string, anchor: HTMLElement) {
  userControl();
  if (!board.value || !await flush()) return;
  emit('source', { board_id: board.value.board_id, item_id: itemId, source_ref_id: sourceId }, anchor);
}
async function loadCatalog(more = false) {
  clearTimeout(catalogTimer);
  const after = more ? catalogNext.value ?? undefined : undefined;
  const sequence = ++catalogSequence; catalogLoading.value = true; catalogError.value = '';
  if (!more) { catalog.value = []; catalogNext.value = null; catalogTotal.value = null; relatedItem.value = null; relatedSequence++; }
  try { const response = await (catalogKind.value === 'prebuild' ? api.boardCatalog(catalogKind.value, after, { category: catalogCategory.value, query: catalogQuery.value.trim() }) : api.boardCatalog(catalogKind.value, after)); if (sequence === catalogSequence) { catalog.value = more ? [...catalog.value, ...response.items] : response.items; catalogNext.value = response.next ?? null; catalogTotal.value = response.total ?? null; } }
  catch (failure) { if (sequence === catalogSequence) catalogError.value = message(failure); }
  finally { if (sequence === catalogSequence) catalogLoading.value = false; }
}
function openAdd() { userControl(); addOpen.value = true; catalogQuery.value = ''; void loadCatalog(); }
function openGallery() { userControl(); galleryOpen.value = true; void refreshList(); }
function protectedItem(id: string) { return (preparingQuestion.value && submitted.value?.context.item_id === id) || (runOnBoard.value?.pending && runOnBoard.value.item_id === id); }
async function askQuestion() {
  const item = questionItem.value, current = board.value, text = question.value.trim();
  if (!current || !text || busy.value || questionBusy.value) return;
  const blockId = item ? view.value.selected_block_id ?? item.block_id : null;
  const request: BoardQuestionRequest = { client_request_id: crypto.randomUUID(), message: text, context: { board_id: current.board_id, board_revision: current.revision, ...(item ? { item_id: item.item_id } : {}), ...(blockId ? { block_id: blockId } : {}) } };
  submitted.value = request; submittedTitle.value = item?.title ?? uiText('全画板'); preparingQuestion.value = true; questionError.value = '';
  const pane = item ? panes.get(item.item_id) : undefined;
  showPanel('discussion');
  try {
    // Capture before other asynchronous saves: later card selections and slider changes
    // must not change the object or scene that this question refers to.
    if (item?.target.kind === 'presentation') {
      const receipt = await pane?.captureFollowUp();
      if (!receipt) throw new Error(uiText('演示还在准备，请等它显示后再提问。'));
      request.context.presentation = receipt;
    }
    if (!await saveChanges()) throw new Error(uiText('请先保存画板，再发送追问。'));
    for (const [id, other] of panes) if (id !== item?.item_id) await other.flush();
    if (board.value?.board_id !== current.board_id) throw new Error(uiText('画板已切换，问题保留在原画板中。'));
    request.context.board_revision = board.value.revision;
    emit('follow-up', request);
  } catch (failure) { questionError.value = message(failure); submitted.value = null; }
  finally { preparingQuestion.value = false; }
}
async function refreshDelivery(delivery: BoardDelivery, notice = true): Promise<void> {
  if (board.value?.board_id !== delivery.board_id) return;
  if (notice) deliveryNotice.value = delivery;
  if (!queuedDelivery || queuedDelivery.board_id !== delivery.board_id || queuedDelivery.after_revision < delivery.after_revision) queuedDelivery = delivery;
  if (deliveryRefreshTask) return deliveryRefreshTask;
  deliveryRefreshTask = Promise.resolve().then(async () => {
    refreshingDelivery.value = true; deliveryError.value = '';
    while (queuedDelivery && !disposed) {
      const requested: BoardDelivery = queuedDelivery; queuedDelivery = null;
      if (board.value?.board_id !== requested.board_id) continue;
      if (board.value.revision >= requested.after_revision) { failedDelivery = null; continue; }
      try {
        // Read committed content first. A failed or in-flight local layout save must not hide it.
        const result = await api.boardRead(requested.board_id);
        if (board.value?.board_id !== requested.board_id || disposed) continue;
        mergeSaved(result.document);
        if (result.document.revision < requested.after_revision) throw new Error(uiText('更新后的讲解暂时未读到，请重试。'));
        failedDelivery = null;
        // Merge preserves the camera, selection, visited iframe, and local title/positions.
        if (dirty.value && !saving.value) touch();
      } catch (failure) {
        if (board.value?.board_id !== requested.board_id || disposed) continue;
        failedDelivery = queuedDelivery ?? requested; queuedDelivery = null;
        deliveryError.value = uiText('回答已保存，画板刷新失败：{0}', [message(failure)]);
        break;
      }
    }
  }).finally(() => { refreshingDelivery.value = false; deliveryRefreshTask = null; });
  return deliveryRefreshTask;
}
function retryDeliveryRefresh() { if (failedDelivery) void refreshDelivery(failedDelivery, false); }
async function locate(location: BoardLocation): Promise<boolean> {
  userControl();
  await openBoard(location.board_id);
  if (board.value?.board_id !== location.board_id) return false;
  galleryOpen.value = false; addOpen.value = false;
  if (!location.item_id) return true;
  const item = board.value.items.find(item => item.item_id === location.item_id);
  if (!item) { error.value = uiText('这项内容已不在画板上，请重新搜索。'); return false; }
  mobileTab.value = 'canvas';
  updateView({ ...view.value, selected_item_id: item.item_id, reading_item_id: item.item_id, selected_block_id: location.block_id ?? null,
    playback: view.value.playback ? { ...view.value.playback, show_all: true, following: false } : null });
  await nextTick(); canvas.value?.locate(item.item_id);
  showPanel('reading');
  await nextTick();
  const found = await panes.get(item.item_id)?.locate(location.block_id, location.query);
  if (found === false) { error.value = uiText('这段解释已不在画板上，请重新搜索。'); return false; }
  return true;
}
function locateItem(id: string) { if (board.value) void locate({ board_id: board.value.board_id, item_id: id }); }
async function locateDelivery(delivery = deliveryNotice.value) {
  if (!delivery) return;
  await refreshDelivery(delivery, false);
  if (board.value?.board_id === delivery.board_id && board.value.revision < delivery.after_revision) return;
  await locate({ board_id: delivery.board_id, item_id: delivery.item_ids[0], block_id: delivery.block_ids[0] });
}
async function searchBoards() {
  const query = searchQuery.value.trim(), sequence = ++searchSequence;
  searchError.value = ''; searchResults.value = [];
  if (!query) { searchLoading.value = false; return; }
  searchLoading.value = true;
  try { const result = await api.boardSearch(query); if (sequence === searchSequence) searchResults.value = result.items; }
  catch (failure) { if (sequence === searchSequence) searchError.value = uiText('画板搜索失败：{0}', [message(failure)]); }
  finally { if (sequence === searchSequence) searchLoading.value = false; }
}
async function findRelated(item: BoardCatalogItem) {
  if (relatedItem.value === item) { relatedItem.value = null; relatedSequence++; return; }
  const sequence = ++relatedSequence; relatedItem.value = item; relatedBoards.value = []; relatedLoading.value = true; relatedError.value = '';
  try { const result = await api.boardRelated(item.source); if (sequence === relatedSequence) relatedBoards.value = result.items; }
  catch (failure) { if (sequence === relatedSequence) relatedError.value = message(failure); }
  finally { if (sequence === relatedSequence) relatedLoading.value = false; }
}
function beforeUnload(event: BeforeUnloadEvent) { if (dirty.value || saving.value || mutating.value || preparingQuestion.value) { event.preventDefault(); event.returnValue = ''; } }
function dialogKeydown(event: KeyboardEvent) {
  const dialog = root.value?.querySelector<HTMLElement>('[role="dialog"]');
  if (!dialog) return;
  if (event.key === 'Escape' && !busy.value) {
    event.preventDefault();
    if (deleteOpen.value) deleteOpen.value = false;
    else if (addOpen.value) addOpen.value = false;
    else if (board.value) galleryOpen.value = false;
    else void close();
  }
  if (event.key !== 'Tab') return;
  const focusable = [...dialog.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),[tabindex="0"]')].filter(node => node.getClientRects().length);
  const first = focusable[0], last = focusable.at(-1);
  if (!first || !last) return;
  if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
  else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
}
watch([galleryOpen, addOpen, deleteOpen], async () => {
  await nextTick();
  const dialog = root.value?.querySelector<HTMLElement>('[role="dialog"]');
  if (dialog) (dialog.querySelector<HTMLElement>('input,button:not(:disabled)') ?? dialog).focus();
}, { immediate: true });
watch(() => props.boardId, id => { if (id && id !== board.value?.board_id) void openBoard(id); else if (!id && !board.value) galleryOpen.value = true; }, { immediate: true });
watch(() => props.pendingImport, source => { if (source && source !== handledImport) { if (board.value) void importPending(); else openGallery(); } });
watch(catalogKind, () => { catalogQuery.value = ''; if (addOpen.value) void loadCatalog(); });
watch(catalogCategory, () => { if (addOpen.value && catalogKind.value === 'prebuild') void loadCatalog(); });
watch(catalogQuery, () => {
  if (!addOpen.value || catalogKind.value !== 'prebuild') return;
  clearTimeout(catalogTimer); ++catalogSequence; catalog.value = []; catalogNext.value = null; catalogTotal.value = null; catalogLoading.value = true;
  catalogTimer = setTimeout(() => { void loadCatalog(); }, 240);
});
onBeforeUnmount(() => clearTimeout(catalogTimer));
watch(searchQuery, () => { clearTimeout(searchTimer); ++searchSequence; searchLoading.value = !!searchQuery.value.trim(); searchTimer = setTimeout(() => { void searchBoards(); }, 240); });
watch(() => props.pendingLocate, async location => { if (location && await locate(location)) emit('located'); }, { immediate: true });
watch([() => props.run, () => board.value?.board_id], ([run]) => {
  if (!run) return;
  if (run.discussion) {
    const previous = retainedDiscussion.value[run.board_id] ?? [];
    const next = run.discussion;
    const index = previous.findIndex(turn => (next.requestId && turn.requestId === next.requestId) || discussionTurnKey(turn) === discussionTurnKey(next));
    retainedDiscussion.value[run.board_id] = index < 0 ? [...previous, next] : previous.map((turn, i) => i === index ? next : turn);
  }
  if (run.accepted && !acknowledgedRequests.has(run.request_id)) {
    acknowledgedRequests.add(run.request_id);
    if (drafts.value[run.board_id]?.trim() === run.question.trim()) drafts.value[run.board_id] = '';
  }
  if (run.error && !run.pending && !restoredRequests.has(run.request_id)) {
    restoredRequests.add(run.request_id);
    if (!drafts.value[run.board_id]) drafts.value[run.board_id] = run.question;
  }
  for (const delivery of mergeBoardDeliveries(run.commits)) void refreshDelivery(delivery, false);
  if (run.delivery && !run.commits?.some(part => boardDeliveryKey(part) === boardDeliveryKey(run.delivery!))) void refreshDelivery(run.delivery);
}, { immediate: true, deep: true });
watch(() => `${props.run?.board_id}:${props.run?.turn_id}:${props.run?.pending}`, () => {
  if (props.run?.board_id === board.value?.board_id && props.run?.accepted) void loadConversation();
});
watch(() => `${discussionTurns.value.some(turn => turn.pending)}:${runOnBoard.value?.can_stop}`, scheduleConversationRefresh);
async function recoverBoardReads() {
  if (disposed || busy.value) return;
  if (listLoadError) await refreshList();
  if (disposed) return;
  if (failedBoardId) { await openBoard(failedBoardId); if (failedBoardId) return; }
  if (disposed) return;
  if (props.pendingLocate && await locate(props.pendingLocate)) emit('located');
  if (conversationError.value) await loadConversation();
}
window.addEventListener('workspace-recovered', recoverBoardReads);
window.addEventListener('beforeunload', beforeUnload);
document.addEventListener('fullscreenchange', fullscreenChanged);
document.addEventListener('keydown', fullscreenKeydown);
void refreshList();
onBeforeUnmount(() => { disposed = true; pausePlayback(); loadSequence++; catalogSequence++; searchSequence++; conversationSequence++; clearTimeout(timer); clearTimeout(searchTimer); clearTimeout(conversationTimer); window.removeEventListener('beforeunload', beforeUnload); window.removeEventListener('workspace-recovered', recoverBoardReads); document.removeEventListener('fullscreenchange', fullscreenChanged); document.removeEventListener('keydown', fullscreenKeydown); void exitCanvasFullscreen(); });
defineExpose({ flush });
</script>

<template>
  <section ref="root" class="board-workspace" :class="{ 'board-canvas-fullscreen': canvasFullscreen }" :aria-label="uiText('知识画板工作区')" :aria-busy="busy" @keydown="dialogKeydown">
    <header v-show="!canvasFullscreen" class="board-topbar">
      <button class="board-back" type="button" :aria-label="uiText('关闭画板，回到阅读')" :disabled="busy" @click="close"><ArrowLeft :size="18" aria-hidden="true" /></button>
      <button class="board-library-trigger" type="button" @click="openGallery"><LayoutDashboard :size="17" aria-hidden="true" /><span>{{ uiText('知识画板') }}</span><ChevronDown :size="13" aria-hidden="true" /></button>
      <span v-if="board" class="board-top-divider"></span>
      <input v-if="board" class="board-title-input" :value="board.title" :aria-label="uiText('画板名称')" maxlength="160" :disabled="busy" @input="rename" @blur="saveChanges" @keydown.enter="($event.target as HTMLInputElement).blur()" />
      <span v-else class="board-top-spacer"></span>
      <span v-if="board" class="board-save-state" :class="{ unsaved: dirty || error }" role="status"><LoaderCircle v-if="saving" :size="13" class="board-spinner" aria-hidden="true" /><Check v-else-if="!dirty && !error" :size="13" aria-hidden="true" />{{ saveLabel }}</span>
      <button class="board-search-trigger" type="button" :aria-label="uiText('搜索画板与全文')" @click="openGallery"><Search :size="17" aria-hidden="true" /></button>
      <button v-if="board" class="board-add-button" type="button" :disabled="busy" @click="openAdd"><Plus :size="16" aria-hidden="true" /><span>{{ uiText('添加内容') }}</span></button>
    </header>
    <div v-if="error && !galleryOpen && !addOpen && !deleteOpen" class="board-error" role="alert"><span>{{ error }}</span><button type="button" :disabled="busy || saving" @click="flush">{{ uiText('重试保存') }}</button><button v-if="dirty" type="button" :disabled="busy || saving" @click="discardAndReload">{{ uiText('放弃未保存修改并重新载入') }}</button><button type="button" :disabled="busy" @click="openGallery">{{ uiText('查看画板列表') }}</button></div>
    <div v-else-if="savedNotice" class="board-notice" role="status">{{ savedNotice }}</div>
    <div v-if="board" v-show="!canvasFullscreen" class="board-mobile-switch" role="group" :aria-label="uiText('画板、讲解与讨论')"><button type="button" :class="{ active: mobileTab === 'canvas' }" @click="mobileTab = 'canvas'"><LayoutDashboard :size="15" aria-hidden="true" />{{ uiText('画板') }}</button><button type="button" :class="{ active: mobileTab === 'reading' }" @click="showPanel('reading')"><BookOpen :size="15" aria-hidden="true" />{{ uiText('讲解') }}</button><button type="button" :class="{ active: mobileTab === 'discussion' }" @click="showPanel('discussion')"><MessageSquareText :size="15" aria-hidden="true" />{{ uiText('讨论') }}</button></div>
    <div v-if="canvasFullscreen" class="board-fullscreen-tools" role="group" :aria-label="uiText('画板、讲解与讨论')">
      <button type="button" :aria-pressed="!railCollapsed && railTab === 'reading'" @click="showPanel('reading')"><BookOpen :size="16" aria-hidden="true" />{{ uiText('讲解') }}</button>
      <button type="button" :aria-pressed="!railCollapsed && railTab === 'discussion'" @click="showPanel('discussion')"><MessageSquareText :size="16" aria-hidden="true" />{{ uiText('讨论') }}</button>
      <button type="button" :aria-pressed="fullscreenQuestionOpen" @click="toggleQuestion"><Plus :size="16" aria-hidden="true" />{{ fullscreenQuestionOpen ? uiText('收起提问') : uiText('提问') }}</button>
      <button type="button" :aria-label="uiText('退出全屏')" :title="uiText('退出全屏')" @click="exitCanvasFullscreen"><Minimize :size="16" aria-hidden="true" /><span>{{ uiText('退出全屏') }}</span></button>
    </div>
    <p v-if="canvasFullscreen && fullscreenNotice && railCollapsed" class="board-fullscreen-notice" role="status">{{ fullscreenNotice }}</p>
    <div v-if="routes.length" v-show="!canvasFullscreen" class="board-playback" role="group" :aria-label="uiText('板书讲解')" :data-route-id="currentRoute?.route_id" :data-step-id="view.playback?.step_id" :data-following="view.playback?.following ?? false" :data-playing="playing" :data-show-all="view.playback?.show_all ?? true">
      <select :value="currentRoute?.route_id ?? ''" :aria-label="uiText('选择讲解路径')" @change="chooseRoute(($event.target as HTMLSelectElement).value)"><option value="" disabled>{{ uiText('选择讲解路径') }}</option><option v-for="route in routes" :key="route.route_id" :value="route.route_id">{{ route.title }}</option></select>
      <div class="board-playback-controls">
        <button type="button" :aria-label="uiText('上一步')" :disabled="!currentRoute || (routeProjection?.index ?? -1) <= 0" @click="seekStep((routeProjection?.index ?? 0) - 1)"><SkipBack :size="15" aria-hidden="true" /></button>
        <button type="button" class="board-play-toggle" :aria-label="playing ? uiText('暂停讲解') : uiText('播放讲解')" @click="playRoute"><Pause v-if="playing" :size="15" aria-hidden="true" /><Play v-else :size="15" aria-hidden="true" /><span>{{ playing ? uiText('暂停') : uiText('播放') }}</span></button>
        <button type="button" :aria-label="uiText('下一步')" :disabled="!currentRoute || (routeProjection?.index ?? -1) >= currentRoute.steps.length - 1" @click="seekStep((routeProjection?.index ?? -1) + 1)"><SkipForward :size="15" aria-hidden="true" /></button>
      </div>
      <select v-if="currentRoute" class="board-step-select" :value="view.playback?.step_id ?? ''" :aria-label="uiText('跳到讲解步骤')" @change="seekStep(currentRoute.steps.findIndex(step => step.step_id === ($event.target as HTMLSelectElement).value))"><option v-for="(step, index) in currentRoute.steps" :key="step.step_id" :value="step.step_id">{{ index + 1 }} / {{ currentRoute.steps.length }} · {{ step.explanation ? contentTitle(step.explanation) : uiText('观察画面') }}</option></select>
      <button type="button" class="board-show-all" @click="showAll">{{ uiText('全图') }}</button>
      <button v-if="currentRoute && (!view.playback?.following || view.playback.show_all)" type="button" class="board-resume-following" @click="resumeFollowing">{{ uiText('继续跟随') }}</button>
    </div>
    <div v-if="mappingChoices.length" class="board-mapping-choices" role="group" :aria-label="uiText('这个图元的相关解释')"><span>{{ uiText('选择要阅读的解释') }}</span><button v-for="choice in mappingChoices" :key="`${choice.item_id}:${choice.block_id}`" type="button" @click="readContent(choice, true)">{{ contentTitle(choice) }}</button></div>
    <div v-if="board" class="board-body" :class="[`mobile-${mobileTab}`, { 'has-selection': activeItem, 'board-rail-collapsed': railCollapsed }]">
      <BoardCanvas ref="canvas" :items="board.items" :links="board.links ?? []" :view="view" :disabled="busy" :fullscreen="canvasFullscreen" :visible-item-ids="routeProjection?.visibleItemIds" :visible-link-ids="routeProjection?.visibleLinkIds" :highlight-targets="highlightedTargets" :progress="graphicProgress" :transition="routeProjection?.step?.transition ?? 'instant'" @view="updateCanvasView" @move="move" @select="select" @activate-target="activateVisual" @user-control="userControl" @add="openAdd" @ask="suggestQuestion('')" @toggle-fullscreen="toggleCanvasFullscreen" />
      <button v-if="railCollapsed && !canvasFullscreen" type="button" class="board-expand-rail" :aria-label="uiText('展开讲解与讨论')" @click="showPanel(railTab)"><PanelRightOpen :size="16" aria-hidden="true" /><span>{{ uiText('讲解与讨论') }}</span></button>
      <aside v-show="!railCollapsed" class="board-side-rail" :aria-label="uiText('讲解与讨论')">
        <div class="board-rail-header"><div class="board-rail-tabs" role="tablist" :aria-label="uiText('讲解与讨论')"><button type="button" role="tab" :aria-selected="railTab === 'reading'" @click="showPanel('reading')"><BookOpen :size="15" />{{ uiText('讲解') }}</button><button type="button" role="tab" :aria-selected="railTab === 'discussion'" @click="showPanel('discussion')"><MessageSquareText :size="15" />{{ uiText('讨论') }}<span v-if="discussionTurns.length">{{ discussionTurns.length }}</span><i v-if="runOnBoard?.pending" class="board-discussion-live-dot"></i></button></div><button type="button" class="board-collapse-rail" :aria-label="uiText('收起讲解与讨论')" :title="uiText('收起讲解与讨论')" @click="collapseRail"><PanelRightClose :size="17" aria-hidden="true" /></button></div>
      <div v-show="railTab === 'reading'" class="board-reader-stack" :class="{ 'has-region': focusedPresentationId && focusedPresentationId !== readingItem?.item_id }">
        <BoardReadingPane v-if="!readingItem" :board-id="board.board_id" :item="null" />
        <BoardReadingPane v-for="item in visitedItems" v-show="item.item_id === readingItem?.item_id || item.item_id === focusedPresentationId" :key="`${boardMount}:${board.board_id}:${item.item_id}`" :ref="instance => setPane(item.item_id, instance)" :board-id="board.board_id" :item="item" :items="board.items" :deliveries="board.deliveries" :busy="busy || protectedItem(item.item_id)" :mapped-block-ids="mappedBlocks(item.item_id)" :mapped-item="mappedItem(item.item_id)" :highlighted-block-id="item.item_id === readingItem?.item_id ? highlightedBlock : null" @source="(id, anchor) => source(item.item_id, id, anchor)" @remove="removeItem" @select-item="locateItem" @activate-block="activateBlock" @activate-target="activateVisual" @user-control="userControl" />
      </div>
        <BoardDiscussion v-show="railTab === 'discussion'" :turns="discussionTurns" :items="board.items" :loading="conversationLoading" :error="conversationError" @retry="loadConversation" @locate="locateItem" @source="source" @suggest="suggestQuestion" />
      </aside>
    </div>
    <div v-else class="board-unopened"><Shapes :size="40" aria-hidden="true" /><p>{{ loading ? uiText('正在打开画板…') : uiText('打开一张画板，让学习接着发生。') }}</p><button v-if="!loading" type="button" @click="openGallery">{{ uiText('查看我的画板') }}</button></div>

    <section v-if="board" v-show="!canvasFullscreen || fullscreenQuestionOpen" class="board-conversation" :aria-label="uiText('在画板上继续提问')">
      <div v-if="boardCommits.length" class="board-saved-parts" :aria-label="uiText('已保存的讲解')">
        <button v-for="(part, index) in boardCommits" :key="boardDeliveryKey(part)" type="button" class="board-part-location" :data-board-part="index + 1" @click="locateDelivery(part)"><Check :size="13" aria-hidden="true" /><span>{{ uiText('已保存第{0}部分', [index + 1]) }}</span><span>{{ uiText('查看') }}</span></button>
        <p v-if="runOnBoard?.incomplete && !runOnBoard.pending" role="status">{{ uiText('讲解尚未全部完成，已保存的部分可以继续查看。') }}</p>
      </div>
      <div v-if="deliveryNotice" class="board-delivery-notice" role="status"><Check :size="14" aria-hidden="true" /><span>{{ uiText('新的讲解和图解已加入画板') }}</span><button type="button" :disabled="refreshingDelivery" @click="locateDelivery()">{{ refreshingDelivery ? uiText('正在读取…') : uiText('定位新回答') }}</button><button class="board-notice-dismiss" type="button" :aria-label="uiText('关闭回答提示')" @click="deliveryNotice = null"><X :size="13" aria-hidden="true" /></button></div>
      <div v-if="deliveryError" class="board-question-error" role="alert">{{ deliveryError }}<button type="button" :disabled="refreshingDelivery" @click="retryDeliveryRefresh">{{ uiText('重试') }}</button></div>
      <div v-if="runOnBoard && (runOnBoard.pending || runOnBoard.error || (boardCommits.length && runOnBoard.incomplete))" class="board-run-status" :class="{ failed: runOnBoard.error }" :role="runOnBoard.error ? 'alert' : 'status'">
        <LoaderCircle v-if="runOnBoard.pending" :size="14" class="board-spinner" aria-hidden="true" /><CornerDownRight v-else :size="14" aria-hidden="true" />
        <span><strong>{{ uiText('针对：{0}', [targetTitle]) }}</strong>{{ runOnBoard.error || runOnBoard.status_text }}</span>
        <button v-if="runOnBoard.can_retry" type="button" @click="emit('retry-follow-up', runOnBoard.request_id)">{{ uiText('重试这次追问') }}</button>
        <button v-if="runOnBoard.can_stop" type="button" @click="emit('cancel-follow-up')"><Square :size="11" aria-hidden="true" />{{ uiText('停止') }}</button>
      </div>
      <p v-else-if="props.run?.pending" class="board-question-hint" role="status">{{ uiText('另一张画板的追问仍在进行，可以先继续浏览。') }}</p>
      <p v-if="questionError" class="board-question-error" role="alert">{{ questionError }}</p>
      <form class="board-question-form" @submit.prevent="askQuestion">
        <div class="board-question-target"><CornerDownRight :size="14" aria-hidden="true" /><span>{{ questionItem ? uiText('继续理解：{0}', [questionItem.title]) : uiText('全画板 · 从任何问题开始') }}<template v-if="view.selected_block_id"> · {{ uiText('所选解释段落') }}</template></span><button v-if="questionItem" type="button" class="board-whole-context" @click="wholeBoard">{{ uiText('全画板') }}</button><small>{{ uiText('讨论与图解都会留在这里') }}</small></div>
        <div class="board-question-row"><textarea v-model="question" :aria-label="uiText('在画板上提问')" :placeholder="questionItem ? uiText('哪里还不明白？也可以描述你的理解…') : uiText('想理解什么？可以请我梳理主题、画出关系，或继续讲解…')" rows="1" @focus="userControl" @keydown.ctrl.enter.prevent="askQuestion" @keydown.meta.enter.prevent="askQuestion"></textarea><button class="board-question-send" type="submit" :disabled="!question.trim() || busy || questionBusy" :aria-label="uiText('发送画板追问')"><LoaderCircle v-if="preparingQuestion || awaitingAcceptance" :size="17" class="board-spinner" aria-hidden="true" /><ArrowUp v-else :size="18" aria-hidden="true" /></button></div>
        <p v-if="preparingQuestion || awaitingAcceptance" class="board-question-hint" role="status">{{ preparingQuestion ? uiText('正在保留提问对象与演示现场…') : uiText('正在发送追问…') }}</p>
      </form>
    </section>

    <div v-if="galleryOpen" class="board-dialog-scrim" @click.self="board && (galleryOpen = false)">
      <section class="board-gallery" role="dialog" aria-modal="true" :aria-label="uiText('我的知识画板')">
        <header><div><span class="board-dialog-kicker">{{ uiText('持续学习的空间') }}</span><h2>{{ uiText('我的知识画板') }}</h2></div><button type="button" :aria-label="uiText('关闭画板列表')" @click="board ? galleryOpen = false : close()"><X :size="19" aria-hidden="true" /></button></header>
        <p class="board-gallery-intro">{{ pendingImport ? uiText('选择一张画板，把这段内容与已有的思路放在一起。') : uiText('围绕一个问题，把讨论、图解和逐渐形成的理解留在一起。') }}</p>
        <div v-if="error" class="board-dialog-error" role="alert">{{ error }}<button v-if="dirty" type="button" :disabled="busy || saving" @click="discardAndReload">{{ uiText('放弃未保存修改并重新载入') }}</button></div>
        <label class="board-search-field"><Search :size="16" aria-hidden="true" /><input v-model="searchQuery" type="search" :aria-label="uiText('搜索画板与全文')" :placeholder="uiText('搜索所有画板的标题、回答和理解…')" /></label>
        <form class="board-create-form" @submit.prevent="createBoard"><input v-model="newTitle" :aria-label="uiText('新画板名称')" :placeholder="uiText('为一个知识点新建画板…')" maxlength="160" /><button type="submit" :disabled="busy"><Plus :size="16" aria-hidden="true" />{{ uiText('新建画板') }}</button></form>
        <div v-if="searchQuery.trim()" class="board-search-results" :aria-label="uiText('画板搜索结果')" :aria-busy="searchLoading">
          <p v-if="searchLoading" class="board-gallery-empty" role="status">{{ uiText('正在搜索画板…') }}</p>
          <p v-else-if="searchError" class="board-question-error" role="alert">{{ searchError }}<button type="button" @click="searchBoards">{{ uiText('重试') }}</button></p>
          <template v-else><button v-for="(hit, index) in searchResults" :key="`${hit.board_id}:${hit.item_id}:${index}`" class="board-search-result" type="button" :disabled="busy" @click="locate({ ...hit, query: searchQuery.trim() })"><small>{{ hit.board_title }}<span>·</span>{{ hit.kind === 'board' ? uiText('画板') : uiText(boardKindLabels[hit.kind]) }}</small><strong>{{ hit.title }}</strong><p>{{ boardTextPreview(hit.snippet) }}</p></button><p v-if="!searchResults.length" class="board-gallery-empty">{{ uiText('没有找到匹配的画板内容。') }}</p></template>
        </div>
        <div v-else class="board-gallery-list">
          <button v-for="item in boards" :key="item.board_id" class="board-gallery-item" type="button" :class="{ current: board?.board_id === item.board_id }" :disabled="busy" @click="openBoard(item.board_id)"><span class="board-gallery-icon"><LayoutDashboard :size="23" aria-hidden="true" /></span><span><strong>{{ item.title }}</strong><small>{{ uiText('{0} 项内容', [item.item_count]) }}<span>·</span>{{ boardDate(item.updated_at) }}</small></span><span class="board-gallery-arrow" aria-hidden="true">↗</span></button>
          <p v-if="!boards.length && !loading" class="board-gallery-empty">{{ uiText('还没有画板。先为眼前的知识点起一个名字。') }}</p>
        </div>
        <footer v-if="board"><button type="button" class="board-delete-link" :disabled="runOnBoard?.pending || preparingQuestion" @click="deleteOpen = true; galleryOpen = false"><Trash2 :size="13" aria-hidden="true" />{{ uiText('删除当前画板') }}</button><span>{{ uiText('内容保存在当前学习空间') }}</span></footer>
      </section>
    </div>

    <div v-if="addOpen" class="board-dialog-scrim" @click.self="!mutating && (addOpen = false)">
      <section class="board-add-dialog" role="dialog" aria-modal="true" :aria-label="uiText('添加到画板')">
        <header><div><span class="board-dialog-kicker">{{ board?.title }}</span><h2>{{ uiText('把内容连到这里') }}</h2></div><button type="button" :disabled="mutating" :aria-label="uiText('关闭添加内容')" @click="addOpen = false"><X :size="19" aria-hidden="true" /></button></header>
        <div v-if="error" class="board-dialog-error" role="alert">{{ error }}<button v-if="dirty" type="button" :disabled="busy || saving" @click="discardAndReload">{{ uiText('放弃未保存修改并重新载入') }}</button></div>
        <div class="board-kind-tabs" role="tablist" :aria-label="uiText('内容类型')"><button v-for="kind in boardKinds" :key="kind" type="button" role="tab" :aria-selected="catalogKind === kind" :class="{ active: catalogKind === kind }" @click="catalogKind = kind">{{ uiText(boardKindLabels[kind]) }}</button></div>
        <p class="board-kind-description">{{ uiText(boardKindDescriptions[catalogKind]) }}</p>
        <div v-if="catalogKind === 'prebuild'" class="board-catalog-filters">
          <select v-model="catalogCategory" :aria-label="uiText('预构建内容类型')"><option v-for="(label, category) in boardArtifactLabels" :key="category" :value="category">{{ uiText(label) }}</option></select>
          <span v-if="catalogTotal !== null">{{ uiText('共 {0} 项', [catalogTotal]) }}</span>
        </div>
        <input v-model="catalogQuery" class="board-catalog-search" :aria-label="uiText('查找可添加的内容')" :placeholder="uiText('按标题或内容查找…')" />
        <div class="board-catalog-list" role="tabpanel">
          <p v-if="catalogLoading && !catalog.length" class="board-catalog-empty"><LoaderCircle :size="18" class="board-spinner" aria-hidden="true" />{{ uiText('正在读取已有内容…') }}</p>
          <p v-if="catalogError" class="board-catalog-empty" role="alert">{{ catalogError }}<button type="button" @click="loadCatalog(!!catalog.length)"><RefreshCw :size="14" aria-hidden="true" />{{ uiText('重试') }}</button></p>
          <div v-for="(item, index) in filteredCatalog" :key="index" class="board-catalog-row"><button class="board-catalog-item" type="button" :disabled="mutating" @click="importSource(item.source)"><span><strong>{{ item.title }}</strong><small>{{ boardTextPreview(item.summary) }}</small></span><Plus :size="18" aria-hidden="true" /></button><button v-if="item.kind === 'prebuild'" type="button" class="board-related-trigger" :aria-expanded="relatedItem === item" @click="findRelated(item)"><LayoutDashboard :size="12" aria-hidden="true" />{{ uiText('查找已关联的画板') }}</button><div v-if="relatedItem === item" class="board-related-results"><p v-if="relatedLoading" role="status">{{ uiText('正在读取…') }}</p><p v-else-if="relatedError" role="alert">{{ relatedError }}<button type="button" @click="relatedItem = null; findRelated(item)">{{ uiText('重试') }}</button></p><template v-else><button v-for="hit in relatedBoards" :key="`${hit.board_id}:${hit.item_id}`" type="button" @click="locate(hit)"><span>{{ hit.board_title }}</span><small>{{ uiText('在画板中查看') }} ↗</small></button><p v-if="!relatedBoards.length">{{ uiText('这个知识点还没有关联画板。') }}</p></template></div></div>
          <p v-if="!catalogLoading && !catalogError && !filteredCatalog.length" class="board-catalog-empty">{{ catalogQuery ? uiText('没有找到匹配的内容。') : catalogKind === 'prebuild' ? uiText('当前材料尚无这一类预构建内容。') : uiText('当前工作区中，这份材料还没有这一类记录。旧学习记录保留在原来的范围。') }}</p>
          <button v-if="catalogNext !== null" class="board-catalog-more" type="button" :disabled="catalogLoading" @click="loadCatalog(true)">{{ catalogLoading ? uiText('正在读取已有内容…') : uiText('继续读取更多内容') }}</button>
        </div>
      </section>
    </div>

    <div v-if="deleteOpen" class="board-dialog-scrim" @click.self="!mutating && (deleteOpen = false)"><section class="board-delete-dialog" role="dialog" aria-modal="true" :aria-label="uiText('删除画板')"><h2>{{ uiText('删除“{0}”？', [board?.title ?? '']) }}</h2><p>{{ uiText('这张画板和其中留存的内容将被删除。原文、原聊天与理解记录仍会保留。') }}</p><p v-if="error" class="board-dialog-error" role="alert">{{ error }}</p><div><button type="button" :disabled="mutating" @click="deleteOpen = false">{{ uiText('保留画板') }}</button><button type="button" class="destructive" :disabled="mutating" @click="deleteBoard">{{ uiText('删除画板') }}</button></div></section></div>
  </section>
</template>

<style scoped>
.board-catalog-filters{display:flex;align-items:center;justify-content:space-between;gap:12px;margin:0 0 12px;color:var(--stone);font-size:11px}.board-catalog-filters select{min-width:0;max-width:75%;padding:8px 10px;border:1px solid var(--hairline);border-radius:7px;background:var(--reader-card);color:var(--steel);font:inherit}.board-catalog-filters span{white-space:nowrap}
.board-saved-parts{display:flex;flex-wrap:wrap;gap:6px 8px;margin-bottom:8px}.board-part-location{display:inline-flex;align-items:center;gap:7px;padding:6px 10px;border:1px solid var(--line);border-radius:9px;background:var(--surface-soft);color:var(--steel);font:inherit;font-size:12px;cursor:pointer}.board-part-location>span:last-child{color:var(--brand-accent)}.board-saved-parts p{flex-basis:100%;margin:2px 0;font-size:12px;color:var(--steel)}
.board-playback{display:flex;align-items:center;gap:10px;flex-shrink:0;padding:9px 22px;border-bottom:1px solid var(--hairline);background:var(--reader-card)}.board-playback select{min-width:0;max-width:240px;padding:7px 9px;border:1px solid var(--hairline);border-radius:6px;background:var(--surface-soft);color:var(--steel);font:inherit;font-size:11px}.board-playback-controls{display:flex;align-items:center;gap:3px;flex-shrink:0}.board-playback button{display:inline-flex;align-items:center;justify-content:center;gap:5px;min-height:32px;padding:5px 9px;border:1px solid var(--hairline);border-radius:6px;background:transparent;color:var(--steel);font:inherit;font-size:11px;cursor:pointer}.board-playback .board-play-toggle{color:var(--brand-accent)}.board-playback .board-step-select{flex:1;max-width:360px}.board-playback .board-resume-following{color:var(--brand-accent);white-space:nowrap}.board-mapping-choices{display:flex;align-items:center;gap:8px;flex-wrap:wrap;padding:9px 22px;border-bottom:1px solid var(--hairline);background:var(--surface-soft);font-size:11px;color:var(--steel)}.board-mapping-choices button{padding:5px 8px;border:1px solid var(--hairline);border-radius:6px;background:var(--reader-card);color:var(--brand-accent);font:inherit;text-align:left;cursor:pointer}.board-reader-stack.has-region{grid-template-rows:minmax(160px,1fr) minmax(200px,1fr)}.board-reader-stack.has-region>*{grid-area:auto}.board-reader-stack.has-region>.board-retained-presentation{min-height:0}@media(max-width:700px){.board-playback{gap:7px;flex-wrap:wrap;padding:8px 12px}.board-playback>select:first-child{flex:1;max-width:none;width:calc(100% - 152px)}.board-playback .board-step-select{max-width:none;flex-basis:150px}.board-playback button{padding:5px 7px}.board-mapping-choices{padding:8px 12px}.board-reader-stack.has-region{grid-template-rows:minmax(170px,1fr) minmax(200px,1fr)}}
button{min-height:0;padding:0;border-radius:0}
.board-search-trigger{display:grid;place-items:center;width:30px;height:32px;flex-shrink:0;border:0;background:transparent;color:var(--steel);cursor:pointer}.board-search-field{display:flex;align-items:center;gap:9px;padding:10px 12px;margin-bottom:12px;border:1px solid var(--hairline);border-radius:8px;background:var(--surface-soft);color:var(--stone)}.board-search-field input{width:100%;min-width:0;border:0;background:transparent;color:var(--ink);font-size:12px;outline:0}.board-search-field:focus-within{outline:2px solid var(--brand-accent);outline-offset:2px}.board-search-results{overflow:auto;min-height:100px;overscroll-behavior:contain}.board-search-result{display:flex;flex-direction:column;gap:6px;width:100%;padding:14px 8px;border:0;border-bottom:1px solid var(--hairline);background:transparent;text-align:left;color:var(--ink);cursor:pointer}.board-search-result:hover{background:var(--surface-soft)}.board-search-result small{display:flex;gap:7px;font-size:10px;color:var(--stone)}.board-search-result strong{font-size:13px;font-weight:550;line-height:1.7}.board-search-result p{margin:0;color:var(--steel);font-size:12px;line-height:1.8;white-space:pre-wrap;overflow-wrap:anywhere}
.board-conversation{flex-shrink:0;padding:10px 22px 13px;border-top:1px solid var(--hairline);background:var(--reader-card)}.board-question-form{max-width:1050px;margin:0 auto}.board-question-target{display:flex;align-items:center;gap:6px;color:var(--steel);font-size:11px;line-height:1.6;margin-bottom:7px}.board-question-target>svg{flex-shrink:0;color:var(--brand-accent)}.board-question-target>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.board-question-target small{margin-left:auto;color:var(--stone);font-size:10px;white-space:nowrap}.board-question-row{display:flex;align-items:flex-end;gap:12px;border:1px solid var(--hairline);border-radius:10px;padding:8px 9px 8px 12px;background:var(--surface-soft)}.board-question-row:focus-within{border-color:var(--brand-accent)}.board-question-row textarea{flex:1;min-width:0;max-height:120px;min-height:40px;resize:vertical;background:transparent;border:0;outline:0;font:inherit;font-size:12px;line-height:1.7;color:var(--ink);padding:0}.board-question-row textarea::placeholder{color:var(--stone)}.board-question-send{display:grid;place-items:center;flex-shrink:0;width:32px;height:32px;border:0;border-radius:8px;background:var(--brand-accent);color:white;cursor:pointer}.board-question-hint{font-size:11px;line-height:1.7;color:var(--stone);margin:7px 0 0}.board-question-error{display:flex;gap:12px;align-items:center;font-size:12px;line-height:1.7;color:var(--brand-error);margin:0 0 9px}.board-question-error button,.board-run-status button,.board-delivery-notice button{display:inline-flex;align-items:center;gap:5px;flex-shrink:0;border:0;background:transparent;color:inherit;font-size:11px;cursor:pointer;text-decoration:underline;text-underline-offset:3px}.board-run-status{display:flex;gap:9px;align-items:center;max-width:1050px;margin:0 auto 10px;padding:3px 0 8px;border-bottom:1px solid var(--hairline-soft);color:var(--steel);font-size:11px;line-height:1.7}.board-run-status>svg{flex-shrink:0;color:var(--brand-accent)}.board-run-status>span{flex:1;min-width:0;overflow-wrap:anywhere}.board-run-status strong{display:block;color:var(--stone);font-size:10px;font-weight:400}.board-run-status.failed{color:var(--brand-error)}.board-delivery-notice{display:flex;align-items:center;gap:8px;margin:0 auto 10px;max-width:1050px;font-size:11px;line-height:1.7;color:var(--steel)}.board-delivery-notice>svg{color:var(--brand-accent);flex-shrink:0}.board-delivery-notice>span{flex:1}.board-delivery-notice .board-notice-dismiss{display:grid;place-items:center;width:24px;height:24px;text-decoration:none;color:var(--stone)}
@media(max-width:700px){.board-conversation{padding:9px 12px 11px}.board-question-target small{display:none}.board-question-row{gap:7px;padding:8px 8px 8px 10px}.board-question-row textarea{font-size:13px;min-height:42px}.board-run-status{gap:7px;flex-wrap:wrap}.board-run-status>span{flex-basis:calc(100% - 26px)}.board-run-status>button{margin-left:22px}.board-run-status>button+button{margin-left:4px}.board-search-trigger{width:25px}.board-delivery-notice{flex-wrap:wrap}.board-delivery-notice>span{min-width:150px}}
.board-dialog-error{padding:11px 13px;margin:0 0 15px;border-radius:7px;background:color-mix(in srgb,var(--brand-error) 8%,var(--reader-card));color:var(--brand-error);font-size:12px;line-height:1.8}.board-dialog-error>button{display:block;margin-top:7px;background:transparent;border:0;border-bottom:1px solid currentColor;color:inherit;font-size:11px;cursor:pointer}
.board-catalog-more{display:block;width:100%;margin-top:12px;padding:11px;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--steel);font-size:12px;cursor:pointer}.board-kind-tabs button[role=tab]{white-space:normal;line-height:1.5}
.board-catalog-row{border-bottom:1px solid var(--hairline-soft);padding-bottom:10px}.board-catalog-row .board-catalog-item{border-bottom:0;padding-bottom:9px}.board-related-trigger{display:flex;gap:5px;align-items:center;border:0;background:transparent;color:var(--stone);font-size:10px;cursor:pointer}.board-related-results{margin-top:10px;padding:10px;border-radius:7px;background:var(--surface-soft)}.board-related-results>button{display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;padding:8px 3px;border:0;background:transparent;color:var(--steel);font-size:12px;text-align:left;cursor:pointer}.board-related-results small{font-size:10px;white-space:nowrap;color:var(--brand-accent)}.board-related-results p{font-size:11px;line-height:1.8;color:var(--stone);margin:3px 0}.board-related-results p>button{margin-left:8px;border:0;background:transparent;color:var(--brand-accent);font-size:11px;cursor:pointer}
.board-workspace{position:relative;display:flex;flex-direction:column;width:100%;height:100%;min-height:0;background:var(--reader-canvas);color:var(--ink);font-family:var(--sans);font-size:14px}.board-topbar{display:flex;align-items:center;gap:14px;flex-shrink:0;min-height:65px;padding:12px 22px;border-bottom:1px solid var(--hairline);background:var(--reader-card)}button,input{font:inherit}.board-back,.board-library-trigger{display:flex;align-items:center;justify-content:center;gap:8px;border:0;background:transparent;color:var(--steel);cursor:pointer}.board-back{height:34px;width:30px;flex-shrink:0}.board-library-trigger{white-space:nowrap;font-size:12px}.board-library-trigger>svg:first-child{color:var(--brand-accent)}.board-top-divider{height:22px;width:1px;background:var(--hairline)}.board-title-input{flex:1;min-width:50px;padding:6px 3px;border:1px solid transparent;background:transparent;color:var(--ink);font-size:14px;font-weight:550;text-overflow:ellipsis;border-radius:4px}.board-title-input:hover{border-color:var(--hairline)}.board-top-spacer{flex:1}.board-save-state{display:flex;gap:5px;align-items:center;white-space:nowrap;color:var(--stone);font-size:10px}.board-save-state.unsaved{color:#ae7a32}.board-add-button{display:flex;gap:5px;align-items:center;white-space:nowrap;border:0;background:var(--brand-accent);color:white;padding:9px 12px;border-radius:7px;font-size:12px;cursor:pointer}.board-body{display:grid;grid-template-columns:minmax(0,1fr) minmax(350px,40%);flex:1;min-height:0;overflow:hidden}.board-reader-stack{display:grid;min-width:0;min-height:0;overflow:hidden}.board-reader-stack>*{grid-area:1/1}.board-mobile-switch{display:none}.board-error,.board-notice{display:flex;align-items:center;gap:12px;flex-shrink:0;padding:9px 22px;border-bottom:1px solid var(--hairline);font-size:12px;line-height:1.7}.board-error{background:color-mix(in srgb,var(--brand-error) 8%,var(--reader-card));color:var(--brand-error)}.board-error>span{flex:1}.board-error button{border:0;border-bottom:1px solid currentColor;background:transparent;color:inherit;padding:0;cursor:pointer;white-space:nowrap;font-size:11px}.board-notice{background:var(--surface-soft);color:var(--steel)}.board-unopened{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;flex:1;color:var(--stone)}.board-unopened p{font-size:14px}.board-unopened button{border:1px solid var(--hairline);border-radius:7px;padding:9px 14px;background:var(--reader-card);color:var(--steel);cursor:pointer}.board-dialog-scrim{position:absolute;z-index:20;inset:0;display:flex;align-items:center;justify-content:center;padding:28px;background:#25232032;backdrop-filter:blur(3px)}.board-gallery,.board-add-dialog,.board-delete-dialog{display:flex;flex-direction:column;width:min(660px,100%);max-height:100%;min-height:0;background:var(--reader-card);border:1px solid var(--hairline);border-radius:16px;box-shadow:0 18px 70px #25232024;padding:28px}.board-gallery>header,.board-add-dialog>header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start}.board-dialog-kicker{font-size:10px;letter-spacing:.06em;color:var(--stone)}.board-gallery h2,.board-add-dialog h2{font-size:24px;font-weight:550;line-height:1.6;margin:6px 0 0}.board-gallery>header>button,.board-add-dialog>header>button{display:grid;place-items:center;width:29px;height:29px;border:0;background:transparent;color:var(--steel);cursor:pointer}.board-gallery-intro{font-size:12px;line-height:1.9;color:var(--steel);margin:14px 0 23px}.board-create-form{display:flex;gap:8px;margin-bottom:22px}.board-create-form input,.board-catalog-search{min-width:0;flex:1;background:var(--surface-soft);border:1px solid var(--hairline);border-radius:7px;padding:11px 12px;font-size:12px;color:var(--ink)}.board-create-form button{display:flex;align-items:center;justify-content:center;gap:5px;padding:10px 13px;border:0;background:var(--brand-accent);color:white;border-radius:7px;white-space:nowrap;font-size:12px;cursor:pointer}.board-gallery-list{display:grid;grid-template-columns:1fr 1fr;gap:12px;overflow:auto;min-height:90px;padding:2px;overscroll-behavior:contain}.board-gallery-item{display:flex;gap:12px;align-items:center;padding:17px 15px;border:1px solid var(--hairline);border-radius:10px;background:transparent;text-align:left;color:var(--ink);cursor:pointer;min-width:0}.board-gallery-item:hover,.board-gallery-item.current{border-color:var(--brand-accent);background:color-mix(in srgb,var(--brand-accent) 4%,var(--reader-card))}.board-gallery-icon{display:grid;place-items:center;width:39px;height:45px;flex-shrink:0;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--steel)}.board-gallery-item>span:nth-child(2){display:flex;min-width:0;flex:1;flex-direction:column;gap:8px}.board-gallery-item strong{overflow:hidden;white-space:nowrap;text-overflow:ellipsis;font-size:13px;font-weight:550}.board-gallery-item small{display:flex;align-items:center;gap:7px;color:var(--stone);font-size:10px}.board-gallery-arrow{font-size:15px;color:var(--stone)}.board-gallery-empty{grid-column:1/-1;color:var(--stone);font-size:12px;align-self:center;text-align:center;line-height:1.8}.board-gallery>footer{display:flex;justify-content:space-between;gap:20px;align-items:center;border-top:1px solid var(--hairline);padding-top:18px;margin-top:24px;color:var(--stone);font-size:10px}.board-delete-link{display:flex;align-items:center;gap:5px;border:0;background:transparent;padding:0;color:var(--stone);cursor:pointer;font-size:10px}.board-delete-link:hover{color:var(--brand-error)}.board-kind-tabs{display:flex;gap:4px;padding:4px;background:var(--surface-soft);border-radius:8px;margin-top:22px}.board-kind-tabs button{flex:1;padding:9px 5px;border:0;border-radius:6px;background:transparent;white-space:nowrap;color:var(--steel);font-size:11px;cursor:pointer}.board-kind-tabs button.active{background:var(--reader-card);color:var(--ink);box-shadow:0 1px 5px #25232012}.board-kind-description{font-size:12px;line-height:1.8;color:var(--steel);margin:15px 0}.board-catalog-search{flex:none;width:100%;margin-bottom:15px}.board-catalog-list{min-height:180px;overflow:auto;overscroll-behavior:contain}.board-catalog-item{display:flex;align-items:center;gap:20px;width:100%;padding:15px 2px;border:0;border-bottom:1px solid var(--hairline-soft);background:transparent;color:var(--ink);text-align:left;cursor:pointer}.board-catalog-item:hover{background:var(--surface-soft)}.board-catalog-item>span{display:flex;flex:1;min-width:0;flex-direction:column;gap:7px}.board-catalog-item strong{font-size:13px;font-weight:550;line-height:1.7}.board-catalog-item small{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-size:11px;line-height:1.8;color:var(--stone)}.board-catalog-item>svg{flex-shrink:0;color:var(--brand-accent)}.board-catalog-empty{display:flex;align-items:center;justify-content:center;gap:8px;flex-wrap:wrap;min-height:120px;font-size:12px;color:var(--stone);line-height:1.9}.board-catalog-empty button{display:flex;align-items:center;gap:5px;border:0;background:transparent;color:var(--brand-accent);cursor:pointer}.board-delete-dialog{width:min(440px,100%)}.board-delete-dialog h2{font-size:20px;font-weight:550;margin:0 0 14px;overflow-wrap:anywhere}.board-delete-dialog p{font-size:13px;line-height:1.9;color:var(--steel)}.board-delete-dialog>div{display:flex;justify-content:flex-end;gap:10px;margin-top:20px}.board-delete-dialog button{padding:9px 14px;border:1px solid var(--hairline);border-radius:7px;background:var(--reader-card);color:var(--steel);cursor:pointer}.board-delete-dialog button.destructive{background:var(--brand-error);border-color:var(--brand-error);color:white}button:disabled{opacity:.5;cursor:wait}button:focus-visible,input:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}.board-spinner{animation:board-spin 1.2s linear infinite}@keyframes board-spin{to{transform:rotate(360deg)}}@media(prefers-reduced-motion:reduce){.board-spinner{animation:none}}@media(max-width:900px){.board-body{grid-template-columns:minmax(0,1fr) minmax(320px,44%)}.board-topbar{gap:10px;padding:12px 16px}.board-save-state{display:none}}@media(max-width:700px){.board-topbar{min-height:58px;padding:10px 12px;gap:8px}.board-library-trigger>span{display:none}.board-top-divider{display:none}.board-title-input{font-size:13px}.board-add-button{padding:8px 10px}.board-add-button>span{font-size:11px}.board-mobile-switch{display:flex;flex-shrink:0;gap:4px;background:var(--reader-card);padding:0 16px 9px;border-bottom:1px solid var(--hairline)}.board-mobile-switch button{display:flex;align-items:center;justify-content:center;gap:6px;flex:1;border:0;border-radius:7px;background:transparent;padding:8px;color:var(--steel);font-size:12px;cursor:pointer}.board-mobile-switch button.active{background:var(--surface-soft);color:var(--ink)}.board-mobile-switch button>span{padding:0 4px;font-size:10px;color:var(--stone)}.board-body{display:flex;position:relative}.board-body>.board-canvas,.board-reader-stack{width:100%;height:100%}.board-body.mobile-canvas>.board-reader-stack{display:none}.board-body.mobile-reading>.board-canvas{display:none}.board-dialog-scrim{padding:16px}.board-gallery,.board-add-dialog{padding:22px 20px;border-radius:13px}.board-gallery h2,.board-add-dialog h2{font-size:22px}.board-gallery-list{grid-template-columns:1fr}.board-gallery-intro{margin-bottom:18px}.board-create-form{flex-wrap:wrap}.board-create-form input{min-width:130px}.board-gallery>footer>span{display:none}.board-kind-tabs{gap:0;margin-top:18px}.board-kind-tabs button{font-size:10px}.board-error{flex-wrap:wrap;padding:9px 15px;gap:8px}.board-error>span{flex-basis:100%}.board-notice{padding:8px 15px;font-size:11px}}
</style>

<style scoped>
.board-workspace .board-body{position:relative;grid-template-columns:minmax(0,1fr) clamp(320px,32%,420px)}
.board-body.board-rail-collapsed{grid-template-columns:minmax(0,1fr)}
.board-topbar{min-height:58px;padding-top:9px;padding-bottom:9px}
.board-conversation{padding-top:8px;padding-bottom:10px}
.board-question-row{align-items:center;padding-top:6px;padding-bottom:6px}
.board-question-row textarea{min-height:28px}
.board-rail-header{display:flex;align-items:center;flex-shrink:0;border-bottom:1px solid var(--hairline)}
.board-rail-header .board-rail-tabs{flex:1;min-width:0;border-bottom:0;padding:0 18px;gap:18px;height:44px}
.board-collapse-rail,.board-expand-rail{display:flex;align-items:center;justify-content:center;gap:7px;border:1px solid var(--hairline);border-radius:8px;background:var(--reader-card);color:var(--steel);cursor:pointer}
.board-collapse-rail{width:32px;height:32px;padding:0;margin-right:10px;border-color:transparent}
.board-expand-rail{position:absolute;z-index:4;right:16px;top:16px;padding:9px 11px;font-size:11px;box-shadow:0 3px 12px #25232009}
.board-collapse-rail:hover,.board-expand-rail:hover{background:var(--surface-soft);color:var(--brand-accent)}
.board-fullscreen-tools{position:absolute;top:16px;left:16px;z-index:10;display:flex;align-items:center;gap:3px;max-width:calc(100% - 32px);padding:5px;border:1px solid var(--hairline);border-radius:10px;background:var(--reader-card);box-shadow:0 3px 12px #25232009}
.board-fullscreen-tools button{display:flex;align-items:center;justify-content:center;gap:5px;min-height:34px;padding:5px 9px;border:0;border-radius:6px;background:transparent;color:var(--steel);font-size:12px;white-space:nowrap;cursor:pointer}
.board-fullscreen-tools button:hover,.board-fullscreen-tools button[aria-pressed=true]{background:var(--surface-soft);color:var(--brand-accent)}
.board-fullscreen-tools button:last-child{border-left:1px solid var(--hairline);border-radius:0;padding-left:12px}
.board-fullscreen-notice{position:absolute;z-index:9;top:70px;left:16px;max-width:min(330px,calc(100% - 32px));margin:0;padding:9px 12px;border:1px solid var(--hairline);border-radius:8px;background:var(--reader-card);color:var(--steel);font-size:11px;line-height:1.7;box-shadow:0 3px 12px #25232009}
.board-canvas-fullscreen{width:100%;height:100%;min-height:0}
.board-canvas-fullscreen .board-body{flex:1}
@media(max-width:700px){
  .board-question-row textarea{min-height:32px}
  .board-fullscreen-tools{top:12px;left:12px;max-width:calc(100% - 24px);gap:0}
  .board-fullscreen-tools button{padding:5px 7px;font-size:11px;min-height:36px}
  .board-fullscreen-tools button:last-child{padding-left:9px}
  .board-fullscreen-tools button:last-child span{display:none}
  .board-rail-header{display:none}
  .board-canvas-fullscreen .board-side-rail{padding-top:66px}
  .board-canvas-fullscreen .board-rail-header{display:flex}
  .board-canvas-fullscreen .board-rail-header .board-rail-tabs{display:flex}
  .board-canvas-fullscreen .board-body.mobile-canvas>.board-canvas{display:block}
}
@media(pointer:coarse){.board-fullscreen-tools button,.board-collapse-rail{min-height:44px}.board-collapse-rail{width:44px}}
@media(pointer:coarse) and (max-width:700px){.board-canvas-fullscreen .board-side-rail{padding-top:76px}}
</style>

<style scoped>
.board-body{grid-template-columns:minmax(0,1fr) minmax(340px,37%)}.board-side-rail{display:flex;flex-direction:column;min-width:0;min-height:0;border-left:1px solid var(--hairline);background:var(--reader-card)}.board-rail-tabs{display:flex;align-items:center;flex-shrink:0;gap:20px;height:48px;padding:0 22px;border-bottom:1px solid var(--hairline)}.board-rail-tabs button{display:flex;align-items:center;gap:7px;height:100%;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--stone);font-size:12px;cursor:pointer}.board-rail-tabs button[aria-selected=true]{color:var(--brand-accent);border-bottom-color:var(--brand-accent)}.board-rail-tabs span{font-size:10px;color:var(--stone)}.board-discussion-live-dot{width:5px;height:5px;border-radius:50%;background:var(--brand-accent)}.board-side-rail .board-reader-stack{flex:1;min-height:0}.board-whole-context{flex-shrink:0;padding:3px 7px;border:1px solid var(--hairline);border-radius:5px;background:var(--surface-soft);color:var(--brand-accent);font-size:10px;cursor:pointer}.board-question-target>span{min-width:0}.board-question-form{max-width:none}.board-question-row textarea{font-size:13px}.board-question-target small{margin-left:auto}.board-side-rail :deep(.board-reading-pane){border-left:0}
@media(min-width:1400px){.board-body{grid-template-columns:minmax(0,1fr) 460px}}
@media(max-width:700px){.board-body>.board-side-rail{width:100%;height:100%;border-left:0}.board-body.mobile-canvas>.board-side-rail{display:none}.board-body.mobile-reading>.board-canvas,.board-body.mobile-discussion>.board-canvas{display:none}.board-rail-tabs{display:none}.board-side-rail .board-reader-stack{height:100%}.board-mobile-switch{flex-shrink:0}.board-question-target{gap:5px}.board-whole-context{padding:3px 6px}.board-conversation{max-height:46%;overflow:auto}.board-question-target>span{white-space:nowrap}.board-run-status{margin-bottom:6px;padding-bottom:4px}}
</style>
