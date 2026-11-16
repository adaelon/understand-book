<script setup lang="ts">
import { computed } from 'vue';
import { t as uiText } from '../i18n';
import type { CourseLessonView } from '../generated/CourseLessonView';

const props = defineProps<{ lesson: CourseLessonView | null; blocked?: boolean; goalChanged?: boolean; error?: string }>();
const emit = defineEmits<{ action: [action: { kind: 'pause' | 'resume' }]; retry: [] }>();
const status = computed(() => props.lesson?.progress.status);
const label = computed(() => !props.lesson ? uiText('选择课程，开始带学') : props.goalChanged ? uiText('本课目标已变化')
  : uiText(({ active: '本课带学中', paused: '本课带学已暂停', completed: '本课已完成', skipped: '本课已跳过' })[status.value!]));
</script>

<template>
  <div class="course-tutor-control" :aria-label="uiText('本课带学')">
    <span role="status" :class="{ active: status === 'active' && !goalChanged }"><i aria-hidden="true" />{{ label }}</span>
    <button v-if="status === 'active'" type="button" :disabled="blocked" @click="emit('action', { kind: 'pause' })">{{ uiText('暂停带学') }}</button>
    <button v-else-if="status === 'paused'" type="button" :disabled="blocked || goalChanged" @click="emit('action', { kind: 'resume' })">{{ uiText('继续本课') }}</button>
    <small>{{ !lesson ? uiText('点击开始学习后，按当前课程一路学下去。') : goalChanged ? uiText('已有记录保留，请从课程目录按新目标开始。') : status === 'active' ? uiText('课程、原文、实验与答疑共用这次学习记录。') : uiText('学习位置和回答已保留，仍可随时提问。') }}</small>
    <small v-if="error" role="alert">{{ error }} <button type="button" @click="emit('retry')">{{ uiText('重试保存') }}</button></small>
  </div>
</template>

<style scoped>
.course-tutor-control{display:flex;flex-wrap:wrap;align-items:center;gap:4px 14px;font:12px/1.7 var(--sans);color:var(--muted);min-width:0}
span{display:inline-flex;align-items:center;gap:7px}i{width:6px;height:6px;border-radius:50%;background:currentColor}.active{color:var(--brand-green-deep)}
button{font:inherit;color:var(--brand-accent);border:0;background:transparent;padding:6px 0;min-height:36px;cursor:pointer}button:disabled{opacity:.5;cursor:default}button:focus-visible{outline:2px solid var(--brand-accent);outline-offset:3px;border-radius:4px}
small{flex-basis:100%;font-size:11px;overflow-wrap:anywhere}@media(pointer:coarse){button{min-height:44px}}
</style>
