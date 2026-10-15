<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { ArrowUpRight, BookOpen, Smartphone } from '@lucide/vue';
import MarkdownIt from 'markdown-it';
import ReaderWorkspace, { type WorkspaceAuxTab } from './ReaderWorkspace.vue';
import RightRail from './RightRail.vue';
import ConversationLearningGoal from './ConversationLearningGoal.vue';
import TutorControl from './TutorControl.vue';
import MobileLearningSheet from './MobileLearningSheet.vue';
import type { TutorSession } from '../generated/TutorSession';
import type { OuterOutcome } from '../api';

// This entry is served by Vite only; it is absent from production build inputs.
const embedded = new URLSearchParams(location.search).has('embedded');
const devices = [{ label: '标准手机', width: 390, height: 669 }, { label: '长屏手机', width: 390, height: 844 }, { label: '小屏手机', width: 320, height: 568 }];
const device = ref(devices[0]);
const preview = ref<HTMLIFrameElement>();
function showState(state: string) { preview.value?.contentWindow?.postMessage({ previewState: state }, location.origin); }
const markdown = new MarkdownIt();
const lesson = `## 1.4.2 SmartNIC：把网络工作放到更合适的地方

上一节，我们先看设备的空闲能力什么时候用尽。这一节，换一个观察角度：**网络变快以后，谁在忙？**

> 网络带宽提高，虚拟网络策略也日趋复杂。

带宽增长，意味着每秒要处理的数据更多；策略变复杂，意味着每个数据包需要做的事情也更多。

### 先找到被占用的资源

如果这些工作都由 CPU 承担，网络处理就会占用原本可以运行应用的计算资源。

所以问题不只是“再加几台服务器”，还包括：**哪些工作，可以从 CPU 身上移走？**

### 网卡也可以分担工作

SmartNIC 能在网卡上执行一部分数据处理。把适合的网络工作交给它，就能把一部分 CPU 资源留给应用。

试着连起这条线：

**带宽与策略增长 → 网络处理变多 → 占用 CPU → 把部分工作交给网卡。**

带着这条线回看书里的例子，设备名称就有了来由。接下来，用自己的话说说这里的架构为什么发生变化。`;
function outcome(answer: string): OuterOutcome {
  return { answer, answer_view: { parts: [{ kind: 'markdown', text: answer }], sources: [] }, effects: [], trace: [], memory_updates: [], profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] }, incomplete: false, warning: null, turns: 1, tokens_spent: 0 };
}
function turn(id: string, user: string, answer: string, teaching = false) {
  return { turnId: id, user, hasTeaching: teaching, pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], outcome: outcome(answer) };
}
const chat = ref([turn('preview-lesson', '接着讲讲 SmartNIC，需求是怎么推动架构变化的？', lesson, true)]);
const agentInput = ref('');
const teaching = ref(true);
const learningSession = ref({ id: 'preview-learning', user_intent: '学懂 1.4 需求如何推动架构设计', status: 'active' } as TutorSession);
const recordedAnswer = ref('');
const hintSeen = ref(false);
const recordsOpen = ref(false);
const recordsTitle = ref('我的理解');
const requestedTab = ref<WorkspaceAuxTab>('agent');
const requestedTabRevision = ref(0);
const fullscreen = ref(false);
const workspace = ref<InstanceType<typeof ReaderWorkspace>>();
const logical = { contextKey: 'mobile-preview', revision: 1, activePreset: 'technical_read', openSlots: ['technical.structure_map', 'technical.agent'], focusedSlot: null };
const realFetch = window.fetch;
if (embedded) {
  // Use in-memory demo responses so every preview action stays local.
  window.fetch = async (input, init) => {
    const url = new URL(input instanceof Request ? input.url : String(input), location.href);
    if (url.origin !== location.origin || !url.pathname.startsWith('/api/')) return realFetch(input, init);
    const body = typeof init?.body === 'string' ? JSON.parse(init.body) : {};
    if (url.pathname === '/api/tutor/activities') return Response.json({ activities: body.turn_id === 'preview-lesson' ? [{
      delivery_ref: 'preview-response', status: teaching.value ? 'active' : 'paused', attempted: !!recordedAnswer.value, help_seen: hintSeen.value ? ['preview-hint'] : [],
      move: { move_id: 'smartnic-response', prompt: '网络带宽和策略复杂度增加时，为什么要把一部分工作交给 SmartNIC？\n\n试着把「需求 → 资源 → 工作去向」连起来。', actions: ['submit', 'revise', 'request_hint', 'skip'] },
      assessment: recordedAnswer.value ? { status: 'unassessed', action_ref: 'preview-record' } : null,
    }] : [] });
    if (url.pathname === '/api/tutor/action') {
      if (body.action === 'request_hint') {
        hintSeen.value = true;
        return Response.json({ event_id: 'preview-hint', help: { event_id: 'preview-hint', text: '先想一想：如果所有网络数据包都交给 CPU，它还能把多少时间留给应用？再看看网卡能分担哪一部分工作。' } });
      }
      if (body.response) recordedAnswer.value = body.response;
      return Response.json({ event_id: 'preview-record', help: null });
    }
    return Response.json({ activities: [], interpretations: [], rows: [], next: null });
  };
}
function requestTab(tab: WorkspaceAuxTab) { requestedTab.value = tab; requestedTabRevision.value++; }
function sendQuestion() {
  if (!agentInput.value.trim()) return;
  chat.value.push(turn(`preview-${chat.value.length}`, agentInput.value, '可以先抓住一个关系：网络处理也需要计算资源。\n\n当 CPU 需要处理更多数据包、执行更复杂的策略时，留给应用的资源就会受到影响。SmartNIC 分担其中适合卸载的工作，让 CPU 更专注于应用。'));
  agentInput.value = '';
}
function clarify() {
  chat.value.push(turn(`preview-${chat.value.length}`, '这一步没跟上', '我们换个角度看。\n\n把 CPU 想成同时做两件事的人：一边运行应用，一边处理网络数据。现在网络工作越来越多，他花在应用上的时间就会减少。\n\nSmartNIC 像一个能分担网络工作的帮手。重点是**把适合的工作交出去，释放 CPU 的时间**。'));
}
function openRecords(title = '我的理解') { recordsTitle.value = title; recordsOpen.value = true; }
function toggleTeaching() {
  teaching.value = !teaching.value;
  learningSession.value.status = teaching.value ? 'active' : 'paused';
  window.dispatchEvent(new Event('tutor-state-changed'));
}
async function receivePreviewState(event: MessageEvent) {
  if (event.origin !== location.origin || event.source !== window.parent || !event.data?.previewState) return;
  recordsOpen.value = false;
  workspace.value?.showAssistant('agent');
  await nextTick();
  document.querySelector<HTMLButtonElement>('[aria-label="关闭学习与会话"]')?.click();
  document.querySelector<HTMLButtonElement>('[aria-label="收起输入"]')?.click();
  document.querySelector<HTMLButtonElement>('[aria-label="收起作答"]')?.click();
  await nextTick();
  const selector: Record<string, string> = { answer: '.learning-respond', settings: '.mobile-learning-title' };
  const state = event.data.previewState;
  if (state === 'ask') document.querySelector<HTMLTextAreaElement>('.agent-input textarea')?.focus();
  if (selector[state]) document.querySelector<HTMLButtonElement>(selector[state])?.click();
  if (state === 'reading') { document.querySelector<HTMLTextAreaElement>('.agent-input textarea')?.blur(); document.querySelector('.transcript')?.scrollTo({ top: 0 }); }
}
onMounted(async () => {
  if (!embedded) return;
  workspace.value?.showAssistant('agent');
  window.addEventListener('message', receivePreviewState);
  await nextTick();
  requestAnimationFrame(() => document.querySelector('.transcript')?.scrollTo({ top: 0 }));
});
onBeforeUnmount(() => { if (embedded) window.fetch = realFetch; window.removeEventListener('message', receivePreviewState); });
</script>

