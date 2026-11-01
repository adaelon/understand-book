<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { ArrowDown, ArrowUpRight, LoaderCircle, MessageSquareText } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { answerDisplayParts } from '../answer-markdown';
import { renderMarkdown } from '../md';
import { discussionTurnKey, type BoardDiscussionTurn } from '../board-conversation';
import type { BoardItem } from '../generated/BoardItem';
import type { AgentAnswerPart } from '../generated/AgentAnswerPart';
import { boardDeliveryKey } from '../board-workspace';
import AgentActivities from './AgentActivities.vue';

const props = defineProps<{ turns: BoardDiscussionTurn[]; items: BoardItem[]; loading?: boolean; error?: string }>();
const emit = defineEmits<{
  (event: 'retry'): void;
  (event: 'locate', itemId: string): void;
  (event: 'source', itemId: string, sourceId: string, anchor: HTMLElement): void;
  (event: 'suggest', text: string): void;
  (event: 'regenerate', turn: BoardDiscussionTurn): void;
  (event: 'preview-map', turn: BoardDiscussionTurn): void;
}>();
const root = ref<HTMLElement>();
const nearBottom = ref(true);
const hasNew = ref(false);
function answerItem(turn: BoardDiscussionTurn) {
  return props.items.find(item => item.target.kind === 'answer' && item.target.session_id === turn.sessionId && item.target.turn_id === turn.turnId);
}
function presentationItem(turn: BoardDiscussionTurn, part: Extract<AgentAnswerPart, { kind: 'presentation' }>) {
  return props.items.find(item => item.target.kind === 'presentation' && item.target.receipt.session_id === turn.sessionId
    && item.target.receipt.turn_id === turn.turnId && item.target.receipt.reference.presentation_id === part.presentation_id && item.target.receipt.reference.revision === part.revision);
}
const display = computed(() => props.turns.map(turn => {
  const draft = !turn.outcome && turn.draft?.view ? turn.draft.view : null;
  const view = turn.outcome?.answer_view ?? draft;
  const parts: AgentAnswerPart[] = view?.parts?.length ? view.parts : turn.outcome?.answer ? [{ kind: 'markdown', text: turn.outcome.answer }] : [];
  return { turn, parts, isDraft: !!draft, display: answerDisplayParts(parts, view?.sources ?? [], false, !!answerItem(turn)),
    pendingSummaryHtml: turn.pendingBoardEdit?.summary ? renderMarkdown(turn.pendingBoardEdit.summary) : '' };
}));
function sourceClick(turn: BoardDiscussionTurn, parts: AgentAnswerPart[], event: MouseEvent) {
  const anchor = (event.target as HTMLElement).closest<HTMLButtonElement>('[data-answer-source-index]');
  const item = answerItem(turn);
  if (!anchor || !item) return;
  const part = parts[Number(anchor.dataset.answerSourceIndex)];
  if (part?.kind === 'sources' && part.source_ref_ids[0]) emit('source', item.item_id, part.source_ref_ids[0], anchor);
}
function onScroll() {
  if (!root.value) return;
  nearBottom.value = root.value.scrollHeight - root.value.scrollTop - root.value.clientHeight < 72;
  if (nearBottom.value) hasNew.value = false;
}
async function latest() { await nextTick(); if (root.value) root.value.scrollTop = root.value.scrollHeight; nearBottom.value = true; hasNew.value = false; }
watch(() => props.turns, async () => { if (nearBottom.value) await latest(); else hasNew.value = true; }, { deep: true });
</script>

