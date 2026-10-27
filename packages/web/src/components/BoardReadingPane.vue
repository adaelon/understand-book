<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { BookOpen, CornerDownRight, Link2, MessageSquareText, Shapes, Trash2, UserRound } from '@lucide/vue';
import { t as uiText } from '../i18n';
import { renderMarkdown } from '../md';
import { boardDate, boardKindLabels } from '../board-workspace';
import type { BoardItem } from '../generated/BoardItem';
import type { BoardDelivery } from '../generated/BoardDelivery';
import type { AgentAnswerPart } from '../generated/AgentAnswerPart';
import type { BoardVisualTarget } from '../generated/BoardVisualTarget';
import AgentPresentation from './AgentPresentation.vue';

const props = defineProps<{ boardId: string; item: BoardItem | null; items?: BoardItem[]; deliveries?: BoardDelivery[]; busy?: boolean; highlightedBlockId?: string | null; mappedBlockIds?: string[]; mappedItem?: boolean }>();
const emit = defineEmits<{
  (event: 'source', sourceId: string, anchor: HTMLElement): void;
  (event: 'remove', itemId: string): void;
  (event: 'select-item', itemId: string): void;
  (event: 'activate-block', target: { item_id: string; block_id: string | null }): void;
  (event: 'activate-target', target: BoardVisualTarget): void;
  (event: 'user-control'): void;
}>();
const presentation = ref<InstanceType<typeof AgentPresentation>>();
const root = ref<HTMLElement>();
const located = ref(false);
let locateTimer: ReturnType<typeof setTimeout> | undefined;
const parent = computed(() => props.items?.find(item => item.item_id === props.item?.reply_to_item_id));
const isSavedPart = computed(() => props.item?.target.kind === 'answer' && props.deliveries?.some(delivery =>
  !!delivery.commit_id && delivery.board_id === props.boardId && delivery.item_ids.includes(props.item!.item_id)));
