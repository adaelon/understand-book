<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from "vue";
import { ArrowDownToLine, BookOpen, Ellipsis, ExternalLink, Maximize2, Minimize2, Minus, Plus, StickyNote, ThumbsDown, ThumbsUp, X } from '@lucide/vue';
import { api as sharedApi } from "../api";
import { bindSceneApi, network, sceneKey } from "../network-context";
import { openLinkedPresentation } from "../network-presentation";
const api = bindSceneApi(sharedApi);
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
import { presentationDocument } from "../presentation-document";
import { acceptsPresentationMessage, resolvePresentationEditingMessage } from "../presentation-host";
import { renderMarkdown } from "../md";
import { sourceChipLabel } from "../source-chip.js";

const props = defineProps<{ sessionId: string; turnId: string; reference: PresentationRef; busy?: boolean; teaching?: boolean; workspace?: boolean; sharedQuestion?: string; toolbarTarget?: HTMLElement; teachingTarget?: HTMLElement; noteMemId?: string; noteReceipt?: PresentationFollowUp; continuationEnabled?: boolean; chatHead?: string | null; answerAvailable?: boolean }>();
const emit = defineEmits<{
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
const sharePresentation = inject(sharePresentationKey, null);
const localShare = ref<ShareSource | null>(null);
const exporting = ref(false);
async function exportCurrent() {
  if (!ready.value || !view.value || !frame.value || exporting.value) return;
  const current = generation, selectedView = view.value, selectedFrame = frame.value, channel = frameChannel;
  const turnId = props.turnId, memId = props.noteMemId;
  exporting.value = true; saveNotice.value = '';
  const load = async () => presentationShareSource(selectedView, await capturePresentation(selectedFrame, channel), id => api.agentSourceResolve(turnId, id, memId));
  try {
    if (sharePresentation) await sharePresentation(load);
    else { const source = await load(); if (current === generation) localShare.value = source; }
  } catch (failure) { if (current === generation) saveNotice.value = `图解未导出：${failure instanceof Error ? failure.message : String(failure)}`; }
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
watch([ready, error, () => props.teaching], () => emit('teaching-host-available', ready.value && !error.value && !!props.teaching));
const zoomPercent = ref(100);
const readableOpen = ref(false);
const toolsOpen = ref(false);
const toolsId = useId();
const toolbar = ref<HTMLElement>();
const toolsPanel = ref<HTMLElement>();
const toolsTop = ref(60);
const toolsButton = ref<HTMLButtonElement>();
function closeTools() {
  toolsOpen.value = false;
  void nextTick(() => toolsButton.value?.focus({ preventScroll: true }));
}
function positionTools() {
  toolsTop.value = Math.max(12, Math.min((toolbar.value?.getBoundingClientRect().bottom ?? 44) + 8,
    window.innerHeight - (toolsPanel.value?.scrollHeight ?? 0) - 12));
}
watch(toolsOpen, async open => {
  if (!open) return;
  await nextTick();
  positionTools();
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
  '演示状态已保存', '已恢复上次保存的演示状态', '已恢复记录时的演示状态', '已恢复提问时的演示状态',
].includes(saveNotice.value));
const frameLoadCount = ref(0);
const frameEditing = ref(false);
let requestedRegion: string | undefined;
let teachingHelpInFlight: Promise<void> = Promise.resolve();
let saveQueue: Promise<unknown> = Promise.resolve();
let requestId = 0;
let pendingSnapshot: { id: number; owner?: 'feedback'; resolve: (state: PresentationState) => void; reject: (error: Error) => void } | undefined;
let generation = 0;
let frameChannel = crypto.randomUUID();
let themeObserver: MutationObserver | undefined;
let visibilityObserver: IntersectionObserver | undefined;
let hostVisible = true;
const readableText = computed(() => view.value?.readable_view.parts.map(part => part.kind === "markdown" ? part.text : "").join("") ?? "");

const feedback = ref<PresentationFeedback>();
const feedbackLoading = ref(false), feedbackSaving = ref(false), feedbackDetails = ref(false);
const feedbackError = ref(''), feedbackSceneNotice = ref('');
const feedbackReasons = ref<PresentationFeedbackReason[]>([]), feedbackComment = ref('');
const failedFeedback = ref<{ selection: PresentationFeedbackSelection | null }>();
const feedbackSceneRetry = ref<(() => Promise<void>)>();
let feedbackOperation = 0;
let feedbackQuestion: { text: string; receipt: PresentationFollowUp | null } | undefined;
const reasonLabels: Record<PresentationFeedbackReason, string> = {
  easier_to_understand: '更容易理解', apt_example: '例子贴切', useful_interaction: '交互有用',
  unclear: '没讲清楚', incorrect_content: '内容有误', hard_to_read: '画面难读', hard_to_operate: '操作不便',
};
const availableReasons = computed<PresentationFeedbackReason[]>(() => feedback.value?.selection?.choice === 'like'
  ? ['easier_to_understand', 'apt_example', 'useful_interaction'] : ['unclear', 'incorrect_content', 'hard_to_read', 'hard_to_operate']);
const feedbackDetailsDirty = computed(() => feedbackComment.value !== (feedback.value?.selection?.comment ?? '')
  || JSON.stringify(feedbackReasons.value) !== JSON.stringify(feedback.value?.selection?.reasons ?? []));
function cancelFeedbackScene() {
  feedbackOperation++;
  feedbackSceneRetry.value = undefined;
  if (feedbackSceneNotice.value === '正在附上演示状态…') feedbackSceneNotice.value = '未附上演示状态';
  if (pendingSnapshot?.owner === 'feedback') {
    pendingSnapshot.reject(new Error('反馈已更新。')); pendingSnapshot = undefined;
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
    feedbackSceneNotice.value = result.selection ? result.saved_state ? '已附上反馈时的演示状态' : '未附上演示状态' : '';
  } catch (failure) { if (current === generation) feedbackError.value = `反馈读取失败：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { if (current === generation) feedbackLoading.value = false; }
}
async function attachFeedbackScene(saved: PresentationFeedback, current: number, operation: number) {
  const active = () => current === generation && operation === feedbackOperation;
  let captured: PresentationState | undefined, receipt: PresentationFollowUp | undefined;
  const attach = async () => {
    if (!active()) return;
    feedbackSceneRetry.value = undefined; feedbackSceneNotice.value = '正在附上演示状态…';
    try {
      // Capture once. A retry saves/attaches this observation, never a newer scene.
      if (!captured) captured = await captureScene('feedback');
      if (!active()) return;
      if (!receipt) receipt = await saveState(captured);
      if (!active()) return;
      const result = await api.presentationFeedbackAttachState(saved.session_id, saved.turn_id, saved.reference, saved.feedback_revision, receipt);
      if (!active()) return;
      feedback.value = result; feedbackSceneNotice.value = '已附上反馈时的演示状态';
    } catch (failure) {
      if (!active()) return;
      feedbackSceneNotice.value = `未附上演示状态：${failure instanceof Error ? failure.message : String(failure)}`;
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
    else if (result.saved_state) feedbackSceneNotice.value = '已附上反馈时的演示状态';
    else void attachFeedbackScene(result, current, operation);
  } catch (failure) {
    if (current !== generation) return;
    failedFeedback.value = { selection };
    feedbackError.value = `反馈未保存：${failure instanceof Error ? failure.message : String(failure)}`;
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
  const text = [`我${selection.choice === 'like' ? '喜欢' : '不喜欢'}「${view.value?.title ?? '此演示'}」的版本 ${saved.reference.revision}。`,
    selection.reasons.map(reason => reasonLabels[reason]).join('、'), selection.comment].filter(Boolean).join(' ').replace(/[\r\n]+/g, ' ');
  if (!question.value.includes(text)) question.value = [question.value.trim(), text].filter(Boolean).join('\n\n');
  feedbackQuestion = { text, receipt: saved.saved_state };
  questionOpen.value = true; feedbackDetails.value = false; toolsOpen.value = false;
  if (props.workspace) emit('edit-question');
  else void nextTick(() => root.value?.querySelector<HTMLInputElement>('.presentation-follow-up input')?.focus());
}

watch([() => props.sessionId, () => props.turnId, () => props.reference.presentation_id, () => props.reference.revision, () => props.noteMemId, sceneKey], async () => {
  setFrameEditing(false);
  const current = ++generation;
  cancelFeedbackScene(); feedback.value = undefined; feedbackLoading.value = false; feedbackSaving.value = false;
  feedbackDetails.value = false; feedbackError.value = ''; feedbackSceneNotice.value = ''; failedFeedback.value = undefined;
  feedbackReasons.value = []; feedbackComment.value = ''; clearFeedbackQuestion();
  localShare.value = null; exporting.value = false;
  toolsOpen.value = false;
  frameChannel = crypto.randomUUID();
  pendingSnapshot?.reject(new Error("内容已切换，请重新追问。")); pendingSnapshot = undefined;
  saveNotice.value = "";
  requestedRegion = undefined;
  teachingHelpInFlight = Promise.resolve();
  view.value = undefined; documentText.value = ""; error.value = ""; ready.value = false;
  try {
    const result = props.noteMemId ? await api.notePresentationRead(props.noteMemId) : await api.presentationRead(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    const doc = presentationDocument(result, frameChannel);
    view.value = result; documentText.value = doc;
    if (!props.noteMemId) void loadFeedback();
    if (result.restored_state_revision) saveNotice.value = "已恢复上次保存的演示状态";
  } catch { if (current === generation) error.value = "内容暂时无法读取，请重新打开此回答。"; }
}, { immediate: true });

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
  return getComputedStyle(root.value).display !== "none";
}
function onFrameLoad() {
  frameLoadCount.value += 1;
  setFrameEditing(false);
  theme();
  sendZoom();
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "visibility", visible: hostVisible }, "*");
}
async function retry() {
  if (!view.value) return;
  cancelFeedbackScene();
  pendingSnapshot?.reject(new Error('内容正在重新加载。')); pendingSnapshot = undefined;
  generation++; frameChannel = crypto.randomUUID(); ready.value = false; error.value = "";
  feedbackSaving.value = false;
  documentText.value = "";
  await nextTick();
  documentText.value = presentationDocument(view.value, frameChannel);
  if (!props.noteMemId) void loadFeedback();
}
async function receive(event: MessageEvent) {
  if (!acceptsPresentationMessage(event, frame.value?.contentWindow, frameChannel) || error.value) return;
  const message = event.data;
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
      if (!host) throw new Error('当前演示没有可关联的教学邀请。');
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
    if (!message.located) saveNotice.value = '这个位置暂时不可用，可结合当前画面继续提问。';
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
    saveNotice.value = "已恢复控件；此版本未提供自定义参数和步骤的恢复方法。";
    return;
  }
  if (message.kind === "state") {
    if (message.request_id !== undefined) {
      if (pendingSnapshot && pendingSnapshot.id === message.request_id) { pendingSnapshot.resolve(message.state); pendingSnapshot = undefined; }
    } else if (!props.noteMemId) { void saveState(message.state).catch(() => {}); }
    return;
  }
  if (message.kind === "error") {
    error.value = "此内容运行出错，请稍后重试。";
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
function locateRegion(regionId: string) {
  if (!view.value?.teaching_binding?.regions.some(region => region.region_id === regionId)) return;
  if (!ready.value) { requestedRegion = regionId; return; }
  frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: 'locate-region', region_id: regionId }, '*');
}
function focusResponse() { return teachingHost.value?.focusResponse() ?? false; }
async function awaitTeachingHelp() {
  await teachingHelpInFlight.catch(() => {});
  await teachingHost.value?.waitForHelp();
}
function saveState(state: PresentationState) {
  const current = generation;
  const session = props.sessionId, turn = props.turnId, reference = { ...props.reference };
  const operation = saveQueue.catch(() => {}).then(async () => {
    if (current !== generation) throw new Error("内容已切换，请重新追问。");
    return api.presentationSaveState(session, turn, reference, state);
  });
  saveQueue = operation;
  return operation.then(receipt => {
    if (current === generation) saveNotice.value = "演示状态已保存";
    return receipt;
  }, failure => {
    if (current === generation) saveNotice.value = `演示状态保存失败：${failure instanceof Error ? failure.message : String(failure)}`;
    throw failure;
  });
}
async function captureScene(owner?: 'feedback'): Promise<PresentationState> {
  if (!owner) cancelFeedbackScene();
  if (!ready.value || error.value || pendingSnapshot) throw new Error("页面尚未准备好，请稍后重试");
  let timer: ReturnType<typeof setTimeout>;
  return new Promise<PresentationState>((resolve, reject) => {
    const id = ++requestId;
    pendingSnapshot = { id, owner, resolve: value => { clearTimeout(timer); resolve(value); }, reject: error => { clearTimeout(timer); reject(error); } };
    timer = setTimeout(() => { pendingSnapshot = undefined; reject(new Error("暂时无法读取当前演示状态，请稍后重试。")); }, 10000);
    frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "snapshot", request_id: id }, "*");
  });
}
async function snapshotScene(): Promise<{ receipt: PresentationFollowUp; state: PresentationState }> {
  const current = generation;
  const state = await captureScene();
  if (current !== generation) throw new Error('内容已切换，请重新追问。');
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
  } catch (failure) { saveNotice.value = `演示状态未保存：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { saving.value = false; }
}
async function recordNote() {
  if (!ready.value || saving.value || props.noteMemId) return;
  saving.value = true;
  const current = generation;
  try {
    const { receipt, state } = await snapshotScene();
    if (current === generation) emit("record-note", receipt, view.value?.title ?? "演示", [state.visible_step, state.observed_result].filter(Boolean).join(" · ").slice(0, 240));
  } catch (failure) { if (current === generation) saveNotice.value = `记录未开始：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { saving.value = false; }
}
async function followUp() {
  if (!ready.value || error.value || saving.value || props.busy) return;
  if (props.noteMemId && props.noteReceipt) {
    emit("follow-up", question.value.trim() || "解释记录时的结果", props.noteReceipt);
    question.value = "";
    return;
  }
  saving.value = true;
  const current = generation;
  const draft = question.value;
  const message = draft.trim() || "解释现在的结果";
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
    if (current === generation) saveNotice.value = `追问未发送：${failure instanceof Error ? failure.message : String(failure)}`;
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
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    if (!ready.value) {
      await new Promise<void>((resolve, reject) => {
        const stop = watch([ready, error], () => {
          if (!ready.value && !error.value) return;
          stop(); clearTimeout(timer);
          if (error.value) reject(new Error(error.value)); else resolve();
        });
        timer = setTimeout(() => { stop(); reject(new Error('内容尚未准备好，请重试。')); }, 10000);
      });
    }
    if (current !== generation) return;
    if (!props.noteMemId) await teachingSnapshot();
    const restored = props.noteMemId ? await api.notePresentationRead(props.noteMemId, true) : await api.presentationRead(props.sessionId, props.turnId, props.reference, receipt);
    if (current !== generation) return;
    view.value = restored;
    await retry();
    saveNotice.value = props.noteMemId ? "已恢复记录时的演示状态" : "已恢复提问时的演示状态";
  } catch (failure) { if (current === generation) saveNotice.value = `恢复失败：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { clearTimeout(timer); saving.value = false; }
}
defineExpose({ followUp, restore, locateRegion, focusResponse });
onMounted(() => {
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
  emit('teaching-host-available', false); emit('response-available', false);
  setFrameEditing(false);
  pendingSnapshot?.reject(new Error("内容已关闭。")); pendingSnapshot = undefined;
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
    :class="{ expanded, linked: network.linked }"
    :aria-label="view?.title || '富回答'"
    :aria-modal="expanded && !workspace || undefined"
    :role="expanded ? workspace ? 'region' : 'dialog' : undefined"
    :data-frame-editing="frameEditing"
    :data-content-generation="generation"
  >
    <Teleport :to="toolbarTarget || 'body'" :disabled="!toolbarTarget">
      <header ref="toolbar" class="presentation-toolbar">
        <strong class="presentation-title" :title="view?.title">{{ view?.title || '正在读取内容…' }}</strong>
        <button v-if="view && !noteMemId && !network.linked" class="presentation-note" type="button" :disabled="!ready || saving" @click="recordNote"><StickyNote :size="16" aria-hidden="true" /><span>记一下</span></button>
        <button v-if="view" ref="toolsButton" class="presentation-tools-toggle" type="button" aria-label="演示工具" :aria-expanded="toolsOpen" :aria-controls="toolsId" @click="toolsOpen = !toolsOpen"><Ellipsis :size="21" aria-hidden="true" /></button>
        <Teleport to="body" :disabled="!toolsOpen">
        <div v-if="toolsOpen" class="presentation-tools-backdrop" aria-hidden="true" @pointerdown="toolsOpen = false"></div>
        <div v-if="view" :id="toolsId" ref="toolsPanel" class="presentation-tools" :class="{ 'is-open': toolsOpen }" :style="toolsOpen ? { top: `${toolsTop}px`, maxHeight: `calc(100dvh - ${toolsTop}px - 12px)` } : undefined">
        <div class="presentation-tools-heading"><span>演示工具</span><button type="button" aria-label="关闭演示工具" @click="closeTools"><X :size="18" aria-hidden="true" /></button></div>
        <span v-if="view" class="presentation-zoom" role="group" aria-label="演示缩放">
          <span class="presentation-zoom-label">画面缩放</span>
          <button type="button" aria-label="缩小演示" :disabled="zoomPercent <= 50" @click="setZoom(zoomPercent - 10)"><Minus :size="16" aria-hidden="true" /></button>
          <button type="button" aria-label="恢复演示原始大小" @click="setZoom(100)">{{ zoomPercent }}%</button>
          <button type="button" aria-label="放大演示" :disabled="zoomPercent >= 150" @click="setZoom(zoomPercent + 10)"><Plus :size="16" aria-hidden="true" /></button>
        </span>
        <button class="presentation-tool-action" type="button" :disabled="!ready || saving || exporting" @click="toolsOpen = false; exportCurrent()"><ArrowDownToLine :size="16" aria-hidden="true" /><span>{{ exporting ? "正在取图…" : "导出长图" }}</span></button>
        <button class="presentation-tool-action" type="button" aria-label="文字说明与来源" :aria-expanded="readableOpen" @click="readableOpen = !readableOpen; toolsOpen = false"><BookOpen :size="16" aria-hidden="true" /><span>说明与来源</span></button>
        <button v-if="view && network.linked" type="button" :aria-expanded="questionOpen" @click="questionOpen = !questionOpen">追问</button>
        <button v-if="view && network.enabled && !network.linked && !noteMemId" class="presentation-tool-action" type="button" title="在附属窗口打开" @click="toolsOpen = false; openAttached()"><ExternalLink :size="16" aria-hidden="true" /><span>新窗口</span></button>
        <small v-if="saveNotice && !prominentNotice" class="presentation-save-notice" role="status" :title="saveNotice">{{ saveNotice }}</small>
        <div v-if="view && !noteMemId" class="presentation-feedback" role="group" aria-label="评价这一版演示">
          <span class="feedback-label">这一版</span>
          <button type="button" aria-label="喜欢" :aria-pressed="feedback?.selection?.choice === 'like'" :disabled="feedbackLoading || feedbackSaving || !feedback" @click="chooseFeedback('like')"><ThumbsUp :size="15" aria-hidden="true" /><span>喜欢</span></button>
          <button type="button" aria-label="不喜欢" :aria-pressed="feedback?.selection?.choice === 'dislike'" :disabled="feedbackLoading || feedbackSaving || !feedback" @click="chooseFeedback('dislike')"><ThumbsDown :size="15" aria-hidden="true" /><span>不喜欢</span></button>
          <button v-if="feedback?.selection" type="button" :aria-expanded="feedbackDetails" :disabled="feedbackSaving" @click="feedbackDetails = !feedbackDetails">补充原因</button>
          <span class="feedback-status" role="status">
            <template v-if="feedbackLoading">正在读取反馈…</template>
            <template v-else-if="feedbackSaving">正在保存反馈…</template>
            <template v-else-if="!feedbackError && feedback?.updated_at">
              <time :datetime="feedback.updated_at" :title="feedback.updated_at">{{ feedback.selection ? '反馈已保存' : '反馈已撤回' }}</time>
              <span v-if="feedback.selection && feedbackSceneNotice"> · {{ feedbackSceneNotice }}</span>
            </template>
          </span>
          <span v-if="feedbackError" class="feedback-error" role="alert">{{ feedbackError }}</span>
          <button v-if="feedbackError" type="button" :disabled="feedbackSaving || feedbackLoading" @click="failedFeedback ? saveFeedback(failedFeedback.selection) : loadFeedback()">重试反馈</button>
          <button v-if="feedbackSceneRetry" type="button" @click="feedbackSceneRetry()">重试附上演示状态</button>
          <form v-if="feedbackDetails && feedback?.selection" class="feedback-details" @submit.prevent="saveFeedbackDetails">
            <fieldset :disabled="feedbackSaving">
              <legend>原因（选填，可多选）</legend>
              <label v-for="reason in availableReasons" :key="reason"><input v-model="feedbackReasons" type="checkbox" :value="reason" />{{ reasonLabels[reason] }}</label>
              <label class="feedback-comment">补充（选填）<textarea v-model="feedbackComment" rows="2" placeholder="说说哪里有帮助，或哪里需要改进…"></textarea></label>
              <div class="feedback-actions">
                <button type="submit">保存补充</button>
                <button type="button" @click="saveFeedback(null)">撤回反馈</button>
                <button type="button" :disabled="busy || !!feedbackError || feedbackDetailsDirty" :title="feedbackDetailsDirty ? '请先保存补充' : '将已保存的反馈放入追问草稿'" @click="prepareFeedbackQuestion">带入追问</button>
                <button type="button" @click="feedbackDetails = false">关闭</button>
              </div>
            </fieldset>
          </form>
        </div>
        </div>
        </Teleport>
        <button v-if="view && !network.linked" ref="expandButton" type="button" class="presentation-expand" @click="toolsOpen = false; toggleExpanded()" :aria-label="expanded ? '收起' : '展开'" :title="expanded ? '收起演示' : '展开演示'" :aria-expanded="expanded"><component :is="expanded ? Minimize2 : Maximize2" :size="17" aria-hidden="true" /><span>{{ expanded ? '收起' : '展开' }}</span></button>
      </header>
    </Teleport>
    <Teleport to="body">
      <div v-if="prominentNotice" class="presentation-notice" role="status">
        <span>{{ saveNotice }}</span><button type="button" aria-label="关闭演示提示" @click="saveNotice = ''"><X :size="16" aria-hidden="true" /></button>
      </div>
    </Teleport>
    <p v-if="error" class="presentation-error" role="alert">{{ error }} <button v-if="view" type="button" @click="retry">重试</button></p>
    <template v-else>
      <p v-if="!ready" class="presentation-status" role="status">正在准备内容…</p>
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
      <Teleport :to="teachingTarget || 'body'" :disabled="!teachingTarget">
        <nav v-if="view?.teaching_binding?.regions.length" class="teaching-regions" aria-label="回到相关画面">
          <button v-for="region in view.teaching_binding.regions" :key="region.region_id" :disabled="!ready" @click="locateRegion(region.region_id)">{{ region.observation }}</button>
        </nav>
        <TutorActivities ref="teachingHost" v-if="view && ready && teaching" :session-id="sessionId" :turn-id="turnId" :reference="reference" :presentation-target="() => frame" :snapshot="teachingSnapshot" :before-response="awaitTeachingHelp" :ready="ready" :busy="busy || saving" :sources="view.sources"
          @response-available="emit('response-available', $event)" @source="(id, anchor) => emit('source', id, anchor)" />
        <TutorContinuation v-if="view && ready && teaching && !noteMemId && chatHead !== undefined"
          :session-id="sessionId" :turn-id="turnId" :reference="reference" :snapshot="teachingSnapshot" :head="chatHead ?? null" :enabled="!!continuationEnabled" :busy="busy || saving"
          @available="emit('continuation-available', $event)"
          @choose="(original, option, receipt) => emit('choose-continuation', original, option, receipt)" @locate="emit('locate-turn', $event)" @edit="emit('edit-question')" @read="emit('return-reading')" />
        <button v-if="workspace && ready && answerAvailable" class="presentation-clarify" :disabled="busy || saving" @click="clarifyAnswer">这一步没跟上</button>
      </Teleport>
      <form v-if="view && !workspace && (!network.linked || questionOpen)" class="presentation-follow-up" @submit.prevent="followUp">
        <input v-model="question" aria-label="针对当前演示追问" placeholder="解释当前结果，或描述要修改的内容…" :disabled="saving || busy" />
        <button type="submit" :disabled="!ready || saving || busy">{{ saving ? '正在准备追问…' : question.trim() ? '发送追问' : '解释现在的结果' }}</button>
      </form>
    </template>
    <section v-if="view && readableOpen" class="presentation-readable" aria-label="文字说明与来源">
      <div class="readable-heading"><strong>文字说明与来源 · 版本 {{ view.reference.revision }}</strong><button type="button" @click="readableOpen = false" aria-label="关闭文字说明与来源">×</button></div>
      <div class="readable-text" v-html="renderMarkdown(readableText)"></div>
      <p v-for="assumption in view.assumptions" :key="assumption">{{ assumption }}</p>
      <button v-for="source in view.sources" :key="source.source_ref_id" type="button" :title="source.label" @click="emit('source', source.source_ref_id, $event.currentTarget as HTMLElement)">{{ sourceChipLabel(view.sources, source.source_ref_id) }}</button>
    </section>
    <ShareImagePanel v-if="localShare" :source="localShare" @close="localShare = null" />
  </section>
</template>

<style scoped>
.agent-presentation { position: relative; container-type: inline-size; border: 1px solid var(--line, #e6dfd8); border-radius: 24px; background: var(--canvas, #faf9f5); color: var(--ink, #252523); overflow: hidden; margin: 12px 0; min-width: 0; }
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
.presentation-notice { position: fixed; z-index: 1210; bottom: max(12px, env(safe-area-inset-bottom)); left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 12px; width: max-content; max-width: min(520px, calc(100vw - 24px)); padding: 10px 12px 10px 16px; border: 1px solid var(--line); border-radius: 12px; background: var(--canvas); color: var(--ink); box-shadow: 0 6px 28px #25232026; font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; }
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
iframe { display: block; width: 100%; height: clamp(180px, calc(100dvh - 220px), 420px); min-height: 180px; border: 0; background: var(--canvas, #faf9f5); }
.presentation-follow-up { position: relative; flex: 0 0 auto; display: flex; flex-wrap: wrap; gap: 8px; padding: 6px 10px max(6px, env(safe-area-inset-bottom)); }
.presentation-follow-up input { flex: 1; min-width: 120px; color: inherit; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 8px; }
.presentation-follow-up small { flex: 1 0 100%; overflow-wrap: anywhere; }
button:disabled { opacity: .55; cursor: default; }
.expanded { position: fixed; inset: max(20px, env(safe-area-inset-top)) max(20px, env(safe-area-inset-right)) max(20px, env(safe-area-inset-bottom)) max(20px, env(safe-area-inset-left)); z-index: 90; margin: 0; display: flex; flex-direction: column; max-height: calc(100dvh - max(40px, env(safe-area-inset-top) + env(safe-area-inset-bottom))); box-shadow: 0 16px 80px #0005; }
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
.presentation-error { padding: 0 16px; }
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
  .expanded { inset: max(8px, env(safe-area-inset-top)) max(8px, env(safe-area-inset-right)) max(8px, env(safe-area-inset-bottom)) max(8px, env(safe-area-inset-left)); max-height: calc(100dvh - max(16px, env(safe-area-inset-top) + env(safe-area-inset-bottom))); }
}
@media (max-width: 700px) {
  .presentation-toolbar { flex-wrap: nowrap; gap: 4px; padding: 6px 10px; }
  .presentation-title { flex: 1 1 0; font-size: 13px; }
  .presentation-toolbar > button { flex: none; min-height: 40px; padding: 8px; }
  .presentation-tools-toggle { display: inline-flex !important; width: 40px; }
  .presentation-expand { width: 40px; }
  .presentation-expand > span { display: none; }
  .presentation-tools { display: none; }
  .presentation-tools-backdrop { display: block !important; position: fixed; z-index: 1190; inset: 0; background: #2523200a; }
  .presentation-tools.is-open { position: fixed; z-index: 1200; right: max(12px, env(safe-area-inset-right)); display: flex; flex-wrap: wrap; align-items: center; gap: 8px; width: min(340px, calc(100vw - 24px)); max-height: calc(100dvh - 24px); overflow: auto; padding: 14px; border: 1px solid var(--line); border-radius: 18px; background: var(--canvas); color: var(--ink); font-size: 13px; box-shadow: 0 12px 40px #25232026, 0 2px 8px #2523200a; }
  .presentation-tools-heading { display: flex !important; flex: 1 0 100%; align-items: center; justify-content: space-between; color: var(--ink); font-size: 14px; font-weight: 600; }
  .presentation-tools-heading button { min-width: 36px; min-height: 36px; margin: -6px -6px -6px 0; }
  .presentation-zoom { flex: 1 0 100%; padding: 4px; border-radius: 12px; }
  .presentation-zoom-label { display: inline !important; flex: 1; padding-left: 8px; color: var(--steel); }
  .presentation-zoom button { min-width: 40px; min-height: 40px; color: var(--ink); }
  .presentation-zoom button:nth-last-child(2) { min-width: 58px; }
  .presentation-tools > .presentation-tool-action { flex: 1 0 calc(50% - 4px); justify-content: flex-start; min-height: 44px; padding: 10px; color: var(--ink); }
  .presentation-save-notice { flex-basis: 100%; max-width: none; padding-inline: 10px; }
  .presentation-feedback { margin-top: 2px; padding-top: 12px; border-top: 1px solid var(--line); gap: 4px; }
  .feedback-label { margin-right: auto; }
  .presentation-feedback > button { min-height: 40px; }
  .feedback-status:empty { display: none; }
  .feedback-status, .feedback-error { flex-basis: 100%; line-height: 1.6; }
  .feedback-details { position: static; width: 100%; max-height: none; border: 0; box-shadow: none; border-radius: 0; padding: 8px 0 0; }
}
</style>
