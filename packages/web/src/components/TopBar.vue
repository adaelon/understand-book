<script setup lang="ts">
import { t as uiText } from '../i18n';

import { ListChecks, Settings } from "@lucide/vue";
import { ref, watch } from "vue";
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
  chapterTitle: string;
  guideChapterTitle?: string;
  guideChapterEnabled?: boolean;
  guideChapterBusy?: boolean;
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
const collapsed = ref(false);
watch(() => props.focusReading, value => { collapsed.value = !!value; });
const emit = defineEmits<{
  (e: "open-typography"): void;
  (e: "guide-chapter"): void;
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

function closeMobileAfterAction(event: MouseEvent) {
  if ((event.target as HTMLElement).closest("button")) emit("close-mobile");
}
</script>

<template>
  <button v-if="collapsed && !focusReading" class="topbar-expand ghost-pill" :aria-label="uiText('展开工具栏')" @click="collapsed = false">{{ uiText('菜单') }}</button>
  <button
    v-if="mobileCollapsible && mobileOpen"
    type="button"
    class="topbar-mobile-backdrop"
    :aria-label="uiText('关闭页面菜单')"
    @click="emit('close-mobile')"
  ></button>
  <header
    class="topbar"
    :class="{ 'mobile-collapsible': mobileCollapsible, 'mobile-open': mobileOpen, 'desktop-collapsed': collapsed, 'video-topbar': videoMode }"
    @click="closeMobileAfterAction"
    @keydown.esc="emit('close-mobile')"
  >
    <div class="topbar-brand">
      <span class="brand-mark"><img :src="brandLogo" class="brand-logo" width="26" height="26" alt="" />understand-book</span>
      <span class="breadcrumb">{{ chapterTitle || uiText('阅读工作区') }}</span>
    </div>
    <div class="topbar-actions">
      <LanguageSwitcher />
      <span v-if="!videoMode" class="progress">{{ progressPct }}%</span>
      <button v-if="guideChapterTitle" class="ghost-pill guide-chapter-button" :disabled="guideChapterBusy || chatUnavailable" @click="emit('guide-chapter')">{{ guideChapterEnabled ? uiText('带我读这一章') : uiText('开启学习并带我读这一章') }}：{{ guideChapterTitle }}</button>
      <button v-if="focusAvailable" class="ghost-pill topbar-focus" :aria-pressed="!!focusReading" @click="emit('toggle-focus')">{{ focusReading ? uiText('退出专注') : musicMode ? uiText('专注聆听') : videoMode ? uiText('专注观看') : uiText('专注阅读') }}</button>
      <button v-if="typographyAvailable" data-typography-trigger class="ghost-pill" @click="emit('open-typography')">{{ uiText('阅读设置') }}</button>
      <button v-if="!videoMode" class="ghost-pill" :class="{ active: leftRailOpen }" @click="emit('toggle-left-rail')">{{ uiText('目录') }}</button>
      <button class="ghost-pill" :disabled="chatUnavailable" @click="emit('new-chat')">{{ uiText('新对话') }}</button>
      <button v-if="boardAvailable" class="ghost-pill" @click="emit('open-board')">{{ uiText('画板') }}</button>
      <button v-if="notesAvailable" class="ghost-pill" @click="emit('open-notes')">{{ uiText('空间笔记') }}</button>
      <button
        v-if="buildIntentAvailable"
        class="topbar-icon-button"
        :class="{ active: buildIntentOpen }"
        :title="uiText('准备阅读资料')"
        :aria-label="uiText('准备阅读资料')"
        @click="emit('open-build-intent')"
      >
        <ListChecks :size="17" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button v-if="workbenchAvailable" class="ghost-pill" @click="emit('open-workbench')">{{ uiText('高级构建') }}</button>
      <button class="ghost-pill" @click="emit('open-book')">{{ videoMode ? uiText('切换材料') : uiText('打开书') }}</button>
      <button
        v-if="desktopHost"
        class="topbar-icon-button"
        :title="uiText('桌面设置')"
        :aria-label="uiText('打开桌面设置')"
        @click="emit('open-settings')"
      >
        <Settings :size="17" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <slot name="account" />
      <button class="ghost-pill topbar-collapse" :aria-label="uiText('收起工具栏')" @click="collapsed = true">{{ uiText('收起') }}</button>
      <button type="button" class="topbar-mobile-close">{{ uiText('关闭') }}</button>
    </div>
  </header>
</template>

<style scoped>
@media (min-width: 1024px) {
  .topbar:lang(en) { height: auto; min-height: 56px; flex-basis: auto; flex-wrap: wrap; padding-block: 10px; }
  .topbar:lang(en) .topbar-actions { flex: 1 1 48rem; flex-wrap: wrap; justify-content: flex-end; max-width: 100%; }
  .topbar:lang(en) .guide-chapter-button { max-width: 19rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
}
@media (max-width: 1023px) {
  .topbar-actions .guide-chapter-button {
    max-width: 100%;
    white-space: normal;
    overflow-wrap: anywhere;
    text-align: left;
  }
}
</style>
