<script setup lang="ts">
defineProps<{ enabled: boolean; label: string; busy: boolean; unavailable: boolean; error: string }>();
const emit = defineEmits<{ (e: 'toggle'): void; (e: 'manage'): void }>();
</script>
<template>
  <div class="tutor-control">
    <button type="button" role="switch" :aria-checked="enabled" :disabled="busy || unavailable" @click="emit('toggle')">{{ label }}</button>
    <button type="button" aria-label="管理教学会话" @click="emit('manage')">{{ error ? '教学设置需处理' : '学习会话' }}</button>
  </div>
</template>
<style scoped>
.tutor-control { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
button { min-height: 44px; border: 1px solid var(--line); border-radius: 8px; padding: 6px 10px; color: var(--ink); background: var(--canvas); font: inherit; font-size: .82rem; }
[aria-checked=true] { border-color: var(--accent); }
</style>
