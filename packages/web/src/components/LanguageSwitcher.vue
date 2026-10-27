<script setup lang="ts">
import { getLocale, switchLocale, type Locale } from '../i18n';
defineProps<{ resetToken?: string }>();
</script>

<template>
  <select class="language-switcher" aria-label="Language / 语言" :value="getLocale()" @change="switchLocale(($event.target as HTMLSelectElement).value as Locale, resetToken)">
    <option value="zh-CN">中文</option>
    <option value="en">English</option>
  </select>
</template>

<style scoped>
.language-switcher { flex: 0 0 auto; max-width: 8rem; min-height: 32px; padding: .35rem .5rem; border: 1px solid var(--line, #d8d3c8); border-radius: 7px; background: var(--canvas, #fffdf8); color: var(--ink, #302d27); font: inherit; font-size: 12px; cursor: pointer; }
.language-switcher:focus-visible { outline: 2px solid #b65a17; outline-offset: 3px; }
</style>