<template>
  <section class="board-discussion" :aria-label="uiText('画板讨论')">
    <div ref="root" class="board-discussion-scroll" @scroll="onScroll">
      <div v-if="loading && !turns.length" class="board-discussion-loading" role="status"><LoaderCircle :size="16" class="board-spinner" />{{ uiText('正在读取画板讨论…') }}</div>
      <div v-if="error" class="board-discussion-error" role="alert">{{ error }}<button type="button" @click="emit('retry')">{{ uiText('重试') }}</button></div>
      <div v-if="!turns.length && !loading" class="board-discussion-empty">
        <MessageSquareText :size="25" aria-hidden="true" />
        <h3>{{ uiText('从一个问题开始') }}</h3>
        <p>{{ uiText('说出你想理解的主题，我们一起梳理概念、画出关系，把理解接着往前推。') }}</p>
        <button type="button" @click="emit('suggest', uiText('帮我梳理这个主题，选择合适的图解画出关键概念和它们的关系。'))">{{ uiText('梳理主题，画出关系') }}<ArrowUpRight :size="14" /></button>
        <button type="button" @click="emit('suggest', uiText('请结合这张画板，帮我找出还没弄清楚的地方，并一步步讲解。'))">{{ uiText('看看哪里还没弄懂') }}<ArrowUpRight :size="14" /></button>
      </div>
      <article v-for="entry in display" :key="discussionTurnKey(entry.turn)" class="board-discussion-turn" :data-turn-id="entry.turn.turnId" :data-session-id="entry.turn.sessionId">
        <div class="board-discussion-question"><span>{{ uiText('我') }}</span><p>{{ entry.turn.user }}</p></div>
        <div class="board-discussion-answer">
          <span class="board-discussion-author">{{ uiText('一起理解') }}</span>
          <AgentActivities v-if="entry.turn.activities?.length" :activities="entry.turn.activities" :active="entry.turn.pending" :status-text="entry.turn.runStatus" />
          <div v-if="entry.display.length" class="md" :class="{ 'answer-draft': entry.isDraft }" :aria-label="entry.isDraft ? uiText('回答草稿') : uiText('回答')" @click="sourceClick(entry.turn, entry.parts, $event)">
            <template v-for="(part, index) in entry.display" :key="index">
              <div v-if="part.kind === 'html'" class="answer-markdown" v-html="part.html"></div>
              <button v-else-if="presentationItem(entry.turn, part)" class="board-discussion-location" type="button" @click="emit('locate', presentationItem(entry.turn, part)!.item_id)">{{ uiText('在画板中打开演示') }}<ArrowUpRight :size="13" /></button>
              <p v-else class="board-discussion-status">{{ entry.turn.pending ? uiText('演示正在加入画板…') : uiText('这段回答中的演示未保留在当前画板。') }}</p>
            </template>
          </div>
          <p v-if="(entry.turn.pending || entry.turn.runStatus) && !(entry.turn.pending && entry.turn.activities?.length)" class="board-discussion-status" role="status"><LoaderCircle v-if="entry.turn.pending" :size="13" class="board-spinner" />{{ entry.turn.runStatus || uiText('正在思考并整理画板…') }}</p>
          <p v-if="entry.turn.error" class="board-discussion-error" role="alert">{{ entry.turn.error }}</p>
          <div v-if="entry.turn.pendingBoardEdit" class="board-pending-edit" role="status">
            <strong>{{ entry.turn.pendingBoardEdit.requires_confirmation ? '思维导图已准备好' : '这次改图需要重新整理' }}</strong>
            <p>{{ entry.turn.pendingBoardEdit.message }}</p>
            <details v-if="entry.pendingSummaryHtml" class="board-pending-details">
              <summary>查看尚未应用的建议</summary>
              <div class="md board-pending-proposal" v-html="entry.pendingSummaryHtml"></div>
            </details>
            <button v-if="entry.turn.pendingBoardEdit.requires_confirmation" type="button" @click="emit('preview-map', entry.turn)">预览重组后的导图</button><button v-else type="button" @click="emit('regenerate', entry.turn)">按当前内容重新生成</button>
            <small>{{ entry.turn.pendingBoardEdit.requires_confirmation ? '确认后会创建新导图，原内容继续保留。' : '已保留你的修改；重新生成会使用当前选中的分支。' }}</small>
          </div>
          <p v-if="entry.turn.outcome?.incomplete" class="board-discussion-status">{{ uiText('这次讲解还未完成，可以接着提问。') }}</p>
          <div v-if="entry.turn.commits?.length" class="board-discussion-parts"><button v-for="(part, index) in entry.turn.commits" :key="boardDeliveryKey(part)" type="button" class="board-discussion-location" @click="emit('locate', part.item_ids[0])">{{ uiText('查看第{0}部分图解', [index + 1]) }}<ArrowUpRight :size="13" /></button></div>
        </div>
      </article>
    </div>
    <button v-if="hasNew" class="board-discussion-latest" type="button" @click="latest"><ArrowDown :size="13" />{{ uiText('查看新回复') }}</button>
  </section>
</template>

