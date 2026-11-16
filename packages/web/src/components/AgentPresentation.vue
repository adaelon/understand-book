<script setup lang="ts">
import { applicationOverlayTarget } from "../application-overlay";
import { activeNoteMessage, registerNoteDestinations, startThoughtHere, thoughtSurfaceVisible, useNoteAttachments, type NoteDestination } from '../note-placements';
import { keepNoteDraft, registerThoughtContext } from '../personal-notes';
import AttachedThought from './AttachedThought.vue';
import { t as uiText } from '../i18n';

import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { ArrowDownToLine, BookOpen, Ellipsis, ExternalLink, LayoutDashboard, Maximize2, Minimize2, Minus, Plus, RotateCcw, Save, StickyNote, ThumbsDown, ThumbsUp, X } from '@lucide/vue';
import { api as sharedApi, ApiError } from "../api";
import { bindSceneApi, network, readerKey, sceneKey } from "../network-context";
import { openLinkedPresentation } from "../network-presentation";
let api = bindSceneApi(sharedApi);
import { sharePresentationKey, type ShareSource } from '../reading-share';
import { capturePresentation, presentationShareSource } from '../presentation-share';
import ShareImagePanel from './ShareImagePanel.vue';
import TutorActivities from "./TutorActivities.vue";
import TutorContinuation from './TutorContinuation.vue';
import type { TutorContinuationView } from '../generated/TutorContinuationView';
import type { PresentationRef } from "../generated/PresentationRef";
import type { PresentationView } from "../generated/PresentationView";
import type { PresentationState } from "../generated/PresentationState";
import type { PresentationFollowUp } from "../generated/PresentationFollowUp";
import type { PresentationFeedback } from "../generated/PresentationFeedback";
import type { PresentationFeedbackChoice } from "../generated/PresentationFeedbackChoice";
import type { PresentationFeedbackReason } from "../generated/PresentationFeedbackReason";
import type { PresentationFeedbackSelection } from "../generated/PresentationFeedbackSelection";
import { registerPresentationHost, type PageOperationResult } from "../presentation-interaction";
import type { PresentationCommand } from "../generated/PresentationCommand";
import { presentationDocument } from "../presentation-document";
import { acceptsPresentationMessage, resolvePresentationEditingMessage } from "../presentation-host";
import { renderMarkdown } from "../md";
import { sourceChipLabel } from "../source-chip.js";

const props = defineProps<{ sessionId: string; turnId: string; reference: PresentationRef; busy?: boolean; teaching?: boolean; workspace?: boolean; sharedQuestion?: string; toolbarTarget?: HTMLElement; teachingTarget?: HTMLElement; socraticTarget?: (prompt: string) => HTMLElement | undefined; noteMemId?: string; noteReceipt?: PresentationFollowUp; boardId?: string; boardItemId?: string; boardReceipt?: PresentationFollowUp; continuationEnabled?: boolean; chatHead?: string | null; answerAvailable?: boolean }>();
const emit = defineEmits<{
  (e: 'add-to-board', receipt: PresentationFollowUp): void;
  (e: 'board-state-saved', receipt: PresentationFollowUp): void;
  (e: 'region-activated', regionId: string): void;
  (e: "record-note", receipt: PresentationFollowUp, title: string, summary: string): void;
  (e: "source", id: string, anchor: HTMLElement): void;
  (e: "follow-up", message: string, receipt: PresentationFollowUp): void;
  (e: "expand", expanded: boolean): void;
  (e: "update:sharedQuestion", question: string): void;
  (e: 'choose-continuation', view: TutorContinuationView, optionId: string, receipt?: PresentationFollowUp): void;
  (e: 'clarify-answer', receipt: PresentationFollowUp): void;
  (e: 'locate-turn', turnId: string): void;
  (e: 'edit-question'): void;
  (e: 'return-reading'): void;
  (e: 'continuation-available', available: boolean): void;
  (e: 'response-available', available: boolean): void;
  (e: 'teaching-host-available', available: boolean): void;
}>();
const boardContext = computed(() => props.boardId && props.boardItemId ? { board_id: props.boardId, item_id: props.boardItemId } : null);
const presentationScope = () => boardContext.value ? readerKey() : sceneKey();
const regionIds = computed(() => new Set([
  ...(view.value?.teaching_binding?.regions.map(region => region.region_id) ?? []),
  ...(view.value?.action_contract.flatMap(action => 'region_id' in action ? [action.region_id] : []) ?? []),
]));
const sharePresentation = inject(sharePresentationKey, null);
const localShare = ref<ShareSource | null>(null);
const exporting = ref(false);
async function exportCurrent() {
  if (!ready.value || !view.value || !frame.value || exporting.value) return;
  const current = generation, selectedView = view.value, selectedFrame = frame.value, channel = frameChannel;
  const sourceApi = api;
  const turnId = props.turnId, memId = props.noteMemId, board = boardContext.value;
  exporting.value = true; saveNotice.value = '';
  const load = async () => {
    const scene = await capturePresentation(selectedFrame, channel);
    if (board) {
      const observed = await sourceApi.boardPresentationObserve(board.board_id, board.item_id, scene.state.observed_result, scene.state.source_ref_ids);
      if (!observed.accepted) throw new Error(uiText('图解的文字或来源暂不可用，请重新打开后重试。'));
    }
    return presentationShareSource(selectedView, scene, id => board
      ? sourceApi.boardSourceResolve({ ...board, source_ref_id: id }) : sourceApi.agentSourceResolve(turnId, id, memId));
  };
  try {
    if (sharePresentation) await sharePresentation(load);
    else { const source = await load(); if (current === generation) localShare.value = source; }
  } catch (failure) { if (current === generation) saveNotice.value = uiText('图解未导出：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { if (current === generation) exporting.value = false; }
}
watch(() => [network.value.epoch, network.value.workspace?.generation], () => { localShare.value = null; });
const teachingHost = ref<InstanceType<typeof TutorActivities>>();
const frame = ref<HTMLIFrameElement>();
const root = ref<HTMLElement>();
const expandButton = ref<HTMLButtonElement>();
const view = ref<PresentationView>();
const documentText = ref("");
const error = ref("");
const expanded = ref(false);
const ready = ref(false);
const thoughtState = ref<PresentationState | null>(null), thoughtReceipt = ref<PresentationFollowUp | null>(null);
const thoughtAttachments = useNoteAttachments(() => props.sessionId);
const presentationThoughts = computed(() => thoughtAttachments.items.value.filter(item => {
  const target = item.attachment.target;
  return target.kind === 'presentation' && target.receipt.turn_id === props.turnId && target.receipt.reference.presentation_id === props.reference.presentation_id && target.receipt.reference.revision === props.reference.revision;
}));
const pageThoughts = computed(() => presentationThoughts.value.filter(item => {
  const target = item.attachment.target;
  return target.kind === 'presentation' && (target.visible_step != null ? target.visible_step === thoughtState.value?.visible_step
    : target.receipt.state_revision === (thoughtReceipt.value?.state_revision ?? view.value?.restored_state_revision));
}));
watch(() => view.value, value => { thoughtState.value = value?.restored_state ?? null; thoughtReceipt.value = null; });
function visibleForThought() { return ready.value && !props.noteMemId && thoughtSurfaceVisible(root.value); }
async function currentThoughtDestination(): Promise<NoteDestination> {
  const {receipt,state} = await snapshotScene();
  return {kind:'attachment',target:{kind:'presentation',receipt,visible_step:state.visible_step},label:`${view.value?.title ?? '演示'} · ${state.visible_step || '当前状态'}`};
}
const unregisterThoughtDestinations = registerNoteDestinations(() => visibleForThought()
  ? [{key:`presentation:${props.reference.presentation_id}:${props.reference.revision}`,label:'当前演示页',detail:view.value?.title ?? '记录当前画面',resolve:currentThoughtDestination}] : []);
const unregisterThoughtContext = registerThoughtContext(() => visibleForThought() ? {
  session_id:props.sessionId,turn_id:props.turnId,label:`${view.value?.title ?? '演示'} · ${thoughtState.value?.visible_step || '当前状态'}`,
  ...(thoughtReceipt.value ? {origin:{kind:'presentation' as const,receipt:thoughtReceipt.value,visible_step:thoughtState.value?.visible_step ?? null}} : {}),
} : null, 30);
onBeforeUnmount(() => { unregisterThoughtDestinations(); unregisterThoughtContext(); });
watch([ready, error, () => props.teaching, boardContext], () => emit('teaching-host-available', ready.value && !error.value && !!props.teaching && !boardContext.value));
const zoomPercent = ref(100);
const readableOpen = ref(false);
const toolsOpen = ref(false);
const toolsId = useId();
const toolbar = ref<HTMLElement>();
const toolsPanel = ref<HTMLElement>();
const toolsTop = ref(60);
const toolsRight = ref(12);
const toolsButton = ref<HTMLButtonElement>();
function closeTools() {
  toolsOpen.value = false;
  void nextTick(() => toolsButton.value?.focus({ preventScroll: true }));
}
function positionTools() {
  toolsTop.value = Math.max(12, Math.min((toolbar.value?.getBoundingClientRect().bottom ?? 44) + 8,
    window.innerHeight - (toolsPanel.value?.scrollHeight ?? 0) - 12));
  const end = toolsButton.value?.getBoundingClientRect().right ?? window.innerWidth;
  toolsRight.value = Math.max(12, Math.min(window.innerWidth - end, window.innerWidth - (toolsPanel.value?.offsetWidth ?? 0) - 12));
}
watch(toolsOpen, async open => {
  if (!open) return;
  await nextTick();
  positionTools();
  toolsPanel.value?.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true });
});
let toolsViewportWidth = window.innerWidth;
function resizeTools() {
  if (window.innerWidth !== toolsViewportWidth) toolsOpen.value = false;
  else if (toolsOpen.value) positionTools();
  toolsViewportWidth = window.innerWidth;
}
const questionOpen = ref(false);
function sendZoom() {
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "zoom", scale: zoomPercent.value / 100 }, "*");
}
function setZoom(value: number) {
  zoomPercent.value = Math.max(50, Math.min(150, value));
  sendZoom();
}
const localQuestion = ref("");
const question = computed({ get: () => props.sharedQuestion ?? localQuestion.value, set: value => {
  localQuestion.value = value; emit("update:sharedQuestion", value);
} });
watch(() => props.workspace, value => { if (value !== undefined) expanded.value = value; }, { immediate: true });
const saving = ref(false);
const saveNotice = ref("");
const prominentNotice = computed(() => !!saveNotice.value && ![
  uiText('演示状态已保存'), uiText('已恢复上次保存的演示状态'), uiText('已恢复记录时的演示状态'), uiText('已恢复提问时的演示状态'), uiText('已恢复加入画板时的演示状态'),
].includes(saveNotice.value));
const frameLoadCount = ref(0);
const frameEditing = ref(false);
let requestedRegion: string | undefined;
let teachingHelpInFlight: Promise<void> = Promise.resolve();
let saveQueue: Promise<unknown> = Promise.resolve();
let requestId = 0;
let pendingSnapshot: { id: number; owner?: 'feedback'; resolve: (state: PresentationState) => void; reject: (error: Error) => void } | undefined;
let snapshotInFlight: Promise<PresentationState> | undefined;
let pendingInteraction: { id: number; resolve: (result: PageOperationResult & { state?: PresentationState }) => void; reject: (error: Error) => void } | undefined;
let unregisterInteraction: (() => void) | undefined;
let generation = 0;
let frameChannel = crypto.randomUUID();
let themeObserver: MutationObserver | undefined;
let visibilityObserver: IntersectionObserver | undefined;
let hostVisible = true;
const readableText = computed(() => view.value?.readable_view.parts.map(part => part.kind === "markdown" || part.kind === "socratic_question" ? part.text : "").join("") ?? "");

