<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  resolveWorkspace,
  type DisplayPreference,
  type WorkspaceLogicalState,
  type WorkspaceProjection,
  type WorkspaceViewport,
} from "../workspace-layout";
import { useViewportEnvironment } from "../useViewportEnvironment";
import type { ReaderSurface } from "../reader-surface";

export type WorkspaceAuxTab = "agent" | "artifacts" | "profile" | "trace" | "formula" | "notes";

const props = withDefaults(defineProps<{
  logical: WorkspaceLogicalState;
  contextKey?: string;
  preference?: DisplayPreference;
  compareIntent?: boolean;
  selectionActive?: boolean;
  expanded?: boolean;
  leftCollapsed?: boolean;
  returnAvailable?: boolean;
  returnLabel?: string;
  enabled?: boolean;
  environment?: WorkspaceViewport | null;
  readerSurface?: ReaderSurface;
  pdfSurfaceAvailable?: boolean;
  globalActionsOpen?: boolean;
}>(), {
  preference: "auto",
  compareIntent: false,
  selectionActive: false,
  expanded: false,
  leftCollapsed: false,
  returnAvailable: false,
  returnLabel: "返回回答",
  enabled: true,
  environment: null,
  readerSurface: "markdown",
  pdfSurfaceAvailable: false,
  globalActionsOpen: false,
});

const emit = defineEmits<{
  (event: "tab-request", tab: WorkspaceAuxTab): void;
  (event: "projection-change", projection: WorkspaceProjection): void;
  (event: "return"): void;
  (event: "reader-surface-request", surface: ReaderSurface): void;
  (event: "global-actions-request"): void;
  (event: "before-display-change"): void;
  (event: "focus-change", active: boolean): void;
}>();

const root = ref<HTMLElement | null>(null);
const viewport = useViewportEnvironment(root);
const foreground = ref<"reader" | "assistant">("reader");
const preference = ref<DisplayPreference>(props.preference);
const outlineOpen = ref(false);
const moreMenu = ref<HTMLDetailsElement | null>(null);
function closeMoreMenu(restoreFocus = false) {
  if (!moreMenu.value?.open) return;
  moreMenu.value.open = false;
  if (restoreFocus) moreMenu.value.querySelector('summary')?.focus();
}
function dismissMoreMenu(event: PointerEvent) {
  if (event.target instanceof Node && !moreMenu.value?.contains(event.target)) closeMoreMenu();
}
onMounted(() => document.addEventListener('pointerdown', dismissMoreMenu));
onBeforeUnmount(() => document.removeEventListener('pointerdown', dismissMoreMenu));
const lastHandledFocusKey = ref<string | null>(null);
const focusReturn = ref<{ preference: DisplayPreference; foreground: "reader" | "assistant"; outlineOpen: boolean } | null>(null);

watch(() => props.preference, (next) => {
  if (focusReturn.value) focusReturn.value.preference = next;
  else preference.value = next;
});
watch(() => props.contextKey ?? props.logical.contextKey, () => {
  preference.value = focusReturn.value?.preference ?? preference.value;
  focusReturn.value = null;
  foreground.value = "reader";
  outlineOpen.value = false;
  emit("focus-change", false);
});

const projectionEnvironment = computed<WorkspaceViewport>(() => {
  const current = props.environment ?? viewport.environment.value;
  if (props.enabled) return current;
  return {
    ...current,
    containerWidth: Math.max(current.containerWidth, 1024),
    containerHeight: Math.max(current.containerHeight, 420),
    visualWidth: Math.max(current.visualWidth, 1024),
    visualHeight: Math.max(current.visualHeight, 420),
  };
});

const projection = computed(() => resolveWorkspace(
  props.logical,
  projectionEnvironment.value,
  {
    foreground: foreground.value,
    compareIntent: props.compareIntent || preference.value === "compare",
    inputFocused: viewport.inputFocused.value,
    composing: viewport.composing.value,
    selectionActive: props.selectionActive,
    expanded: props.expanded,
    lastHandledFocusKey: lastHandledFocusKey.value,
  },
  preference.value,
));

watch(projection, (next) => {
  foreground.value = next.foreground;
  if (next.appliedFocusKey) lastHandledFocusKey.value = next.appliedFocusKey;
  emit("projection-change", next);
}, { immediate: true });

function showReader() {
  closeMoreMenu();
  foreground.value = "reader";
  outlineOpen.value = false;
}

function showAssistant(tab: WorkspaceAuxTab = "agent") {
  closeMoreMenu();
  foreground.value = "assistant";
  outlineOpen.value = false;
  emit("tab-request", tab);
}

async function toggleOutline() {
  outlineOpen.value = !outlineOpen.value;
  if (outlineOpen.value) {
    await nextTick();
    root.value?.querySelector<HTMLElement>('.left-rail input, .left-rail button')?.focus({ preventScroll: true });
  }
}

async function closeOutline() {
  if (!outlineOpen.value) return;
  outlineOpen.value = false;
  await nextTick();
  root.value?.querySelector<HTMLButtonElement>('[aria-controls="reader-outline"]')?.focus({ preventScroll: true });
}

function toggleCompare() {
  emit("before-display-change");
  if (focusReturn.value) {
    preference.value = focusReturn.value.preference;
    focusReturn.value = null;
    emit("focus-change", false);
  }
  preference.value = preference.value === "compare" ? "auto" : "compare";
  outlineOpen.value = false;
}

