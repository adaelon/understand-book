<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue';
import { Ellipsis, Focus, FolderOpen, Globe2, LayoutDashboard, ListChecks, MessageSquarePlus, NotebookPen, PanelLeft, PanelTopClose, PanelTopOpen, Settings, SlidersHorizontal, Wrench, X } from '@lucide/vue';
import { t as uiText } from '../i18n';
import LanguageSwitcher from './LanguageSwitcher.vue';
import brandLogo from '../assets/brand/understand-book-logo.svg';

const props = defineProps<{
  videoMode?: boolean;
  musicMode?: boolean;
  typographyAvailable?: boolean;
  focusAvailable?: boolean;
  focusReading?: boolean;
  chatUnavailable?: boolean;
  boardAvailable?: boolean;
  notesAvailable?: boolean;
  materialTitle?: string;
  chapterTitle: string;
  progressPct: number;
  anchorLid: string | null;
  debugOpen: boolean;
  leftRailOpen: boolean;
  buildIntentOpen: boolean;
  buildIntentAvailable: boolean;
  workbenchAvailable: boolean;
  desktopHost: boolean;
  mobileCollapsible: boolean;
  mobileOpen: boolean;
}>();
const emit = defineEmits<{
  (e: "open-typography"): void;
  (e: "toggle-focus"): void;
  (e: "new-chat"): void;
  (e: "open-book"): void;
  (e: "toggle-left-rail"): void;
  (e: "toggle-debug"): void;
  (e: "open-build-intent"): void;
  (e: "open-workbench"): void;
  (e: "open-board"): void;
  (e: "open-notes"): void;
  (e: "open-settings"): void;
  (e: "close-mobile"): void;
}>();

const collapsed = ref(!!props.focusReading);
const expanded = ref(false);
const toolsOpen = computed(() => expanded.value || props.mobileOpen);
const toolbar = ref<HTMLElement>();
const toolsButton = ref<HTMLButtonElement>();
const expandButton = ref<HTMLButtonElement>();
const panel = ref<HTMLElement>();
const panelId = useId();
const headerBottom = ref(48);
let opener: HTMLElement | null = null;
const hasOutline = computed(() => !props.videoMode && (props.mobileCollapsible || props.focusAvailable));
const focusLabel = computed(() => props.focusReading ? uiText('退出专注') : props.musicMode ? uiText('专注聆听') : props.videoMode ? uiText('专注观看') : uiText('专注阅读'));

function closeTools(restoreFocus = false) {
  expanded.value = false;
  if (props.mobileOpen) emit('close-mobile');
  if (restoreFocus) opener?.focus({ preventScroll: true });
}
function outside(event: PointerEvent) {
  const target = event.target as Element;
  if (toolsOpen.value && !toolbar.value?.contains(target) && target !== opener && !target.closest('.topbar-mobile-backdrop')) closeTools();
}
function act(event: MouseEvent) {
  const target = event.target as Element;
  if (toolsOpen.value && target.closest('button, a')) closeTools(panel.value?.parentElement?.contains(target));
}
function collapse() {
  collapsed.value = true;
  nextTick(() => expandButton.value?.focus());
}
watch(toolsOpen, async open => {
  if (open) {
    opener = props.mobileOpen ? document.activeElement as HTMLElement : toolsButton.value ?? null;
    await nextTick();
    if (toolbar.value) headerBottom.value = toolbar.value.getBoundingClientRect().bottom;
    const surface = panel.value?.parentElement;
    const menu = surface && getComputedStyle(surface).display === 'block' ? surface : panel.value;
    menu?.querySelector<HTMLElement>('button:not(:disabled)')?.focus({ preventScroll: true });
  } else {
    const account = panel.value?.querySelector('details[open]');
    if (account) account.removeAttribute('open');
  }
});
watch(() => props.focusReading, value => { collapsed.value = !!value; closeTools(); });
onMounted(() => document.addEventListener('pointerdown', outside));
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside));
</script>