<template>
  <main v-if="!embedded" class="preview-stage">
    <header class="preview-heading"><span class="preview-brand"><BookOpen :size="19" />问这本书 <i>设计预览 / 01</i></span><span class="local-label">仅本地 · 未发布</span></header>
    <div class="preview-layout">
      <section class="preview-copy">
        <span class="eyebrow">为阅读留出空间</span><h1>把注意力，<br />还给眼前这一段。</h1>
        <p class="preview-intro">提问始终在手边，有想法就接着聊。需要帮助或准备作答时，再用输入框上方的快捷入口。</p>
        <div class="preview-states"><button @click="showState('reading')"><b>01</b><span>阅读<small>正文为主，随时可以接着聊</small></span><ArrowUpRight :size="17" /></button><button @click="showState('ask')"><b>02</b><span>提问<small>常驻输入框，直接写下想法</small></span><ArrowUpRight :size="17" /></button><button @click="showState('answer')"><b>03</b><span>作答<small>进入题目，提问草稿继续保留</small></span><ArrowUpRight :size="17" /></button><button @click="showState('settings')"><b>04</b><span>学习面板<small>目标、教学开关和会话</small></span><ArrowUpRight :size="17" /></button></div>
        <p class="preview-note">右侧可以直接操作。使用演示内容，输入和提交仅保留在此预览中。<a href="/mobile-learning-preview.html?embedded=1" target="_blank">单独打开预览 <ArrowUpRight :size="12" /></a></p>
      </section>
      <section class="preview-device-area" aria-label="手机预览">
        <div class="device-picker"><Smartphone :size="16" /><button v-for="option in devices" :key="option.width + option.height" :aria-pressed="device.width === option.width && device.height === option.height" @click="device = option">{{ option.label }}</button></div>
        <div class="device-frame" :style="{ width: `${device.width}px`, height: `${device.height}px` }"><iframe ref="preview" src="/mobile-learning-preview.html?embedded=1" title="可操作的手机布局预览" /></div>
        <p class="device-size">{{ device.width }} × {{ device.height }} · 浏览器内容区域</p>
      </section>
    </div>
  </main>
  <div v-else class="app mobile-preview-app">
    <ReaderWorkspace ref="workspace" :logical="logical" @tab-request="requestTab">
      <aside class="left-rail"><p>1.4 需求如何推动架构设计</p></aside><div class="resize-handle resize-handle-left" />
      <main class="reader-pane preview-original"><span>AI 基础设施 · 1.4.2</span><h1>SmartNIC</h1><p>网络带宽提高，虚拟网络策略也日趋复杂。SmartNIC 能够在网卡上执行部分数据处理，让网络工作有了新的分工方式。</p><p>读到这里，可以打开问答，继续梳理需求、资源与架构之间的联系。</p><button @click="workspace?.showAssistant('agent')">继续问这本书 →</button></main>
      <div class="resize-handle resize-handle-right" />
      <RightRail v-model:agent-input="agentInput" :chat="chat" :chat-sessions="[]" active-chat-session-id="preview-chat" :fullscreen="fullscreen" :sending="false" :learning-title="learningSession.user_intent" :learning-attention="teaching ? '' : '连续教学已暂停'" :tutor-enabled="teaching" :requested-tab="requestedTab" :requested-tab-revision="requestedTabRevision" :selected-lid="null" :selected-formula="null" :context-notes="[]" :context-highlights="[]" :render-markdown="text => markdown.render(text)" :eff-label="() => ''" :eff-state="() => undefined" :is-goto="() => false" :show-effect-primary="() => false" :show-effect-secondary="() => false" :effect-primary-label="() => ''" :effect-secondary-label="() => ''" :goto-back="() => ''" :ask-draft="null" @toggle-fullscreen="fullscreen = !fullscreen" @return-reading="fullscreen = false; workspace?.showReader()" @workspace-menu="openRecords('阅读设置')" @send-agent="sendQuestion" @clarify-answer="clarify" @new-chat="chat = []; agentInput = ''">
        <template #learning-actions><button type="button" @click="openRecords()">我的理解</button></template>
        <template #learning-details><button type="button" @click="openRecords('学习详情')">学习详情</button></template>
        <template #learning-goal><ConversationLearningGoal :session="learningSession" :enabled="teaching" :busy="false" :available="true" :ready="true" :other-session="false" error="" @goal="learningSession = { ...learningSession, user_intent: $event }" @manage="openRecords('学习详情')" /></template>
        <template #tutor-control><TutorControl :enabled="teaching" :label="teaching ? '围绕你的目标逐步学' : '连续教学已暂停'" :busy="false" :unavailable="false" error="" @toggle="toggleTeaching" /></template>
      </RightRail>
    </ReaderWorkspace>
    <MobileLearningSheet :open="recordsOpen" :title="recordsTitle" @close="recordsOpen = false"><div class="preview-record"><small>{{ learningSession.user_intent }}</small><h3>{{ recordedAnswer ? '你刚才的回答' : '让理解慢慢积累' }}</h3><p>{{ recordedAnswer || '回答这节的问题之后，你写下的理解会出现在这里。' }}</p><span v-if="recordedAnswer">回答已记录，尚未评分。</span></div></MobileLearningSheet>
  </div>
