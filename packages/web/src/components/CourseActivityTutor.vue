<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { t as uiText } from '../i18n';
import { renderMarkdown } from '../md';
import type { CourseExpression } from '../generated/CourseExpression';
import type { CourseReview } from '../generated/CourseReview';
import type { CourseReplyIntent } from '../generated/CourseReplyIntent';
import CourseFeedbackOptions from './CourseFeedbackOptions.vue';
import CourseUnderstandingNote from './CourseUnderstandingNote.vue';

const props = defineProps<{ expression: CourseExpression; review?: CourseReview; draft: string; disabled: boolean; presented: boolean; readonly?: boolean; continuation?: { label: string; detail: string; disabled?: boolean } }>();
const emit = defineEmits<{ 'update:draft':[value:string]; read:[job:string]; retry:[job:string]; reply:[intent:CourseReplyIntent,response:string]; continue:[]; understanding:[]; support:[job:string] }>();
const card = ref<HTMLElement>(), responseCard = ref<HTMLElement>();
const visible = ref(false);
const waiting = computed(() => !props.review || ['pending','running'].includes(props.review.status));
const readyToReply = computed(() => !props.disabled && !!props.review?.displayed && props.review.status === 'ready');
// No follow-up means no answer is being requested, not that the whole lesson is mastered.
const settled = computed(() => props.review?.status === 'ready' && props.review.displayed && !!props.review.feedback && !props.review.feedback.follow_up?.trim()
  && (!props.expression.closed_book_result_ref || !!props.review.feedback.course_reuse?.accepted));
const questionOpen = ref(false),adoptedQuestion=ref(false);
const showReply = computed(() => !settled.value || questionOpen.value || !!props.draft.trim());
watch(() => props.expression.job_ref, () => { questionOpen.value = false; adoptedQuestion.value=false; });
async function openQuestion() {
  questionOpen.value = true;
  await nextTick();
  card.value?.querySelector('textarea')?.focus();
}
let observer: IntersectionObserver | undefined, reading: string | undefined;
function readVisible() {
  const review=props.review;
  if (!props.presented || !visible.value || document.visibilityState==='hidden' || props.disabled || !review || review.status!=='ready' || review.displayed || reading===review.expression.job_ref) return;
  reading=review.expression.job_ref;emit('read',reading);
}
watch([()=>props.review,()=>props.disabled,()=>props.presented,visible],(_value,previous)=>{
  if(previous?.[1] && !props.disabled)reading=undefined;
  readVisible();
},{flush:'post'});
onMounted(()=>{
  observer=new IntersectionObserver(entries=>{visible.value=!!entries[0]?.isIntersecting;readVisible();});
  if(responseCard.value)observer.observe(responseCard.value);
  document.addEventListener('visibilitychange',readVisible);
});
onBeforeUnmount(()=>{observer?.disconnect();document.removeEventListener('visibilitychange',readVisible);});
async function focus() { await nextTick();card.value?.scrollIntoView({block:'start',behavior:'instant'}); }
defineExpose({focus});
</script>

