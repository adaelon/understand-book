<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import VoiceInput from './VoiceInput.vue';
import { api as sharedApi, type QuestionAttempt, type QuestionStep, type TutorActivity } from '../api';
import { getLocale, t } from '../i18n';
import { renderMarkdown } from '../md';
import { bindSceneApi, network, readerPreferenceOwner } from '../network-context';

const props = defineProps<{ activity: TutorActivity; busy: boolean; ensureDisplayed: () => Promise<void> }>();
const emit = defineEmits<{ change: [answer: string, reasoning: string]; reload: []; asking: [busy: boolean] }>();
const api = bindSceneApi(sharedApi);
const input = computed(() => props.activity.move.answer_input);
const choices = computed(() => input.value && 'options' in input.value ? input.value.options : []);
const worked = computed(() => input.value?.kind === 'worked_solution');
const selected = ref<string[]>([]), text = ref('');
const editor = ref<HTMLTextAreaElement>();
const preview = ref(false), selection = ref(''), asking = ref(false), error = ref('');
const voiceActive = ref(false);
const posted = ref<QuestionStep[]>([]);
const steps = computed(() => {
  const exchanges = new Map((props.activity.steps ?? []).map(step => [step.step_ref, step]));
  for (const step of posted.value) if (!exchanges.get(step.step_ref)?.reply) exchanges.set(step.step_ref, step);
  return [...exchanges.values()];
});
const locked = computed(() => props.busy || asking.value || voiceActive.value || props.activity.status !== 'active');
const latest = computed(() => props.activity.attempts?.at(-1));
const draftKey = computed(() => {
  const owner = readerPreferenceOwner(), w = network.value.workspace;
  return owner ? JSON.stringify(['question-draft', owner, w?.learning_workspace_id ?? w?.workspace_id ?? 'local', props.activity.delivery_ref]) : null;
});
function optionIds(answer: string) {
  try { const ids: unknown = JSON.parse(answer); if (Array.isArray(ids)) return ids.filter((id): id is string => typeof id === 'string'); } catch { /* A single choice is its token. */ }
  return answer ? [answer] : [];
}
function submittedText(attempt: QuestionAttempt) {
  if (!choices.value.length) return attempt.answer;
  const ids = optionIds(attempt.answer);
  return choices.value.filter(o => ids.includes(o.id)).map(o => o.id + ' · ' + o.text).join('\n');
}
function notifyParent() {
  const answer = worked.value ? text.value : selected.value.length === 1 && input.value?.kind === 'single_choice'
    ? selected.value[0] : selected.value.length ? JSON.stringify(selected.value) : '';
  emit('change', answer, text.value);
}
function save() {
  if (draftKey.value) {
    try { localStorage.setItem(draftKey.value, JSON.stringify({ selected: selected.value, text: text.value, base: latest.value?.action_ref })); } catch { /* Keep the live editor usable. */ }
  }
  notifyParent();
}
async function insertVoice(value: string) {
  text.value += (text.value && !text.value.endsWith('\n') ? '\n' : '') + value;
  selection.value = ''; save();
  await nextTick(); editor.value?.focus(); editor.value?.setSelectionRange(text.value.length, text.value.length);
}
watch(draftKey, () => {
  const attempt = latest.value;
  selected.value = attempt && choices.value.length ? optionIds(attempt.answer) : [];
  text.value = (worked.value ? attempt?.answer : attempt?.reasoning) ?? '';
  try {
    const draft = JSON.parse(localStorage.getItem(draftKey.value ?? '') ?? 'null');
    if (draft && draft.base === attempt?.action_ref && typeof draft.text === 'string' && Array.isArray(draft.selected)) {
      text.value = draft.text; selected.value = draft.selected;
    }
  } catch { /* The last submitted version still opens. */ }
  selection.value = ''; posted.value = []; preview.value = false; error.value = ''; notifyParent();
}, { immediate: true });
watch(() => latest.value?.action_ref, () => save());
function choose(id: string) {
  selected.value = input.value?.kind === 'single_choice' ? [id] : choices.value
    .filter(o => o.id === id ? !selected.value.includes(id) : selected.value.includes(o.id)).map(o => o.id);
  save();
}
function readSelection() {
  const el = editor.value;
  selection.value = el && worked.value ? el.value.slice(el.selectionStart, el.selectionEnd).trim() : '';
}
let retryRequest: Parameters<typeof api.tutorAskStep>[0] | undefined;
async function ask(previous?: QuestionStep) {
  if (locked.value) return;
  const current = { text: previous?.text ?? (selection.value || text.value.trim()),
    selected: [...(previous?.selected ?? selected.value)] };
  if (!current.text) return;
  const id = props.activity.delivery_ref;
  const retry = retryRequest && retryRequest.delivery_ref === id && retryRequest.text === current.text &&
    JSON.stringify(retryRequest.selected) === JSON.stringify(current.selected);
  const request = { operation_id: previous?.step_ref.replace(/^step:/, '') ?? (retry ? retryRequest!.operation_id : crypto.randomUUID()),
    delivery_ref: id, locale: getLocale(), ...current };
  asking.value = true; emit('asking', true); error.value = ''; retryRequest = request;
  try {
    await props.ensureDisplayed();
    const result = await api.tutorAskStep(request);
    if (id !== props.activity.delivery_ref) return;
    posted.value.push({ step_ref: 'step:' + request.operation_id, ...current, reply: result.text, help_ref: result.help_ref });
    retryRequest = undefined; emit('reload');
  } catch (failure) {
    error.value = t('这一步暂时没有收到引导，可以重试。{0}', [failure instanceof Error ? failure.message : '']);
  } finally { asking.value = false; emit('asking', false); }
}

