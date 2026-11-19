<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Search } from '@lucide/vue';
import type { OutlineItem } from "../App.vue";
import { activeOutline } from '../reader-outline';
import type {
  PaperMinimapBase,
  PaperMinimapLensProjection,
  PaperMinimapLocalization,
  ReaderPaperMinimapState,
} from "../api";
import PaperMinimap from "./PaperMinimap.vue";

const props = defineProps<{
  bookTitle?: string;
  titlePageLid?: string | null;
  outlineItems: OutlineItem[];
  progressPct: number;
  anchorLid: string | null;
  anchorOffset?: number;
  selectedLid: string | null;
  pendingLid?: string | null;
  leafCount: number;
  debugOpen: boolean;
  gotoInput: string;
  searchQuery: string;
  paperEnabled?: boolean;
  paperLoading?: boolean;
  paperError?: string | null;
  paperMinimapBase?: PaperMinimapBase | null;
  paperMinimapState?: ReaderPaperMinimapState | null;
  paperMinimapLens?: PaperMinimapLensProjection | null;
  paperMinimapLocalization?: PaperMinimapLocalization | null;
  paperMinimapEffectReason?: string | null;
  paperMinimapUndoAvailable?: boolean;
  paperMinimapActionBusy?: boolean;
}>();
const emit = defineEmits<{
  (e: "update:gotoInput", value: string): void;
  (e: "update:searchQuery", value: string): void;
  (e: "goto", lid: string): void;
  (e: "paper-minimap-toggle"): void;
  (e: "paper-minimap-mode", mode: ReaderPaperMinimapState["mode"]): void;
  (e: "paper-minimap-layer", layer: string, visible: boolean): void;
  (e: "paper-minimap-pin", landmarkId: string, pinned: boolean): void;
  (e: "paper-minimap-undo"): void;
}>();

const outlineList = ref<HTMLElement | null>(null);
let outlineResizeObserver: ResizeObserver | null = null;
const titlePageLabel = uiText('书名页');
const normalizedQuery = computed(() => props.searchQuery.trim().toLowerCase());
const filteredOutline = computed(() => {
  const q = normalizedQuery.value;
  if (!q) return props.outlineItems;
  return props.outlineItems.filter((item) => {
    return item.title.toLowerCase().includes(q) || item.lid.toLowerCase().includes(q)
      || (item.lid === props.titlePageLid && titlePageLabel.toLowerCase().includes(q));
  });
});
const activeOutlineLid = computed(() => activeOutline(props.outlineItems, props.anchorLid, props.anchorOffset)?.lid ?? null);
const nodeKindLabels: Record<string, string> = {
  chapter: uiText('章'),
  section: uiText('节'),
  paragraph: uiText('段落'),
  formula: uiText('公式'),
  image: uiText('图片'),
  table: uiText('表格'),
  code: uiText('代码'),
};

function outlineKindLabel(kind: string): string {
  return nodeKindLabels[kind] ?? kind;
}

async function scrollActiveOutlineIntoView() {
  await nextTick();
  const list = outlineList.value;
  const active = list?.querySelector<HTMLElement>(".outline-item.active");
  if (!list || !active || !list.clientWidth || !list.clientHeight) return;
  const itemBounds = active.getBoundingClientRect();
  const listBounds = list.getBoundingClientRect();
  // Center within the outline only; let the browser clamp at either end.
  list.scrollTop += itemBounds.top - listBounds.top - list.clientTop
    + (itemBounds.height - list.clientHeight) / 2;
}

function gotoOutline(lid: string) {
  emit("goto", lid);
}

function forwardPaperLayer(layer: string, visible: boolean) {
  emit("paper-minimap-layer", layer, visible);
}

function forwardPaperPin(landmarkId: string, pinned: boolean) {
  emit("paper-minimap-pin", landmarkId, pinned);
}

onMounted(() => {
  let wasVisible = false;
  outlineResizeObserver = new ResizeObserver(([entry]) => {
    const visible = entry.contentRect.width > 0 && entry.contentRect.height > 0;
    // Mobile drawers stay mounted while hidden; center once their layout is available.
    if (visible && !wasVisible) void scrollActiveOutlineIntoView();
    wasVisible = visible;
  });
  if (outlineList.value) outlineResizeObserver.observe(outlineList.value);
});
onBeforeUnmount(() => {
  outlineResizeObserver?.disconnect();
});
watch(activeOutlineLid, () => {
  void scrollActiveOutlineIntoView();
});
</script>