<template>
  <section ref="card" class="activity-tutor" :aria-label="uiText('当前活动反馈')">
    <header><strong>{{ expression.activity==='prediction' ? uiText('带着这个猜想做实验') : uiText('接着你的思路，一起弄明白') }}</strong><small v-if="expression.round_title">{{ uiText('第 {0} 轮 · {1}', [(expression.round_index ?? 0)+1,expression.round_title]) }}</small></header>
    <blockquote>{{ expression.response }}</blockquote>
    <div ref="responseCard" class="activity-response" aria-live="polite">
      <div v-if="waiting" class="teaching-status" role="status"><strong>{{ uiText('正在接住你的思路…') }}</strong><p>{{ uiText('回应会直接出现在这里，你可以对照实验继续想一想。') }}</p></div>
      <div v-else-if="review && ['failed','interrupted'].includes(review.status)" class="teaching-status"><p>{{ uiText('原话已保存，这次教学回应暂未完成。') }}</p><button :disabled="disabled" @click="emit('retry',expression.job_ref)">{{ uiText('重试这次回应') }}</button></div>
      <button v-else-if="review?.status==='ready' && !review.displayed" :disabled="disabled" @click="emit('read',expression.job_ref)">{{ uiText('展开这次教学回应') }}</button>
      <template v-if="review?.feedback">
        <p v-if="expression.intent!=='question' && review.feedback.acknowledgement" class="acknowledgement">{{ review.feedback.acknowledgement }}</p>
        <div class="guidance course-prose" v-html="renderMarkdown(review.feedback.guidance)" />
        <section v-if="review.feedback.task_assessment?.length" class="task-evidence" :aria-label="uiText('本次解题能力检验')">
          <strong>{{ uiText('本次作答的依据') }}</strong>
          <article v-for="(item,index) in review.feedback.task_assessment" :key="item.goal_id">
            <header><b>{{ uiText('判断依据 {0}',[index+1]) }}</b><span>{{ uiText({demonstrated:'已体现',needs_support:'需要补足',unconfirmed:'尚未体现'}[item.status]) }}</span></header>
            <blockquote v-if="item.quote">{{item.quote}}</blockquote>
            <div class="course-prose" v-html="renderMarkdown(item.reason)" />
          </article>
          <small>{{ uiText('这里记录这次作答体现的能力；查看提示、解析后的修订会保留帮助条件。') }}</small>
        </section>
        <aside v-if="review.feedback.follow_up" class="next-question"><strong>{{ uiText('接着想这一点') }}</strong><div class="course-prose" v-html="renderMarkdown(review.feedback.follow_up)" /></aside>
      </template>
    </div>
    <aside v-if="expression.closed_book_result_ref" class="reuse-result" :aria-label="uiText('练习结果与本课要求')">
      <strong>{{ uiText('练习结果与本课要求') }}</strong>
      <template v-if="review?.displayed && review.feedback?.course_reuse"><p>{{ review.feedback.course_reuse.accepted ? uiText('这次练习已满足本段要求。') : uiText('回到本课，还需要补上这一步。') }}</p><div class="course-prose" v-html="renderMarkdown(review.feedback.course_reuse.reason)" /></template>
      <p v-else>{{ uiText('这次练习的回答已保留，按本课要求核对后再继续。') }}</p>
    </aside>
    <div v-if="settled || continuation" class="activity-next" :class="{ settled }">
      <strong v-if="settled">{{ uiText('这一步不用再回答了') }}</strong>
      <template v-if="continuation && !readonly"><p>{{ continuation.detail }}</p><button :class="{ primary:settled }" :disabled="disabled || continuation.disabled" @click="emit('continue')">{{ continuation.label }}</button></template>
      <button v-if="settled && !readonly && !showReply" class="optional-question" :disabled="disabled" @click="openQuestion">{{ uiText('我还有疑问') }}</button>
    </div>
    <form v-if="!readonly && showReply" @submit.prevent="emit('reply',settled || adoptedQuestion ? 'question' : 'answer',draft)">
      <label for="activity-reply">{{ settled || adoptedQuestion ? uiText('还有哪里没明白？可以在这里问。') : uiText('接着回答，也可以说哪里没懂') }}</label>
      <textarea id="activity-reply" :value="draft" @input="emit('update:draft',($event.target as HTMLTextAreaElement).value)" :disabled="disabled" rows="3" :placeholder="uiText('对照眼前的图，说说你的判断或疑问。')" />
      <div class="reply-actions"><button class="primary" :disabled="!readyToReply || !draft.trim()">{{ settled || adoptedQuestion ? uiText('提出我的疑问') : uiText('回应当前问题') }}</button><button v-if="!settled" type="button" :disabled="!readyToReply || !draft.trim()" @click="emit('reply','question',draft)">{{ uiText('这是我的疑问') }}</button><button type="button" :disabled="!readyToReply" @click="emit('reply','explain',uiText('我还没明白，请先把当前这一步讲清楚。'))">{{ uiText('先讲给我听') }}</button><button v-if="settled && !draft.trim()" type="button" @click="questionOpen=false">{{ uiText('收起疑问') }}</button></div>
    </form>
    <CourseFeedbackOptions v-if="review?.feedback" :feedback="review.feedback" :disabled="disabled || readonly" @support="emit('support',expression.job_ref)" @question="value=>{adoptedQuestion=true;emit('update:draft',value);openQuestion()}" />
    <CourseUnderstandingNote :review="review" :disabled="disabled" @open="emit('understanding')" />
  </section>