</template>

<style scoped>
.preview-stage { height: 100dvh; overflow: auto; background: #f3f1eb; color: #282a25; }
.preview-heading { max-width: 1240px; margin: 0 auto; padding: 28px 36px; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #ddd9ce; }
.preview-brand { display: flex; align-items: center; gap: 10px; font-size: 16px; font-weight: 600; }
.preview-brand svg { color: #a85b42; }.preview-brand i { margin-left: 18px; color: #8a8b82; font-size: 11px; font-style: normal; font-weight: 400; letter-spacing: .08em; }
.local-label { color: #818478; font-size: 12px; }
.preview-layout { max-width: 1090px; margin: 0 auto; padding: 38px 36px 60px; display: grid; grid-template-columns: minmax(280px, 1fr) minmax(390px, 1fr); gap: 70px; align-items: start; }
.preview-copy { padding-top: 36px; }.eyebrow { color: #a85b42; letter-spacing: .18em; font-size: 12px; }
h1 { margin: 20px 0; font-size: clamp(30px, 3vw, 42px); font-weight: 500; line-height: 1.55; letter-spacing: -.03em; }
.preview-intro { max-width: 360px; color: #797c71; font-size: 14px; line-height: 1.9; }
.preview-states { margin: 30px 0 22px; display: grid; gap: 4px; }
.preview-states button { display: flex; align-items: center; gap: 16px; padding: 16px 0; border: 0; border-bottom: 1px solid #ddd9ce; border-radius: 0; background: transparent; text-align: left; color: #363b30; cursor: pointer; }
.preview-states b { font-size: 11px; font-weight: 400; color: #a85b42; }.preview-states span { flex: 1; font-size: 15px; }.preview-states small { display: block; color: #888b80; font-size: 12px; margin-top: 5px; }.preview-states button:hover { color: #a85b42; }
.preview-note { color: #929489; font-size: 11px; line-height: 1.9; }.preview-note a { display: flex; align-items: center; gap: 3px; width: fit-content; margin-top: 10px; color: #a85b42; text-decoration: none; }
.preview-device-area { display: flex; align-items: center; flex-direction: column; gap: 18px; }.device-picker { display: flex; align-items: center; gap: 4px; padding: 5px; border: 1px solid #ddd9ce; border-radius: 30px; }.device-picker svg { margin: 0 7px; color: #8c9084; }.device-picker button { border: 0; border-radius: 20px; padding: 7px 12px; color: #7f8278; font-size: 11px; background: transparent; cursor: pointer; }.device-picker button[aria-pressed=true] { background: #e8e4da; color: #333b2c; }
.device-frame { overflow: hidden; border-radius: 24px; outline: 1px solid #ccc7bb; background: var(--canvas); box-shadow: 0 16px 60px #49422c12, 0 0 0 7px #e4e0d6; }.device-frame iframe { display: block; width: 100%; height: 100%; border: 0; }.device-size { margin: 0; font-size: 10px; letter-spacing: .04em; color: #929489; }
.preview-original { padding: 32px 24px; line-height: 1.9; }.preview-original > span { color: var(--steel); font-size: 12px; }.preview-original h1 { font-size: 30px; }.preview-original button { padding: 12px 16px; border: 0; background: var(--brand-green-deep); color: white; border-radius: 10px; }
.preview-record { padding: 6px 0 24px; line-height: 1.8; }.preview-record small, .preview-record > span { color: var(--steel); font-size: 12px; }
@media (max-width: 840px) { .preview-layout { display: flex; flex-direction: column; align-items: center; gap: 28px; padding: 16px 12px 40px; }.preview-copy { padding: 0; width: min(390px, 100%); }.preview-copy h1 { font-size: 27px; }.preview-states { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 20px; }.preview-heading { padding: 18px; }.preview-brand i { display: none; }.preview-device-area { max-width: 100%; }.device-frame { max-width: calc(100vw - 32px); } }
</style>
