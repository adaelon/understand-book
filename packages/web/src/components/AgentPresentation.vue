<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api as sharedApi } from "../api";
import { bindSceneApi, network } from "../network-context";
import { openLinkedPresentation } from "../network-presentation";
const api = bindSceneApi(sharedApi);
import TutorActivities from "./TutorActivities.vue";
import type { PresentationRef } from "../generated/PresentationRef";
import type { PresentationView } from "../generated/PresentationView";
import type { PresentationState } from "../generated/PresentationState";
import type { PresentationFollowUp } from "../generated/PresentationFollowUp";
import { presentationDocument } from "../presentation-document";
import { acceptsPresentationMessage, resolvePresentationEditingMessage } from "../presentation-host";
import { renderMarkdown } from "../md";
import { sourceChipLabel } from "../source-chip.js";

const props = defineProps<{ sessionId: string; turnId: string; reference: PresentationRef; busy?: boolean; teaching?: boolean; workspace?: boolean; sharedQuestion?: string; toolbarTarget?: HTMLElement }>();
const emit = defineEmits<{
  (e: "source", id: string, anchor: HTMLElement): void;
  (e: "follow-up", message: string, receipt: PresentationFollowUp): void;
  (e: "expand", expanded: boolean): void;
  (e: "update:sharedQuestion", question: string): void;
}>();
const teachingHost = ref<InstanceType<typeof TutorActivities>>();
const frame = ref<HTMLIFrameElement>();
const root = ref<HTMLElement>();
const expandButton = ref<HTMLButtonElement>();
const view = ref<PresentationView>();
const documentText = ref("");
const error = ref("");
const expanded = ref(false);
const ready = ref(false);
const zoomPercent = ref(100);
const readableOpen = ref(false);
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
const frameLoadCount = ref(0);
const frameEditing = ref(false);
let saveQueue: Promise<unknown> = Promise.resolve();
let requestId = 0;
let pendingSnapshot: { id: number; resolve: (state: PresentationState) => void; reject: (error: Error) => void } | undefined;
let generation = 0;
let frameChannel = crypto.randomUUID();
let observedRevision = 0;
let themeObserver: MutationObserver | undefined;
let visibilityObserver: IntersectionObserver | undefined;
let hostVisible = true;
const readableText = computed(() => view.value?.readable_view.parts.map(part => part.kind === "markdown" ? part.text : "").join("") ?? "");