</template>

<style scoped>
.reuse-result{margin:18px 0;padding:14px 16px;border-radius:8px;background:var(--surface-soft);font-size:13px}.reuse-result>strong{font-size:12px;color:var(--brand-accent)}.reuse-result>p{margin:6px 0}.reuse-result .course-prose{font-size:12px;color:var(--muted)}
.course-prose{font:inherit;line-height:1.85;white-space:normal;overflow-wrap:anywhere}.course-prose :deep(p){margin:0 0 .85em}.course-prose :deep(p:last-child){margin-bottom:0}.course-prose :deep(.katex-display){overflow-x:auto;overflow-y:hidden;padding:4px 0}.course-prose :deep(li){margin:.4em 0}
.task-evidence{display:grid;gap:12px;margin:18px 0}.task-evidence article{padding:14px;border:1px solid var(--line);border-radius:8px}.task-evidence article header{display:flex;justify-content:space-between;gap:12px}.task-evidence header span{font-size:12px;color:var(--muted)}.task-evidence blockquote{margin:8px 0;font-size:12px}.task-evidence small{color:var(--muted)}
.activity-next{margin-top:20px;padding-top:18px;border-top:1px solid var(--line)}.activity-next strong{display:block;font-size:14px}.activity-next p{font-size:12px;color:var(--muted);margin:5px 0 12px}.activity-next.settled strong{color:var(--brand-accent)}.activity-next button{margin:4px 8px 0 0}.activity-next .optional-question{background:transparent}
.activity-tutor{margin:22px 0;padding:20px;border:1px solid var(--line);border-left:3px solid var(--brand-accent);border-radius:12px;background:var(--canvas);font:14px/1.85 var(--sans);scroll-margin-top:var(--course-teaching-offset,16px);min-width:0}.activity-tutor header{display:grid;gap:5px}.activity-tutor header strong{font-size:16px}.activity-tutor header small{font-size:11px;color:var(--muted)}blockquote{margin:16px 0;padding:10px 14px;border-left:2px solid var(--line);color:var(--muted);background:var(--surface-soft);white-space:pre-line}p{margin:12px 0;white-space:pre-line}.activity-response{min-height:1px}.guidance,textarea{scroll-margin-top:var(--course-teaching-offset,16px)}.teaching-status{padding:14px;background:var(--surface-soft);border-radius:8px}.teaching-status strong{font-size:13px}.teaching-status p{font-size:12px;color:var(--muted)}.next-question{margin:18px 0;padding:12px 16px;background:var(--surface-soft);border-radius:8px}.next-question strong{font-size:12px;color:var(--brand-accent)}.next-question p{margin:6px 0}form{margin-top:20px}label{font-size:12px}textarea{display:block;width:100%;box-sizing:border-box;resize:vertical;min-height:100px;margin:8px 0 12px;padding:12px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);font:inherit;line-height:1.8}.reply-actions{display:flex;flex-wrap:wrap;gap:8px}button{font:inherit;font-size:12px;min-height:42px;padding:9px 12px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.5;cursor:default}.primary{background:var(--brand-accent);border-color:var(--brand-accent);color:var(--brand-on-accent)}.understanding-link{margin-top:14px;padding-inline:0;border:0;background:transparent;color:var(--muted);font-size:11px}button:focus-visible,textarea:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px}@container(min-width:780px){.activity-tutor{scroll-margin-top:16px}}@media(max-width:600px){.activity-tutor{padding:16px 13px;font-size:13px}}
</style>