const icons = { prebuild: BookOpen, artifact: BookOpen, answer: MessageSquareText, presentation: Shapes, understanding: UserRound, drawing: Shapes };
function readableParts(parts: unknown[]) {
  return parts.filter((part): part is AgentAnswerPart => !!part && typeof part === 'object' && 'kind' in part
    && ((part.kind === 'markdown' && 'text' in part && typeof part.text === 'string')
      || (part.kind === 'sources' && 'source_ref_ids' in part && Array.isArray(part.source_ref_ids))
      || (part.kind === 'presentation' && 'presentation_id' in part && typeof part.presentation_id === 'string' && 'revision' in part && typeof part.revision === 'number')));
}
const sections = computed(() => {
  const item = props.item;
  if (!item) return [];
  return item.explanation_blocks?.length ? item.explanation_blocks.map(block => ({ id: block.block_id, markdown: block.markdown, parts: readableParts(block.answer_parts) }))
    : [{ id: item.block_id, markdown: item.readable_content, parts: item.target.kind === 'answer' ? readableParts(item.target.answer_parts) : [] }];
});
function relatedPresentation(part: Extract<AgentAnswerPart, { kind: 'presentation' }>) {
  const answer = props.item?.target;
  return answer?.kind === 'answer' ? props.items?.find(item => item.target.kind === 'presentation'
    && item.target.receipt.session_id === answer.session_id && item.target.receipt.turn_id === answer.turn_id
    && item.target.receipt.reference.presentation_id === part.presentation_id && item.target.receipt.reference.revision === part.revision) : undefined;
}
function openPresentation(part: Extract<AgentAnswerPart, { kind: 'presentation' }>) { const item = relatedPresentation(part); if (item) emit('select-item', item.item_id); }
function sourceLabel(id: string) { return props.item?.source_bindings.find(source => source.source_ref_id === id)?.label_snapshot ?? uiText('查看原文'); }
function openSource(id: string, event: Event) { emit('source', id, event.currentTarget as HTMLElement); }
async function flush() { return await presentation.value?.flushBoardState(); }
async function locate(blockId?: string | null, query?: string, options: { focus?: boolean; scroll?: boolean } = {}) {
  await nextTick();
  if (!root.value) return false;
  const block = blockId ? [...root.value.querySelectorAll<HTMLElement>('[data-board-block-id]')].find(node => node.dataset.boardBlockId === blockId)
    : root.value.querySelector<HTMLElement>('.board-reading-content');
  if (!block) return false;
  const heading = root.value.querySelector<HTMLElement>('.board-reading-heading');
  let target: HTMLElement | null = blockId ? block : heading;
  const term = query?.trim().toLocaleLowerCase();
  if (term && block) target = [...block.querySelectorAll<HTMLElement>('p,li,blockquote,pre,td,h2,h3')].find(node => node.textContent?.toLocaleLowerCase().includes(term)) ?? target;
  if (options.focus !== false) heading?.focus({ preventScroll: true });
  if (target && options.scroll !== false) root.value.scrollTop += target.getBoundingClientRect().top - root.value.getBoundingClientRect().top - 20;
  located.value = true; clearTimeout(locateTimer); locateTimer = setTimeout(() => { located.value = false; }, 2400);
  return true;
}
function activateBlock(blockId: string | null) { if (props.item) emit('activate-block', { item_id: props.item.item_id, block_id: blockId }); }
function activateRegion(anchorId: string) {
  if (props.item?.target.kind === 'presentation') emit('activate-target', { kind: 'presentation_anchor', item_id: props.item.item_id, reference: props.item.target.receipt.reference, anchor_id: anchorId });
}
function locateAnchor(target: Extract<BoardVisualTarget, { kind: 'presentation_anchor' }>) {
  const item = props.item;
  if (item?.target.kind !== 'presentation' || target.item_id !== item.item_id
    || target.reference.presentation_id !== item.target.receipt.reference.presentation_id || target.reference.revision !== item.target.receipt.reference.revision) return false;
  return presentation.value?.locateRegion(target.anchor_id) ?? false;
}
function readingKey(event: KeyboardEvent) { if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) emit('user-control'); }
onMounted(() => { if (props.highlightedBlockId) void locate(props.highlightedBlockId, undefined, { focus: false }); });
onBeforeUnmount(() => clearTimeout(locateTimer));
defineExpose({ flush, captureFollowUp: flush, locate, locateAnchor });
</script>

