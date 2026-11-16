<script setup lang="ts">
import { t as uiText } from '../i18n';
import type { CourseSupportVisit } from '../generated/CourseSupportVisit';
defineProps<{ visit:CourseSupportVisit; disabled?:boolean }>();
defineEmits<{ return:[] }>();
</script>
<template>
  <section class="support-branch" :aria-label="uiText('短补学')">
    <div class="route"><span>{{ uiText('原问题') }}</span><span aria-hidden="true">→</span><strong>{{ uiText('补这一点') }}</strong><span aria-hidden="true">→</span><span>{{ uiText('回到原问题') }}</span></div>
    <h4>{{visit.branch.focus}}</h4><p class="support-purpose">{{ uiText('补清这个关系后，回到刚才的问题继续应用。') }}</p><p class="explanation">{{visit.branch.explanation}}</p>
    <aside><span>{{ uiText('用一个小例子看') }}</span><p>{{visit.branch.example}}</p></aside>
    <div class="return-to"><span>{{ uiText('带回原问题') }}</span><p>{{visit.branch.return_prompt}}</p><details><summary>{{ uiText('你刚才的表达') }}</summary><blockquote>{{visit.original_response}}</blockquote></details></div>
    <button class="primary" :disabled="disabled" @click="$emit('return')">{{ uiText('回到原问题，接着想 →') }}</button><small>{{ uiText('原题和已写的内容都还在。') }}</small>
  </section>
</template>
<style scoped>
.support-purpose{color:var(--muted);font-size:12px;margin-top:-6px}
.support-branch{max-width:680px;margin:26px auto;padding:30px;background:var(--surface);border:1px solid var(--line);border-radius:14px;font:14px/1.9 var(--sans);color:var(--ink)}.route{display:flex;gap:10px;flex-wrap:wrap;font-size:11px;color:var(--steel)}.route strong{color:var(--brand-accent);font-weight:500}h4{font:500 26px/1.5 var(--font-display, var(--sans));margin:22px 0 16px}p{white-space:pre-line;margin:10px 0 0}aside{padding:18px 20px;background:var(--surface-soft);border-radius:10px;margin:24px 0}aside>span,.return-to>span{font-size:11px;color:var(--brand-accent)}.return-to{padding:0 0 24px}details{margin:15px 0 0;color:var(--steel);font-size:12px}summary{cursor:pointer}blockquote{margin:12px 0;padding-left:14px;border-left:2px solid var(--line);white-space:pre-line}button{min-height:46px;border:0;border-radius:8px;background:var(--brand-accent);color:var(--brand-on-accent);font:500 13px/1.5 var(--sans);padding:12px 18px;cursor:pointer}button:disabled{opacity:.5;cursor:default}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}small{display:block;color:var(--steel);font-size:11px;margin-top:10px}@media(max-width:600px){.support-branch{padding:20px 16px;margin:18px 0}h4{font-size:23px}}
</style>