</script>
<template>
  <div class="question-work">
    <fieldset v-if="choices.length" class="options">
      <legend>{{ input?.kind === 'single_choice' ? t('单选题') : t('多选题') }}</legend>
      <label v-for="option in choices" :key="option.id" :class="{ picked: selected.includes(option.id) }">
        <input :type="input?.kind === 'single_choice' ? 'radio' : 'checkbox'" :name="activity.delivery_ref" :checked="selected.includes(option.id)" :disabled="locked" @change="choose(option.id)" />
        <span class="option-id">{{ option.id }}</span><span class="md option-text" v-html="renderMarkdown(option.text)"></span>
      </label>
    </fieldset>
    <div class="editor-title"><label :for="'thought-' + activity.delivery_ref">{{ worked ? t('解题过程与答案') : t('我的思路') }}</label>
      <button v-if="worked && text.trim()" type="button" class="link" :aria-expanded="preview" @click="preview = !preview">{{ preview ? t('收起预览') : t('预览公式') }}</button>
    </div>
    <textarea :id="'thought-' + activity.delivery_ref" ref="editor" v-model="text" data-workspace-input="tutor" :rows="worked ? 5 : 2"
      :disabled="activity.status !== 'active'" :placeholder="worked ? t('写下推导和结果，也可以选中一行问助教。') : t('可以先选答案，也可以写下你犹豫的地方。')"
      @input="readSelection(); save()" @select="readSelection" @keyup="readSelection" @pointerup="readSelection" />
    <div v-if="preview" class="preview md" v-html="renderMarkdown(text)"></div>
    <VoiceInput :scope="draftKey ?? activity.delivery_ref" :disabled="props.busy || asking || activity.status !== 'active'" @insert="insertVoice" @active="voiceActive = $event; emit('asking', $event)" />
    <div class="ask-row"><button type="button" class="ask" :disabled="locked || !text.trim()" @click="ask()">{{ asking ? t('助教正在看这一步…') : selection ? t('问选中的这一步') : t('问这一步') }}</button></div>
    <p v-if="error" role="alert" class="error">{{ error }}</p>
    <div v-if="steps.length" class="thread" :aria-label="t('题内讨论')" aria-live="polite">
      <div v-for="step in steps" :key="step.step_ref" class="exchange">
        <div class="student"><span>{{ t('我的思路') }}<template v-if="step.selected.length"> · {{ t('当时选择了 {0}', [step.selected.join('、')]) }}</template></span><div class="md" v-html="renderMarkdown(step.text)"></div></div>
        <div v-if="step.reply" class="coach" :data-step-help="step.help_ref"><span>{{ t('助教') }}</span><div class="md" v-html="renderMarkdown(step.reply)"></div></div>
        <button v-else type="button" class="link" :disabled="locked" @click="ask(step)">{{ t('重新获取这一步的引导') }}</button>
      </div>
    </div>
    <details v-if="activity.attempts?.length && input" class="attempts">
      <summary>{{ t('已提交的答案（{0}）', [activity.attempts.length]) }}</summary>
      <div v-for="attempt in activity.attempts" :key="attempt.action_ref"><b>{{ t('第 {0} 次作答', [attempt.number]) }}</b><div class="md" v-html="renderMarkdown(submittedText(attempt))"></div></div>
    </details>
  </div>