<template>
  <aside ref="root" class="board-reading-pane" :class="{ 'board-located': located }" :aria-label="uiText('画板内容全文')" @wheel.passive="emit('user-control')" @pointerdown="emit('user-control')" @keydown="readingKey">
    <template v-if="item">
      <header class="board-reading-heading" tabindex="-1">
        <div class="board-reading-eyebrow" :class="item.target.kind"><component :is="icons[item.target.kind]" :size="15" aria-hidden="true" />{{ uiText(item.target.kind === 'drawing' ? '板书图解' : boardKindLabels[item.target.kind]) }}<span>{{ boardDate(item.created_at) }}</span></div>
        <h2 :title="item.title">{{ item.title }}</h2>
        <p v-if="item.target.kind === 'prebuild' || item.target.kind === 'artifact'">{{ uiText('来自学习材料 · 保留加入时的内容') }}</p>
        <p v-else-if="item.target.kind === 'answer'">{{ isSavedPart ? uiText('分段讲解 · 已保存在画板中') : uiText('完整回答 · 已独立保存在画板中') }}</p>
        <p v-else-if="item.target.kind === 'presentation'">{{ uiText('互动演示 · 保留内容与操作现场') }}</p>
        <p v-else-if="item.target.kind === 'understanding'">{{ uiText('自己的表达与 Agent 的理解解释，分别保留') }}</p>
        <p v-else>{{ uiText('图解与解释相互对应，完整内容在这里阅读') }}</p>
        <button v-if="parent" class="board-reply-parent" type="button" @click="emit('select-item', parent.item_id)"><CornerDownRight :size="14" aria-hidden="true" /><span>{{ uiText('接着解释：{0}', [parent.title]) }}</span></button>
      </header>
      <div class="board-reading-content" :data-board-block-id="item.block_id">
        <button v-if="mappedItem" type="button" class="board-block-locate board-item-locate" @click="activateBlock(null)"><Shapes :size="13" aria-hidden="true" />{{ uiText('在画板中定位') }}</button>
        <template v-if="item.target.kind === 'understanding'">
          <section class="board-understanding-quote"><h3><UserRound :size="15" aria-hidden="true" />{{ uiText('我的原话') }}</h3><blockquote>{{ item.target.learner_quote || uiText('这条理解记录没有保留原话。') }}</blockquote></section>
          <section class="board-understanding-interpretation"><h3><MessageSquareText :size="15" aria-hidden="true" />{{ uiText('Agent 对这段理解的解释') }}</h3><div class="board-prose" v-html="renderMarkdown(item.target.interpretation || item.readable_content)"></div></section>
        </template>
        <template v-else-if="item.target.kind !== 'presentation'">
          <div v-if="item.target.kind === 'answer' && item.target.question" class="board-answer-question"><span>{{ uiText('当时的问题') }}</span><p>{{ item.target.question }}</p></div>
          <section v-for="(section, sectionIndex) in sections" :key="section.id ?? sectionIndex" class="board-explanation-block" :class="{ 'block-highlighted': section.id && highlightedBlockId === section.id }" :data-board-block-id="section.id">
          <button v-if="section.id && mappedBlockIds?.includes(section.id)" type="button" class="board-block-locate" @click="activateBlock(section.id)"><Shapes :size="13" aria-hidden="true" />{{ uiText('在画板中定位') }}</button>
          <div v-if="section.parts.length" class="board-prose">
            <template v-for="(part, index) in section.parts" :key="index">
              <div v-if="part.kind === 'markdown'" v-html="renderMarkdown(part.text)"></div>
              <div v-else-if="part.kind === 'sources'" class="board-inline-sources"><button v-for="id in part.source_ref_ids" :key="id" type="button" @click="openSource(id, $event)"><BookOpen :size="12" aria-hidden="true" />{{ sourceLabel(id) }}</button></div>
              <button v-else-if="part.kind === 'presentation' && relatedPresentation(part)" class="board-answer-presentation" type="button" @click="openPresentation(part)"><Shapes :size="17" aria-hidden="true" /><span><strong>{{ relatedPresentation(part)!.title }}</strong><small>{{ uiText('打开回答中的演示') }}</small></span><span aria-hidden="true">↗</span></button>
              <p v-else-if="part.kind === 'presentation'" class="board-missing-presentation">{{ uiText('这段回答中的演示未保留在当前画板。') }}</p>
            </template>
          </div>
          <div v-else class="board-prose" v-html="renderMarkdown(section.markdown)"></div>
          </section>
        </template>
        <div v-else-if="item.target.kind === 'presentation'" class="board-retained-presentation">
          <AgentPresentation :key="item.item_id" ref="presentation" :board-id="boardId" :board-item-id="item.item_id" :board-receipt="item.target.receipt"
            :session-id="item.target.receipt.session_id" :turn-id="item.target.receipt.turn_id" :reference="item.target.receipt.reference"
            @source="(id, anchor) => emit('source', id, anchor)" @region-activated="activateRegion" />
        </div>
        <section v-if="item.source_bindings.length" class="board-reading-sources">
          <h3><Link2 :size="14" aria-hidden="true" />{{ uiText('回到原文') }}</h3>
          <button v-for="source in item.source_bindings" :key="source.source_ref_id" type="button" @click="openSource(source.source_ref_id, $event)">
            <BookOpen :size="15" aria-hidden="true" /><span><strong>{{ source.label_snapshot || uiText('原文来源') }}</strong><small v-if="source.preview_snapshot">{{ source.preview_snapshot }}</small></span><span class="board-source-arrow" aria-hidden="true">↗</span>
          </button>
        </section>
        <footer class="board-item-actions"><button type="button" :disabled="busy" @click="emit('remove', item.item_id)"><Trash2 :size="13" aria-hidden="true" />{{ uiText('从画板移除') }}</button></footer>
      </div>
    </template>
    <div v-else class="board-reading-empty"><span><BookOpen :size="25" aria-hidden="true" /></span><h2>{{ uiText('思路留在画板，全文在这里读') }}</h2><p>{{ uiText('选中一张卡片，查看完整解释、演示与原文。') }}</p></div>
  </aside>