const feedback = ref<PresentationFeedback>();
const feedbackLoading = ref(false), feedbackSaving = ref(false), feedbackDetails = ref(false);
const feedbackError = ref(''), feedbackSceneNotice = ref('');
const feedbackReasons = ref<PresentationFeedbackReason[]>([]), feedbackComment = ref('');
const failedFeedback = ref<{ selection: PresentationFeedbackSelection | null }>();
const feedbackSceneRetry = ref<(() => Promise<void>)>();
let feedbackOperation = 0;
let feedbackQuestion: { text: string; receipt: PresentationFollowUp | null } | undefined;
const reasonLabels: Record<PresentationFeedbackReason, string> = {
  easier_to_understand: uiText('更容易理解'), apt_example: uiText('例子贴切'), useful_interaction: uiText('交互有用'),
  unclear: uiText('没讲清楚'), incorrect_content: uiText('内容有误'), hard_to_read: uiText('画面难读'), hard_to_operate: uiText('操作不便'),
};
const availableReasons = computed<PresentationFeedbackReason[]>(() => feedback.value?.selection?.choice === 'like'
  ? ['easier_to_understand', 'apt_example', 'useful_interaction'] : ['unclear', 'incorrect_content', 'hard_to_read', 'hard_to_operate']);
const feedbackDetailsDirty = computed(() => feedbackComment.value !== (feedback.value?.selection?.comment ?? '')
  || JSON.stringify(feedbackReasons.value) !== JSON.stringify(feedback.value?.selection?.reasons ?? []));
