<script setup lang="ts">
import { t as uiText } from '../i18n';
</script>
<template>
  <div class="book-loading" role="status" :aria-label="uiText('正在加载')">
    <div class="book-loading-book" aria-hidden="true">
      <span class="book-loading-leaf book-loading-left"><span class="book-loading-lines"></span></span>
      <span class="book-loading-leaf book-loading-right"><span class="book-loading-lines"></span></span>
      <span class="book-loading-leaf book-loading-turn"><span class="book-loading-lines"></span></span>
      <span class="book-loading-spine"></span>
      <span class="book-loading-shadow"></span>
    </div>
  </div>
</template>
