<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { ApiError } from '../api';
import { network, type LearningWorkspace } from '../network-context';
import { daysUntilExam, examSprints, sprintRequest, type ExamSprint, type ExamSprintInput } from '../exam-sprints';

const props = defineProps<{ workspace: LearningWorkspace }>();
const emit = defineEmits<{ updated: [workspace: LearningWorkspace]; begin: [message: string] }>();
const sprint = ref<ExamSprint | null>(null), editing = ref(false), busy = ref(false), error = ref('');
const examDate = ref(''), syllabus = ref(''), goalText = ref(''), minutes = ref<number | null>(null), mode = ref<'diagnostic' | 'question'>('diagnostic');
const question = ref(''), clock = ref(Date.now());
let ticket = 0;
const timer = setInterval(() => { clock.value = Date.now(); }, 30_000);
const remaining = computed(() => sprint.value ? daysUntilExam(sprint.value.settings, clock.value) : null);
const remainingLabel = computed(() => remaining.value === null ? '' : remaining.value > 0 ? uiText('距离考试还有 {0} 天', [remaining.value]) : remaining.value === 0 ? uiText('今天考试') : uiText('考试日期已过'));
function draft() {
  const saved = sprint.value?.settings;
  examDate.value = saved?.exam_date ?? ''; syllabus.value = saved?.syllabus ?? '';
  minutes.value = saved?.daily_minutes ?? null; mode.value = saved?.entry_mode ?? 'diagnostic';
  goalText.value = props.workspace.goal.text; editing.value = true;
}
function fail(failure: unknown, id: number) {
  if (id === ticket && !(failure instanceof ApiError && failure.errorCode === 'CLIENT_CONTEXT_STALE')) error.value = failure instanceof Error ? failure.message : uiText('考试安排暂时无法读取');
}
async function load() {
  const id = ++ticket; error.value = '';
  try { const result = await examSprints.read(props.workspace.learning_workspace_id); if (id === ticket) { sprint.value = result.sprint; mode.value = result.sprint?.settings.entry_mode ?? 'diagnostic'; } }
  catch (failure) { fail(failure, id); }
}
async function save() {
  if (busy.value || !examDate.value || !goalText.value.trim()) return;
  const id = ticket; busy.value = true; error.value = '';
  const settings: ExamSprintInput = { exam_date: examDate.value, syllabus: syllabus.value, daily_minutes: minutes.value || null,
    utc_offset_minutes: -new Date().getTimezoneOffset(), entry_mode: mode.value };
  try {
    const result = await examSprints.save(props.workspace, goalText.value, settings);
    if (id === ticket) { sprint.value = result.sprint; editing.value = false; emit('updated', result.workspace); window.dispatchEvent(new Event('tutor-state-changed')); }
  } catch (failure) { fail(failure, id); }
  finally { if (id === ticket) busy.value = false; }
}
function begin() {
  const message = sprintRequest(mode.value, question.value);
  if (message) emit('begin', message);
}
watch(() => `${network.value.epoch}:${props.workspace.learning_workspace_id}`, () => { sprint.value = null; editing.value = false; busy.value = false; question.value = ''; void load(); }, { immediate: true });
onBeforeUnmount(() => { ticket++; clearInterval(timer); });
</script>

