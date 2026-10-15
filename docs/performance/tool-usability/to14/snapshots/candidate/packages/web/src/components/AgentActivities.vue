<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { ChevronRight } from "@lucide/vue";
import type { RunActivity } from "../agent-run-state";
const props = defineProps<{ activities: RunActivity[]; diagnostic?: boolean }>();
const expanded = ref(false);
const listId = useId();
const latest = computed(() => props.activities.at(-1));
const labels = { running: "进行中", succeeded: "已完成", no_result: "无结果", rejected: "未执行", failed: "失败", cancelled: "已停止" };
const usage = computed(() => props.activities.filter(a => a.kind === "model").reduce((sum, a) => sum + (a.usage_total_tokens ?? 0), 0));
const missing = computed(() => props.activities.filter(a => a.kind === "model" && a.status !== "running" && a.usage_total_tokens === null).length);
function durationLabel(duration: number | null): string {
  if (duration === null) return "";
  if (duration < 100) {
    if (!props.diagnostic) return "";
    return duration < 1 ? "<1 毫秒" : `${duration.toFixed(1)} 毫秒`;
  }
  return `${(duration / 1000).toFixed(1)} 秒`;
}
</script>
<template>
  <div v-if="activities.length" class="activity-group" :class="{ diagnostic }">
    <button v-if="!diagnostic && latest" type="button" class="agent-activity activity-summary" :data-step-id="latest.step_id" :data-status="latest.status" :aria-expanded="expanded" :aria-controls="listId" @click="expanded = !expanded">
      <span class="activity-label">{{ latest.label }}</span>
      <ChevronRight class="activity-chevron" :class="{ expanded }" :size="14" aria-hidden="true" />
      <span class="activity-status">{{ labels[latest.status] }}</span>
      <small v-if="durationLabel(latest.duration_ms)">{{ durationLabel(latest.duration_ms) }}</small>
    </button>
    <ol v-if="diagnostic || expanded" :id="listId" class="agent-activities" aria-label="运行活动">
      <li v-for="activity in activities" :key="activity.step_id" class="agent-activity" :class="{ nested: activity.parent_step_id !== null }" :data-step-id="activity.step_id" :data-status="activity.status">
        <span class="activity-label">{{ activity.label }}</span>
        <span class="activity-status">{{ labels[activity.status] }}</span>
        <small v-if="activity.result_count !== null">{{ activity.result_count }} 项结果</small>
        <small v-if="durationLabel(activity.duration_ms)">{{ durationLabel(activity.duration_ms) }}</small>
        <details v-if="diagnostic"><summary>详情</summary><code>{{ activity.name }}</code><span v-if="activity.error_code"> · {{ activity.error_code }}</span><span v-if="activity.parent_step_id !== null"> · 所属步骤 {{ activity.parent_step_id }}</span></details>
      </li>
    </ol>
    <small v-if="diagnostic && activities.length" class="activity-usage">已记录模型用量 {{ usage }} tokens<span v-if="missing"> · {{ missing }} 次请求未提供用量</span></small>
  </div>
</template>
<style scoped>
.agent-activities { display: grid; gap: 6px; margin: 10px 0; padding: 0; list-style: none; }
.agent-activity { display: flex; flex-wrap: wrap; align-items: baseline; gap: 6px 10px; padding: 8px 10px; border: 1px solid var(--border, #ddd); border-radius: 8px; font-size: 12px; }
.nested { margin-left: 16px; border-left-width: 3px; }
.activity-label { flex: 1; }
.activity-status, small { color: var(--muted, #6b7280); }
[data-status="running"] .activity-status { color: var(--accent, #286da8); }
[data-status="failed"] .activity-status { color: #ad3a36; }
details { flex-basis: 100%; overflow-wrap: anywhere; }
.activity-summary { width: 100%; margin: 10px 0; padding: 4px 0; border: 0; border-radius: 4px; background: transparent; color: var(--muted, #6b7280); font: inherit; font-size: 12px; text-align: left; align-items: center; flex-wrap: nowrap; gap: 6px; cursor: pointer; }
.activity-summary .activity-label { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.activity-summary .activity-status { margin-left: auto; }
.activity-summary .activity-status, .activity-summary small { flex-shrink: 0; }
.activity-chevron { flex-shrink: 0; opacity: 0; transition: transform 120ms ease, opacity 120ms ease; }
.activity-summary:hover .activity-chevron, .activity-summary:focus-visible .activity-chevron, .activity-chevron.expanded { opacity: 1; }
.activity-chevron.expanded { transform: rotate(90deg); }
.activity-summary:focus-visible { outline: 2px solid var(--accent, #286da8); outline-offset: 3px; }
.activity-group:not(.diagnostic) .agent-activities { margin-top: 0; padding-left: 10px; border-left: 1px solid var(--border, #ddd); }
.activity-group:not(.diagnostic) li.agent-activity { border: 0; border-radius: 0; padding: 4px 0; }
@media (prefers-reduced-motion: reduce) { .activity-chevron { transition: none; } }
</style>
