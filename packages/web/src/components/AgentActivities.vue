<script setup lang="ts">
import { t as uiText } from '../i18n';

import { computed, ref, useId } from "vue";
import { BookOpen, Check, ChevronRight, CircleAlert, LoaderCircle, Minus, Search, Square, Wrench } from "@lucide/vue";
import type { RunActivity } from "../agent-run-state";

const props = defineProps<{ activities: RunActivity[]; active?: boolean; diagnostic?: boolean; statusText?: string }>();
const expanded = ref(false);
const listId = useId();
const labels = { running: uiText('进行中'), succeeded: uiText('已完成'), no_result: uiText('无结果'), rejected: uiText('未执行'), failed: uiText('失败'), cancelled: uiText('已停止') };
// Public progress is deliberately separate from model attempts and their diagnostics.
const publicActivities = computed(() => props.activities.filter(activity =>
  activity.kind === 'commentary' || (activity.kind === 'tool' && activity.parent_step_id === null && activity.name !== 'agent.progress')));
const visibleActivities = computed(() => props.diagnostic ? props.activities : publicActivities.value);
const operationCount = computed(() => publicActivities.value.filter(activity => activity.kind !== 'commentary').length);
const listVisible = computed(() => props.diagnostic || props.active || expanded.value);
const usage = computed(() => props.activities.filter(a => a.kind === "model").reduce((sum, a) => sum + (a.usage_total_tokens ?? 0), 0));
const missing = computed(() => props.activities.filter(a => a.kind === "model" && a.status !== "running" && a.usage_total_tokens === null).length);
function activityIcon(activity: RunActivity) {
  if (activity.status === 'running') return LoaderCircle;
  if (activity.status === 'failed') return CircleAlert;
  if (activity.status === 'cancelled') return Square;
  if (activity.status === 'no_result' || activity.status === 'rejected') return Minus;
  if (/search/.test(activity.name)) return Search;
  if (/read|structure|^book\.text/.test(activity.name)) return BookOpen;
  return activity.status === 'succeeded' ? Check : Wrench;
}
function durationLabel(duration: number | null): string {
  if (duration === null) return "";
  if (duration < 100) return duration < 1 ? uiText('<1 毫秒') : uiText('{0} 毫秒', [duration.toFixed(1)]);
  return uiText('{0} 秒', [(duration / 1000).toFixed(1)]);
}
</script>
<template>
  <div v-if="visibleActivities.length || active" class="activity-group" :class="{ diagnostic, active }">
    <p v-if="active && !diagnostic" class="activity-heading" role="status"><LoaderCircle :size="14" class="activity-spinner" aria-hidden="true" />{{ statusText || uiText('正在工作') }}<span v-if="operationCount"> · {{ operationCount }} {{ uiText('项操作') }}</span></p>
    <button v-else-if="!diagnostic" type="button" class="activity-summary" :aria-expanded="expanded" :aria-controls="listId" @click="expanded = !expanded">
      <ChevronRight class="activity-chevron" :class="{ expanded }" :size="14" aria-hidden="true" />
      <span>{{ expanded ? uiText('收起工作过程') : uiText('查看工作过程') }}<template v-if="operationCount"> · {{ operationCount }} {{ uiText('项操作') }}</template></span>
    </button>
    <ol v-if="listVisible && visibleActivities.length" :id="listId" class="agent-activities" :aria-label="uiText('工作过程')">
      <li v-for="activity in visibleActivities" :key="activity.step_id" class="agent-activity" :class="{ commentary: activity.kind === 'commentary' && !diagnostic, nested: diagnostic && activity.parent_step_id !== null }" :data-step-id="activity.step_id" :data-status="activity.status">
        <p v-if="activity.kind === 'commentary' && !diagnostic" class="activity-commentary">{{ activity.label }}</p>
        <template v-else>
          <component :is="activityIcon(activity)" class="activity-icon" :class="{ 'activity-spinner': activity.status === 'running' }" :size="14" aria-hidden="true" />
          <span class="activity-label">{{ uiText(activity.label) }}</span>
          <span class="activity-status">{{ labels[activity.status] }}</span>
          <small v-if="activity.result_count !== null">{{ activity.result_count }} {{ uiText('项结果') }}</small>
          <small v-if="diagnostic && durationLabel(activity.duration_ms)">{{ durationLabel(activity.duration_ms) }}</small>
          <details v-if="diagnostic"><summary>{{ uiText('详情') }}</summary><code>{{ activity.name }}</code><span v-if="activity.error_code"> · {{ activity.error_code }}</span><span v-if="activity.parent_step_id !== null"> {{ uiText('· 所属步骤') }} {{ activity.parent_step_id }}</span></details>
        </template>
      </li>
    </ol>
    <small v-if="diagnostic && activities.length" class="activity-usage">{{ uiText('已记录模型用量') }} {{ usage }} tokens<span v-if="missing"> · {{ missing }} {{ uiText('次请求未提供用量') }}</span></small>
  </div>
</template>
<style scoped>
.activity-group { margin: 16px 0 20px; }
.activity-heading, .activity-summary { display: flex; align-items: center; gap: 6px; margin: 0 0 12px; color: var(--steel, #79786f); font-size: 12px; line-height: 1.5; }
.activity-heading > span { margin-left: -3px; }
.activity-summary { padding: 5px 0; border: 0; background: transparent; text-align: left; cursor: pointer; }
.activity-summary:hover { color: var(--ink, #36382f); }
.activity-summary:focus-visible { outline: 2px solid var(--brand-accent, #577b51); outline-offset: 4px; border-radius: 3px; }
.activity-chevron { flex-shrink: 0; transition: transform 120ms ease; }
.activity-chevron.expanded { transform: rotate(90deg); }
.agent-activities { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
.agent-activity { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 7px; min-width: 0; padding: 1px 0; color: var(--steel, #79786f); font-size: 12px; line-height: 1.65; }
.activity-icon { flex: 0 0 auto; }
.activity-label { min-width: 0; overflow-wrap: anywhere; }
.activity-status, small { font-size: 11px; color: var(--steel, #79786f); }
.activity-status::before { content: '·'; margin-right: 7px; }
[data-status="running"] .activity-icon { color: var(--brand-green-deep, #577b51); }
[data-status="failed"] :is(.activity-icon, .activity-status) { color: var(--error, #ad3a36); }
.commentary { display: block; padding: 3px 0; }
.activity-commentary { margin: 0; color: var(--ink, #36382f); font-size: 14px; line-height: 1.85; white-space: pre-wrap; overflow-wrap: anywhere; }
.agent-activity.commentary + .agent-activity, .agent-activity + .commentary { margin-top: 4px; }
.diagnostic .agent-activity { padding: 8px 10px; border: 1px solid var(--line, #ddd); border-radius: 8px; }
.diagnostic .nested { margin-left: 16px; border-left-width: 3px; }
.diagnostic .activity-label { flex: 1; }
details { flex-basis: 100%; overflow-wrap: anywhere; }
.activity-usage { display: block; margin-top: 10px; }
.activity-spinner { animation: activity-spin 1.4s linear infinite; }
@keyframes activity-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .activity-chevron { transition: none; } .activity-spinner { animation: none; } }
</style>