</template>

<style scoped>
.board-explanation-block{position:relative;padding:10px 0;margin:-10px 0 18px;border-radius:6px}.board-explanation-block.block-highlighted{box-shadow:inset 3px 0 var(--brand-accent);background:color-mix(in srgb,var(--brand-accent) 5%,var(--reader-card));padding-left:13px}.board-block-locate{display:flex;align-items:center;gap:5px;margin:0 0 9px;padding:4px 7px;border:1px solid var(--hairline);border-radius:5px;background:var(--surface-soft);color:var(--steel);font-size:10px;cursor:pointer}
button{min-height:0;padding:0;border-radius:0}
.board-reading-pane.board-located .board-reading-heading{background:color-mix(in srgb,var(--brand-accent) 7%,var(--reader-card));transition:background .18s ease}.board-reading-pane.board-located>.board-reading-content[data-board-block-id]:not(:has(.block-highlighted)){box-shadow:inset 3px 0 var(--brand-accent)}.board-reply-parent{display:flex;align-items:center;gap:6px;margin-top:15px;padding:8px 10px;border:1px solid var(--hairline);border-radius:7px;background:var(--surface-soft);color:var(--steel);font-size:11px;line-height:1.65;text-align:left;cursor:pointer}.board-reply-parent>svg{flex-shrink:0}
.board-prose :deep(.katex-display){max-width:100%;overflow-x:auto;overflow-y:hidden}
.board-reading-heading h2{display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden}
.board-answer-presentation{display:flex;align-items:center;gap:11px;width:100%;margin:17px 0;padding:13px;border:1px solid var(--hairline);border-radius:8px;background:var(--surface-soft);color:var(--steel);text-align:left;cursor:pointer}.board-answer-presentation>span:first-of-type{display:flex;flex:1;flex-direction:column;gap:3px}.board-answer-presentation strong{font-size:12px;font-weight:550}.board-answer-presentation small,.board-missing-presentation{font-size:11px;color:var(--stone)}
.board-reading-pane{min-width:0;min-height:0;overflow:auto;background:var(--reader-card);border-left:1px solid var(--hairline);overscroll-behavior:contain}.board-reading-heading{padding:27px 28px 20px;border-bottom:1px solid var(--hairline-soft)}.board-reading-eyebrow{display:flex;align-items:center;gap:6px;font-size:11px;font-weight:600;color:var(--steel)}.board-reading-eyebrow>span{margin-left:auto;color:var(--stone);font-size:10px;font-weight:400}.board-reading-eyebrow.prebuild{color:#769780}.board-reading-eyebrow.answer{color:#788baa}.board-reading-eyebrow.presentation{color:#b88a63}.board-reading-eyebrow.understanding{color:#a887ad}.board-reading-heading h2{margin:14px 0 9px;font-size:23px;font-weight:570;line-height:1.5;overflow-wrap:anywhere}.board-reading-heading p{font-size:11px;line-height:1.8;color:var(--stone);margin:0}.board-reading-content{padding:24px 28px 20px}.board-prose{font-size:14px;line-height:1.95;color:var(--reader-body);overflow-wrap:anywhere;user-select:text}.board-prose :deep(p:first-child){margin-top:0}.board-prose :deep(p:last-child){margin-bottom:0}.board-prose :deep(h1),.board-prose :deep(h2),.board-prose :deep(h3){font-size:1.12em;line-height:1.7;color:var(--ink);margin:1.5em 0 .7em}.board-prose :deep(pre){padding:14px;border:1px solid var(--code-border);border-radius:8px;background:var(--surface-code);overflow:auto;line-height:1.65;font-size:12px}.board-prose :deep(table){display:block;overflow:auto;border-collapse:collapse;font-size:12px}.board-prose :deep(th),.board-prose :deep(td){padding:8px;border:1px solid var(--hairline)}.board-prose :deep(img){max-width:100%}.board-prose :deep(blockquote){margin:1em 0;padding-left:14px;border-left:3px solid var(--hairline);color:var(--steel)}.board-prose :deep(ul),.board-prose :deep(ol){padding-left:1.6em}.board-answer-question{margin:0 0 23px;padding:14px 16px;border:1px solid var(--hairline-soft);border-radius:9px;background:var(--surface-soft)}.board-answer-question>span{font-size:10px;color:var(--stone)}.board-answer-question p{margin:7px 0 0;white-space:pre-wrap;font-size:13px;line-height:1.8}.board-understanding-quote h3,.board-understanding-interpretation h3,.board-reading-sources h3{display:flex;align-items:center;gap:6px;font-size:11px;font-weight:600;color:var(--steel);margin:0 0 12px}.board-understanding-quote blockquote{white-space:pre-wrap;overflow-wrap:anywhere;margin:0 0 28px;padding:17px 19px;border-left:3px solid #a887ad;background:color-mix(in srgb,#a887ad 8%,var(--reader-card));border-radius:0 8px 8px 0;font-size:15px;line-height:1.9;color:var(--ink)}.board-reading-sources{margin-top:32px;padding-top:20px;border-top:1px solid var(--hairline)}.board-reading-sources>button{display:flex;gap:10px;align-items:flex-start;width:100%;border:1px solid var(--hairline-soft);border-radius:8px;margin-top:8px;padding:12px;background:transparent;color:var(--steel);text-align:left;cursor:pointer}.board-reading-sources>button:hover{background:var(--surface-soft)}.board-reading-sources>button>svg{flex-shrink:0;margin-top:3px}.board-reading-sources>button>span:not(.board-source-arrow){display:flex;min-width:0;flex:1;flex-direction:column;gap:5px}.board-reading-sources strong{font-weight:500;font-size:12px;line-height:1.6}.board-reading-sources small{display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;font-size:11px;line-height:1.7;color:var(--stone)}.board-source-arrow{font-size:15px;color:var(--stone)}.board-inline-sources{display:flex;flex-wrap:wrap;gap:5px;margin:5px 0 16px}.board-inline-sources button{display:inline-flex;gap:5px;align-items:center;padding:3px 7px;border:1px solid var(--hairline);border-radius:5px;background:var(--surface-soft);font:inherit;font-size:10px;color:var(--steel);cursor:pointer}.board-item-actions{display:flex;justify-content:flex-end;margin-top:32px;padding-top:14px;border-top:1px solid var(--hairline-soft)}.board-item-actions button{display:flex;gap:5px;align-items:center;border:0;background:transparent;color:var(--stone);font-size:11px;cursor:pointer}.board-item-actions button:hover{color:var(--brand-error)}.board-reading-empty{display:flex;flex-direction:column;align-items:center;justify-content:center;min-height:100%;padding:35px;text-align:center;color:var(--steel)}.board-reading-empty>span{color:var(--stone);margin-bottom:12px}.board-reading-empty h2{font-size:17px;font-weight:500;line-height:1.7}.board-reading-empty p{font-size:12px;line-height:1.8;color:var(--stone)}.board-retained-presentation{margin:-12px -15px 0;min-height:320px}.board-retained-presentation :deep(.agent-presentation){margin-top:0}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:760px){.board-reading-pane{border-left:0}.board-reading-heading{padding:23px 22px 18px}.board-reading-content{padding:22px}.board-reading-heading h2{font-size:21px}.board-retained-presentation{margin:-10px -12px 0}}
</style>