function cancelFeedbackScene() {
  feedbackOperation++;
  feedbackSceneRetry.value = undefined;
  if (feedbackSceneNotice.value === uiText('正在附上演示状态…')) feedbackSceneNotice.value = uiText('未附上演示状态');
  if (pendingSnapshot?.owner === 'feedback') {
    pendingSnapshot.reject(new Error(uiText('反馈已更新。'))); pendingSnapshot = undefined;
  }
}
async function loadFeedback() {
  const current = generation;
  feedbackLoading.value = true; feedbackError.value = '';
  try {
    const result = await api.presentationFeedbackRead(props.sessionId, props.turnId, { ...props.reference });
    if (current !== generation) return;
    feedback.value = result;
    feedbackReasons.value = [...(result.selection?.reasons ?? [])]; feedbackComment.value = result.selection?.comment ?? '';
    feedbackSceneNotice.value = result.selection ? result.saved_state ? uiText('已附上反馈时的演示状态') : uiText('未附上演示状态') : '';
  } catch (failure) { if (current === generation) feedbackError.value = uiText('反馈读取失败：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { if (current === generation) feedbackLoading.value = false; }
}
async function attachFeedbackScene(saved: PresentationFeedback, current: number, operation: number) {
  const active = () => current === generation && operation === feedbackOperation;
  let captured: PresentationState | undefined, receipt: PresentationFollowUp | undefined;
  const attach = async () => {
    if (!active()) return;
    feedbackSceneRetry.value = undefined; feedbackSceneNotice.value = uiText('正在附上演示状态…');
    try {
      // Capture once. A retry saves/attaches this observation, never a newer scene.
      if (!captured) captured = await captureScene('feedback');
      if (!active()) return;
      if (!receipt) receipt = await saveState(captured);
      if (!active()) return;
      const result = await api.presentationFeedbackAttachState(saved.session_id, saved.turn_id, saved.reference, saved.feedback_revision, receipt);
      if (!active()) return;
      feedback.value = result; feedbackSceneNotice.value = uiText('已附上反馈时的演示状态');
    } catch (failure) {
      if (!active()) return;
      feedbackSceneNotice.value = uiText('未附上演示状态：{0}', [failure instanceof Error ? failure.message : String(failure)]);
      if (captured) feedbackSceneRetry.value = attach;
    }
  };
  await attach();
}
async function saveFeedback(selection: PresentationFeedbackSelection | null) {
  if (feedbackSaving.value || feedbackLoading.value) return;
  cancelFeedbackScene();
  const current = generation, operation = feedbackOperation;
  const session = props.sessionId, turn = props.turnId, reference = { ...props.reference };
  feedbackSaving.value = true; feedbackError.value = ''; failedFeedback.value = undefined;
  feedbackSceneNotice.value = ''; clearFeedbackQuestion();
  try {
    const result = selection ? await api.presentationFeedbackSet(session, turn, reference, selection)
      : await api.presentationFeedbackClear(session, turn, reference);
    if (current !== generation) return;
    feedback.value = result;
    feedbackReasons.value = [...(result.selection?.reasons ?? [])]; feedbackComment.value = result.selection?.comment ?? '';
    if (!result.selection) feedbackDetails.value = false;
    else if (result.saved_state) feedbackSceneNotice.value = uiText('已附上反馈时的演示状态');
    else void attachFeedbackScene(result, current, operation);
  } catch (failure) {
    if (current !== generation) return;
    failedFeedback.value = { selection };
    feedbackError.value = uiText('反馈未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]);
  } finally { if (current === generation) feedbackSaving.value = false; }
}
function chooseFeedback(choice: PresentationFeedbackChoice) {
  void saveFeedback(feedback.value?.selection?.choice === choice ? null : { choice, reasons: [], comment: '' });
}
function saveFeedbackDetails() {
  if (feedback.value?.selection) void saveFeedback({ choice: feedback.value.selection.choice, reasons: [...feedbackReasons.value], comment: feedbackComment.value });
}
function clearFeedbackQuestion() {
  if (feedbackQuestion && question.value.includes(feedbackQuestion.text)) question.value = question.value.replace(feedbackQuestion.text, '').trim();
  feedbackQuestion = undefined;
}
function prepareFeedbackQuestion() {
  const saved = feedback.value;
  if (!saved?.selection) return;
  const selection = saved.selection;
  // The inline follow-up uses a single-line input; keep its editable draft binding intact.
  const text = [uiText('我{0}「{1}」的版本 {2}。', [selection.choice === 'like' ? uiText('喜欢') : uiText('不喜欢'), view.value?.title ?? uiText('此演示'), saved.reference.revision]),
    selection.reasons.map(reason => reasonLabels[reason]).join('、'), selection.comment].filter(Boolean).join(' ').replace(/[\r\n]+/g, ' ');
  if (!question.value.includes(text)) question.value = [question.value.trim(), text].filter(Boolean).join('\n\n');
  feedbackQuestion = { text, receipt: saved.saved_state };
  questionOpen.value = true; feedbackDetails.value = false; toolsOpen.value = false;
  if (props.workspace) emit('edit-question');
  else void nextTick(() => root.value?.querySelector<HTMLInputElement>('.presentation-follow-up input')?.focus());
}

async function loadPresentation() {
  setFrameEditing(false);
  pendingInteraction?.reject(new Error(uiText('演示页面已切换，请重新观察。'))); pendingInteraction = undefined;
  const current = ++generation;
  // A publication refresh can retain this component while advancing its scene.
  // Capture the new binding once; old operations keep their original authority.
  const currentApi = bindSceneApi(sharedApi, presentationScope);
  api = currentApi;
  cancelFeedbackScene(); feedback.value = undefined; feedbackLoading.value = false; feedbackSaving.value = false;
  feedbackDetails.value = false; feedbackError.value = ''; feedbackSceneNotice.value = ''; failedFeedback.value = undefined;
  feedbackReasons.value = []; feedbackComment.value = ''; clearFeedbackQuestion();
  localShare.value = null; exporting.value = false;
  toolsOpen.value = false;
  frameChannel = crypto.randomUUID();
  pendingSnapshot?.reject(new Error(uiText('内容已切换，请重新追问。'))); pendingSnapshot = undefined;
  saveNotice.value = "";
  requestedRegion = undefined;
  teachingHelpInFlight = Promise.resolve();
  view.value = undefined; documentText.value = ""; error.value = ""; ready.value = false;
  try {
    const board = boardContext.value;
    const result = board ? await currentApi.boardPresentationRead(board.board_id, board.item_id)
      : props.noteMemId ? await currentApi.notePresentationRead(props.noteMemId) : await currentApi.presentationRead(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    const doc = presentationDocument(result, frameChannel);
    view.value = result; documentText.value = doc;
    if (!props.noteMemId && !board) void loadFeedback();
    if (result.restored_state_revision) saveNotice.value = uiText('已恢复上次保存的演示状态');
  } catch (failure) {
    if (current !== generation) return;
    if (failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE') error.value = uiText('阅读现场已更新，请重新读取演示。');
    else if (failure instanceof ApiError && failure.errorCode === 'PUBLICATION_BINDING_MISMATCH') error.value = uiText('此演示关联的资料版本暂时无法读取。');
    else error.value = uiText('内容读取失败：{0}', [failure instanceof Error ? failure.message : uiText('请稍后重新读取。')]);
  }
}
watch([() => props.sessionId, () => props.turnId, () => props.reference.presentation_id, () => props.reference.revision, () => props.noteMemId, () => props.boardId, () => props.boardItemId, presentationScope], loadPresentation, { immediate: true });

function theme() {
  if (!root.value) return;
  const style = getComputedStyle(root.value);
  const values = Object.fromEntries(["--canvas", "--ink", "--surface", "--line", "--accent"].map(name => [name, style.getPropertyValue(name).trim()]));
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "theme", values, generation }, "*");
}
function setFrameEditing(editing: boolean) {
  if (frameEditing.value === editing) return;
  frameEditing.value = editing;
  root.value?.dispatchEvent(new CustomEvent("workspace-iframe-editing", {
    bubbles: true,
    detail: { editing },
  }));
}
function hostIsVisible(): boolean {
  if (!root.value || root.value.hidden || root.value.closest("[hidden], [aria-hidden='true']")) return false;
  // Board panes stay mounted with v-show. A hidden ancestor must not claim an operation.
  for (let node: HTMLElement | null = root.value; node; node = node.parentElement) {
    const style = getComputedStyle(node);
    if (style.display === 'none' || style.visibility === 'hidden') return false;
  }
  return true;
}
function onFrameLoad() {
  frameLoadCount.value += 1;
  setFrameEditing(false);
  theme();
  sendZoom();
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "visibility", visible: hostVisible }, "*");
}
async function retry() {
  if (!view.value) { await loadPresentation(); return; }
  const selectedView = view.value;
  cancelFeedbackScene();
  pendingSnapshot?.reject(new Error(uiText('内容正在重新加载。'))); pendingSnapshot = undefined;
  pendingInteraction?.reject(new Error(uiText('演示页面正在重新加载。'))); pendingInteraction = undefined;
  const current = ++generation;
  frameChannel = crypto.randomUUID(); ready.value = false; error.value = "";
  feedbackSaving.value = false;
  documentText.value = "";
  await nextTick();
  if (current !== generation) return;
  documentText.value = presentationDocument(selectedView, frameChannel);
  if (!props.noteMemId && !boardContext.value) void loadFeedback();
}
async function receive(event: MessageEvent) {
  if (!acceptsPresentationMessage(event, frame.value?.contentWindow, frameChannel) || error.value) return;
  const message = event.data;
  if (message.kind === 'interaction-result') {
    if (pendingInteraction && pendingInteraction.id === message.request_id) {
      pendingInteraction.resolve(message); pendingInteraction = undefined;
    }
    return;
  }
  if (message.kind === "teaching-action" && ready.value && typeof message.move_id === "string") {
    teachingHost.value?.focusActivity(message.move_id, message.response); return;
  }
  if (message.kind === 'teaching-help' && ready.value && typeof message.move_id === 'string'
    && (message.help_kind === 'hint' || message.help_kind === 'explanation') && typeof message.text === 'string'
    && view.value?.teaching_binding?.regions.some(region => region.region_id === message.region_id)) {
    const current = generation;
    const host = teachingHost.value;
    // A learner response must not overtake a help receipt from the same page.
    teachingHelpInFlight = teachingHelpInFlight.catch(() => {}).then(async () => {
      if (!host) throw new Error(uiText('当前演示没有可关联的教学邀请。'));
      await host.reportPageHelp(message.move_id, message.help_kind, message.text);
    });
    try {
      await teachingHelpInFlight;
      if (current === generation) frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: 'teaching-help-recorded', request_id: message.request_id }, '*');
    } catch (failure) {
      if (current === generation) frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: 'teaching-help-recorded', request_id: message.request_id,
        error: failure instanceof Error ? failure.message : String(failure) }, '*');
    }
    return;
  }
  if (message.kind === 'region-located') {
    if (!message.located) saveNotice.value = uiText('这个位置暂时不可用，可结合当前画面继续提问。');
    return;
  }
  if (message.kind === 'region-activated') {
    if (ready.value && regionIds.value.has(message.region_id)) emit('region-activated', message.region_id);
    return;
  }
  if (message.kind === "editing-focus") {
    const editing = resolvePresentationEditingMessage(message, {
      generation,
      frameFocused: document.activeElement === frame.value,
      visible: hostIsVisible(),
    });
    if (editing !== null) setFrameEditing(editing);
    return;
  }
  if (message.kind === "restore-partial") {
    saveNotice.value = uiText('已恢复控件；此版本未提供自定义参数和步骤的恢复方法。');
    return;
  }
  if (message.kind === "state") {
    thoughtState.value = message.state;
    if (document.activeElement === frame.value && visibleForThought()) activeNoteMessage.value = '';
    if (message.request_id !== undefined) {
      if (pendingSnapshot && pendingSnapshot.id === message.request_id) { pendingSnapshot.resolve(message.state); pendingSnapshot = undefined; }
    } else if (!props.noteMemId) { void saveState(message.state).catch(() => {}); }
    return;
  }
  if (message.kind === "error") {
    error.value = uiText('此内容运行出错，请稍后重试。');
    pendingSnapshot?.reject(new Error(error.value)); pendingSnapshot = undefined;
    return;
  }
  if (message.kind === "source" && view.value?.sources.some(source => source.source_ref_id === message.source_ref_id) && root.value && ready.value) {
    emit("source", message.source_ref_id, root.value); return;
  }
  if (message.kind === "ready") {
    ready.value = true;
    theme();
    sendZoom();
    if (requestedRegion) { const region = requestedRegion; requestedRegion = undefined; locateRegion(region); }
  }
}
async function executeInteraction(command: PresentationCommand, signal: AbortSignal): Promise<PageOperationResult> {
  if (!ready.value || pendingInteraction || error.value) throw new Error(uiText('演示尚未准备好。'));
  const current = generation, channel = frameChannel, target = frame.value?.contentWindow;
  const id = ++requestId;
  const result = await new Promise<PageOperationResult & { state?: PresentationState }>((resolve, reject) => {
    const abort = () => {
      target?.postMessage({ channel, kind: 'interaction-cancel', request_id: id }, '*');
      pendingInteraction?.reject(new Error(uiText('演示操作已停止；请重新观察当前现场。')));
    };
    const timer = setTimeout(() => abort(), 25000);
    const cleanup = () => { clearTimeout(timer); signal.removeEventListener('abort', abort); };
    pendingInteraction = { id, resolve: value => { cleanup(); resolve(value); }, reject: failure => { cleanup(); pendingInteraction = undefined; reject(failure); } };
    signal.addEventListener('abort', abort, { once: true });
    if (signal.aborted) { abort(); return; }
    target?.postMessage({ channel, kind: 'interaction', request_id: id, request: command.request, expected_values: command.expected_values, expected_edits: command.expected_edits }, '*');
  });
  signal.throwIfAborted();
  if (current !== generation) throw new Error(uiText('演示页面已切换。'));
  if (result.state || ['observed', 'applied', 'conflict'].includes(result.status)) {
    if (!result.state) throw new Error(uiText('演示没有返回当前现场。'));
    // Saving the displayed state also records its teaching help. The final tool
    // completion can be cancelled without erasing help already seen.
    const saved = saveState(result.state, {
      command_id: command.command_id, run_turn_id: command.run_turn_id, page_id: channel,
      status: result.status, support: result.support ?? [],
    });
    teachingHelpInFlight = saved.then(() => {}, () => {});
    const receipt = await saved;
    signal.throwIfAborted();
    if (current !== generation) throw new Error(uiText('演示页面已切换。'));
    return { status: result.status, receipt, ...(result.reason ? { reason: result.reason } : {}) };
  }
  return { status: result.status, reason: result.reason };
}
function locateRegion(regionId: string) {
  // A board can select an anchor as this retained item is first opening.
  // Defer the declaration check until its exact presentation version arrives.
  if (!view.value && !error.value) { requestedRegion = regionId; return true; }
  if (!regionIds.value.has(regionId)) return false;
  if (!ready.value) { requestedRegion = regionId; return true; }
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: 'locate-region', region_id: regionId }, '*');
  return true;
}
function focusResponse() { return teachingHost.value?.focusResponse() ?? false; }
async function awaitTeachingHelp() {
  await teachingHelpInFlight.catch(() => {});
  await teachingHost.value?.waitForHelp();
}
function saveState(state: PresentationState, teacherInteraction?: Record<string, unknown>) {
  const current = generation;
  const currentApi = api;
  const board = boardContext.value;
  const session = props.sessionId, turn = props.turnId, reference = { ...props.reference };
  const operation = saveQueue.catch(() => {}).then(async () => {
    if (current !== generation) throw new Error(uiText('内容已切换，请重新追问。'));
    if (board) return teacherInteraction
      ? currentApi.boardPresentationSaveState(board.board_id, board.item_id, state, teacherInteraction)
      : currentApi.boardPresentationSaveState(board.board_id, board.item_id, state);
    return teacherInteraction
      ? currentApi.presentationSaveState(session, turn, reference, state, teacherInteraction)
      : currentApi.presentationSaveState(session, turn, reference, state);
  });
  saveQueue = operation;
  return operation.then(receipt => {
    if (current === generation) {
      saveNotice.value = uiText('演示状态已保存');
      thoughtState.value = state; thoughtReceipt.value = receipt;
      if (board) emit('board-state-saved', receipt);
    }
    return receipt;
  }, failure => {
    if (current === generation) saveNotice.value = uiText('演示状态保存失败：{0}', [failure instanceof Error ? failure.message : String(failure)]);
    throw failure;
  });
}
async function captureScene(owner?: 'feedback'): Promise<PresentationState> {
  if (!owner) cancelFeedbackScene();
  if (!ready.value || error.value || pendingSnapshot) throw new Error(uiText('页面尚未准备好，请稍后重试'));
  let timer: ReturnType<typeof setTimeout>;
  const operation = new Promise<PresentationState>((resolve, reject) => {
    const id = ++requestId;
    pendingSnapshot = { id, owner, resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } };
    timer = setTimeout(() => { pendingSnapshot = undefined; reject(new Error(uiText('暂时无法读取当前演示状态，请稍后重试。'))); }, 10000);
    frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "snapshot", request_id: id }, "*");
  });
  snapshotInFlight = operation;
  try { return await operation; }
  finally { if (snapshotInFlight === operation) snapshotInFlight = undefined; }
}
async function snapshotScene(): Promise<{ receipt: PresentationFollowUp; state: PresentationState }> {
  const current = generation;
  const state = await captureScene();
  if (current !== generation) throw new Error(uiText('内容已切换，请重新追问。'));
  return { receipt: await saveState(state), state };
}
async function teachingSnapshot(): Promise<PresentationFollowUp> { return (await snapshotScene()).receipt; }
async function clarifyAnswer() {
  if (saving.value || props.busy) return;
  saving.value = true;
  const current = generation;
  try {
    const receipt = await teachingSnapshot();
    if (current === generation && !props.busy) emit('clarify-answer', receipt);
  } catch (failure) { saveNotice.value = uiText('演示状态未保存：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { saving.value = false; }
}
async function recordNote() {
  if (!ready.value || saving.value || props.noteMemId || boardContext.value) return;
  saving.value = true;
  const current = generation;
  try {
    const { receipt, state } = await snapshotScene();
    const label = `${view.value?.title ?? uiText('演示')} · ${state.visible_step || '当前状态'}`;
    const value = await api.prepareNote({content:'',note:{association:{kind:'presentation',receipt}},label});
    if (current === generation) {
      const destination: NoteDestination = {kind:'attachment',target:{kind:'presentation',receipt,visible_step:state.visible_step},label};
      const draft = startThoughtHere(destination, value.context); value.context.origin = destination.target; draft.value = value; keepNoteDraft(draft);
    }
  } catch (failure) { if (current === generation) saveNotice.value = uiText('记录未开始：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { saving.value = false; }
}
async function addToBoard() {
  if (!ready.value || saving.value || props.noteMemId || boardContext.value) return;
  saving.value = true;
  const current = generation;
  try {
    const { receipt } = await snapshotScene();
    if (current === generation) emit('add-to-board', receipt);
  } catch (failure) {
    if (current === generation) saveNotice.value = uiText('加入画板未开始：{0}', [failure instanceof Error ? failure.message : String(failure)]);
  } finally { if (current === generation) saving.value = false; }
}
async function saveBoardScene() {
  if (!boardContext.value || !ready.value || saving.value) return;
  saving.value = true;
  const current = generation;
  try { await snapshotScene(); }
  catch (failure) {
    if (current === generation) saveNotice.value = uiText('演示状态保存失败：{0}', [failure instanceof Error ? failure.message : String(failure)]);
  } finally { if (current === generation) saving.value = false; }
}
async function flushBoardState(): Promise<PresentationFollowUp | undefined> {
  if (!boardContext.value) return;
  await snapshotInFlight?.catch(() => {});
  await saveQueue.catch(() => {});
  if (ready.value && !error.value) return (await snapshotScene()).receipt;
  else await saveQueue;
}
async function followUp() {
  if (!ready.value || error.value || saving.value || props.busy || boardContext.value) return;
  if (props.noteMemId && props.noteReceipt) {
    emit("follow-up", question.value.trim() || uiText('解释记录时的结果'), props.noteReceipt);
    question.value = "";
    return;
  }
  saving.value = true;
  const current = generation;
  const draft = question.value;
  const message = draft.trim() || uiText('解释现在的结果');
  cancelFeedbackScene();
  try {
    await awaitTeachingHelp();
    const feedbackReceipt = feedbackQuestion && draft.includes(feedbackQuestion.text) ? feedback.value?.saved_state ?? feedbackQuestion.receipt : null;
    const receipt = feedbackReceipt ?? (await snapshotScene()).receipt;
    if (current !== generation || error.value || props.busy) return;
    emit("follow-up", message, receipt);
    feedbackQuestion = undefined;
    if (question.value === draft) question.value = "";
  } catch (failure) {
    if (current === generation) saveNotice.value = uiText('追问未发送：{0}', [failure instanceof Error ? failure.message : String(failure)]);
  } finally { saving.value = false; }
}
async function openAttached() {
  try { await openLinkedPresentation(props.sessionId, props.turnId, props.reference); }
  catch (failure) { saveNotice.value = failure instanceof Error ? failure.message : String(failure); }
}
function toggleExpanded() {
  expanded.value = !expanded.value;
  emit("expand", expanded.value);
  if (!expanded.value) void nextTick(() => expandButton.value?.focus({ preventScroll: true }));
}
function keydown(event: KeyboardEvent) {
  if (event.defaultPrevented) return;
  if (event.key === 'Escape' && feedbackDetails.value) { feedbackDetails.value = false; return; }
  if (event.key === 'Escape' && toolsOpen.value) { closeTools(); return; }
  if (event.key === "Escape" && document.querySelector(".share-image-panel[open]")) return;
  if (event.key === "Escape" && readableOpen.value) { readableOpen.value = false; return; }
  if (event.key !== "Escape" || !expanded.value) return;
  expanded.value = false;
  emit("expand", false);
  void nextTick(() => expandButton.value?.focus({ preventScroll: true }));
}
async function restore(receipt: PresentationFollowUp) {
  if (saving.value) return;
  saving.value = true;
  const current = generation;
  const currentApi = api;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    if (!ready.value) {
      await new Promise<void>((resolve, reject) => {
        const stop = watch([ready, error], () => {
          if (!ready.value && !error.value) return;
          stop(); clearTimeout(timer);
          if (error.value) reject(new Error(error.value)); else resolve();
        });
        timer = setTimeout(() => { stop(); reject(new Error(uiText('内容尚未准备好，请重试。'))); }, 10000);
      });
    }
    if (current !== generation) return;
    if (!props.noteMemId) await teachingSnapshot();
    if (current !== generation) return;
    const board = boardContext.value;
    const restoredState = board ? await currentApi.boardPresentationRestoreState(board.board_id, board.item_id, receipt) : undefined;
    if (current !== generation) return;
    if (restoredState) await saveState(restoredState);
    const restored = board && restoredState && view.value ? { ...view.value, restored_state: restoredState, restored_state_revision: receipt.state_revision }
      : props.noteMemId ? await currentApi.notePresentationRead(props.noteMemId, true) : await currentApi.presentationRestore(receipt);
    if (current !== generation) return;
    view.value = restored;
    await retry();
    saveNotice.value = board ? uiText('已恢复加入画板时的演示状态') : props.noteMemId ? uiText('已恢复记录时的演示状态') : uiText('已恢复提问时的演示状态');
  } catch (failure) { if (current === generation) saveNotice.value = uiText('恢复失败：{0}', [failure instanceof Error ? failure.message : String(failure)]); }
  finally { clearTimeout(timer); saving.value = false; }
}
defineExpose({ followUp, restore, locateRegion, focusResponse, flushBoardState });
onMounted(() => {
  unregisterInteraction = registerPresentationHost({
    pageId: () => frameChannel,
    accepts: command => !props.noteMemId && ready.value && !error.value && hostIsVisible()
      && (boardContext.value
        ? command.board_target?.board_id === boardContext.value.board_id && command.board_target.item_id === boardContext.value.item_id
        : !command.board_target && command.session_id === props.sessionId)
      && command.content_session_id === props.sessionId && command.delivery_turn_id === props.turnId
      && command.request.reference.presentation_id === props.reference.presentation_id
      && command.request.reference.revision === props.reference.revision,
    execute: executeInteraction,
  });
  window.addEventListener("message", receive);
  window.addEventListener("keydown", keydown);
  themeObserver = new MutationObserver(theme);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["style", "class", "data-theme"] });
  visibilityObserver = new IntersectionObserver(entries => {
    hostVisible = entries[0]?.isIntersecting ?? false;
    frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "visibility", visible: hostVisible }, "*");
  });
  if (root.value) visibilityObserver.observe(root.value);
});
onBeforeUnmount(() => {
  unregisterInteraction?.();
  pendingInteraction?.reject(new Error(uiText('演示页面已关闭。'))); pendingInteraction = undefined;
  emit('teaching-host-available', false); emit('response-available', false);
  setFrameEditing(false);
  pendingSnapshot?.reject(new Error(uiText('内容已关闭。'))); pendingSnapshot = undefined;
  generation++; themeObserver?.disconnect(); visibilityObserver?.disconnect();
  window.removeEventListener("message", receive); window.removeEventListener("keydown", keydown);
  window.removeEventListener('resize', resizeTools);
});
onMounted(() => window.addEventListener('resize', resizeTools));
</script>