<template>
  <button v-if="collapsed && !focusReading" ref="expandButton" class="topbar-expand" :aria-label="uiText('展开工具栏')" :title="uiText('展开工具栏')" @click="collapsed = false; nextTick(() => toolsButton?.focus())">
    <PanelTopOpen :size="17" aria-hidden="true" />
  </button>
  <button v-if="mobileCollapsible && mobileOpen" type="button" class="topbar-mobile-backdrop" tabindex="-1" :aria-label="uiText('关闭页面菜单')" @click="closeTools(true)"></button>
  <header ref="toolbar" class="topbar" :style="{ '--tools-offset': headerBottom + 'px' }" :class="{ 'mobile-collapsible': mobileCollapsible && !$slots.navigation, 'mobile-open': mobileOpen, 'desktop-collapsed': collapsed, 'has-destinations': !!$slots.navigation }"
    @click="act" @keydown.esc="toolsOpen && closeTools(true)" @focusout="($event.relatedTarget && !toolbar?.contains($event.relatedTarget as Node)) && closeTools()">
    <div class="topbar-leading">
      <span class="topbar-signature"><img :src="brandLogo" width="25" height="25" alt="" /><span>Understand Book</span></span>
      <slot name="navigation" />
    </div>
    <div class="topbar-location">
      <component :is="hasOutline ? 'button' : 'div'" class="chapter-position" :type="hasOutline ? 'button' : undefined" :aria-controls="hasOutline ? 'reader-outline' : undefined"
        :aria-expanded="hasOutline ? leftRailOpen : undefined" :aria-label="hasOutline ? uiText('目录') : undefined" :aria-describedby="hasOutline ? panelId + '-position' : undefined"
        :title="hasOutline ? uiText('目录') : undefined" @click="hasOutline && emit('toggle-left-rail')">
        <span v-if="hasOutline" class="outline-label"><PanelLeft :size="16" aria-hidden="true" />{{ uiText('目录') }}</span>
        <span :id="panelId + '-position'" class="reading-titles">
          <span v-if="!hasOutline && materialTitle && materialTitle !== chapterTitle" class="material-title" :title="materialTitle">{{ materialTitle }}</span>
          <strong :title="chapterTitle">{{ chapterTitle || uiText('阅读工作区') }}</strong>
        </span>
      </component>
      <span v-if="!videoMode && mobileCollapsible" class="reading-progress" role="progressbar" :aria-label="uiText('进度')" :aria-valuenow="progressPct" aria-valuemin="0" aria-valuemax="100">
        <i aria-hidden="true"><i :style="{ width: progressPct + '%' }" /></i>{{ progressPct }}%
      </span>
    </div>
    <div class="topbar-actions">
      <div class="tools-popover tools-surface" :class="{ 'tools-open': toolsOpen }">
        <button v-if="focusAvailable" type="button" class="bar-icon topbar-focus" :aria-label="focusLabel" :title="focusLabel" :aria-pressed="!!focusReading" @click="emit('toggle-focus')"><Focus :size="17" aria-hidden="true" /><span>{{ focusLabel }}</span></button>
        <section v-show="toolsOpen" :id="panelId" ref="panel" class="tools-popover topbar-menu" :aria-label="uiText('阅读工具')">
          <div class="tool-group">
            <h2>{{ uiText('阅读工具') }}</h2>
            <button v-if="typographyAvailable" type="button" data-typography-trigger @click="emit('open-typography')"><SlidersHorizontal />{{ uiText('阅读设置') }}</button>
            <button type="button" :disabled="chatUnavailable" @click="emit('new-chat')"><MessageSquarePlus />{{ uiText('新对话') }}</button>
            <button v-if="boardAvailable" type="button" @click="emit('open-board')"><LayoutDashboard />{{ uiText('当前资料的画板') }}</button>
            <button v-if="notesAvailable" type="button" @click="emit('open-notes')"><NotebookPen />{{ uiText('空间笔记') }}</button>
            <button v-if="!$slots.navigation || desktopHost" type="button" @click="emit('open-book')"><FolderOpen />{{ videoMode ? uiText('切换材料') : uiText('打开书') }}</button>
            <button v-if="buildIntentAvailable" type="button" :aria-pressed="buildIntentOpen" @click="emit('open-build-intent')"><ListChecks />{{ uiText('准备阅读资料') }}</button>
            <button v-if="workbenchAvailable" type="button" @click="emit('open-workbench')"><Wrench />{{ uiText('高级构建') }}</button>
          </div>
          <div class="tool-group">
            <h2>{{ uiText('设置') }}</h2>
            <label class="tool-language"><Globe2 />{{ uiText('界面语言') }}<LanguageSwitcher /></label>
            <slot name="account" />
            <slot name="help" />
            <button v-if="desktopHost" type="button" @click="emit('open-settings')"><Settings />{{ uiText('打开桌面设置') }}</button>
            <button type="button" class="topbar-collapse" @click="collapse"><PanelTopClose />{{ uiText('收起工具栏') }}</button>
          </div>
        </section>
      </div>
      <button ref="toolsButton" type="button" class="bar-icon tools-toggle" :aria-label="uiText('更多阅读工具')" :title="toolsOpen ? uiText('关闭页面菜单') : uiText('更多阅读工具')" :aria-expanded="toolsOpen" :aria-controls="panelId" @click.stop="toolsOpen ? closeTools(true) : expanded = true">
        <X v-if="toolsOpen" :size="19" aria-hidden="true" /><Ellipsis v-else :size="20" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.topbar {
  position: relative; z-index: 70; display: flex; align-items: center; flex: 0 0 48px; height: 48px;
  min-width: 0; padding-inline: clamp(12px, 1.6vw, 28px); gap: clamp(12px, 2vw, 32px);
  background: var(--reader-canvas); border-bottom: 1px solid var(--hairline);
}
.topbar-leading, .topbar-signature, .topbar-location, .topbar-actions { display: flex; align-items: center; }
.topbar-leading { align-self: stretch; gap: 20px; flex-shrink: 0; }
.topbar-signature { gap: 8px; white-space: nowrap; color: var(--charcoal); font: 600 17px/1 Georgia, serif; letter-spacing: -.035em; }
.has-destinations .topbar-signature::after { content: ''; margin-left: 8px; width: 1px; height: 18px; background: var(--hairline); }
.topbar-location { flex: 1; min-width: 0; gap: 7px; justify-content: center; }
.chapter-position { display: flex; align-items: center; gap: 9px; min-width: 0; min-height: 40px; padding: 2px 6px; border: 0; border-radius: 5px; text-align: start; background: transparent; }
.outline-label { display: inline-flex; flex: none; align-items: center; gap: 6px; padding-inline-end: 10px; border-inline-end: 1px solid var(--hairline); color: var(--steel); font: 500 13px/1.3 var(--sans); white-space: nowrap; }
button.chapter-position:hover { background: var(--surface-soft); }
.chapter-position[aria-expanded="true"] .outline-label { color: var(--brand-accent-deep); }
.reading-titles { min-width: 0; max-width: 30ch; display: grid; gap: 1px; }
.reading-titles > * { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.material-title { font: 11px/1.3 var(--sans); color: var(--steel); }
.reading-titles strong { font: 600 14px/1.4 var(--sans); color: var(--charcoal); }
.reading-progress { display: inline-flex; align-items: center; gap: 6px; margin-left: 5px; font: 11px/1 var(--sans); font-variant-numeric: tabular-nums; color: var(--steel); }
.reading-progress i { display: block; height: 3px; width: 32px; background: var(--hairline); border-radius: 2px; }
.reading-progress i > i { background: var(--brand-accent); }
.topbar-actions { gap: 3px; flex-shrink: 0; }
.tools-surface { display: contents; }
.bar-icon, .topbar-expand { display: grid; place-items: center; flex: 0 0 auto; width: 34px; height: 34px; min-height: 0; padding: 0; border: 0; border-radius: 6px; color: var(--steel); background: transparent; }
.bar-icon:hover, .bar-icon[aria-expanded="true"] { color: var(--ink); background: var(--surface-soft); }
.topbar-focus { display: inline-flex; justify-content: center; gap: 6px; width: auto; padding-inline: 8px; font: 13px var(--sans); white-space: nowrap; }
.tools-popover { position: absolute; top: calc(100% + 5px); right: 10px; width: min(260px, calc(100% - 20px)); max-height: calc(100dvh - var(--tools-offset) - 14px); overflow: auto; overscroll-behavior: contain; padding: 6px; border: 1px solid var(--hairline); border-radius: 11px; background: var(--reader-card); box-shadow: 0 10px 32px #34241522; }
.tool-group { display: grid; gap: 1px; padding: 3px; }
.tool-group + .tool-group { margin-top: 4px; border-top: 1px solid var(--hairline-soft); }
.tool-group h2 { margin: 5px 9px 3px; color: var(--steel); font: 600 11px/1.4 var(--sans); }
.tool-group > button, .tool-group :deep(.product-tour-launch), .tool-language, .tool-group :deep(.network-account summary) { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 34px; padding: 5px 9px; border: 0; border-radius: 5px; text-align: start; font: 13px/1.45 var(--sans); background: transparent; color: var(--ink); }
.tool-group button:hover:not(:disabled), .tool-group :deep(summary:hover), .tool-group :deep(.product-tour-launch:hover) { background: var(--surface-soft); }
.tool-group button:disabled { color: var(--stone); }
.tool-group svg, .tool-group :deep(summary svg), .tool-group :deep(.product-tour-launch svg) { width: 16px; height: 16px; color: var(--steel); flex-shrink: 0; }
.tool-language :deep(select) { margin-left: auto; min-height: 27px; padding: 2px 5px; }
.tool-group :deep(.network-account-menu) { position: static; min-width: 0; margin: 2px 0 4px 24px; padding: 4px 6px; gap: 1px; border: 0; border-left: 1px solid var(--hairline); border-radius: 0; box-shadow: none; background: transparent; }
.tool-group :deep(.network-account-menu :is(button,a,span)) { padding: 6px; text-align: start; font-size: 12px; border: 0; border-radius: 4px; background: transparent; }
.tool-group :deep(.network-account-menu span) { color: var(--steel); }
.tool-group :deep(.product-tour-launch span) { display: inline; }
.tool-group :deep(.product-tour-launch) { justify-content: flex-start; }
.topbar-expand { position: absolute; z-index: 71; top: 6px; right: 8px; background: var(--reader-card); border: 1px solid var(--hairline); }
.topbar-mobile-backdrop, .topbar.desktop-collapsed { display: none; }
.topbar.desktop-collapsed.mobile-open { display: flex; position: fixed; inset: env(safe-area-inset-top) 0 auto; }
@media (min-width: 1200px) {
  .topbar { display: grid; grid-template-columns: minmax(max-content, 1fr) minmax(0, 30%) minmax(max-content, 1fr); }
  .topbar-actions { justify-self: end; }
}
@media (max-width: 1023px) {
  .topbar { gap: 12px; }
  .topbar-leading { gap: 12px; }
  .topbar-expand, .tool-group > .topbar-collapse, .topbar.mobile-collapsible { display: none; }
  .topbar.mobile-collapsible.mobile-open { display: flex; position: fixed; inset: env(safe-area-inset-top) 0 auto; }
  .topbar-mobile-backdrop { position: fixed; inset: 0; z-index: 69; display: block; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: #211b182b; }
  .topbar-focus span { display: none; }
  .tool-group > button, .tool-group :deep(.product-tour-launch), .tool-group :deep(.network-account summary), .tool-language { min-height: 42px; }
}
@media (max-width: 720px) {
  .topbar { gap: 6px; padding-inline: 8px; }
  .topbar-leading { gap: 4px; }
  .topbar-signature span, .has-destinations .topbar-signature::after, .reading-progress > i { display: none; }
  .topbar-signature img { width: 22px; height: 22px; }
  .topbar-location { justify-content: flex-start; gap: 2px; }
  .chapter-position { gap: 5px; padding: 0 3px; min-height: 44px; }
  .outline-label { gap: 4px; padding-inline-end: 6px; }
  .reading-titles strong { font-size: 13px; }
  .topbar-actions { gap: 0; }
  .bar-icon { width: 40px; height: 44px; }
  .tools-surface { display: none; }
  .tools-surface.tools-open { display: block; }
  .topbar-menu { position: static; width: auto; max-height: none; padding: 0; overflow: visible; border: 0; box-shadow: none; }
  .topbar-focus { justify-content: flex-start; width: calc(100% - 6px); min-height: 44px; margin: 3px; padding-inline: 9px; gap: 10px; background: var(--surface-soft); }
  .topbar-focus span { display: inline; }
}
@media (max-width: 420px) {
  .topbar { gap: 3px; }
  .outline-label svg { display: none; }
}
@media (max-width: 360px) {
  .has-destinations .topbar-signature { display: none; }
}
</style>
