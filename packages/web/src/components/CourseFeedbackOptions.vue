<script setup lang="ts">
import type { CourseFeedback } from '../generated/CourseFeedback';
import { t } from '../i18n';
defineProps<{ feedback:CourseFeedback; disabled?:boolean }>();
defineEmits<{ support:[]; question:[value:string] }>();
</script>
<template>
  <div class="feedback-options">
    <button v-if="feedback.support_branch" class="support-link" :disabled="disabled" @click="$emit('support')"><span>{{t('先补这一点')}}</span><strong>{{feedback.support_branch.focus}}</strong><span aria-hidden="true">↗</span></button>
    <aside v-if="feedback.question_guide" class="question-guide">
      <span class="eyebrow">{{t('让这个问题更容易探究 · 建议问法')}}</span>
      <p>{{feedback.question_guide.suggested_question}}</p>
      <small>{{feedback.question_guide.why}}</small>
      <div class="try-next"><strong>{{t('可以怎样找答案')}}</strong><p>{{feedback.question_guide.try_next}}</p></div>
      <button :disabled="disabled" @click="$emit('question',feedback.question_guide.suggested_question)">{{t('用这个问法继续，可以修改 →')}}</button>
    </aside>
  </div>
</template>
<style scoped>
.feedback-options:empty{display:none}.support-link{display:flex;width:100%;align-items:center;gap:12px;margin:18px 0;text-align:left;padding:14px 16px;background:var(--surface-soft)}.support-link>span:first-child{font-size:11px;color:var(--muted)}.support-link strong{flex:1;font-weight:500;line-height:1.7}.question-guide{border-top:1px solid var(--line);padding-top:18px;margin:18px 0;font:13px/1.8 var(--sans);color:var(--ink)}.eyebrow{font-size:11px;color:var(--brand-accent)}p{margin:8px 0;white-space:pre-line}.question-guide small{color:var(--muted);font-size:12px}.try-next{margin:14px 0}.try-next strong{font-size:11px;font-weight:500}.try-next p{font-size:12px;margin:4px 0}button{font:inherit;font-size:12px;min-height:44px;padding:10px 12px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.5;cursor:default}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}
</style>
