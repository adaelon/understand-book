<script setup lang="ts">
import { t as uiText } from '../i18n';
import type { CourseReview } from '../generated/CourseReview';
import type { CourseExpression } from '../generated/CourseExpression';
defineProps<{ reviews: CourseReview[]; expressions: CourseExpression[]; disabled: boolean }>();
const emit = defineEmits<{ retry: [job: string]; read: [job: string]; understanding: []; continue: [job: string] }>();
const activity = (value: string) => uiText(({prediction:'我的预测',reflection:'实验后的解释',task:'升级方案',revision:'后来的修正'} as Record<string,string>)[value] ?? '我的表达');
</script>
<template>
  <section class="reviews" :aria-label="uiText('针对我的反馈')">
    <header><div><span>{{ uiText('从原话到理解') }}</span><h4>{{ uiText('针对我的反馈') }}</h4></div><button v-if="reviews.some(r => r.understanding.length)" :disabled="disabled" @click="emit('understanding')">{{ uiText('打开我的理解 →') }}</button></header>
    <p v-if="!reviews.length" class="muted" role="status">{{ uiText('思路已保存，正在生成针对性反馈…') }}</p>
    <article v-for="(review,i) in reviews" :key="review.expression.job_ref" class="review">
      <details :open="i === reviews.length - 1"><summary><small v-if="review.expression.round_title && ['prediction','reflection'].includes(review.expression.activity)" class="round-label">{{ uiText('第 {0} 轮 · {1}', [(review.expression.round_index ?? 0) + 1, review.expression.round_title]) }}</small>{{ activity(review.expression.activity) }} <span>{{ ['pending','running'].includes(review.status) ? uiText('正在分析…') : review.status === 'ready' ? (review.displayed ? uiText('已反馈') : uiText('反馈已就绪')) : uiText('需要重试') }}</span></summary>
        <blockquote>{{ review.expression.response }}</blockquote>
        <button v-if="['prediction','reflection'].includes(review.expression.activity)" :disabled="disabled" @click="emit('continue',review.expression.job_ref)">{{ uiText('在课程中接着讨论 →') }}</button>
        <p v-if="['pending','running'].includes(review.status)" class="muted" role="status">{{ uiText('正在结合原文、你的理由和此前看到的帮助分析。可以先继续学习。') }}</p>
        <button v-else-if="review.status === 'ready' && !review.displayed" :disabled="disabled" @click="emit('read',review.expression.job_ref)">{{ uiText('查看针对我的反馈') }}</button>
        <div v-else-if="review.feedback" class="personal-feedback"><p>{{ review.feedback.acknowledgement }}</p><p>{{ review.feedback.guidance }}</p><p v-if="review.feedback.follow_up" class="follow-up">{{ review.feedback.follow_up }}</p></div>
        <div v-else-if="review.status !== 'ready'" class="feedback-error"><p>{{ uiText('你的原话已保存，这次反馈尚未完成。') }}</p><button :disabled="disabled" @click="emit('retry',review.expression.job_ref)">{{ uiText('重新分析这段思路') }}</button></div>
        <details v-if="review.displayed && review.understanding.length" class="understanding"><summary>{{ uiText('已进入我的理解 · 查看保留了什么') }}</summary><div v-for="record in review.understanding" :key="record.evidence_ref"><p>{{ record.interpretation }}</p><div v-for="section in [{ title:'表达中已体现', items:record.demonstrated }, { title:'需要澄清', items:record.difficulties }, { title:'仍待确认', items:record.unconfirmed }]" :key="section.title"><template v-if="section.items.length"><strong>{{ uiText(section.title) }}</strong><ul><li v-for="item in section.items" :key="item">{{ item }}</li></ul></template></div></div><p class="muted">{{ uiText('保留原话、当时的来源和已获得的帮助；后来的修正追加记录，不覆盖最初的想法。') }}</p></details>
      </details>
    </article>
  </section>
</template>
<style scoped>
.round-label{display:block;margin-bottom:4px;font-size:11px;font-weight:400;color:var(--muted)}
.reviews{margin:30px 0;padding:24px;border:1px solid var(--line);border-radius:14px;background:var(--surface-soft);font-size:13px;line-height:1.9}.reviews header{display:flex;justify-content:space-between;gap:12px;align-items:center}.reviews header span{font-size:10px;letter-spacing:.08em;color:var(--muted)}h4{font-size:19px;margin:3px 0 16px;font-weight:550}.review{padding:14px 0;border-top:1px solid var(--line)}summary{cursor:pointer;font-weight:500}summary span{float:right;color:var(--muted);font-size:11px;font-weight:400}blockquote{margin:16px 0;padding:6px 0 6px 15px;border-left:2px solid var(--brand-accent);color:var(--muted);white-space:pre-line}.personal-feedback p{margin:12px 0;white-space:pre-line}.follow-up{padding:12px 16px;border:1px solid var(--line);border-radius:8px;background:var(--canvas)}button{font:inherit;min-height:40px;padding:8px 12px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.5}.muted{font-size:11px;color:var(--muted)}.understanding{font-size:12px;padding-top:12px}.understanding summary{color:var(--brand-accent)}.understanding ul{padding-left:20px;margin:5px 0 14px}.understanding strong{font-size:11px}@media(max-width:600px){.reviews{padding:16px}.reviews header{align-items:flex-start;flex-direction:column;margin-bottom:12px}}
</style>
