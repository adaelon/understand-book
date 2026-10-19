<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { ApiError } from '../api';
import { network, type LearningWorkspace } from '../network-context';
import { sharedWorkspaces } from '../network-client';
import type { SharedBook, SharedRelease, SharedReleasePage } from '../shared-workspaces';
import SharedWorkspaceManifest from './SharedWorkspaceManifest.vue';

defineProps<{ books: SharedBook[] }>();
const emit = defineEmits<{ started: [workspace: LearningWorkspace] }>();
const page = ref<SharedReleasePage | null>(null), selected = ref<SharedRelease | null>(null);
const goal = ref(''), loading = ref(false), busy = ref(false), error = ref('');
let request = 0, operationId = '', submittedGoal = '';
const failureMessage = (failure: unknown) => failure instanceof Error ? failure.message : '共享工作区暂时无法读取';
async function load(offset = 0) {
  const ticket = ++request; loading.value = true; error.value = ''; selected.value = null;
  try { const value = await sharedWorkspaces.list(offset); if (ticket === request) page.value = value; }
  catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) loading.value = false; }
}
async function select(release: SharedRelease) {
  const ticket = ++request; loading.value = true; error.value = ''; selected.value = null;
  try {
    const value = await sharedWorkspaces.read(release.release_id, release.version);
    if (ticket === request) { selected.value = value; goal.value = value.manifest.public_goal; operationId = ''; submittedGoal = ''; }
  } catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) loading.value = false; }
}
async function start() {
  const release = selected.value, text = goal.value.trim();
  if (!release || !text || busy.value || release.archived || release.availability.status !== 'ready') return;
  if (!operationId || submittedGoal !== text) { operationId = crypto.randomUUID(); submittedGoal = text; }
  const ticket = request; busy.value = true; error.value = '';
  try {
    const value = await sharedWorkspaces.use(release.release_id, release.version, operationId, text);
    if (ticket === request) { selected.value = null; emit('started', value.workspace); }
  } catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) busy.value = false; }
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { page.value = null; busy.value = false; void load(); }, { immediate: true });
onBeforeUnmount(() => { request++; });
</script>

<template>
  <section class="shared-catalog" aria-label="共享预构建工作区" :aria-busy="loading || busy">
    <div class="heading"><div><h2>共享预构建工作区</h2><p>从准备好的资料和路线开始，按自己的目标学习。</p></div><button :disabled="loading || busy" @click="load(page?.offset ?? 0)">刷新共享工作区</button></div>
    <p v-if="error" role="alert">{{ error }}</p><p v-if="loading" role="status">正在读取共享工作区…</p>
    <template v-if="selected">
      <article class="shared-selection" aria-label="共享工作区详情">
        <div class="heading"><h3>{{ selected.manifest.name }}</h3><button :disabled="busy" @click="selected = null">返回共享列表</button></div>
        <SharedWorkspaceManifest :release="selected" :books="books" />
        <p v-if="selected.archived" role="status">此共享工作区已归档，暂不接受新的使用。</p>
        <form class="shared-goal" @submit.prevent="start">
          <label>确认你的学习目标<textarea v-model="goal" required :disabled="busy || selected.archived" /></label>
          <p>可以接受上面的目标，也可以改成自己的计划。开始后会建立你的个人工作区，学习记录分别保存。</p>
          <button :disabled="busy || !goal.trim() || selected.archived || selected.availability.status !== 'ready'">{{ busy ? '正在建立个人工作区…' : '确认目标并开始学习' }}</button>
        </form>
      </article>
    </template>
    <template v-else-if="page && !loading">
      <div class="shared-list"><article v-for="release in page.items" :key="`${release.release_id}:${release.version}`" class="shared-card"><div><h3>{{ release.manifest.name }}</h3><small>{{ release.manifest.publisher }} · 第 {{ release.version }} 版</small></div><p>{{ release.manifest.public_goal }}</p><p class="status">{{ release.archived ? '已归档' : release.availability.status === 'ready' ? '资料已就绪' : '尚未完整就绪' }} · {{ release.manifest.materials.length }} 份资料</p><button :disabled="busy" @click="select(release)">查看目标与路线</button></article></div>
      <p v-if="!page.items.length">暂无可用的共享工作区。</p>
      <nav v-if="page.total > page.limit" aria-label="共享工作区分页"><button :disabled="busy || !page.offset" @click="load(Math.max(0, page.offset - page.limit))">上一页共享工作区</button><span>{{ page.offset + 1 }}–{{ page.offset + page.items.length }} / {{ page.total }}</span><button :disabled="busy || page.offset + page.limit >= page.total" @click="load(page.offset + page.limit)">下一页共享工作区</button></nav>
    </template>
  </section>
</template>

<style scoped>
.shared-catalog { margin: 20px 0; padding: 22px; border: 1px solid var(--line); border-radius: 16px; background: var(--canvas); }.heading { display: flex; gap: 12px; align-items: center; justify-content: space-between; }.heading>div { min-width: 0; } h2,h3 { margin: 0; font-weight: 500; overflow-wrap: anywhere; }h2 { font-size: 20px; }h3 { font-size: 17px; } p { line-height: 1.75; overflow-wrap: anywhere; } .heading p,small,.status,.shared-goal p { color: var(--muted); font-size: 13px; }button { font: inherit; border: 1px solid var(--line); border-radius: 8px; color: var(--ink); background: var(--canvas); padding: 9px 12px; min-height: 40px; cursor: pointer; }button:disabled { opacity: .5; cursor: default; }.shared-list { display: grid; grid-template-columns: repeat(auto-fit,minmax(220px,1fr)); gap: 14px; margin-top: 12px; }.shared-card { border: 1px solid var(--line); border-radius: 12px; padding: 18px; min-width: 0; }.shared-goal { margin-top: 22px; padding: 18px; border-radius: 12px; background: var(--surface-soft); }.shared-goal label { display: block; line-height: 1.8; }.shared-goal textarea { display: block; box-sizing: border-box; width: 100%; min-height: 90px; margin-top: 6px; border: 1px solid var(--line); border-radius: 8px; background: var(--canvas); color: var(--ink); padding: 10px; font: inherit; }.shared-goal button { background: var(--brand-accent); color: var(--brand-on-accent); }nav { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 18px; }@media(max-width:540px){.shared-catalog { padding: 15px; }.heading { align-items: flex-start; flex-direction: column; }.shared-list { grid-template-columns: minmax(0,1fr); }.shared-goal { padding: 14px; }.shared-goal textarea { font-size: 16px; } }
</style>