<template>
  <section class="exam-panel" :aria-label="uiText('考试冲刺安排')">
    <div class="heading"><h3>{{ uiText('考试冲刺') }}</h3><button :disabled="busy || workspace.archived" @click="draft">{{ sprint ? uiText('调整考试安排') : uiText('设置考试安排') }}</button></div>
    <p v-if="error" role="alert">{{ error }} <button @click="load">{{ uiText('重新读取') }}</button></p>
    <template v-if="sprint && !editing">
      <strong>{{ remainingLabel }}</strong><p>{{ uiText('考试日期：') }}{{ sprint.settings.exam_date }}<span v-if="sprint.settings.daily_minutes"> {{ uiText('· 每天') }} {{ sprint.settings.daily_minutes }} {{ uiText('分钟') }}</span></p>
      <p>{{ sprint.settings.syllabus || uiText('考试范围暂未提供，先依据课程注明的范围开始。') }}</p>
      <p v-if="sprint.goal_revision !== workspace.goal.revision" class="hint">{{ uiText('学习目标已更新；继续时会按最新目标安排，原作答和记录保留。') }}</p>
      <p class="hint">{{ uiText('今天的安排将依据当前目标、剩余时间和已有作答继续。你可以随时直接提出问题。') }}</p>
      <fieldset :disabled="busy || workspace.archived"><legend>{{ uiText('今天从哪里开始') }}</legend><label><input v-model="mode" type="radio" value="diagnostic" />{{ uiText('先做短诊断') }}</label><label><input v-model="mode" type="radio" value="question" />{{ uiText('直接讲这题') }}</label></fieldset>
      <label v-if="mode === 'question'">{{ uiText('你的问题') }}<textarea v-model="question" :placeholder="uiText('输入不会的题目或步骤，也可以进入阅读后选中原文提问。')" /></label>
      <button :disabled="workspace.archived || !workspace.members.some(member => member.readable) || (mode === 'question' && !question.trim())" @click="begin">{{ mode === 'diagnostic' ? uiText('打开今日诊断') : uiText('打开这道题') }}</button>
    </template>
    <form v-if="editing" @submit.prevent="save">
      <label>{{ uiText('考试日期') }}<input v-model="examDate" type="date" required /></label>
      <label>{{ uiText('考试范围') }}<textarea v-model="syllabus" :placeholder="uiText('可以填写老师划定的章节、题型或考纲；暂时没有也可开始。')" /></label>
      <label>{{ uiText('这次复习目标') }}<textarea v-model="goalText" required /></label>
      <label>{{ uiText('每天可用分钟数（选填）') }}<input v-model.number="minutes" type="number" min="1" max="1440" /></label>
      <fieldset><legend>{{ uiText('开始方式') }}</legend><label><input v-model="mode" type="radio" value="diagnostic" />{{ uiText('先做短诊断') }}</label><label><input v-model="mode" type="radio" value="question" />{{ uiText('直接提问，跳过诊断') }}</label></fieldset>
      <p class="hint">{{ uiText('保存会更新目标版本，后续安排按新日期和范围生成，已有学习记录继续保留。') }}</p>
      <div class="actions"><button :disabled="busy">{{ uiText('保存考试安排') }}</button><button type="button" :disabled="busy" @click="editing = false">{{ uiText('取消') }}</button></div>
    </form>
    <p v-if="!sprint && !editing" class="hint">{{ uiText('填写考试日期和已知范围，让学习安排贴近剩余时间。') }}</p>
  </section>
</template>

<style scoped>
.exam-panel{margin:18px 0;padding:18px;border:1px solid var(--line);border-radius:12px;background:var(--surface-soft)}.heading,.actions{display:flex;align-items:center;justify-content:space-between;gap:12px}h3{margin:0;font-size:18px}p,.hint{line-height:1.7;font-size:13px}.hint{color:var(--muted)}label{display:block;margin:12px 0;line-height:1.8}textarea,input[type=date],input[type=number]{display:block;box-sizing:border-box;width:100%;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);padding:9px;font:inherit}textarea{min-height:76px}fieldset{border:0;padding:0;margin:12px 0}fieldset label{display:inline-flex;gap:6px;margin-right:18px}button{min-height:40px;padding:9px 12px;font:inherit;background:var(--canvas);color:var(--ink);border:1px solid var(--line);border-radius:8px;cursor:pointer}button:disabled{opacity:.5;cursor:default}.actions{justify-content:flex-start}@media(max-width:540px){.heading{align-items:flex-start;flex-direction:column}textarea,input{font-size:16px}}
</style>