<template>
  <aside id="reader-outline" class="left-rail" :aria-label="uiText('阅读目录')">
    <header class="outline-masthead">
      <span v-if="bookTitle" class="outline-kicker">{{ uiText('目录') }}</span>
      <h2 class="outline-book-title" :title="bookTitle">{{ bookTitle || uiText('目录') }}</h2>
    </header>
    <PaperMinimap
      v-if="props.paperEnabled"
      :base="props.paperMinimapBase ?? null"
      :state="props.paperMinimapState ?? null"
      :lens="props.paperMinimapLens ?? null"
      :localization="props.paperMinimapLocalization ?? null"
      :loading="props.paperLoading"
      :error="props.paperError"
      :effect-reason="props.paperMinimapEffectReason"
      :undo-available="props.paperMinimapUndoAvailable"
      :action-busy="props.paperMinimapActionBusy"
      @toggle="emit('paper-minimap-toggle')"
      @goto="gotoOutline"
      @mode-change="emit('paper-minimap-mode', $event)"
      @layer-toggle="forwardPaperLayer"
      @pin-toggle="forwardPaperPin"
      @undo="emit('paper-minimap-undo')"
    />

    <div class="rail-section outline-search-field">
      <Search :size="15" aria-hidden="true" />
      <input
        id="outline-search"
        class="search-pill outline-search"
        :aria-label="uiText('搜索目录')"
        :value="props.searchQuery"
        :placeholder="uiText('搜索目录')"
        @input="emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
      />
    </div>

    <div class="rail-section outline-section">
      <nav ref="outlineList" class="outline-list" :aria-label="uiText('书籍目录')">
        <button
          v-for="item in filteredOutline"
          :key="item.lid"
          class="outline-item"
          :class="{ active: item.lid === activeOutlineLid }"
          :style="{ paddingLeft: `${0.45 + item.depth * 0.75}rem` }"
          :title="item.title"
          :aria-current="item.lid === activeOutlineLid ? 'location' : undefined"
          :aria-busy="item.lid === props.pendingLid || undefined"
          @click="gotoOutline(item.lid)"
        >
          <span class="outline-kind">{{ outlineKindLabel(item.kind) }}</span>
          <span class="outline-title">{{ item.lid === titlePageLid ? titlePageLabel : item.title }}<small v-if="item.lid === props.pendingLid" role="status" class="outline-pending">{{ uiText('正在跳转…') }}</small></span>
        </button>
      </nav>
      <p v-if="filteredOutline.length === 0" class="rail-muted">{{ uiText('没有匹配的目录项。') }}</p>
    </div>

    <div class="rail-section rail-position">
      <div class="rail-heading">{{ uiText('位置') }}</div>
      <div class="position-row"><span>{{ uiText('进度') }}</span><strong>{{ props.progressPct }}%</strong></div>
      <div v-if="props.debugOpen" class="position-row"><span>{{ uiText('叶子数') }}</span><strong>{{ props.leafCount }}</strong></div>
      <div class="progress-track"><span :style="{ width: props.progressPct + '%' }"></span></div>
    </div>

    <details v-if="props.debugOpen" class="debug-panel" open>
      <summary>{{ uiText('调试坐标') }}</summary>
      <p>{{ uiText('锚点:') }} <code>{{ props.anchorLid || "-" }}</code></p>
      <p>{{ uiText('选中:') }} <code>{{ props.selectedLid || "-" }}</code></p>
      <div class="debug-goto">
        <input
          :value="props.gotoInput"
          :placeholder="uiText('输入 LID')"
          @input="emit('update:gotoInput', ($event.target as HTMLInputElement).value)"
          @keyup.enter="emit('goto', props.gotoInput)"
        />
        <button @click="emit('goto', props.gotoInput)">{{ uiText('跳转') }}</button>
      </div>
    </details>
  </aside>
</template>

<style scoped>
.outline-masthead { margin: 2px 4px 14px; max-block-size: min(22dvh, 9rem); overflow-y: auto; scrollbar-width: thin; }
.outline-kicker { display: flex; align-items: center; gap: 7px; margin-bottom: 5px; color: var(--steel); font: 600 11px/1.4 var(--sans); }
.outline-kicker::before { content: ''; width: 16px; height: 2px; background: var(--brand-accent); }
.outline-book-title { margin: 0; color: var(--charcoal); font: 600 18px/1.45 var(--reader-font, Georgia, serif); overflow-wrap: anywhere; text-wrap: balance; }
.outline-search-field { position: relative; margin-bottom: 12px; }
.outline-search-field svg { position: absolute; left: 10px; top: 50%; transform: translateY(-50%); color: var(--steel); pointer-events: none; }
.outline-search { min-height: 34px; padding: 0 10px 0 32px; border-radius: 6px; font: 13px var(--sans); }
.outline-pending { display: block; color: var(--accent); font-size: .75rem; font-weight: 400; }
@media (max-width: 1023px) {
  .outline-search { min-height: 44px; }
}
</style>

