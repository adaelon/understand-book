<script setup lang="ts">
import { computed } from 'vue';
import { ArrowUpRight, Check } from '@lucide/vue';
import type { CourseReview } from '../generated/CourseReview';
const props=defineProps<{review?:CourseReview;disabled?:boolean}>();
defineEmits<{open:[]}>();
const record=computed(()=>props.review?.displayed?props.review.understanding[0]:undefined);
</script>
<template>
  <aside v-if="record" class="course-understanding-note" aria-label="这一步的理解小结">
    <span class="note-label"><Check :size="14"/>这一步，你的理解</span>
    <strong>{{record.learner_summary?.headline || record.demonstrated[0] || record.difficulties[0] || '留下了这次真实的想法'}}</strong>
    <p v-if="record.learner_summary">{{record.learner_summary.explanation}}</p>
    <div><small>{{record.assistance || '查看依据中的帮助条件'}}</small><button type="button" :disabled="disabled" @click="$emit('open')">本课理解<ArrowUpRight :size="14"/></button></div>
  </aside>
</template>
<style scoped>
.course-understanding-note{margin:18px 0 0;padding:17px 18px;background:var(--surface-soft);border-radius:10px;border-left:2px solid var(--brand-accent);color:var(--ink)}.note-label{display:flex;align-items:center;gap:6px;font:500 11px/1.5 var(--sans);color:var(--brand-accent)}strong{display:block;font:500 15px/1.7 var(--sans);margin:7px 0}p{font:13px/1.8 var(--sans);margin:6px 0}div{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:8px;margin-top:8px}small{font:11px/1.6 var(--sans);color:var(--muted)}button{display:flex;align-items:center;gap:4px;background:transparent;border:0;color:var(--brand-accent);font:500 12px/1.5 var(--sans);min-height:40px;padding:4px;cursor:pointer}
</style>
