<script setup lang="ts">
import { ref, watch } from 'vue';
import type { TutorSession } from '../generated/TutorSession';
const props = defineProps<{ session: TutorSession | null; enabled: boolean; busy: boolean; available: boolean; error: string }>();
const emit = defineEmits<{ (e: 'enable'): void; (e: 'goal', value: string): void; (e: 'resume'): void }>();
const editing = ref(false);
const draft = ref('');
watch(() => props.session, () => { if (!props.error) editing.value = false; });
function edit() { draft.value = props.session?.user_intent ?? ''; editing.value = true; }
</script>
<template>
  <section class="conversation-learning-goal" aria-label="本次学习目标">
    <template v-if="session">
      <div class="goal-heading"><span>本次想弄懂</span><button :disabled="busy || !available" @click="edit">调整目标</button></div>
      <p class="goal-text">{{ session.user_intent }}</p>
      <p v-if="session.status !== 'active'">{{ session.status === 'ended' ? '这次学习已结束' : '连续教学已暂停' }}<button v-if="enabled" :disabled="busy" @click="emit('resume')">继续这次学习</button></p>
    </template>
    <template v-else>
      <strong>这次想弄懂什么？</strong>
      <p>直接在下方输入目标或第一个问题，从这里开始。</p>
      <button v-if="enabled" :disabled="busy || !available" @click="edit">设定学习目标</button>
    </template>
    <p v-if="!enabled">可以直接提问，也可以<button :disabled="busy || !available" @click="emit('enable')">开启连续教学</button>，围绕目标逐步学习。</p>
    <form v-if="editing" @submit.prevent="draft.trim() && emit('goal', draft.trim())">
      <label>学习目标<input v-model="draft" :disabled="busy" /></label>
      <button :disabled="busy || !draft.trim()">保存目标</button><button type="button" :disabled="busy" @click="editing = false">取消</button>
    </form>
    <p v-if="error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.conversation-learning-goal { margin: 8px 0 12px; padding: 12px 14px; border: 1px solid var(--line); border-radius: 12px; background: var(--canvas); font-size: .85rem; }
.goal-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; color: var(--muted); }
p { margin: 6px 0; line-height: 1.6; }
.goal-text { color: var(--ink); overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; }
button { font: inherit; color: var(--accent); background: transparent; border: none; cursor: pointer; min-height: 36px; padding: 4px 6px; }
button:disabled { opacity: .5; cursor: default; }
input { display: block; box-sizing: border-box; width: 100%; font: inherit; padding: 8px; border: 1px solid var(--line); border-radius: 6px; background: var(--canvas); color: var(--ink); }
</style>