<style scoped>
.board-spinner{animation:board-discussion-spin 1.2s linear infinite;flex-shrink:0}
@keyframes board-discussion-spin{to{transform:rotate(360deg)}}
@media(prefers-reduced-motion:reduce){.board-spinner{animation:none}}
.board-discussion{position:relative;display:flex;flex:1;min-height:0;min-width:0;background:var(--reader-card)}.board-discussion-scroll{flex:1;min-width:0;overflow:auto;padding:22px 24px 32px;overscroll-behavior:contain}.board-discussion-loading{display:flex;align-items:center;gap:9px;font-size:12px;color:var(--stone)}.board-discussion-empty{padding:22px 2px;color:var(--steel)}.board-discussion-empty>svg{color:var(--brand-accent)}.board-discussion-empty h3{font-size:19px;font-weight:550;color:var(--ink);margin:18px 0 10px}.board-discussion-empty p{font-size:13px;line-height:1.9;margin:0 0 22px}.board-discussion-empty button{display:flex;align-items:center;justify-content:space-between;width:100%;gap:10px;margin:9px 0;padding:12px 14px;border:1px solid var(--hairline);border-radius:9px;background:var(--surface-soft);font:inherit;font-size:12px;text-align:left;color:var(--steel);cursor:pointer}.board-discussion-turn{margin-bottom:28px;overflow-wrap:anywhere}.board-discussion-question{padding:12px 15px;border:1px solid var(--hairline);border-radius:12px 12px 3px 12px;background:var(--surface-soft)}.board-discussion-question>span{font-size:10px;color:var(--stone)}.board-discussion-question p{margin:5px 0 0;font-size:13px;line-height:1.85;white-space:pre-wrap}.board-discussion-answer{padding-top:17px}.board-discussion-author{font-size:10px;color:var(--brand-accent);letter-spacing:.04em}.board-discussion-answer .md{font-size:13px;line-height:1.9;margin-top:8px;overflow-wrap:anywhere}.board-discussion-answer :deep(pre),.board-discussion-answer :deep(table){max-width:100%;overflow:auto}.board-discussion-answer :deep(p:first-child){margin-top:0}.board-discussion-status{display:flex;align-items:center;gap:6px;margin:9px 0 0;color:var(--stone);font-size:11px;line-height:1.7}.board-discussion-error{margin:10px 0;color:var(--brand-error);font-size:12px;line-height:1.8}.board-discussion-error button{margin-left:10px;border:0;background:transparent;color:inherit;text-decoration:underline;cursor:pointer}.board-discussion-location{display:inline-flex;align-items:center;gap:6px;margin-top:10px;padding:7px 10px;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--brand-accent);font:inherit;font-size:11px;cursor:pointer}.board-discussion-parts{display:flex;flex-wrap:wrap;gap:6px}.board-discussion-latest{position:absolute;bottom:12px;left:50%;transform:translateX(-50%);display:flex;gap:7px;align-items:center;padding:8px 13px;border:1px solid var(--hairline);border-radius:20px;background:var(--reader-card);color:var(--brand-accent);font:inherit;font-size:11px;box-shadow:0 3px 14px #25232012;cursor:pointer}@media(max-width:700px){.board-discussion-scroll{padding:18px 20px 28px}}
</style>
<style scoped>
.board-pending-details{margin:7px 0 10px;border-top:1px solid var(--hairline)}
.board-pending-details summary{padding:8px 0;color:var(--brand-accent);font-size:11px;cursor:pointer}
.board-pending-details summary:focus-visible{outline:2px solid var(--brand-accent);outline-offset:2px;border-radius:3px}
.board-pending-edit .board-pending-proposal{margin:3px 0 12px;font-size:12px;line-height:1.8}
.board-pending-proposal :deep(h1),.board-pending-proposal :deep(h2),.board-pending-proposal :deep(h3){font-size:14px;line-height:1.6;margin:14px 0 8px}
.board-pending-proposal :deep(pre){padding:10px 12px;white-space:pre-wrap;overflow-wrap:anywhere;font-size:11px;line-height:1.8}
.board-pending-proposal :deep(pre code){white-space:inherit;overflow-wrap:inherit}
@media(max-width:700px){.board-pending-details summary{min-height:44px;align-content:center}}
</style>
<style scoped>
.board-pending-edit{margin-top:12px;padding:12px;border:1px solid var(--hairline);border-radius:9px;background:var(--surface-soft);font-size:12px;line-height:1.8;color:var(--steel)}.board-pending-edit strong{font-weight:550;color:var(--ink)}.board-pending-edit p{margin:6px 0}.board-pending-edit button{padding:7px 10px;border:1px solid var(--hairline);border-radius:6px;background:var(--reader-card);color:var(--brand-accent);font-size:11px;cursor:pointer}.board-pending-edit small{display:block;margin-top:7px;font-size:10px;color:var(--stone)}
</style>
