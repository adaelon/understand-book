<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';
import { Target } from '@lucide/vue';
import type { TutorSession } from '../generated/TutorSession';
const props = defineProps<{ session: TutorSession | null; enabled: boolean; busy: boolean; available: boolean; ready: boolean; otherSession: boolean; error: string }>();
const emit = defineEmits<{ (e: 'goal', value: string): void; (e: 'resume'): void; (e: 'manage'): void; (e: 'retry'): void }>();
const editing = ref(false);
const draft = ref('');
const requestedGoal = ref<string | null>(null);
const goalInput = ref<HTMLInputElement>();
const editButton = ref<HTMLButtonElement>();
function cancelEdit() { if (props.busy) return; editing.value = false; requestedGoal.value = null; void nextTick(() => editButton.value?.focus()); }
function onEditorKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); cancelEdit(); }
  if (event.key !== 'Tab') return;
  const controls = Array.from((event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('input:not(:disabled), button:not(:disabled)'));
  const first = controls[0], last = controls[controls.length - 1];
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
}
watch(() => props.session, session => {
  if (requestedGoal.value && !props.error && session?.user_intent === requestedGoal.value) {
    editing.value = false; requestedGoal.value = null;
    void nextTick(() => editButton.value?.focus());
  }
});
async function edit() { draft.value = props.session?.user_intent ?? ''; requestedGoal.value = null; editing.value = true; await nextTick(); goalInput.value?.focus(); }
function save() {
  if (props.busy || !props.available || !draft.value.trim()) return;
  requestedGoal.value = draft.value.trim(); emit('goal', requestedGoal.value);
}
</script>
<template>
  <section v-if="session || otherSession || editing" class="conversation-learning-goal" aria-label="本次学习目标">
    <template v-if="session">
      <div class="goal-heading">
        <span class="goal-icon" aria-hidden="true"><Target :size="18" /></span>
        <span class="goal-label">本次学习</span>
        <p class="goal-text">{{ session.user_intent }}</p>
        <div class="goal-actions"><button ref="editButton" type="button" aria-label="调整目标" :disabled="busy || !available" @click="edit"><span class="desktop-adjust">调整目标</span><span class="mobile-adjust">调整</span></button><button class="goal-manage" type="button" @click="emit('manage')">详情</button></div>
      </div>
      <div v-if="session.status !== 'active'" class="goal-resume"><span>{{ session.status === 'ended' ? '这次学习已结束' : '连续教学已暂停' }}</span><button type="button" :disabled="busy || !available || !ready" @click="emit('resume')">{{ enabled ? '继续这次学习' : '开启并继续这次学习' }} <span aria-hidden="true">→</span></button></div>
    </template>
    <template v-else-if="otherSession">
      <p>当前学习属于其他书籍。</p><button ref="editButton" type="button" :disabled="busy || !available" @click="edit">为这本书设定目标</button>
    </template>
    <div v-if="editing" class="goal-editor-overlay" @click.self="cancelEdit">
    <form role="dialog" aria-modal="true" aria-label="调整目标" @submit.prevent="save" @keydown="onEditorKeydown">
      <h3>调整目标</h3>
      <label>学习目标<input ref="goalInput" v-model="draft" :disabled="busy || !available" /></label>
      <div class="goal-actions"><button :disabled="busy || !available || !draft.trim()">保存目标</button><button type="button" :disabled="busy" @click="cancelEdit">取消</button></div>
      <p v-if="error" role="alert">{{ error }} <button type="button" :disabled="busy" @click="emit('retry')">重试</button></p>
    </form>
    </div>
  </section>
</template>
<style scoped>
.conversation-learning-goal { flex: 0 0 auto; margin: 8px 16px 0; padding: 12px 0; border-bottom: 1px solid var(--line); font-size: .82rem; min-width: 0; }
.goal-heading { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; column-gap: 12px; color: var(--steel); font-size: .76rem; }
.goal-icon { grid-row: 1 / 3; display: grid; place-items: center; width: 36px; height: 36px; border-radius: 12px; background: var(--reader-coral-soft); color: var(--brand-green-deep); }
.goal-label { grid-column: 2; font-size: .7rem; letter-spacing: .06em; }
.goal-heading .goal-actions { grid-column: 3; grid-row: 1 / 3; }
.goal-resume { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 2px 8px; color: var(--steel); font-size: .76rem; }
.goal-actions { display: flex; flex-wrap: wrap; gap: 6px; }
p { margin: 4px 0; line-height: 1.6; overflow-wrap: anywhere; }
.goal-text { grid-column: 2; grid-row: 2; margin: 2px 0 0; color: var(--ink); font-size: .85rem; font-weight: 500; overflow-wrap: anywhere; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; }
.mobile-adjust { display: none; }
button { font: inherit; color: var(--steel); background: transparent; border: none; border-radius: 6px; cursor: pointer; min-height: 36px; padding: 4px 6px; }
button:hover:not(:disabled) { color: var(--brand-green-deep); background: var(--surface-soft); }
button:disabled { opacity: .5; cursor: default; }
.goal-editor-overlay { position: absolute; inset: 0; z-index: 20; display: flex; align-items: flex-end; background: #0003; }
form { width: 100%; max-height: 100%; overflow-y: auto; box-sizing: border-box; margin: 0; padding: 16px; border: 1px solid var(--line); border-radius: 14px 14px 0 0; background: var(--canvas); }
form h3 { margin: 0 0 12px; color: var(--ink); font-size: 1rem; }
form .goal-actions { margin-top: 12px; }
input { display: block; box-sizing: border-box; width: 100%; font: inherit; margin-top: 6px; padding: 8px; border: 1px solid var(--line); border-radius: 6px; background: var(--canvas); color: var(--ink); }
@media (pointer: coarse) { button { min-height: 44px; } input { font-size: 16px; } }
@media (max-width: 1023px) {
  .conversation-learning-goal { margin: 0; padding: 0 12px; }
  .goal-heading { grid-template-columns: auto minmax(0, 1fr) auto; min-height: 44px; }
  .goal-icon { display: none; }
  .goal-label { grid-column: 1; font-size: .7rem; letter-spacing: 0; }
  .goal-text { grid-row: 1; grid-column: 2; display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .goal-heading .goal-actions { grid-column: 3; grid-row: 1; }
  .goal-manage, .desktop-adjust { display: none; }
  .mobile-adjust { display: inline; }
  .goal-heading button { min-height: 44px; min-width: 44px; }
  .goal-resume { padding-bottom: 4px; }
}
</style>
