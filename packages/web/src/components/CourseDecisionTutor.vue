<script setup lang="ts">
import CourseFeedbackOptions from './CourseFeedbackOptions.vue';
import CourseUnderstandingNote from './CourseUnderstandingNote.vue';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { t as uiText } from '../i18n';
import type { CourseDecisionView } from '../generated/CourseDecisionView';
import type { CourseReview } from '../generated/CourseReview';
import type { CourseReplyIntent } from '../generated/CourseReplyIntent';
const props = defineProps<{ decision: CourseDecisionView; reviews: CourseReview[]; draft: string; disabled: boolean; finished?: boolean }>();
const emit = defineEmits<{ 'update:draft':[value:string]; reply:[intent:CourseReplyIntent, response:string]; read:[job:string]; retry:[job:string]; advance:[]; inspect:[]; pause:[]; support:[job:string]; understanding:[] }>();
const adoptedQuestion=ref(false);
watch(()=>props.decision.latest_job,()=>{adoptedQuestion.value=false;});
const card = ref<HTMLElement>();
const responseCard = ref<HTMLElement>();
const visible = ref(false);
const review = computed(()=>props.reviews.find(r=>r.expression.job_ref===props.decision.latest_job));
const waiting = computed(()=>!!props.decision.latest_job && (!review.value || ['pending','running'].includes(review.value.status)));
const readyToReply = computed(()=>!props.disabled && !waiting.value && (!props.decision.latest_job || !!review.value?.displayed));
const history = computed(()=>props.reviews.filter(r=>['task','decision','transfer'].includes(r.expression.activity) && r.expression.job_ref!==props.decision.latest_job));
let observer:IntersectionObserver | undefined;
let reading:string | null = null;
function readVisible() {
  const r=review.value;
  if (!visible.value || document.visibilityState==='hidden' || props.disabled || !r || r.status!=='ready' || r.displayed || reading===r.expression.job_ref) return;
  reading=r.expression.job_ref; emit('read',reading);
}
watch([review,()=>props.disabled,visible],(_value,previous)=>{ if(previous?.[1] && !props.disabled)reading=null; readVisible(); },{flush:'post'});
onMounted(()=>{ observer=new IntersectionObserver(entries=>{visible.value=!!entries[0]?.isIntersecting;readVisible();}); if(responseCard.value)observer.observe(responseCard.value); document.addEventListener('visibilitychange',readVisible); });
onBeforeUnmount(()=>{observer?.disconnect();document.removeEventListener('visibilitychange',readVisible);});
async function focus() { await nextTick();(review.value ? responseCard.value : card.value)?.scrollIntoView({block:'start',behavior:'instant'}); }
defineExpose({focus});
</script>
<template>
  <section ref="card" class="decision-tutor" style="scroll-margin-top:var(--course-teaching-offset,16px)" :aria-label="uiText('当前决策教学')">
    <header><span>{{ decision.phase==='transfer' ? uiText('换个情境，自己判断') : uiText('一起把这道决策题弄清楚') }}</span><small>{{ decision.phase==='transfer' ? '2 / 2' : '1 / 2' }}</small></header>
    <details class="task-conditions" :open="!review"><summary>{{ uiText('查看本题条件') }}</summary><p>{{ decision.question }}</p></details>
    <blockquote v-if="review">{{ review.expression.response }}</blockquote>
    <div ref="responseCard" class="teaching-response">
    <div v-if="waiting" class="teaching-status" role="status"><strong>{{ uiText('正在接住你的思路…') }}</strong><p>{{ uiText('会根据这段回答决定继续、补讲，还是一起看一处实验。你的原话已保存。') }}</p></div>
    <div v-else-if="review && ['failed','interrupted'].includes(review.status)" class="teaching-status" role="status"><p>{{ uiText('原话已保存，这次教学回应暂未完成。') }}</p><button :disabled="disabled" @click="emit('retry',review.expression.job_ref)">{{ uiText('重试这次回应') }}</button></div>
    <button v-else-if="review?.status==='ready' && !review.displayed" :disabled="disabled" @click="emit('read',review.expression.job_ref)">{{ uiText('展开这次教学回应') }}</button>
    <template v-if="review?.feedback">
      <p v-if="review.expression.intent!=='question' && review.feedback.acknowledgement" class="acknowledgement">{{ review.feedback.acknowledgement }}</p>
      <h4 v-if="decision.focus_title">{{ decision.focus_title }}</h4>
      <p class="guidance">{{ review.feedback.guidance }}</p>
      <aside v-if="decision.support" class="teaching-support"><strong>{{ uiText('先把这一点讲清楚') }}</strong><p>{{ decision.support }}</p></aside>
      <div v-if="decision.attention" class="attention"><strong>{{ uiText('现在看这里') }}</strong><p>{{ decision.attention }}</p><button :disabled="disabled" @click="emit('inspect')">{{ uiText('对照题目参数') }}</button></div>
      <div v-if="review.feedback.follow_up" class="next-question"><strong>{{ uiText('接着想这一点') }}</strong><p>{{ review.feedback.follow_up }}</p></div>
    </template>
    </div>
    <aside v-if="review?.expression.closed_book_result_ref" class="reuse-result" :aria-label="uiText('练习结果与本课要求')">
      <strong>{{ uiText('练习结果与本课要求') }}</strong>
      <template v-if="review.displayed && review.feedback?.course_reuse"><p>{{ review.feedback.course_reuse.accepted ? uiText('这次练习已满足本段要求。') : uiText('回到本课，还需要补上这一步。') }}</p><p>{{ review.feedback.course_reuse.reason }}</p></template>
      <p v-else>{{ uiText('这次练习的回答已保留，按本课要求核对后再继续。') }}</p>
    </aside>
    <CourseFeedbackOptions v-if="review?.feedback" :feedback="review.feedback" :disabled="disabled || finished" @support="emit('support',review!.expression.job_ref)" @question="value=>{adoptedQuestion=true;emit('update:draft',value)}" />
    <CourseUnderstandingNote :review="review" :disabled="disabled" @open="emit('understanding')" />
    <div v-if="finished" class="ready"><strong>{{ uiText('这次决策学习已完成') }}</strong><p>{{ uiText('已保留你在原题和新情境中的判断，以及过程中获得的帮助。') }}</p></div>
    <div v-else-if="decision.can_continue" class="ready"><p>{{ decision.phase==='transfer' ? uiText('这次新情境的判断已有依据，可以收束这一课。') : uiText('本题的关键思路已经清楚，换个情境验证一次。') }}</p><button class="primary" :disabled="disabled" @click="emit('advance')">{{ decision.phase==='transfer' ? uiText('完成这一课 ✓') : uiText('换个情境试试 →') }}</button></div>
    <form v-else @submit.prevent="emit('reply',adoptedQuestion?'question':'answer',draft)">
      <label for="decision-reply">{{ uiText('说说你的想法，也可以直接提出疑问') }}</label>
      <textarea id="decision-reply" :value="draft" @input="emit('update:draft',($event.target as HTMLTextAreaElement).value)" rows="4" :disabled="disabled" :placeholder="uiText('不确定的地方可以直接说；不用重新描述这道题。')" />
      <div class="reply-actions"><button class="primary" :disabled="!readyToReply || !draft.trim()">{{ adoptedQuestion?uiText('提出我的疑问'):uiText('回应当前问题') }}</button><button type="button" :disabled="!readyToReply || !draft.trim()" @click="emit('reply','question',draft)">{{ uiText('这是我的疑问') }}</button></div>
    </form>
    <footer v-if="!finished"><button :disabled="!readyToReply" @click="emit('reply','explain',uiText('我还没明白，请先把当前这一步讲清楚。'))">{{ uiText('先讲给我听') }}</button><button :disabled="disabled" @click="emit('pause')">{{ uiText('先暂停，保留这里') }}</button></footer>
    <details v-if="history.length" class="teaching-history"><summary>{{ uiText('回看这道题的交流') }}</summary><article v-for="item in history" :key="item.expression.job_ref"><blockquote>{{ item.expression.response }}</blockquote><template v-if="item.feedback"><p v-if="item.expression.intent!=='question' && item.feedback.acknowledgement">{{ item.feedback.acknowledgement }}</p><p>{{ item.feedback.guidance }}</p><p v-if="item.feedback.follow_up">{{ item.feedback.follow_up }}</p></template></article></details>
  </section>
