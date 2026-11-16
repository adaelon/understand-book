<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { t } from '../i18n';
import type { CourseExpression } from '../generated/CourseExpression';
const props=defineProps<{ expressions:CourseExpression[]; resolved:string[]; disabled?:boolean }>();
defineEmits<{ ask:[response:string]; revisit:[job:string]; resolve:[fact:string,resolved:boolean] }>();
const open=ref(false),draft=ref(''),context=ref(''),uncertain=ref(''),test=ref(''),scaffold=ref(false),input=ref<HTMLTextAreaElement>();
const questions=computed(()=>props.expressions.filter(e=>e.intent==='question'));
const pending=computed(()=>questions.value.filter(e=>!props.resolved.includes(e.fact_ref)).length);
function latest(job:string):string {let next=props.expressions.find(e=>e.reply_to===job);return next ? latest(next.job_ref) : job;}
function assemble(){draft.value=[context.value.trim() && t('我在看{0}。',[context.value.trim()]),uncertain.value.trim() && t('我不明白：{0}',[uncertain.value.trim()]),test.value.trim() && t('我想弄清：{0}',[test.value.trim()])].filter(Boolean).join('\n');scaffold.value=false;void focus();}
async function focus(){open.value=true;await nextTick();input.value?.focus();}
function clear(){draft.value='';context.value='';uncertain.value='';test.value='';}
defineExpose({clear,focus});
</script>
<template>
  <section class="course-questions" :aria-label="t('我的问题')">
    <header><div><strong>{{t('我的问题')}}</strong><small>{{questions.length ? t('{0} 个还在探索',[pending]) : t('疑问也是继续学习的起点。')}}</small></div><button :disabled="disabled" @click="focus">{{t('＋ 记下一个疑问')}}</button></header>
    <form v-if="open" @submit.prevent="$emit('ask',draft)">
      <label for="course-question">{{t('你想弄明白什么？')}}</label><textarea id="course-question" ref="input" v-model="draft" rows="3" :disabled="disabled" :placeholder="t('一句话也可以，从眼前不明白的地方说起。')"/>
      <button type="button" class="text" :disabled="disabled" @click="scaffold=!scaffold">{{t(scaffold?'收起提示':'不知道怎么问？试着补三句话')}}</button>
      <div v-if="scaffold" class="scaffold"><label>{{t('我在看什么')}}<input v-model="context" :placeholder="t('某段解释、某组数据或某个现象')" :disabled="disabled"/></label><label>{{t('哪一步对不上')}}<input v-model="uncertain" :placeholder="t('预期与实际有什么不同')" :disabled="disabled"/></label><label>{{t('我想弄清什么')}}<input v-model="test" :placeholder="t('原因、条件，或者改一个量会怎样')" :disabled="disabled"/></label><button type="button" :disabled="disabled || ![context,uncertain,test].some(v=>v.trim())" @click="assemble">{{t('放进问题框，再改一改')}}</button></div>
      <div class="actions"><button class="primary" :disabled="disabled || !draft.trim()">{{t('带着这个问题探索 →')}}</button><button type="button" class="text" @click="open=false">{{t('收起，保留草稿')}}</button></div>
    </form>
    <ol v-if="questions.length"><li v-for="question in questions" :key="question.fact_ref"><span :class="{resolved:resolved.includes(question.fact_ref)}">{{t(resolved.includes(question.fact_ref)?'我已想明白':'还在探索')}}</span><p>{{question.response}}</p><div class="actions"><button class="text" :disabled="disabled" @click="$emit('revisit',latest(question.job_ref))">{{t('接着这个问题')}}</button><button class="text" :disabled="disabled" @click="$emit('resolve',question.fact_ref,!resolved.includes(question.fact_ref))">{{t(resolved.includes(question.fact_ref)?'还有疑问':'这点我想明白了')}}</button></div></li></ol>
  </section>
</template>
<style scoped>
.course-questions{margin:26px 0;border-top:1px solid var(--line);padding-top:22px;font:13px/1.8 var(--sans);color:var(--ink)}header{display:flex;gap:14px;align-items:center;justify-content:space-between}header strong{font:500 19px/1.5 var(--font-display, var(--sans))}header small{display:block;color:var(--steel);font-size:11px;margin-top:4px}form{margin-top:18px}label{display:block;font-size:12px}textarea,input{box-sizing:border-box;width:100%;display:block;margin:7px 0 10px;border:1px solid var(--line);background:var(--canvas);color:var(--ink);border-radius:8px;padding:10px 12px;font:inherit;resize:vertical}.actions{display:flex;flex-wrap:wrap;gap:8px;margin-top:10px}button{min-height:42px;padding:8px 12px;font:inherit;font-size:12px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.5;cursor:default}.text{background:transparent;border:0;padding-inline:0;margin-right:16px;color:var(--steel)}.primary{background:var(--brand-accent);border-color:var(--brand-accent);color:var(--brand-on-accent)}.scaffold{padding:15px;background:var(--surface-soft);border-radius:8px;margin:12px 0}.scaffold label+label{margin-top:12px}ol{list-style:none;padding:0;margin:20px 0 0}li{border-top:1px solid var(--line);padding:17px 0}li>span{font-size:10px;color:var(--brand-accent)}li>span.resolved{color:var(--steel)}li p{margin:5px 0 0;white-space:pre-line;overflow-wrap:anywhere}button:focus-visible,textarea:focus-visible,input:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@media(max-width:480px){header{align-items:flex-start;flex-direction:column}}
</style>
