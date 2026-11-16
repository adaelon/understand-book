<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { t as uiText } from '../i18n';
import { renderMarkdown } from '../md';
import type { CourseExperiment } from '../generated/CourseExperiment';
import type { CourseMathState } from '../generated/CourseMathState';
import CourseMathExperiment from './CourseMathExperiment.vue';
const props = defineProps<{ activity: Extract<CourseExperiment, {kind:'inquiry'}>; response: string | null; visualization?: CourseMathState | null; draftKey?: string; disabled: boolean; readonly?: boolean }>();
const emit = defineEmits<{ save: [response:string, visualization:CourseMathState|null]; dirty: [value:boolean] }>();
const draft = ref(props.response ?? '');
const state = ref<CourseMathState | null>(props.visualization ?? props.activity.visualization ?? null);
const storageKey = props.draftKey ? `course-investigation:${props.draftKey}` : null;
if(storageKey && !props.readonly){try{const saved=JSON.parse(localStorage.getItem(storageKey) ?? 'null');if(saved){draft.value=saved.response; if(saved.visualization?.kind===state.value?.kind)state.value=saved.visualization;}}catch{/* Ignore unavailable draft storage. */}}
watch(() => props.response, value => { if(value !== null) draft.value = value; });
watch(() => props.visualization, (value, previous) => { if(value && JSON.stringify(value)!==JSON.stringify(previous))state.value=value; });
const dirty=computed(()=>draft.value !== (props.response ?? '') || JSON.stringify(state.value)!==JSON.stringify(props.visualization ?? props.activity.visualization ?? null));
watch(dirty,value=>emit('dirty',value),{immediate:true});
watch([draft,state],()=>{if(storageKey){try{localStorage.setItem(storageKey,JSON.stringify({response:draft.value,visualization:state.value}));}catch{/* Saved submissions remain in LearningStore. */}}},{deep:true});
const labels = {reading:'阅读取证',reasoning:'推理论证',practice:'动手练习'};
function save() { emit('save', draft.value, state.value ? {...state.value} : null); }
</script>
<template>
  <section class="inquiry-activity" :aria-label="uiText(labels[activity.activity])">
    <span class="activity-kind">{{ uiText(labels[activity.activity]) }}</span>
    <h4>{{ activity.instruction }}</h4>
    <div class="activity-material prose" v-html="renderMarkdown(activity.material)" />
    <CourseMathExperiment v-if="state" v-model="state" :disabled="disabled || readonly" />
    <form @submit.prevent="save">
      <label for="course-investigation">{{ activity.response_prompt }}</label>
      <textarea id="course-investigation" v-model="draft" rows="6" :disabled="disabled || readonly" :placeholder="uiText('写下找到的依据、推理过程或练习结果。')" />
      <div class="activity-actions"><button v-if="!readonly" class="primary" :disabled="disabled || !draft.trim() || (!dirty && !!response)">{{ uiText('保存我的探究结果') }}</button><span v-if="response && !dirty" role="status">{{ uiText('探究结果已保存') }}</span><span v-else-if="state">{{ uiText('保存时会一并记录图形参数。') }}</span></div>
    </form>
  </section>
</template>
<style scoped>
.inquiry-activity{display:grid;gap:16px;min-width:0}.activity-kind{font-size:12px;letter-spacing:.08em;color:var(--text-muted,#667085)}h4{font-size:20px;line-height:1.5;margin:0}.activity-material{padding:18px;border:1px solid var(--border,#ddd);border-radius:12px;overflow-wrap:anywhere;overflow-x:auto;line-height:1.8}.activity-material :deep(table){border-collapse:collapse;width:100%}.activity-material :deep(td),.activity-material :deep(th){border:1px solid #ddd;padding:8px}.activity-material :deep(pre){overflow:auto}form{display:grid;gap:12px}label{font-weight:600;line-height:1.6}textarea{box-sizing:border-box;width:100%;resize:vertical;min-height:130px;padding:14px;font:inherit;line-height:1.7;border:1px solid var(--border,#ccc);border-radius:10px;background:var(--surface,#fff);color:inherit}.activity-actions{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.activity-actions span{font-size:13px;color:var(--text-muted,#667085)}button{padding:10px 16px;border-radius:8px;cursor:pointer}button:disabled{opacity:.55;cursor:default}
</style>
