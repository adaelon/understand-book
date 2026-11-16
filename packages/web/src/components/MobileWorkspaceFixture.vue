<script setup lang="ts">
import { ref } from "vue";
import TopBar from "./TopBar.vue";
import ReaderPane, { type Segment } from "./ReaderPane.vue";
import ReaderWorkspace from "./ReaderWorkspace.vue";
import RightRail from "./RightRail.vue";
import type { WorkspaceAuxTab } from "./ReaderWorkspace.vue";
import type { ReaderSelectionSnapshot } from "../useReaderSelection";
import type { ResidentGoal } from "../api";
import ConversationLearningGoal from './ConversationLearningGoal.vue';
import type { TutorSession } from '../generated/TutorSession';
import { renderMarkdown } from '../md';
import mathAnswer from '../fixtures/agent-math-answer.md?raw';
import { agentFailureMessage } from '../agent-run-state';

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
const fixtureParams = new URLSearchParams(location.search);
const learning = fixtureParams.has('learning');
const learningSession = ref({ id: 'learning', user_intent: '学懂 1.4 需求如何推动架构设计', status: 'active' } as TutorSession);
const savedAnswerSelection = ref('');
const learningAction = ref('');
const running = ref(fixtureParams.has('running-task'));
const targetGoalId = ref<string | null>(null);
const chatGoals = ref<ResidentGoal[]>(fixtureParams.has('task') ? [{
  id: 'fixture-goal', revision: 1, interpretation: '全书的主线是怎样的呢，给我个演示页我看看',
  requirements: [{ id: 'page', description: '交付全书主线演示', basis_turn_id: 'long-turn', verification: 'presentation_delivery' }],
  working: { focus: '整理全书主线', open_questions: [], next_move: '完成演示', items: [{ id: 'page', description: '制作演示', status: 'in_progress' }] },
  result_refs: [], status: 'open', last_stop_reason: null,
}] : []);
const requestedTab = ref<WorkspaceAuxTab>('agent');
const requestedTabRevision = ref(0);
const fullscreen = ref(false);
const selectionFixture = fixtureParams.has('answer-selection');
const mathFixture = fixtureParams.has('math-answer');
const answer = mathFixture ? mathAnswer : selectionFixture
  ? '- 给了第二个名字 → 终点取**那个名字**对应节点的末尾；\n- 没给 → 终点取**第一个名字自己**的末尾，也就是只读这一个位置的范围。\n\n' + '后续解释用于保留对话的滚动位置。\n\n'.repeat(20)
  : '<p>两条曲线表示两种解释各自的预测，页面明确标注它们不是实测数据。请先写下预测，再通过测量判断哪种解释成立。</p>'.repeat(16);
const failedAnswer = fixtureParams.get('failed-answer');
const chat = failedAnswer ? [{ turnId: 'failed-turn', user: '我知道向量范数了，那矩阵范数是什么，又怎么理解呢', pending: false,
  questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], outcome: null,
  error: agentFailureMessage({ error_code: failedAnswer, category: 'validation',
    message: '$.move must satisfy anyOf (0 matching variants); $.move.delivery_text is required; $.move.board_candidate_revision is required' }),
}] : longChat || mathFixture ? [{ turnId: 'long-turn', user: mathFixture ? '请解释线性注意力的状态更新与读出。' : '请解释预测与实测的区别。', pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [], outcome: {
  answer, answer_view: { parts: selectionFixture ? [
    { kind: 'markdown' as const, text: answer },
    { kind: 'sources' as const, source_ref_ids: ['fixture-source'] },
    { kind: 'markdown' as const, text: '\n\n另一个独立回答片段。' },
  ] : [{ kind: 'markdown' as const, text: answer }], sources: [] }, effects: [],
  trace: [{ tool: 'book.query', args: 'fixture-tool-arguments', result_digest: 'fixture-tool-result', model_tool_loop: 2 }], memory_updates: [],
  profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] }, incomplete: false, warning: null, turns: 1, tokens_spent: 0,
} }] : [];
function requestTab(tab: WorkspaceAuxTab) { requestedTab.value = tab; requestedTabRevision.value++; }
const selectionText = ref("");
const agentInput = ref("未发送草稿");
const sendCount = ref(0);
const newChatCount = ref(0);
const mobileGlobalActionsOpen = ref(false);
const focusReading = ref(false);
const workspace = ref<{ toggleOutline: () => void; toggleFocus: () => void; showReader: () => void } | null>(null);

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
      :learning-title="learning ? learningSession.user_intent : undefined"
      @return-reading="fullscreen = false; workspace?.showReader()"
      @workspace-menu="mobileGlobalActionsOpen = true"
      @toggle-fullscreen="fullscreen = !fullscreen"
      :chat-sessions="[]"
      active-chat-session-id="fixture-chat"
      :sending="running"
      :can-stop="running"
      :chat-goals="chatGoals"
      :target-goal-id="targetGoalId"
      @stop-agent="running = false; chatGoals[0].last_stop_reason = 'AGENT_RUN_CANCELLED'"
      @continue-goal="running = true"
      @cancel-goal="chatGoals[0].status = 'cancelled'"
      @target-goal="targetGoalId = $event"
      @clear-goal-target="targetGoalId = null"
      selected-lid="1.1"
      :selected-formula="null"
      :context-notes="[]"
      :context-highlights="[]"
      :render-markdown="selectionFixture || mathFixture ? renderMarkdown : (source) => source"
      :eff-label="() => ''"
      :eff-state="() => undefined"
      :is-goto="() => false"
      :show-effect-primary="() => false"
      :show-effect-secondary="() => false"
      :effect-primary-label="() => ''"
      :effect-secondary-label="() => ''"
      :goto-back="() => ''"
      :ask-draft="null"
      @send-agent="sendCount += 1; agentInput = ''"
      @save-answer-selection="(_turn: unknown, text: string) => savedAnswerSelection = text"
    >
      <template v-if="learning" #learning-actions><button @click="learningAction = 'understanding'">我的理解</button></template>
      <template v-if="learning" #learning-details><button @click="learningAction = 'details'">学习详情</button></template>
      <template v-if="learning" #learning-goal>
        <ConversationLearningGoal :session="learningSession" :busy="false" :available="true" :ready="true" :other-session="false" error="" @goal="learningSession = { ...learningSession, user_intent: $event }" @manage="learningAction = 'details'" />
      </template>
    </RightRail>
    <output hidden class="fixture-answer-note">{{ savedAnswerSelection }}</output>
    <output hidden class="fixture-learning-action">{{ learningAction }}</output>
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