</template>
<style scoped>
.question-work { display: grid; gap: .65rem; margin: 1rem 0; min-width: 0; }
.options { display: grid; gap: .45rem; padding: 0; margin: 0 0 .5rem; border: 0; min-width: 0; }
legend,.editor-title label { font-size: .85rem; font-weight: 650; color: var(--ink); }
legend { margin-bottom: .5rem; }
.options label { display: flex; gap: .65rem; align-items: baseline; background: var(--canvas); border: 1px solid var(--line); border-radius: 9px; padding: .65rem .8rem; cursor: pointer; min-height: 44px; box-sizing: border-box; }
.options label.picked { border-color: var(--brand-green); background: color-mix(in srgb,var(--brand-green) 8%,var(--canvas)); }
.options input { accent-color: var(--brand-green-deep); margin: 0; flex: none; }
.option-id { color: var(--brand-green-deep); font-weight: 650; }
.option-text,.md { min-width: 0; overflow-wrap: anywhere; line-height: 1.6; }
.question-work .md :deep(p) { margin: 0; }
.editor-title,.ask-row { display: flex; justify-content: space-between; align-items: center; }
textarea { width: 100%; box-sizing: border-box; padding: .7rem .8rem; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); color: var(--ink); font: inherit; font-size: 16px; line-height: 1.65; resize: vertical; }
textarea:focus-visible,button:focus-visible,label:focus-within { outline: 2px solid var(--brand-green); outline-offset: 2px; }
button { font: inherit; cursor: pointer; } button:disabled { opacity: .5; cursor: default; }
.ask { padding: .43rem .85rem; border: 1px solid var(--line); border-radius: 8px; color: var(--brand-green-deep); background: var(--canvas); font-size: .84rem; min-height: 40px; }
.link { padding: .25rem 0; background: transparent; border: 0; color: var(--brand-green-deep); font-size: .8rem; }
.preview,.thread { padding: .75rem .9rem; border: 1px solid var(--line); border-radius: 9px; background: var(--canvas); min-width: 0; }
.exchange + .exchange { margin-top: .8rem; border-top: 1px solid var(--line); padding-top: .8rem; }
.student,.coach { display: grid; gap: .2rem; }
.student>span,.coach>span,.attempts b { font-size: .75rem; font-weight: 600; color: var(--steel); }
.coach { margin-top: .6rem; border-left: 2px solid var(--brand-green); padding-left: .7rem; } .coach>span { color: var(--brand-green-deep); }
.attempts { font-size: .83rem; color: var(--slate); } .attempts summary { cursor: pointer; padding-block: .25rem; } .attempts>div { margin-top: .5rem; }
.error { color: var(--reader-coral); font-size: .8rem; margin: 0; }
.md :deep(.katex-display) { overflow-x: auto; }
</style>