<template>
  <section
    ref="root"
    class="agent-presentation"
    @pointerdown="activeNoteMessage=''"
    :class="{ expanded, linked: network.linked }"
    :aria-label="view?.title || uiText('富回答')"
    :aria-modal="expanded && !workspace || undefined"
    :role="expanded ? workspace ? 'region' : 'dialog' : undefined"
    :data-frame-editing="frameEditing"
    :data-content-generation="generation"
  >
    <Teleport :to="toolbarTarget || 'body'" :disabled="!toolbarTarget">
      <header ref="toolbar" class="presentation-toolbar" :class="{ 'workspace-toolbar': workspace }">
        <strong class="presentation-title" :title="view?.title">{{ view?.title || (error ? uiText('内容未能加载') : uiText('正在读取内容…')) }}</strong>
        <button v-if="view && !noteMemId && !boardContext && !network.linked" class="presentation-note" type="button" :disabled="!ready || saving" @click="recordNote"><StickyNote :size="16" aria-hidden="true" /><span>{{ uiText('记一下') }}</span></button>
        <button v-if="view && boardContext" class="presentation-board-save" type="button" :disabled="!ready || saving" @click="saveBoardScene"><Save :size="16" aria-hidden="true" /><span>{{ uiText('保存画板状态') }}</span></button>
        <button v-if="view" ref="toolsButton" class="presentation-tools-toggle" type="button" :aria-label="uiText('演示工具')" :title="uiText('演示工具')" :aria-expanded="toolsOpen" :aria-controls="toolsId" @click="toolsOpen = !toolsOpen"><Ellipsis :size="21" aria-hidden="true" /><span v-if="workspace" class="workspace-tools-label">{{ uiText('演示工具') }}</span></button>
        <Teleport :to="applicationOverlayTarget()" :disabled="!toolsOpen">
        <div v-if="toolsOpen" class="presentation-tools-backdrop" aria-hidden="true" @pointerdown="closeTools"></div>
        <div v-if="view" :id="toolsId" ref="toolsPanel" class="presentation-tools" :class="{ 'is-open': toolsOpen }" :style="toolsOpen ? { top: `${toolsTop}px`, right: `${toolsRight}px`, maxHeight: `calc(100dvh - ${toolsTop}px - 12px)` } : undefined">
        <div class="presentation-tools-heading"><span>{{ uiText('演示工具') }}</span><button type="button" :aria-label="uiText('关闭演示工具')" @click="closeTools"><X :size="18" aria-hidden="true" /></button></div>
        <span v-if="view" class="presentation-zoom" role="group" :aria-label="uiText('演示缩放')">
          <span class="presentation-zoom-label">{{ uiText('画面缩放') }}</span>
          <button type="button" :aria-label="uiText('缩小演示')" :disabled="zoomPercent <= 50" @click="setZoom(zoomPercent - 10)"><Minus :size="16" aria-hidden="true" /></button>
          <button type="button" :aria-label="uiText('恢复演示原始大小')" @click="setZoom(100)">{{ zoomPercent }}%</button>
          <button type="button" :aria-label="uiText('放大演示')" :disabled="zoomPercent >= 150" @click="setZoom(zoomPercent + 10)"><Plus :size="16" aria-hidden="true" /></button>
        </span>
        <button class="presentation-tool-action" type="button" :disabled="!ready || saving || exporting" @click="toolsOpen = false; exportCurrent()"><ArrowDownToLine :size="16" aria-hidden="true" /><span>{{ exporting ? uiText('正在取图…') : uiText('导出长图') }}</span></button>
        <button class="presentation-tool-action" type="button" :aria-label="uiText('文字说明与来源')" :aria-expanded="readableOpen" @click="readableOpen = !readableOpen; toolsOpen = false"><BookOpen :size="16" aria-hidden="true" /><span>{{ uiText('说明与来源') }}</span></button>
        <button v-if="!noteMemId && !boardContext && !network.linked" class="presentation-tool-action presentation-add-board" type="button" :disabled="!ready || saving" @click="toolsOpen = false; addToBoard()"><LayoutDashboard :size="16" aria-hidden="true" /><span>{{ uiText('加入画板') }}</span></button>
        <button v-if="boardContext && boardReceipt" class="presentation-tool-action presentation-board-restore" type="button" :disabled="!ready || saving" @click="toolsOpen = false; restore(boardReceipt)"><RotateCcw :size="16" aria-hidden="true" /><span>{{ uiText('恢复加入时状态') }}</span></button>
        <button v-if="view && network.linked && !boardContext" type="button" :aria-expanded="questionOpen" @click="questionOpen = !questionOpen">{{ uiText('追问') }}</button>
        <button v-if="view && network.enabled && !network.linked && !noteMemId && !boardContext" class="presentation-tool-action" type="button" :title="uiText('在附属窗口打开')" @click="toolsOpen = false; openAttached()"><ExternalLink :size="16" aria-hidden="true" /><span>{{ uiText('新窗口') }}</span></button>
        <small v-if="saveNotice && !prominentNotice" class="presentation-save-notice" role="status" :title="saveNotice">{{ saveNotice }}</small>
        <div v-if="view && !noteMemId && !boardContext" class="presentation-feedback" role="group" :aria-label="uiText('评价这一版演示')">
          <span class="feedback-label">{{ uiText('这一版') }}</span>
          <button type="button" :aria-label="uiText('喜欢')" :aria-pressed="feedback?.selection?.choice === 'like'" :disabled="feedbackLoading || feedbackSaving || !feedback" @click="chooseFeedback('like')"><ThumbsUp :size="15" aria-hidden="true" /><span>{{ uiText('喜欢') }}</span></button>
          <button type="button" :aria-label="uiText('不喜欢')" :aria-pressed="feedback?.selection?.choice === 'dislike'" :disabled="feedbackLoading || feedbackSaving || !feedback" @click="chooseFeedback('dislike')"><ThumbsDown :size="15" aria-hidden="true" /><span>{{ uiText('不喜欢') }}</span></button>
          <button v-if="feedback?.selection" type="button" :aria-expanded="feedbackDetails" :disabled="feedbackSaving" @click="feedbackDetails = !feedbackDetails">{{ uiText('补充原因') }}</button>
          <span class="feedback-status" role="status">
            <template v-if="feedbackLoading">{{ uiText('正在读取反馈…') }}</template>
            <template v-else-if="feedbackSaving">{{ uiText('正在保存反馈…') }}</template>
            <template v-else-if="!feedbackError && feedback?.updated_at">
              <time :datetime="feedback.updated_at" :title="feedback.updated_at">{{ feedback.selection ? uiText('反馈已保存') : uiText('反馈已撤回') }}</time>
              <span v-if="feedback.selection && feedbackSceneNotice"> · {{ feedbackSceneNotice }}</span>
            </template>
          </span>
          <span v-if="feedbackError" class="feedback-error" role="alert">{{ feedbackError }}</span>
          <button v-if="feedbackError" type="button" :disabled="feedbackSaving || feedbackLoading" @click="failedFeedback ? saveFeedback(failedFeedback.selection) : loadFeedback()">{{ uiText('重试反馈') }}</button>
          <button v-if="feedbackSceneRetry" type="button" @click="feedbackSceneRetry()">{{ uiText('重试附上演示状态') }}</button>
          <form v-if="feedbackDetails && feedback?.selection" class="feedback-details" @submit.prevent="saveFeedbackDetails">
            <fieldset :disabled="feedbackSaving">
              <legend>{{ uiText('原因（选填，可多选）') }}</legend>
              <label v-for="reason in availableReasons" :key="reason"><input v-model="feedbackReasons" type="checkbox" :value="reason" />{{ reasonLabels[reason] }}</label>
              <label class="feedback-comment">{{ uiText('补充（选填）') }}<textarea v-model="feedbackComment" rows="2" :placeholder="uiText('说说哪里有帮助，或哪里需要改进…')"></textarea></label>
              <div class="feedback-actions">
                <button type="submit">{{ uiText('保存补充') }}</button>
                <button type="button" @click="saveFeedback(null)">{{ uiText('撤回反馈') }}</button>
                <button type="button" :disabled="busy || !!feedbackError || feedbackDetailsDirty" :title="feedbackDetailsDirty ? uiText('请先保存补充') : uiText('将已保存的反馈放入追问草稿')" @click="prepareFeedbackQuestion">{{ uiText('带入追问') }}</button>
                <button type="button" @click="feedbackDetails = false">{{ uiText('关闭') }}</button>
              </div>
            </fieldset>
          </form>
        </div>
        </div>
        </Teleport>
        <button v-if="view && !network.linked" ref="expandButton" type="button" class="presentation-expand" @click="toolsOpen = false; toggleExpanded()" :aria-label="expanded ? uiText('收起') : uiText('展开')" :title="expanded ? uiText('收起演示') : uiText('展开演示')" :aria-expanded="expanded"><component :is="expanded ? Minimize2 : Maximize2" :size="17" aria-hidden="true" /><span>{{ expanded ? uiText('收起') : uiText('展开') }}</span></button>
      </header>
    </Teleport>
    <Teleport :to="applicationOverlayTarget()">
      <div v-if="prominentNotice" class="presentation-notice" role="status">
        <span>{{ saveNotice }}</span><button type="button" :aria-label="uiText('关闭演示提示')" @click="saveNotice = ''"><X :size="16" aria-hidden="true" /></button>
      </div>
    </Teleport>
    <p v-if="error" class="presentation-error" role="alert"><span>{{ error }}</span><button type="button" @click="retry">{{ view ? uiText('重新加载') : uiText('重新读取') }}</button></p>
    <template v-else>
      <p v-if="!ready" class="presentation-status" role="status">{{ uiText('正在准备内容…') }}</p>
      <iframe
        v-if="documentText"
        ref="frame"
        :srcdoc="documentText"
        :title="view?.title"
        :data-load-count="frameLoadCount"
        sandbox="allow-scripts"
        referrerpolicy="no-referrer"
        @load="onFrameLoad"
      />
      <div v-if="pageThoughts.length" class="presentation-thoughts"><AttachedThought v-for="item in pageThoughts" :key="item.attachment.attachment_id" :item="item" /></div>
      <details v-if="presentationThoughts.length > pageThoughts.length" class="presentation-other-thoughts"><summary>其他演示页的想法 · {{ presentationThoughts.length - pageThoughts.length }}</summary><template v-for="item in presentationThoughts" :key="item.attachment.attachment_id"><button v-if="!pageThoughts.includes(item) && item.attachment.target.kind === 'presentation'" @click="restore(item.attachment.target.receipt)">{{ item.attachment.label }} ↗</button></template></details>
      <Teleport :to="teachingTarget || 'body'" :disabled="!teachingTarget">
        <nav v-if="view?.teaching_binding?.regions.length" class="teaching-regions" :aria-label="uiText('回到相关画面')">
          <button v-for="region in view.teaching_binding.regions" :key="region.region_id" :disabled="!ready" @click="locateRegion(region.region_id)">{{ region.observation }}</button>
        </nav>
        <TutorActivities ref="teachingHost" v-if="view && ready && teaching && !boardContext" :session-id="sessionId" :turn-id="turnId" :reference="reference" :presentation-target="() => frame" :snapshot="teachingSnapshot" :before-response="awaitTeachingHelp" :ready="ready" :busy="busy || saving" :sources="view.sources"
          :socratic-target="socraticTarget"
          @response-available="emit('response-available', $event)" @source="(id, anchor) => emit('source', id, anchor)" />
        <TutorContinuation v-if="view && ready && teaching && !noteMemId && !boardContext && chatHead !== undefined"
          :session-id="sessionId" :turn-id="turnId" :reference="reference" :snapshot="teachingSnapshot" :head="chatHead ?? null" :enabled="!!continuationEnabled" :busy="busy || saving"
          @available="emit('continuation-available', $event)"
          @choose="(original, option, receipt) => emit('choose-continuation', original, option, receipt)" @locate="emit('locate-turn', $event)" @edit="emit('edit-question')" @read="emit('return-reading')" />
        <button v-if="workspace && ready && answerAvailable && !boardContext" class="presentation-clarify" :disabled="busy || saving" @click="clarifyAnswer">{{ uiText('这一步没跟上') }}</button>
      </Teleport>
      <form v-if="view && !workspace && !boardContext && (!network.linked || questionOpen)" class="presentation-follow-up" @submit.prevent="followUp">
        <input v-model="question" :aria-label="uiText('针对当前演示追问')" :placeholder="uiText('解释当前结果，或描述要修改的内容…')" :disabled="saving || busy" />
        <button type="submit" :disabled="!ready || saving || busy">{{ saving ? uiText('正在准备追问…') : question.trim() ? uiText('发送追问') : uiText('解释现在的结果') }}</button>
      </form>
    </template>
    <section v-if="view && readableOpen" class="presentation-readable" :aria-label="uiText('文字说明与来源')">
      <div class="readable-heading"><strong>{{ uiText('文字说明与来源 · 版本') }} {{ view.reference.revision }}</strong><button type="button" @click="readableOpen = false" :aria-label="uiText('关闭文字说明与来源')">×</button></div>
      <div class="readable-text" v-html="renderMarkdown(readableText)"></div>
      <p v-for="assumption in view.assumptions" :key="assumption">{{ assumption }}</p>
      <button v-for="source in view.sources" :key="source.source_ref_id" type="button" :title="source.label" @click="emit('source', source.source_ref_id, $event.currentTarget as HTMLElement)">{{ sourceChipLabel(view.sources, source.source_ref_id) }}</button>
    </section>
    <ShareImagePanel v-if="localShare" :source="localShare" @close="localShare = null" />
  </section>
