<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { ApiError } from '../api';
import { network, type LearningWorkspace } from '../network-context';
import { sharedWorkspaces } from '../network-client';
import { sharedBookKey, sharedBookLabel, type SharedBook, type SharedRelease, type SharedReleasePage } from '../shared-workspaces';
import MaterialCover from './MaterialCover.vue';
import SharedWorkspaceManifest from './SharedWorkspaceManifest.vue';
import { examSprints, sprintRequest, type ExamSprintInput } from '../exam-sprints';

const props = defineProps<{ books: SharedBook[] }>();
const materialBook = (reference: SharedBook['published_book_ref']) => props.books.find(book => sharedBookKey(book.published_book_ref) === sharedBookKey(reference));
const emit = defineEmits<{ started: [workspace: LearningWorkspace, message?: string] }>();
const page = ref<SharedReleasePage | null>(null), selected = ref<SharedRelease | null>(null);
const goal = ref(''), loading = ref(false), busy = ref(false), error = ref('');
const sprintEnabled = ref(false), examDate = ref(''), syllabus = ref(''), minutes = ref<number | null>(null);
const entryMode = ref<'diagnostic' | 'question'>('diagnostic'), question = ref('');
let request = 0, operationId = '', submittedGoal = '';
let pendingRead: AbortController | null = null;
const failureMessage = (failure: unknown) => failure instanceof Error ? failure.message : '共享工作区暂时无法读取';
async function readCatalog<T>(read: (signal: AbortSignal) => Promise<T>): Promise<T> {
  pendingRead?.abort();
  const controller = new AbortController(); pendingRead = controller;
  const timeout = setTimeout(() => controller.abort(new Error('共享工作区读取超时，请重试。')), 15_000);
  try { return await read(controller.signal); }
  finally { clearTimeout(timeout); if (pendingRead === controller) pendingRead = null; }
}
async function load(offset = 0) {
  const ticket = ++request; loading.value = true; error.value = ''; selected.value = null;
  try { const value = await readCatalog(signal => sharedWorkspaces.list(offset, signal)); if (ticket === request) page.value = value; }
  catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) loading.value = false; }
}
async function select(release: SharedRelease) {
  const ticket = ++request; loading.value = true; error.value = ''; selected.value = null;
  try {
    const value = await readCatalog(signal => sharedWorkspaces.read(release.release_id, release.version, signal));
    if (ticket === request) { selected.value = value; goal.value = value.manifest.public_goal; operationId = ''; submittedGoal = ''; sprintEnabled.value = false; examDate.value = ''; syllabus.value = ''; minutes.value = null; question.value = ''; entryMode.value = 'diagnostic'; }
  } catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) loading.value = false; }
}
async function start() {
  const release = selected.value, text = goal.value.trim();
  if (!release || !text || busy.value || release.archived || release.availability.status !== 'ready') return;
  const sprint: ExamSprintInput | undefined = sprintEnabled.value ? { exam_date: examDate.value, syllabus: syllabus.value,
    daily_minutes: minutes.value || null, utc_offset_minutes: -new Date().getTimezoneOffset(), entry_mode: entryMode.value } : undefined;
  if (sprint && !examDate.value) return;
  const parameters = JSON.stringify([text, sprint]);
  if (!operationId || submittedGoal !== parameters) { operationId = crypto.randomUUID(); submittedGoal = parameters; }
  const ticket = request; busy.value = true; error.value = '';
  try {
    const value = sprint ? await examSprints.useCourse(release.release_id, release.version, operationId, text, sprint)
      : await sharedWorkspaces.use(release.release_id, release.version, operationId, text);
    if (ticket === request) { selected.value = null; emit('started', value.workspace, sprint ? sprintRequest(entryMode.value, question.value) : undefined); }
  } catch (failure) { if (ticket === request && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failureMessage(failure); }
  finally { if (ticket === request) busy.value = false; }
}
watch(() => `${network.value.epoch}:${network.value.identity?.user_id}`, () => { page.value = null; busy.value = false; void load(); }, { immediate: true });
onBeforeUnmount(() => { request++; pendingRead?.abort(); });
</script>

<template>
  <section class="shared-catalog" aria-label="共享预构建工作区" :aria-busy="loading || busy">
    <div class="heading"><div><h2>课程工作区</h2><p>教材、课程与复习路线已提前准备，按自己的目标开始。公共资料不占个人上传空间。</p></div><button :disabled="loading || busy" @click="load(page?.offset ?? 0)">刷新共享工作区</button></div>
    <p v-if="error" role="alert">{{ error }}</p><p v-if="loading" role="status">正在读取共享工作区…</p>
    <template v-if="selected">
      <article class="shared-selection" aria-label="共享工作区详情">
        <div class="heading"><h3>{{ selected.manifest.name }}</h3><button :disabled="busy" @click="selected = null">返回共享列表</button></div>
        <SharedWorkspaceManifest :release="selected" :books="books" />
        <p v-if="selected.archived" role="status">此共享工作区已归档，暂不接受新的使用。</p>
        <form class="shared-goal" @submit.prevent="start">
          <label>确认你的学习目标<textarea v-model="goal" required :disabled="busy || selected.archived" /></label>
          <label><input v-model="sprintEnabled" type="checkbox" :disabled="busy" />用于考试冲刺</label>
          <template v-if="sprintEnabled">
            <label>考试日期<input v-model="examDate" type="date" required :disabled="busy" /></label>
            <label>已知考试范围<textarea v-model="syllabus" :disabled="busy" placeholder="老师划定的章节、题型或考纲，暂无可留空。" /></label>
            <label>每天可用分钟数（选填）<input v-model.number="minutes" type="number" min="1" max="1440" :disabled="busy" /></label>
            <fieldset :disabled="busy"><legend>从哪里开始</legend><label><input v-model="entryMode" type="radio" value="diagnostic" />先做短诊断</label><label><input v-model="entryMode" type="radio" value="question" />直接提问，跳过诊断</label></fieldset>
            <label v-if="entryMode === 'question'">第一道问题（选填）<textarea v-model="question" :disabled="busy" placeholder="可以现在输入，或进入阅读后选中原文提问。" /></label>
          </template>
          <p>可以接受上面的目标，也可以改成自己的计划。开始后会建立你的个人工作区，学习记录分别保存。</p>
          <button :disabled="busy || !goal.trim() || selected.archived || selected.availability.status !== 'ready'">{{ busy ? '正在建立个人工作区…' : '确认目标并开始学习' }}</button>
        </form>
      </article>
    </template>
    <template v-else-if="page && !loading">
      <div class="shared-list"><article v-for="release in page.items" :key="`${release.release_id}:${release.version}`" class="shared-card"><div class="shared-card-covers" aria-hidden="true"><div v-for="material in release.manifest.materials.slice(0, 3)" :key="sharedBookKey(material.published_book_ref)" :class="{ 'is-video': materialBook(material.published_book_ref)?.media_kind === 'video' }"><MaterialCover :title="sharedBookLabel(material.published_book_ref, books, release.manifest.materials)" :cover="materialBook(material.published_book_ref)?.cover" :video="materialBook(material.published_book_ref)?.media_kind === 'video'" :music="materialBook(material.published_book_ref)?.media_kind === 'music'" compact /></div></div><div><h3>{{ release.manifest.name }}</h3><small>{{ release.manifest.publisher }} · 第 {{ release.version }} 版</small></div><p>{{ release.manifest.public_goal }}</p><p class="status">{{ release.archived ? '已归档' : release.availability.status === 'ready' ? '资料已就绪' : '尚未完整就绪' }} · {{ release.manifest.materials.length }} 份资料</p><button :disabled="busy" @click="select(release)">查看目标与路线</button></article></div>
      <p v-if="!page.items.length">暂无可用的共享工作区。</p>
      <nav v-if="page.total > page.limit" aria-label="共享工作区分页"><button :disabled="busy || !page.offset" @click="load(Math.max(0, page.offset - page.limit))">上一页共享工作区</button><span>{{ page.offset + 1 }}–{{ page.offset + page.items.length }} / {{ page.total }}</span><button :disabled="busy || page.offset + page.limit >= page.total" @click="load(page.offset + page.limit)">下一页共享工作区</button></nav>
    </template>
  </section>
</template>

<style scoped>
.shared-card-covers { display: flex; align-items: center; gap: 12px; margin: -18px -18px 18px; padding: 22px; border-radius: 11px 11px 0 0; background: #ede9df; min-height: 130px; }.shared-card-covers > div { width: 58px; height: 87px; }.shared-card-covers > .is-video { width: 142px; height: 87px; }
.shared-catalog { margin: 20px 0; padding: 22px; border: 1px solid var(--line); border-radius: 16px; background: var(--canvas); }.heading { display: flex; gap: 12px; align-items: center; justify-content: space-between; }.heading>div { min-width: 0; } h2,h3 { margin: 0; font-weight: 500; overflow-wrap: anywhere; }h2 { font-size: 20px; }h3 { font-size: 17px; } p { line-height: 1.75; overflow-wrap: anywhere; } .heading p,small,.status,.shared-goal p { color: var(--muted); font-size: 13px; }button { font: inherit; border: 1px solid var(--line); border-radius: 8px; color: var(--ink); background: var(--canvas); padding: 9px 12px; min-height: 40px; cursor: pointer; }button:disabled { opacity: .5; cursor: default; }.shared-list { display: grid; grid-template-columns: repeat(auto-fit,minmax(220px,1fr)); gap: 14px; margin-top: 12px; }.shared-card { border: 1px solid var(--line); border-radius: 12px; padding: 18px; min-width: 0; }.shared-goal { margin-top: 22px; padding: 18px; border-radius: 12px; background: var(--surface-soft); }.shared-goal label { display: block; line-height: 1.8; }.shared-goal textarea { display: block; box-sizing: border-box; width: 100%; min-height: 90px; margin-top: 6px; border: 1px solid var(--line); border-radius: 8px; background: var(--canvas); color: var(--ink); padding: 10px; font: inherit; }.shared-goal button { background: var(--brand-accent); color: var(--brand-on-accent); }nav { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 18px; }@media(max-width:540px){.shared-catalog { padding: 15px; }.heading { align-items: flex-start; flex-direction: column; }.shared-list { grid-template-columns: minmax(0,1fr); }.shared-goal { padding: 14px; }.shared-goal textarea { font-size: 16px; } }
</style>
