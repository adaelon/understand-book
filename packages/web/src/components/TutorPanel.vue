<script setup lang="ts">
import { computed, ref } from 'vue';
import type { TutorSession } from '../generated/TutorSession';
import type { TutorState } from '../generated/TutorState';
import type { TutorAction } from '../generated/TutorAction';
import type { TutorSessionMode } from '../generated/TutorSessionMode';
import UnderstandingSpace from './UnderstandingSpace.vue';
const props = defineProps<{ state: TutorState | null; busy: boolean; error: string; pending: boolean; sourceId?: string; label: string;
  readiness?: { status: string; limitations: string[]; reason: string } | null; readinessError?: string }>();
const emit = defineEmits<{ (e: 'action', action: TutorAction): void; (e: 'retry'): void; (e: 'close'): void; (e: 'refreshReadiness'): void }>();
const intent = ref('');
const sessions = computed(() => Object.values(props.state?.sessions ?? {}).filter((session): session is TutorSession => !!session));
function start() {
  if (!intent.value.trim()) return;
  emit('action', { kind: 'start', user_intent: intent.value.trim(), explicit_constraints: [],
    material_scope: props.sourceId ? [{ source_id: props.sourceId, scope_refs: [], role: 'primary' }] : [], default_teaching_intent: null });
}
</script>
<template>
  <Teleport to="body">
    <div class="tutor-backdrop" @click.self="emit('close')">
      <section role="dialog" aria-modal="true" aria-label="学习会话" class="tutor-panel">
        <header><h2>学习会话</h2><button @click="emit('close')">关闭</button></header>
        <p role="status">{{ label }}</p>
        <p data-testid="teaching-readiness">{{ readinessError || (readiness?.status === 'ready' ? '可以开始或继续学习' : readiness?.reason) || '正在读取学习就绪状态' }} <button @click="emit('refreshReadiness')">刷新学习状态</button></p>
        <ul v-if="readiness?.limitations?.length"><li v-for="limitation in readiness.limitations" :key="limitation">{{ limitation }}</li></ul>
        <p>{{ readiness?.status === 'ready' ? '从当前问题和原文开始；可用的教学资料会在后续学习中使用。' : '学习基础准备好后即可开始；等待期间可以先保存学习意图。' }}你随时可以要求直接讲解。</p>
        <p v-if="error" role="alert">{{ error }} <button :disabled="busy" @click="emit('retry')">重试</button></p>
        <form @submit.prevent="start">
          <label>本次想学什么 <textarea v-model="intent" rows="2" /></label>
          <button :disabled="busy || pending || !state || !sourceId || !intent.trim()">{{ readiness?.status === 'ready' && state?.control.enabled ? '开始学习' : '保存学习意图' }}</button>
        </form>
        <article v-for="session in sessions" :key="session.id">
          <h3>{{ session.user_intent }} <small v-if="state?.control.current_tutor_session_id === session.id">当前</small></h3>
          <p>{{ { active: '学习进行中', paused: '已暂停', ended: '已结束' }[session.status] }}</p>
          <label>默认教法
            <select :value="session.default_teaching_intent ?? ''" :disabled="busy || pending" @change="emit('action', { kind: 'set_mode', session_id: session.id, mode: (($event.target as HTMLSelectElement).value || null) as TutorSessionMode | null })">
              <option value="">随本次需求</option><option value="direct_explanation">直接讲解</option><option value="guided_inquiry">引导思考</option>
            </select>
          </label>
          <p v-if="sourceId && !session.material_scope.some(material => material.source_id === sourceId)">请回到本次学习的书籍继续，或为当前书籍开始新目标。</p>
          <button :disabled="busy || pending || !state?.control.enabled || readiness?.status !== 'ready' || !session.material_scope.some(material => material.source_id === sourceId)" @click="emit('action', { kind: 'resume', session_id: session.id })">{{ session.status === 'active' ? '继续教学' : '继续这次学习' }}</button>
          <button v-if="session.status === 'active'" :disabled="busy || pending" @click="emit('action', { kind: 'pause', session_id: session.id })">暂停</button>
          <button v-if="session.status !== 'ended'" :disabled="busy || pending" @click="emit('action', { kind: 'end', session_id: session.id })">结束这次学习</button>
        </article>
        <UnderstandingSpace :key="sourceId" />
      </section>
    </div>
  </Teleport>
</template>
<style scoped>
.tutor-backdrop { position: fixed; inset: 0; z-index: 220; background: #0005; display: grid; place-items: center; padding: 12px; }
.tutor-panel { width: min(640px, 100%); max-height: 90dvh; overflow: auto; padding: 20px; border-radius: 14px; background: var(--canvas); color: var(--ink); }
header { display: flex; justify-content: space-between; align-items: center; }
button, select, textarea { font: inherit; padding: 8px; min-height: 44px; max-width: 100%; }
textarea { display: block; width: 100%; box-sizing: border-box; }
article { border-top: 1px solid var(--line); margin-top: 16px; padding-top: 8px; }
button { margin: 4px; }
</style>
