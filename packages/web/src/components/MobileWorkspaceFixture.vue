<script setup lang="ts">
import { ref } from "vue";
import TopBar from "./TopBar.vue";
import TutorControl from "./TutorControl.vue";
import ReaderPane, { type Segment } from "./ReaderPane.vue";
import ReaderWorkspace from "./ReaderWorkspace.vue";
import RightRail from "./RightRail.vue";
import type { WorkspaceAuxTab } from "./ReaderWorkspace.vue";
import type { ReaderSelectionSnapshot } from "../useReaderSelection";

const logical = {
  contextKey: "fixture:chat",
  revision: 1,
  activePreset: "technical_read",
  openSlots: ["technical.structure_map", "technical.agent"],
  focusedSlot: null,
};
const segments: Segment[] = [
  { lid: "1.1", kind: "paragraph", text: "移动阅读原生选区包含中文、emoji 😀 与跨行文本。", formula: null, imageAsset: null },
  { lid: "1.2", kind: "code", text: "fn main() {\n    let very_long_identifier = \"保留缩进与很长很长的一行代码😀\";\n}", formula: null, imageAsset: null },
];
const longChat = new URLSearchParams(location.search).has('long-chat');
const requestedTab = ref<WorkspaceAuxTab>('agent');
const requestedTabRevision = ref(0);
const fullscreen = ref(false);
const answer = '<p>两条曲线表示两种解释各自的预测，页面明确标注它们不是实测数据。请先写下预测，再通过测量判断哪种解释成立。</p>'.repeat(16);
const chat = longChat ? [{ turnId: 'long-turn', user: '请解释预测与实测的区别。', pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], outcome: {
  answer, answer_view: { parts: [{ kind: 'markdown' as const, text: answer }], sources: [] }, effects: [], trace: [], memory_updates: [],
  profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] }, incomplete: false, warning: null, turns: 1, tokens_spent: 0,
} }] : [];
function requestTab(tab: WorkspaceAuxTab) { requestedTab.value = tab; requestedTabRevision.value++; }
const selectionText = ref("");
const agentInput = ref("未发送草稿");
const sendCount = ref(0);
const newChatCount = ref(0);
const mobileGlobalActionsOpen = ref(false);
const focusReading = ref(false);
const workspace = ref<{ toggleOutline: () => void; toggleFocus: () => void } | null>(null);

function onSelection(snapshot: ReaderSelectionSnapshot | null) {
  if (snapshot) selectionText.value = snapshot.text;
}
</script>

<template>
  <div class="app">
  <TopBar
    focus-available
    :focus-reading="focusReading"
    @toggle-focus="workspace?.toggleFocus()"
    chapter-title="移动阅读"
    :progress-pct="42"
    :anchor-lid="null"
    :debug-open="false"
    :left-rail-open="false"
    :build-intent-open="false"
    :build-intent-available="false"
    :workbench-available="false"
    :desktop-host="false"
    mobile-collapsible
    :mobile-open="mobileGlobalActionsOpen"
    @new-chat="newChatCount += 1"
    @toggle-left-rail="workspace?.toggleOutline()"
    @close-mobile="mobileGlobalActionsOpen = false"
  >
    <template v-if="longChat" #tutor-control><TutorControl :enabled="false" label="Tutor 已关闭" :busy="false" :unavailable="false" error="" /></template>
  </TopBar>
  <ReaderWorkspace
    ref="workspace"
    :logical="logical"
    @tab-request="requestTab"
    @focus-change="focusReading = $event"
    :global-actions-open="mobileGlobalActionsOpen"
    @global-actions-request="mobileGlobalActionsOpen = !mobileGlobalActionsOpen"
  >
    <aside id="reader-outline" class="left-rail">目录中的超长标题仍由抽屉承载</aside>
    <div class="resize-handle resize-handle-left"></div>
    <ReaderPane
      :segments="segments"
      :viewport-anchor="null"
      :selected-lid="null"
      :render-seg="(segment) => segment.text"
      :render-markdown="(source) => source"
      :markdown-heading-level="() => null"
      :is-asset="(segment) => segment.kind === 'code'"
      :is-highlighted="() => false"
      :highlights-of="() => []"
      :highlight-cards-of="() => []"
      :visible-notes="[]"
      :hl-excerpt="() => ''"
      :image-meta="() => null"
      :image-asset="() => null"
      @prose-selection-change="onSelection"
    />
    <div class="resize-handle resize-handle-right"></div>
    <RightRail
      v-model:agent-input="agentInput"
      :chat="chat"
      :requested-tab="requestedTab"
      :requested-tab-revision="requestedTabRevision"
      :fullscreen="fullscreen"
      @toggle-fullscreen="fullscreen = !fullscreen"
      :chat-sessions="[]"
      active-chat-session-id="fixture-chat"
      :sending="false"
      :show-trace="{}"
      :latest-trace="[]"
      selected-lid="1.1"
      :selected-formula="null"
      :context-notes="[]"
      :context-highlights="[]"
      :render-markdown="(source) => source"
      :eff-label="() => ''"
      :eff-state="() => undefined"
      :is-goto="() => false"
      :show-effect-primary="() => false"
      :show-effect-secondary="() => false"
      :effect-primary-label="() => ''"
      :effect-secondary-label="() => ''"
      :goto-back="() => ''"
      :ask-draft="null"
      @send-agent="sendCount += 1"
    />
    <button v-if="selectionText" class="fixture-selection-action" type="button">
      冻结：{{ selectionText }}
    </button>
    <output class="fixture-send-count" :data-count="sendCount">{{ sendCount }}</output>
    <output class="fixture-new-chat-count" :data-count="newChatCount">{{ newChatCount }}</output>
  </ReaderWorkspace>
  </div>
</template>

<style scoped>
.fixture-selection-action {
  position: fixed;
  z-index: 100;
  left: 50%;
  bottom: calc(4rem + env(safe-area-inset-bottom));
  transform: translateX(-50%);
}
.fixture-send-count {
  position: fixed;
  inset: auto auto 0 0;
  opacity: 0;
}
.fixture-new-chat-count {
  position: fixed;
  inset: auto 0 0 auto;
  opacity: 0;
}
</style>

