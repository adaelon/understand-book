<script setup lang="ts">
import { computed } from 'vue';
import { ArrowRight, BookOpen, Plus } from '@lucide/vue';
import { t } from '../i18n';
import { learningTaskAction, learningTaskKey, learningTaskStage, learningTaskCollection, type LearningTaskView } from '../learning-tasks';
import type { LearningCollectionTarget } from '../learning-collections';
import type { SharedBook } from '../shared-workspaces';
import { bookTitle } from '../book-title';

const props = defineProps<{ tasks: LearningTaskView[]; books?: SharedBook[]; loading: boolean; error: string; busy: boolean }>();
const emit = defineEmits<{ resume: [task: LearningTaskView]; collection: [target: LearningCollectionTarget]; courses: []; materials: []; retry: [] }>();
const current = computed(() => props.tasks.find(task => task.status !== 'unavailable') ?? props.tasks[0]);
const others = computed(() => props.tasks.filter(task => task !== current.value));
function collectionTitle(task: LearningTaskView) {
  if (task.course_context?.collection_title) return task.course_context.collection_title;
  const book = props.books?.find(book => book.published_book_ref.book_id === task.published_book_ref.book_id
    && book.published_book_ref.publication_id === task.published_book_ref.publication_id);
  return bookTitle({ title: book?.title, book_id: task.published_book_ref.book_id });
}
</script>

<template>
  <section class="learning-home-task" :aria-label="t('当前学习任务')" :aria-busy="loading">
    <div class="task-main">
      <span class="task-eyebrow">{{ t(current ? '接着上次，往前一步' : '从一个想弄懂的问题开始') }}</span>
      <template v-if="current">
        <div class="task-belonging" :aria-label="t('学习归属')">
          <button class="task-collection" :disabled="busy || loading" @click="emit('collection', learningTaskCollection(current))"><BookOpen :size="15" />{{ collectionTitle(current) }}<ArrowRight :size="14" /></button>
          <p v-if="current.course_context" class="task-course-position"><span>{{ current.course_context.unit_title }}</span><span>{{ t('第 {0} 课 · 共 {1} 课', [current.course_context.lesson_number, current.course_context.lesson_count]) }}</span></p>
        </div>
        <h2>{{ current.title }}</h2>
        <p class="task-stage">{{ learningTaskStage(current) }}<span v-if="current.status === 'paused'"> · {{ t('已暂停') }}</span></p>
        <p v-if="current.message" class="task-message">{{ t(current.message) }}</p>
        <p v-else-if="current.question" class="task-question">{{ current.question }}</p>
        <button class="task-primary" :disabled="busy || loading || current.status === 'unavailable'" @click="emit('resume', current)">{{ learningTaskAction(current) }}<ArrowRight :size="17" /></button>
      </template>
      <template v-else-if="!loading && !error">
        <h2>{{ t('选一本书，或开始一套课程') }}</h2>
        <p>{{ t('沿着完整的学习路线，一次学清一个问题。') }}</p>
        <button class="task-primary" :disabled="busy" @click="emit('courses')"><BookOpen :size="17" />{{ t('看看可以学什么') }}</button>
      </template>
      <h2 v-else>{{ t(error ? '接续记录暂时不可用' : '正在接回你的学习…') }}</h2>
      <p v-if="loading" role="status" class="task-status">{{ t('正在接回你的学习任务…') }}</p>
      <p v-if="error" role="alert" class="task-status">{{ error }} <button :disabled="loading" @click="emit('retry')">{{ t('重试') }}</button></p>
      <details v-if="others.length" class="other-tasks"><summary>{{ t('其他未完成的学习') }} · {{ others.length }}</summary>
        <button v-for="task in others" :key="learningTaskKey(task)" :disabled="busy || task.status === 'unavailable'" @click="emit('resume', task)"><span><span class="other-collection">{{ collectionTitle(task) }}</span><strong>{{ task.title }}</strong><small>{{ task.message ? t(task.message) : learningTaskStage(task) }}</small></span><ArrowRight :size="16" /></button>
      </details>
    </div>
    <div class="task-materials"><span>{{ t('也可以从自己的材料开始') }}</span><p>{{ t('带着一段文字、一本书或一个问题，继续探索。') }}</p><button :disabled="busy" @click="emit('materials')"><Plus :size="16" />{{ t('用自己的资料学习') }}</button></div>
  </section>
</template>

<style scoped>
.learning-home-task{display:grid;grid-template-columns:minmax(0,1fr) 240px;gap:40px;padding:30px 32px;margin:0 0 28px;background:var(--surface-soft);border:1px solid var(--line);border-radius:14px;color:var(--ink)}
.task-belonging{margin-top:14px}.task-collection{max-width:100%;justify-content:flex-start;padding:5px 0;border:0;background:transparent;color:var(--brand-accent-deep);text-align:left;overflow-wrap:anywhere}.task-collection svg{flex:none}.task-course-position{display:flex;flex-wrap:wrap;gap:5px 14px;font-size:12px;margin:2px 0 10px}.other-collection{display:block;color:var(--muted);font-size:11px;margin-bottom:3px}
.task-main{min-width:0}.task-eyebrow{font-size:12px;color:var(--brand-accent-deep);letter-spacing:.04em}h2{margin:12px 0;font:500 27px/1.5 var(--serif,var(--sans));overflow-wrap:anywhere}p{font-size:14px;line-height:1.8;margin:10px 0 18px;color:var(--muted)}.task-stage{color:var(--steel);margin-bottom:8px}.task-question{display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;white-space:pre-wrap;color:var(--ink)}
button{display:inline-flex;align-items:center;justify-content:center;gap:9px;min-height:42px;padding:9px 14px;border:1px solid var(--line);border-radius:8px;background:var(--canvas);color:var(--ink);font:500 13px/1.5 var(--sans);cursor:pointer}button:disabled{opacity:.5;cursor:default}.task-primary{background:var(--brand-accent);color:var(--brand-on-accent);border-color:var(--brand-accent)}.task-materials{border-left:1px solid var(--line);padding-left:28px;align-self:center}.task-materials>span{font-size:13px;font-weight:500}.task-materials p{font-size:12px}.task-status{font-size:12px;margin-bottom:0}.task-status button{min-height:28px;padding:3px 8px}.other-tasks{margin-top:20px;font-size:12px}.other-tasks summary{cursor:pointer;color:var(--steel)}.other-tasks>button{display:flex;justify-content:space-between;width:100%;text-align:left;margin-top:8px}.other-tasks span{min-width:0}.other-tasks strong,.other-tasks small{display:block;overflow-wrap:anywhere}.other-tasks small{font-weight:400;color:var(--muted);margin-top:4px}
@media(max-width:760px){.learning-home-task{grid-template-columns:1fr;gap:22px;padding:22px 20px;margin-bottom:20px}h2{font-size:23px}.task-materials{border-left:0;border-top:1px solid var(--line);padding:17px 0 0}.task-materials p{margin:6px 0 12px}}
</style>