function toggleFocus() {
  emit("before-display-change");
  if (focusReturn.value) {
    const previous = focusReturn.value;
    preference.value = previous.preference;
    foreground.value = previous.foreground;
    outlineOpen.value = previous.outlineOpen;
    focusReturn.value = null;
  } else {
    focusReturn.value = { preference: preference.value, foreground: foreground.value, outlineOpen: outlineOpen.value };
    preference.value = "focus";
    showReader();
    // A user choice supersedes a pending presentation focus, without a layout command.
    if (props.logical.focusedSlot) lastHandledFocusKey.value = `${props.logical.contextKey}:${props.logical.revision}:${props.logical.focusedSlot}`;
  }
  emit("focus-change", !!focusReturn.value);
}

defineExpose({ showReader, showAssistant, toggleOutline, toggleFocus });
</script>

<template>
  <section
    ref="root"
    class="workspace-shell"
    :class="{ 'outline-open': outlineOpen }"
    :data-mode="projection.mode"
    :data-focus-reading="!!focusReturn"
    :data-foreground="projection.foreground"
    :data-navigation="projection.navigation"
    :data-input-priority="projection.inputPriority ? 'true' : 'false'"
    :data-mobile-workspace="props.enabled ? 'true' : 'false'"
    @keydown.esc="closeOutline"
  >
    <header v-if="projection.navigation !== 'desktop'" class="workspace-mobile-top">
      <button type="button" aria-controls="reader-outline" :aria-expanded="outlineOpen" @click="toggleOutline">目录</button>
      <span v-if="projection.deferredFocus" role="status">当前编辑结束后显示请求区域</span>
      <span v-else-if="projection.unavailableFocus" role="status">请求区域尚未挂载</span>
      <button v-if="props.returnAvailable" type="button" class="workspace-return" @click="emit('return')">{{ props.returnLabel }}</button>
      <button type="button" class="workspace-focus" :aria-pressed="!!focusReturn" @click="toggleFocus">{{ focusReturn ? '退出专注' : '专注阅读' }}</button>
      <button type="button" :aria-pressed="preference === 'compare'" @click="toggleCompare">对照</button>
    </header>

    <button
      v-if="props.returnAvailable && projection.navigation === 'desktop'"
      type="button"
      class="workspace-return workspace-return-desktop"
      @click="emit('return')"
    >{{ props.returnLabel }}</button>

    <div
      v-if="props.pdfSurfaceAvailable"
      class="workspace-reader-surface-switch"
      role="group"
      aria-label="阅读表面"
    >
      <button
        type="button"
        :aria-pressed="props.readerSurface === 'markdown'"
        @click="emit('reader-surface-request', 'markdown')"
      >Markdown</button>
      <button
        type="button"
        :aria-pressed="props.readerSurface === 'pdf'"
        @click="emit('reader-surface-request', 'pdf')"
      >PDF</button>
      <span class="workspace-surface-hint">{{ props.readerSurface === 'pdf' ? '原版排版 · 字体设置用于重排正文' : '重排正文 · 可在阅读设置中调整字体' }}</span>
    </div>

    <div class="workspace-grid" :class="{ 'left-collapsed': props.leftCollapsed }">
      <slot />
    </div>

    <button
      v-if="outlineOpen && projection.navigation !== 'desktop'"
      type="button"
      class="workspace-outline-backdrop"
      aria-label="关闭目录"
      @click="closeOutline"
    ></button>

    <nav v-if="projection.navigation !== 'desktop'" class="workspace-mobile-nav" aria-label="阅读工作区">
      <button type="button" :class="{ active: projection.foreground === 'reader' }" @click="showReader">阅读</button>
      <button type="button" :class="{ active: projection.foreground === 'assistant' }" @click="showAssistant('agent')">问答</button>
      <button type="button" @click="showAssistant('notes')">笔记</button>
      <details ref="moreMenu" class="workspace-more" @keydown.esc.stop.prevent="closeMoreMenu(true)">
        <summary>更多</summary>
        <div class="workspace-more-items" aria-label="更多阅读功能">
          <button type="button" @click="showAssistant('artifacts')">成果</button>
          <button type="button" @click="showAssistant('profile')">画像</button>
          <button type="button" @click="showAssistant('trace')">轨迹</button>
          <button type="button" @click="showAssistant('formula')">公式</button>
          <button type="button" :aria-pressed="preference === 'compare'" @click="toggleCompare(); closeMoreMenu()">对照</button>
        </div>
      </details>
      <button
        type="button"
        :class="{ active: props.globalActionsOpen }"
        aria-haspopup="menu"
        :aria-expanded="props.globalActionsOpen"
        @click="emit('global-actions-request')"
      >菜单</button>
      <button v-if="projection.navigation === 'compact'" type="button" :aria-pressed="preference === 'compare'" @click="toggleCompare">对照</button>
      <button v-if="projection.navigation === 'compact'" type="button" class="workspace-focus" :aria-pressed="!!focusReturn" @click="toggleFocus">{{ focusReturn ? '退出专注' : '专注阅读' }}</button>
      <button v-if="projection.navigation === 'compact' && props.returnAvailable" type="button" class="workspace-return" @click="emit('return')">{{ props.returnLabel }}</button>
    </nav>
  </section>
</template>
