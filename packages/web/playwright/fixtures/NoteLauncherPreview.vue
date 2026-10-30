<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue';
import TopBar from '../../src/components/TopBar.vue';
import LeftRail from '../../src/components/LeftRail.vue';
import ReaderPane, { type Segment } from '../../src/components/ReaderPane.vue';
import ReaderWorkspace, { type WorkspaceAuxTab } from '../../src/components/ReaderWorkspace.vue';
import RightRail from '../../src/components/RightRail.vue';
import ConversationLearningGoal from '../../src/components/ConversationLearningGoal.vue';
import TutorControl from '../../src/components/TutorControl.vue';
import { registerThoughtContext } from '../../src/personal-notes';
import { renderMarkdown, renderInlineMarkdown } from '../../src/md';
import type { OuterOutcome } from '../../src/api';
import type { TutorSession } from '../../src/generated/TutorSession';

const title = '为什么钢铁做成的军舰不会沉入海底？';
const paragraphs = [
  `# ${title}`,
  '一块实心钢铁放进水里会下沉，可是用钢铁造出的军舰，比一块钢铁重得多，却能浮在海面上。这是为什么呢？要弄清这个问题，需要同时考虑物体的重量，以及它能排开多少水。',
  '物体进入水中，会占据一部分原本属于水的空间，把那里的水挤开。水也会对物体产生一个向上的托力，这就是浮力。阿基米德原理告诉我们：**物体受到的浮力，等于它排开的水所受到的重力。** 简单地说，在同样的水中，排开的水越多，浮力就越大。',
  '我们可以用一个小实验认识这个关系。把小石块用细线挂在弹簧测力计上，先记下它在空气中静止时的示数。再把石块完全浸入水中，保持它悬在水里，不接触容器的底部和侧壁，等它静止后重新读数。测力计的示数会减小，因为水的浮力帮助托住了石块，测力计需要提供的拉力变小了。石块本身的重量并没有因此改变。',
  '船身虽然由钢铁制成，但里面留有很大的空间。船能够排开足够多的水，水给它的浮力就能与整艘船的重量相等。装上货物以后，船会下沉一些，排开更多水，直到再次达到平衡。',
];
const segments: Segment[] = paragraphs.map((text, index) => ({ lid: `1.${index}`, text, kind: index ? 'paragraph' : 'chapter', formula: null, imageAsset: null }));
const workspace = ref<InstanceType<typeof ReaderWorkspace>>();
const logical = { contextKey: 'note-launcher-preview', revision: 1, activePreset: 'technical_read', openSlots: ['technical.structure_map', 'technical.agent'], focusedSlot: null };
const agentInput = ref('我又加了些东西，船往下沉了一点，可是还浮着。它都变重了，为什么没有一直沉到底呀？');
const fullscreen = ref(false), leftCollapsed = ref(false), teaching = ref(true), search = ref('');
const requestedTab = ref<WorkspaceAuxTab>('agent'), requestedTabRevision = ref(0);
const goal = ref({ id: 'preview-learning', user_intent: '铁放进水里会沉下去，大船也是铁做的，为什么不会沉呀？', status: 'active' } as TutorSession);
function turn(user: string, answer: string) {
  const outcome: OuterOutcome = { answer, answer_view: { parts: [{ kind: 'markdown', text: answer }], sources: [] }, effects: [], trace: [], memory_updates: [], profile_usage: { snapshot_revision: 0, injected_fact_ids: [], claimed_used_fact_ids: [], influences: [] }, incomplete: false, warning: null, turns: 1, tokens_spent: 0 };
  return { turnId: crypto.randomUUID(), user, outcome, pending: false, questionAnchorLid: null, questionQuote: null, questionSelection: null, effectLabels: [] };
}
const chat = ref([turn('船这么重，水怎么托得住它？', '可以先想象一个空碗和一颗铁球。\n\n铁球小而密，排开的水不够多；空碗里面留出了空间，轻轻放在水面上，就能排开更多的水。\n\n**关键是：排开的水有多重，水就能给它多大的浮力。**\n\n船也是这样。货物增加时，船先下沉一点，排开的水就更多了，浮力也随之增大。等浮力与总重量相等，船就会停在新的水位。\n\n你可以先记下自己的猜想，再回到正文看看这个关系。')]);
function send() { if (!agentInput.value.trim()) return; chat.value.push(turn(agentInput.value, '这里可以把你的想法先记下来：船下沉一点，会排开更多的水。\n\n当增加的浮力补上增加的重量，船就会在新的位置重新平衡。')); agentInput.value = ''; }
function requestTab(tab: WorkspaceAuxTab) { requestedTab.value = tab; requestedTabRevision.value++; }
const stopContext = registerThoughtContext(() => ({ book_id: 'steel-ship', publication_id: 'preview', lid: '1.2', label: title }));
onBeforeUnmount(stopContext);
onMounted(() => { if (innerWidth < 1024) workspace.value?.showAssistant('agent'); });
</script>
<template>
  <div class="app" data-note-launcher-preview>
    <TopBar :chapter-title="title" :progress-pct="18" anchor-lid="1.2" :debug-open="false" :left-rail-open="!leftCollapsed" :build-intent-open="false" :build-intent-available="false" :workbench-available="false" :desktop-host="false" :mobile-collapsible="true" :mobile-open="false" :typography-available="false" :board-available="false" @toggle-left-rail="leftCollapsed = !leftCollapsed" @new-chat="chat = []; agentInput = ''">
      <template #account><span class="preview-tag">随手记 · 本地预览</span></template>
    </TopBar>
    <ReaderWorkspace ref="workspace" :logical="logical" :left-collapsed="leftCollapsed" @tab-request="requestTab">
      <LeftRail :outline-items="[{ lid: '1.0', title, depth: 0, kind: 'chapter' }]" :progress-pct="18" anchor-lid="1.0" :selected-lid="null" :leaf-count="5" :debug-open="false" goto-input="" v-model:search-query="search" />
      <div class="resize-handle resize-handle-left" />
      <ReaderPane :segments="segments" viewport-anchor="1.0" :selected-lid="null" :render-seg="seg => renderInlineMarkdown(seg.text.replace(/^# /, ''))" :render-markdown="renderMarkdown" :markdown-heading-level="seg => seg.kind === 'chapter' ? 1 : null" :is-asset="() => false" :is-highlighted="() => false" :highlights-of="() => []" :highlight-cards-of="() => []" :visible-notes="[]" :hl-excerpt="() => ''" :image-meta="() => null" :image-asset="() => null" />
      <div class="resize-handle resize-handle-right" />
      <RightRail v-model:agent-input="agentInput" :chat="chat" :chat-sessions="[]" active-chat-session-id="note-preview-chat" :fullscreen="fullscreen" :sending="false" :learning-title="goal.user_intent" :tutor-enabled="teaching" :requested-tab="requestedTab" :requested-tab-revision="requestedTabRevision" :selected-lid="null" :selected-formula="null" :context-notes="[]" :context-highlights="[]" :render-markdown="renderMarkdown" :eff-label="() => ''" :eff-state="() => undefined" :is-goto="() => false" :show-effect-primary="() => false" :show-effect-secondary="() => false" :effect-primary-label="() => ''" :effect-secondary-label="() => ''" :goto-back="() => ''" :ask-draft="null" @toggle-fullscreen="fullscreen = !fullscreen" @return-reading="fullscreen = false; workspace?.showReader()" @send-agent="send">
        <template #learning-goal><ConversationLearningGoal :session="goal" :enabled="teaching" :busy="false" :available="true" :ready="true" error="" @goal="goal = { ...goal, user_intent: $event }" /></template>
        <template #tutor-control><TutorControl :enabled="teaching" label="围绕你的目标逐步学" :busy="false" :unavailable="false" error="" @toggle="teaching = !teaching" /></template>
      </RightRail>
    </ReaderWorkspace>
  </div>
</template>
<style scoped>
.preview-tag { color: #8d7c66; font-size: 12px; white-space: nowrap; padding-inline: 6px; }
</style>
