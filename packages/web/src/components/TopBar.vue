<script setup lang="ts">
import { ListChecks, Settings } from "@lucide/vue";
import { ref, watch } from "vue";

const props = defineProps<{
  typographyAvailable?: boolean;
  focusAvailable?: boolean;
  focusReading?: boolean;
  chatUnavailable?: boolean;
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
const collapsed = ref(false);
watch(() => props.focusReading, value => { collapsed.value = !!value; });
const emit = defineEmits<{
  (e: "open-typography"): void;
  (e: "toggle-focus"): void;
  (e: "new-chat"): void;
  (e: "open-book"): void;
  (e: "toggle-left-rail"): void;
  (e: "toggle-debug"): void;
  (e: "open-build-intent"): void;
  (e: "open-workbench"): void;
  (e: "open-settings"): void;
  (e: "close-mobile"): void;
}>();

function closeMobileAfterAction(event: MouseEvent) {
  if ((event.target as HTMLElement).closest("button")) emit("close-mobile");
}
</script>

<template>
  <button v-if="collapsed" class="topbar-expand ghost-pill" aria-label="展开工具栏" @click="collapsed = false">菜单</button>
  <div v-if="mobileCollapsible && !mobileOpen && $slots['tutor-control']" class="mobile-tutor-control"><slot name="tutor-control" /></div>
  <button
    v-if="mobileCollapsible && mobileOpen"
    type="button"
    class="topbar-mobile-backdrop"
    aria-label="关闭页面菜单"
    @click="emit('close-mobile')"
  ></button>
  <header
    class="topbar"
    :class="{ 'mobile-collapsible': mobileCollapsible, 'mobile-open': mobileOpen, 'desktop-collapsed': collapsed }"
    @click="closeMobileAfterAction"
    @keydown.esc="emit('close-mobile')"
  >
    <div class="topbar-brand">
      <span class="brand-mark">understand-book</span>
      <span class="breadcrumb">{{ chapterTitle || "阅读工作区" }}</span>
    </div>
    <div class="topbar-actions">
      <slot name="tutor-control" />
      <span class="progress">{{ progressPct }}%</span>
      <button v-if="focusAvailable" class="ghost-pill topbar-focus" :aria-pressed="!!focusReading" @click="emit('toggle-focus')">{{ focusReading ? '退出专注' : '专注阅读' }}</button>
      <button v-if="typographyAvailable" data-typography-trigger class="ghost-pill" @click="emit('open-typography')">阅读设置</button>
      <button class="ghost-pill" :class="{ active: leftRailOpen }" @click="emit('toggle-left-rail')">目录</button>
      <button class="ghost-pill" :disabled="chatUnavailable" @click="emit('new-chat')">新对话</button>
      <button
        v-if="buildIntentAvailable"
        class="topbar-icon-button"
        :class="{ active: buildIntentOpen }"
        title="构建方案"
        aria-label="打开构建方案"
        @click="emit('open-build-intent')"
      >
        <ListChecks :size="17" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button v-if="workbenchAvailable" class="ghost-pill" @click="emit('open-workbench')">高级构建</button>
      <button class="ghost-pill" @click="emit('open-book')">打开书</button>
      <button
        v-if="desktopHost"
        class="topbar-icon-button"
        title="桌面设置"
        aria-label="打开桌面设置"
        @click="emit('open-settings')"
      >
        <Settings :size="17" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <button class="ghost-pill" :class="{ active: debugOpen }" @click="emit('toggle-debug')">调试</button>
      <slot name="account" />
      <button class="ghost-pill topbar-collapse" aria-label="收起工具栏" @click="collapsed = true">收起</button>
      <button type="button" class="topbar-mobile-close">关闭</button>
    </div>
  </header>
</template>

<style scoped>
.mobile-tutor-control { display: none; }
@media (max-width: 1023px) {
  .mobile-tutor-control { display: flex; justify-content: flex-end; padding: 4px 8px; background: var(--canvas); }
}
</style>