</template>
<style scoped>
.reuse-result{margin:18px 0;padding:14px 16px;border-radius:8px;background:var(--surface-soft);font-size:13px}.reuse-result>strong{font-size:12px;color:var(--brand-accent)}.reuse-result>p{margin:6px 0}
.teaching-response{min-height:1px;scroll-margin-top:var(--course-teaching-offset,16px)}.decision-tutor{scroll-margin-top:var(--course-teaching-offset,16px)}
.decision-tutor{min-width:0;scroll-margin-top:16px;font-size:14px;line-height:1.85}.decision-tutor header{display:flex;gap:12px;justify-content:space-between;color:var(--brand-accent);font-size:12px;font-weight:600;margin-bottom:18px}.decision-tutor header small{color:var(--muted)}p{white-space:pre-line;margin:12px 0}h4{font-size:19px;margin:18px 0 12px;line-height:1.5}blockquote{margin:14px 0;padding:12px 16px;border-left:2px solid var(--brand-accent);background:var(--surface-soft);white-space:pre-line;color:var(--muted)}summary{cursor:pointer;color:var(--muted);font-size:12px}.task-conditions p{font-size:13px}.teaching-status,.teaching-support,.next-question,.ready{padding:16px;border:1px solid var(--line);border-radius:10px;margin:16px 0;background:var(--surface-soft)}.teaching-status p{color:var(--muted);font-size:12px}.attention{padding:12px 0;border-block:1px solid var(--line);margin:16px 0;font-size:12px}.attention strong,.next-question strong,.teaching-support strong{font-size:12px}.next-question{border-left:3px solid var(--brand-accent);background:var(--canvas)}form{margin-top:22px}label{display:block;font-size:12px;margin-bottom:8px}textarea{width:100%;box-sizing:border-box;resize:vertical;min-height:120px;padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--canvas);color:var(--ink);font:inherit;line-height:1.8}button{font:inherit;font-size:12px;min-height:42px;padding:9px 13px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.5;cursor:default}.primary{background:var(--brand-accent);color:var(--brand-on-accent);border-color:var(--brand-accent)}.reply-actions,footer{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}footer button{border:0;color:var(--muted);padding-inline:0;margin-right:12px}button:focus-visible,textarea:focus-visible{outline:2px solid var(--brand-accent);outline-offset:2px}.teaching-history{margin-top:24px;border-top:1px solid var(--line);padding-top:16px}.teaching-history article{font-size:12px;padding-block:10px;border-bottom:1px solid var(--line)}
</style>