watch([() => props.sessionId, () => props.turnId, () => props.reference.presentation_id, () => props.reference.revision], async () => {
  setFrameEditing(false);
  const current = ++generation;
  frameChannel = crypto.randomUUID();
  pendingSnapshot?.reject(new Error("内容已切换，请重新追问。")); pendingSnapshot = undefined;
  saveNotice.value = "";
  view.value = undefined; documentText.value = ""; error.value = ""; ready.value = false; observedRevision = 0;
  try {
    const result = await api.presentationRead(props.sessionId, props.turnId, props.reference);
    if (current !== generation) return;
    const doc = presentationDocument(result, frameChannel);
    view.value = result; documentText.value = doc;
    if (result.restored_state_revision) saveNotice.value = "已恢复上次保存的现场";
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
  generation++; frameChannel = crypto.randomUUID(); observedRevision = 0; ready.value = false; error.value = "";
  documentText.value = "";
  await nextTick();
  documentText.value = presentationDocument(view.value, frameChannel);
}
async function receive(event: MessageEvent) {
  if (!acceptsPresentationMessage(event, frame.value?.contentWindow, frameChannel) || error.value) return;
  const message = event.data;
  if (message.kind === "teaching-action" && ready.value && typeof message.move_id === "string") {
    teachingHost.value?.focusActivity(message.move_id, message.response); return;
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
    } else { void saveState(message.state).catch(() => {}); }
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
  if (message.kind !== "observe" || !Number.isInteger(message.revision) || message.revision <= observedRevision
      || typeof message.text !== "string" || !Array.isArray(message.source_ref_ids)) return;
  const current = generation;
  observedRevision = message.revision;
  try {
    const result = await api.presentationObserve(props.sessionId, props.turnId, props.reference, message.text, message.source_ref_ids);
    if (current !== generation || observedRevision !== message.revision || error.value) return;
    if (!result.accepted) throw new Error("rejected");
    ready.value = true;
    frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "accepted", revision: message.revision }, "*");
    theme();
    sendZoom();
  } catch { if (current === generation && observedRevision === message.revision) error.value = "此内容的文字或来源无法显示，请重新生成。"; }
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
    if (current === generation) saveNotice.value = "现场已保存";
    return receipt;
  }, failure => {
    if (current === generation) saveNotice.value = `现场保存失败：${failure instanceof Error ? failure.message : String(failure)}`;
    throw failure;
  });
}
async function teachingSnapshot(): Promise<PresentationFollowUp> {
  if (!ready.value || error.value || pendingSnapshot) throw new Error("页面尚未准备好，请稍后重试");
  let timer: ReturnType<typeof setTimeout>;
  const snapshot = await new Promise<PresentationState>((resolve, reject) => {
    const id = ++requestId;
    pendingSnapshot = { id, resolve: value => { clearTimeout(timer); resolve(value); }, reject };
    timer = setTimeout(() => { pendingSnapshot = undefined; reject(new Error("未收到页面现场")); }, 10000);
    frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "snapshot", request_id: id }, "*");
  });
  return saveState(snapshot);
}
async function followUp() {
  if (!ready.value || error.value || saving.value || props.busy) return;
  saving.value = true;
  const current = generation;
  const draft = question.value;
  const message = draft.trim() || "解释现在的结果";
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const snapshot = await new Promise<PresentationState>((resolve, reject) => {
      const id = ++requestId;
      pendingSnapshot = { id, resolve, reject };
      timer = setTimeout(() => { pendingSnapshot = undefined; reject(new Error("未收到页面现场，请重试。")); }, 10000);
      frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "snapshot", request_id: id }, "*");
    });
    clearTimeout(timer);
    const receipt = await saveState(snapshot);
    if (current !== generation || error.value || props.busy) return;
    emit("follow-up", message, receipt);
    if (question.value === draft) question.value = "";
  } catch (failure) {
    if (current === generation) saveNotice.value = `追问未发送：${failure instanceof Error ? failure.message : String(failure)}`;
  } finally { clearTimeout(timer); saving.value = false; }
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
    const snapshot = await new Promise<PresentationState>((resolve, reject) => {
      const id = ++requestId;
      pendingSnapshot = { id, resolve, reject };
      timer = setTimeout(() => { pendingSnapshot = undefined; reject(new Error("未收到页面现场")); }, 10000);
      frame.value?.contentWindow?.postMessage({ channel: frameChannel, kind: "snapshot", request_id: id }, "*");
    });
    clearTimeout(timer);
    await saveState(snapshot);
    const restored = await api.presentationRead(props.sessionId, props.turnId, props.reference, receipt);
    if (current !== generation) return;
    view.value = restored;
    await retry();
    saveNotice.value = "已回到提问时的现场";
  } catch (failure) { if (current === generation) saveNotice.value = `恢复失败：${failure instanceof Error ? failure.message : String(failure)}`; }
  finally { clearTimeout(timer); saving.value = false; }
}
defineExpose({ followUp, restore });
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
  setFrameEditing(false);
  pendingSnapshot?.reject(new Error("内容已关闭。")); pendingSnapshot = undefined;
  generation++; themeObserver?.disconnect(); visibilityObserver?.disconnect();
  window.removeEventListener("message", receive); window.removeEventListener("keydown", keydown);
});
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
      <header class="presentation-toolbar">
        <strong :title="view?.title">{{ view?.title || '正在读取内容…' }}</strong>
        <span v-if="view" class="presentation-zoom" role="group" aria-label="演示缩放">
          <button type="button" aria-label="缩小演示" :disabled="zoomPercent <= 50" @click="setZoom(zoomPercent - 10)">−</button>
          <button type="button" aria-label="恢复演示原始大小" @click="setZoom(100)">{{ zoomPercent }}%</button>
          <button type="button" aria-label="放大演示" :disabled="zoomPercent >= 150" @click="setZoom(zoomPercent + 10)">＋</button>
        </span>
        <button v-if="view" type="button" aria-label="文字说明与来源" :aria-expanded="readableOpen" @click="readableOpen = !readableOpen">说明与来源</button>
        <button v-if="view && network.linked" type="button" :aria-expanded="questionOpen" @click="questionOpen = !questionOpen">追问</button>
        <button v-if="view && network.enabled && !network.linked" title="在附属窗口打开" @click="openAttached">新窗口</button>
        <button v-if="view && !network.linked" ref="expandButton" type="button" class="presentation-expand" @click="toggleExpanded" :aria-expanded="expanded">{{ expanded ? '收起' : '展开' }}</button>
        <small v-if="saveNotice" class="presentation-save-notice" role="status" :title="saveNotice">{{ saveNotice }}</small>
      </header>
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
      <TutorActivities ref="teachingHost" v-if="view && ready && teaching" :session-id="sessionId" :turn-id="turnId" :reference="reference" :snapshot="teachingSnapshot" :ready="ready" />
      <form v-if="view && !workspace && (!network.linked || questionOpen)" class="presentation-follow-up" @submit.prevent="followUp">
        <input v-model="question" aria-label="针对当前现场追问" placeholder="解释当前结果，或描述要修改的内容…" :disabled="saving || busy" />
        <button type="submit" :disabled="!ready || saving || busy">{{ saving ? '正在保存现场…' : question.trim() ? '发送追问' : '解释现在的结果' }}</button>
      </form>
    </template>
    <section v-if="view && readableOpen" class="presentation-readable" aria-label="文字说明与来源">
      <div class="readable-heading"><strong>文字说明与来源 · 版本 {{ view.reference.revision }}</strong><button type="button" @click="readableOpen = false" aria-label="关闭文字说明与来源">×</button></div>
      <div class="readable-text" v-html="renderMarkdown(readableText)"></div>
      <p v-for="assumption in view.assumptions" :key="assumption">{{ assumption }}</p>
      <button v-for="source in view.sources" :key="source.source_ref_id" type="button" :title="source.label" @click="emit('source', source.source_ref_id, $event.currentTarget as HTMLElement)">{{ sourceChipLabel(view.sources, source.source_ref_id) }}</button>
    </section>
  </section>
</template>

<style scoped>
.agent-presentation { position: relative; container-type: inline-size; border: 1px solid var(--line, #e6dfd8); border-radius: 12px; background: var(--canvas, #faf9f5); color: var(--ink, #252523); overflow: hidden; margin: 12px 0; min-width: 0; }
.presentation-toolbar { flex: 1 1 auto; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 6px; padding: 6px 10px; min-width: 0; color: var(--ink); font-size: 13px; }
.presentation-toolbar strong { flex: 1 1 100px; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
button { min-height: 30px; font: inherit; color: var(--accent, #a9583e); border: 1px solid var(--line, #e6dfd8); border-radius: 6px; background: var(--surface, #efe9de); padding: 3px 7px; cursor: pointer; }
.presentation-zoom { display: inline-flex; flex: none; gap: 2px; }
.presentation-zoom button { min-width: 28px; }
.presentation-save-notice { color: var(--steel); font-size: 11px; max-width: 180px; overflow-wrap: anywhere; }
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
</style>
