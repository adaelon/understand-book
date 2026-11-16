<script setup lang="ts">
import { t as uiText } from '../i18n';
import { computed } from 'vue';
import type { CourseExperiment } from '../generated/CourseExperiment';
import type { CoursePreset } from '../generated/CoursePreset';
const props = defineProps<{ experiment: Extract<CourseExperiment, {kind:'resource_bounds'}>; preset: CoursePreset | null; observed: CoursePreset[]; disabled: boolean }>();
defineEmits<{ observe: [preset: CoursePreset] }>();
const configurations = computed<{ id: CoursePreset; label: string }[]>(() => props.experiment.batch_size ? [
  {id:'batch_one',label:'1 条请求'},{id:'batch_four',label:'4 条请求'},{id:'batch_eight',label:'8 条请求'},
] : [
  { id: 'baseline', label: '原配置' }, { id: 'double_compute', label: '只把算力翻倍' }, { id: 'double_bandwidth', label: '只把带宽翻倍' },
]);
const times = computed(() => {
  const e = props.experiment;
  const batch = props.preset === 'batch_four' ? 4 : props.preset === 'batch_eight' ? 8 : props.preset === 'batch_one' ? 1 : e.batch_size ?? 1;
  const work = e.work_gflops * batch / (e.batch_size ?? 1);
  const compute = work / (e.compute_tflops * (props.preset === 'double_compute' ? 2 : 1));
  const read = e.read_gb / (e.bandwidth_tb_s * (props.preset === 'double_bandwidth' ? 2 : 1));
  return { compute, read, work, batch, lower: Math.max(compute, read), scale: Math.max(e.work_gflops * (e.batch_size ? 8 / e.batch_size : 1) / e.compute_tflops, e.read_gb / e.bandwidth_tb_s) };
});
const ms = (n: number) => `${Number(n.toFixed(4))} ms`;
</script>
<template>
  <div class="bounds-experiment">
    <p class="instruction">{{ experiment.instruction }}</p>
    <dl class="givens"><div v-if="experiment.batch_size"><dt>{{ uiText('当前批次') }}</dt><dd>{{ uiText('{0} 条请求', [times.batch]) }}</dd></div><div><dt>{{ uiText('计算工作量') }}</dt><dd>{{ times.work }} GFLOPs</dd></div><div><dt>{{ uiText('权重读取量') }}</dt><dd>{{ experiment.read_gb }} GB</dd></div></dl>
    <div class="configurations" :aria-label="uiText('实验配置')">
      <button v-for="item in configurations" :key="item.id" :aria-pressed="preset === item.id" :disabled="disabled" @click="$emit('observe', item.id)"><span v-if="observed.includes(item.id)" :aria-label="uiText('已观察')">✓ </span>{{ uiText(item.label) }}</button>
    </div>
    <div v-if="preset" class="plot" aria-live="polite">
      <div class="plot-heading"><span>{{ uiText(configurations.find(c => c.id === preset)?.label ?? '') }}</span><span>{{ uiText('同一起点 · 同一时间刻度') }}</span></div>
      <div v-for="row in [{ key: 'compute', label: uiText('计算'), value: times.compute }, { key: 'read', label: uiText('读取'), value: times.read }]" :key="row.key" class="time-row">
        <div class="time-label"><span>{{ row.label }}</span><strong>{{ ms(row.value) }}</strong></div>
        <div class="track"><div class="bar" :class="row.key" :style="{ width: `${row.value / times.scale * 100}%` }"></div></div>
      </div>
      <div class="axis"><span>0 ms</span><span>{{ ms(times.scale) }}</span></div>
      <div class="lower"><span>{{ uiText('充分重叠时的时间下界') }}</span><strong>{{ ms(times.lower) }}</strong></div>
      <details><summary>{{ uiText('查看参数与计算') }}</summary><table><thead><tr><th>{{ uiText('资源') }}</th><th>{{ uiText('处理速度') }}</th><th>{{ uiText('工作量 ÷ 处理速度') }}</th></tr></thead><tbody><tr><th>{{ uiText('计算') }}</th><td>{{ experiment.compute_tflops * (preset === 'double_compute' ? 2 : 1) }} TFLOP/s</td><td>{{ ms(times.compute) }}</td></tr><tr><th>{{ uiText('读取') }}</th><td>{{ experiment.bandwidth_tb_s * (preset === 'double_bandwidth' ? 2 : 1) }} TB/s</td><td>{{ ms(times.read) }}</td></tr></tbody></table></details>
      <p class="caption">{{ uiText('两项都要完成，下界取较大者。细短的时间条请结合数值阅读；实际耗时可能更长。') }}</p>
    </div>
    <p v-else class="empty">{{ uiText(experiment.batch_size ? '从 1 条请求开始，比较批量变化后的时间。' : '从“原配置”开始，看看时间主要花在哪里。') }}</p>
  </div>
</template>
<style scoped>
.instruction{line-height:1.85}.givens{display:flex;flex-wrap:wrap;gap:14px 40px;padding:16px 20px;background:var(--surface-soft);border-radius:12px}.givens div{display:grid;gap:5px}dt,.caption{font-size:12px;color:var(--muted)}dd{margin:0;font-variant-numeric:tabular-nums;font-weight:600}.configurations{display:flex;flex-wrap:wrap;gap:8px;margin:20px 0}button{font:inherit;font-size:13px;cursor:pointer;padding:11px 14px;border:1px solid var(--line);border-radius:9px;background:var(--canvas);color:var(--ink);min-height:44px}button[aria-pressed=true]{border-color:var(--brand-accent);background:var(--surface-soft);box-shadow:inset 0 0 0 1px var(--brand-accent)}button:disabled{opacity:.6;cursor:wait}.plot{padding:20px;border:1px solid var(--line);border-radius:12px}.plot-heading{display:flex;justify-content:space-between;flex-wrap:wrap;gap:6px;font-size:12px;color:var(--muted);margin-bottom:24px}.time-row{margin-top:20px}.time-label{display:flex;justify-content:space-between;gap:12px;font-size:14px;margin-bottom:8px}.time-label strong{font-variant-numeric:tabular-nums}.track{height:22px;background:var(--surface-soft);border-left:1px solid var(--muted);position:relative}.bar{height:100%;background:#427a89;transition:width .2s}.bar.read{background:var(--brand-accent)}.axis{display:flex;justify-content:space-between;color:var(--muted);font-size:11px;margin-top:8px}.lower{display:flex;justify-content:space-between;gap:12px;flex-wrap:wrap;padding:18px 0;margin-top:15px;border-top:1px solid var(--line);font-size:14px}.lower strong{font-size:22px;font-variant-numeric:tabular-nums}.caption{line-height:1.7;margin-bottom:0}.empty{padding:28px 20px;border:1px dashed var(--line);border-radius:12px;color:var(--muted);line-height:1.7}summary{font-size:12px;cursor:pointer;padding:9px 0}table{border-collapse:collapse;width:100%;font-size:12px;margin:8px 0}th,td{text-align:left;padding:8px 4px;border-bottom:1px solid var(--line);overflow-wrap:anywhere}@media(prefers-reduced-motion:reduce){.bar{transition:none}}@media(max-width:540px){.plot{padding:14px}.configurations button{flex:1}.givens{padding:14px;gap:14px}.plot-heading span:last-child{font-size:11px}}
</style>