</template>

<style scoped>
.agent-presentation { position: relative; container-type: inline-size; border: 1px solid var(--line, #e6dfd8); border-radius: 24px; background: var(--canvas, #faf9f5); color: var(--ink, #252523); overflow: hidden; margin: 12px 0; min-width: 0; }
.presentation-thoughts{flex:none;padding:0 16px;max-height:28vh;overflow:auto}.presentation-other-thoughts{flex:none;margin:6px 16px;font-size:11px;color:#93764e}.presentation-other-thoughts summary{cursor:pointer}.presentation-other-thoughts button{display:block;background:none;border:0;color:inherit;text-align:left;font:inherit;padding:6px;cursor:pointer}
.presentation-toolbar { position: relative; flex: 1 1 auto; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 10px 14px; min-width: 0; color: var(--ink); font-size: 13px; }
.presentation-title { flex: 1 1 140px; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-weight: 600; letter-spacing: .01em; }
button { min-height: 30px; font: inherit; color: var(--accent, #a9583e); border: 1px solid var(--line, #e6dfd8); border-radius: 6px; background: var(--surface, #efe9de); padding: 3px 7px; cursor: pointer; }
.presentation-toolbar button, .presentation-tools button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; min-height: 34px; padding: 6px 9px; border-color: transparent; border-radius: 9px; background: transparent; color: var(--steel); }
.presentation-toolbar button:hover, .presentation-tools button:hover { background: var(--surface); color: var(--ink); }
.presentation-toolbar button:focus-visible, .presentation-tools button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.presentation-toolbar .presentation-note { color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
.presentation-tools { display: contents; }
.presentation-tools-toggle, .presentation-tools-heading, .presentation-zoom-label, .presentation-tools-backdrop { display: none !important; }
.presentation-zoom { display: inline-flex; flex: none; align-items: center; gap: 0; border-radius: 9px; background: var(--surface-soft, #f5f0e8); }
.presentation-zoom button { min-width: 30px; padding-inline: 6px; font-variant-numeric: tabular-nums; }
.presentation-save-notice { color: var(--steel); font-size: 11px; max-width: 180px; overflow-wrap: anywhere; }
.presentation-notice { position: fixed; z-index: 1210; bottom: max(12px, env(safe-area-inset-bottom)); left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 12px; width: max-content; max-width: min(520px, calc(var(--app-content-width, 100vw) - 24px)); padding: 10px 12px 10px 16px; border: 1px solid var(--line); border-radius: 12px; background: var(--canvas); color: var(--ink); box-shadow: 0 6px 28px #25232026; font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
.presentation-notice button { display: grid; place-items: center; flex: none; width: 36px; height: 36px; padding: 0; border: 0; border-radius: 50%; background: var(--surface-soft); color: var(--steel); }
.teaching-regions { display: flex; flex-wrap: wrap; gap: 6px; padding: 8px 12px; }
.teaching-regions button { text-align: left; }
.presentation-feedback { position: relative; order: 2; flex: 1 0 100%; min-width: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 6px; }
.feedback-label, .feedback-status { color: var(--steel, #65605a); font-size: 12px; }
.presentation-feedback button[aria-pressed="true"] { background: var(--accent, #a9583e); color: var(--canvas, #faf9f5); border-color: var(--accent, #a9583e); }
.presentation-feedback button:focus-visible, .feedback-details input:focus-visible, .feedback-details textarea:focus-visible { outline: 2px solid var(--accent, #a9583e); outline-offset: 2px; }
.feedback-error, .feedback-status { overflow-wrap: anywhere; }
.feedback-details { position: absolute; z-index: 4; top: 100%; right: 0; width: min(400px, 100%); max-height: 50dvh; overflow-y: auto; box-sizing: border-box; padding: 12px; background: var(--canvas, #faf9f5); border: 1px solid var(--line, #e6dfd8); border-radius: 10px; box-shadow: 0 6px 24px #0002; }
.feedback-details fieldset { border: 0; padding: 0; margin: 0; min-width: 0; }
.feedback-details legend { margin-bottom: 6px; }
.feedback-details label { display: inline-flex; align-items: center; gap: 4px; padding: 5px; }
.feedback-details .feedback-comment { display: flex; flex-direction: column; align-items: stretch; padding: 6px 0; }
.feedback-comment textarea { box-sizing: border-box; width: 100%; resize: vertical; padding: 8px; border: 1px solid var(--line, #e6dfd8); border-radius: 6px; background: var(--surface, #efe9de); color: inherit; font: inherit; }
.feedback-actions { display: flex; flex-wrap: wrap; gap: 6px; }
iframe { display: block; width: 100%; height: clamp(180px, calc(var(--app-content-height, 100dvh) - 220px), 420px); min-height: 180px; border: 0; background: var(--canvas, #faf9f5); }
.presentation-follow-up { position: relative; flex: 0 0 auto; display: flex; flex-wrap: wrap; gap: 8px; padding: 6px 10px max(6px, env(safe-area-inset-bottom)); }
.presentation-follow-up input { flex: 1; min-width: 120px; color: inherit; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 8px; }
.presentation-follow-up small { flex: 1 0 100%; overflow-wrap: anywhere; }
button:disabled { opacity: .55; cursor: default; }
.expanded { position: fixed; inset: max(20px, env(safe-area-inset-top)) max(20px, env(safe-area-inset-right)) max(20px, env(safe-area-inset-bottom)) max(20px, env(safe-area-inset-left)); z-index: 90; margin: 0; display: flex; flex-direction: column; max-height: calc(var(--app-content-height, 100dvh) - max(40px, env(safe-area-inset-top) + env(safe-area-inset-bottom))); box-shadow: 0 16px 80px #0005; }
.expanded { overflow-y: auto; }
.expanded iframe { flex: 1 0 160px; height: auto !important; min-height: 160px; }
.expanded > .presentation-toolbar, .linked > .presentation-toolbar { flex: 0 0 auto; }
.linked { flex: 1; min-height: 0; margin: 0; border-radius: 0; display: flex; flex-direction: column; }
.linked iframe { flex: 1; height: auto; min-height: 100px; }
.presentation-readable { position: absolute; z-index: 2; bottom: 0; right: 0; width: min(520px, 100%); box-sizing: border-box; max-height: 70%; overflow: auto; padding: 12px 16px; background: var(--canvas); border: 1px solid var(--line, #e6dfd8); border-radius: 8px 0 0 0; box-shadow: 0 -4px 20px #0002; }
.readable-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.presentation-readable button { margin: 4px; }
.readable-text { line-height: 1.65; overflow-wrap: anywhere; }
.readable-text :deep(pre) { overflow: auto; }
.presentation-error { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 0 16px; line-height: 1.6; overflow-wrap: anywhere; }
.presentation-error span { flex: 1 1 200px; }
.presentation-error button { flex: none; min-height: 36px; }
.presentation-status { position: absolute; z-index: 1; top: 52px; left: 16px; margin: 0; padding: 3px 8px; border-radius: 6px; background: var(--surface, #efe9de); }
@container (max-width: 420px) {
  .presentation-follow-up input,
  .presentation-follow-up button { flex: 1 1 100%; width: 100%; }
  .presentation-readable { padding-inline: 12px; }
}
@media (pointer: coarse) {
  .presentation-toolbar button { min-height: 40px; min-width: 36px; }
}
@media (max-width: 600px) {
  .expanded { inset: max(8px, env(safe-area-inset-top)) max(8px, env(safe-area-inset-right)) max(8px, env(safe-area-inset-bottom)) max(8px, env(safe-area-inset-left)); max-height: calc(var(--app-content-height, 100dvh) - max(16px, env(safe-area-inset-top) + env(safe-area-inset-bottom))); }
}
.workspace-toolbar { flex-wrap: nowrap; }
.workspace-toolbar .presentation-tools-toggle { display: inline-flex !important; }
.workspace-toolbar .presentation-tools { display: none; }
@media (max-width: 700px) {
  .presentation-toolbar { flex-wrap: nowrap; gap: 4px; padding: 6px 10px; }
  .presentation-title { flex: 1 1 0; font-size: 13px; }
  .presentation-toolbar > button { flex: none; min-height: 40px; padding: 8px; }
  .presentation-tools-toggle { display: inline-flex !important; width: 40px; }
  .presentation-expand { width: 40px; }
  .presentation-expand > span { display: none; }
  .presentation-tools { display: none; }
  .workspace-tools-label { display: none; }
}
.presentation-tools-backdrop { display: block !important; position: fixed; z-index: 1190; inset: 0; background: #2523200a; }
.presentation-tools.is-open { position: fixed; z-index: 1200; right: max(12px, env(safe-area-inset-right)); display: flex; flex-wrap: wrap; align-items: center; gap: 8px; width: min(340px, calc(var(--app-content-width, 100vw) - 24px)); max-height: calc(var(--app-content-height, 100dvh) - 24px); overflow: auto; padding: 14px; border: 1px solid var(--line); border-radius: 18px; background: var(--canvas); color: var(--ink); font-size: 13px; box-shadow: 0 12px 40px #25232026, 0 2px 8px #2523200a; }
.presentation-tools.is-open .presentation-tools-heading { display: flex !important; flex: 1 0 100%; align-items: center; justify-content: space-between; color: var(--ink); font-size: 14px; font-weight: 600; }
.presentation-tools.is-open .presentation-tools-heading button { min-width: 36px; min-height: 36px; margin: -6px -6px -6px 0; }
.presentation-tools.is-open .presentation-zoom { flex: 1 0 100%; padding: 4px; border-radius: 12px; }
.presentation-tools.is-open .presentation-zoom-label { display: inline !important; flex: 1; padding-left: 8px; color: var(--steel); }
.presentation-tools.is-open .presentation-zoom button { min-width: 40px; min-height: 40px; color: var(--ink); }
.presentation-tools.is-open .presentation-zoom button:nth-last-child(2) { min-width: 58px; }
.presentation-tools.is-open > .presentation-tool-action { flex: 1 0 calc(50% - 4px); justify-content: flex-start; min-height: 44px; padding: 10px; color: var(--ink); }
.presentation-tools.is-open .presentation-save-notice { flex-basis: 100%; max-width: none; padding-inline: 10px; }
.presentation-tools.is-open .presentation-feedback { margin-top: 2px; padding-top: 12px; border-top: 1px solid var(--line); gap: 4px; }
.presentation-tools.is-open .feedback-label { margin-right: auto; }
.presentation-tools.is-open .presentation-feedback > button { min-height: 40px; }
.presentation-tools.is-open .feedback-status:empty { display: none; }
.presentation-tools.is-open .feedback-status, .presentation-tools.is-open .feedback-error { flex-basis: 100%; line-height: 1.6; }
.presentation-tools.is-open .feedback-details { position: static; width: 100%; max-height: none; border: 0; box-shadow: none; border-radius: 0; padding: 8px 0 0; }
</style>
