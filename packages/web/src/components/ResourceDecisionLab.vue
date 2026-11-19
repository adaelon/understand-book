<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { t as uiText } from '../i18n';
import CourseResourceObjects from './CourseResourceObjects.vue';
import { resourceModel, withBatch } from '../course-resource-model';
import type { CourseExperiment } from '../generated/CourseExperiment';
import type { CoursePreset } from '../generated/CoursePreset';
import type { CourseResourceState } from '../generated/CourseResourceState';
const props = defineProps<{ experiment: Extract<CourseExperiment, {kind:'resource_bounds'}>; lab: CourseResourceState; saved: CourseResourceState | null; observed: CoursePreset[]; disabled: boolean; review?: boolean; inputPrefix?: string; sceneKey?: string; presented?: boolean }>();
const emit = defineEmits<{ observe: [preset: CoursePreset]; save: [state: CourseResourceState]; change: [state: CourseResourceState]; question:[text:string] }>();
const original = () => ({ work_gflops: props.experiment.work_gflops, read_gb: props.experiment.read_gb, compute_tflops: props.experiment.compute_tflops, bandwidth_tb_s: props.experiment.bandwidth_tb_s, target_ms: props.lab.target_ms, ...(props.experiment.batch_size ? {batch_size:props.experiment.batch_size} : {}) });
const state = ref<CourseResourceState>(props.saved ? { ...props.saved } : original());
defineExpose({ useState: (value: CourseResourceState) => { state.value = { ...value }; } });
const plot = ref<'objects' | '2d' | '3d'>('objects');
const batchMode = computed(() => props.experiment.batch_size !== undefined);
const presets = computed(() => batchMode.value ? ([['batch_one','1 条请求'],['batch_four','4 条请求'],['batch_eight','8 条请求']] as const) : ([['baseline','原配置'],['double_compute','算力翻倍'],['double_bandwidth','带宽翻倍']] as const));
const model = computed(() => resourceModel(state.value));
function changeBatch(event:Event) { const input=event.target as HTMLInputElement,batch=input.valueAsNumber; if(Number.isInteger(batch)&&batch>=1&&batch<=256)state.value=withBatch(state.value,batch); else input.value=String(state.value.batch_size); }
const adjusting = ref(false);
watch(state, value => emit('change', { ...value }), { deep: true, immediate: true });
const dirty = computed(() => !props.saved || Object.keys(state.value).some(key => state.value[key as keyof CourseResourceState] !== props.saved?.[key as keyof CourseResourceState]));
const x = computed(() => model.value.compute);
const y = computed(() => model.value.read);
const lower = computed(() => Math.max(x.value, y.value));
const limit = computed(() => Math.max(12, Math.ceil(Math.max(x.value, y.value, state.value.target_ms) / 5) * 5));
const feasible = computed(() => lower.value <= state.value.target_ms);
const digits = (n: number) => n.toFixed(n < 1 ? 3 : 2);
const controls: { key: keyof CourseResourceState; title: string; symbol: string; unit: string; min: number; max: number; step: number }[] = [
  { key: 'work_gflops', title: '计算量', symbol: 'F', unit: 'GFLOPs', min: 20, max: 4800, step: 20 },
  { key: 'read_gb', title: '读取量', symbol: 'R', unit: 'GB', min: 1, max: 160, step: 1 },
  { key: 'compute_tflops', title: '算力', symbol: 'C', unit: 'TFLOP/s', min: 10, max: 2000, step: .1 },
  { key: 'bandwidth_tb_s', title: '带宽', symbol: 'W', unit: 'TB/s', min: .25, max: 16, step: .05 },
  { key: 'target_ms', title: '时间目标', symbol: 'T', unit: 'ms', min: 1, max: 30, step: 1 },
];
function setValue(control: typeof controls[number], event: Event) {
  const input = event.target as HTMLInputElement;
  const value = input.valueAsNumber;
  if (Number.isFinite(value) && value >= control.min && value <= (control.key==='work_gflops' ? control.max * (state.value.batch_size ?? 1) : control.max)) state.value[control.key] = value;
  else input.value = String(state.value[control.key]);
}
function preset(which: CoursePreset) {
  state.value = original();
  if (which === 'double_compute') state.value.compute_tflops *= 2;
  if (which === 'double_bandwidth') state.value.bandwidth_tb_s *= 2;
  const batch=({batch_one:1,batch_four:4,batch_eight:8} as Partial<Record<CoursePreset,number>>)[which];
  if(batch!==undefined)state.value=withBatch(state.value,batch);
  emit('observe', which);
}
const px = (value: number) => 56 + value / limit.value * 268;
const py = (value: number) => 280 - value / limit.value * 232;
const yaw = ref(.7), pitch = ref(.58);
const project = (a: number, b: number, c: number) => {
  const nx = a / limit.value - .5, ny = b / limit.value - .5;
  const depth = nx * Math.sin(yaw.value) + ny * Math.cos(yaw.value);
  return { x: 210 + (nx * Math.cos(yaw.value) - ny * Math.sin(yaw.value)) * 226,
    y: 228 + depth * 130 * Math.sin(pitch.value) - c / limit.value * 190 * Math.cos(pitch.value), depth };
};
const pointString = (vertices: number[][]) => vertices.map(p => { const v = project(p[0]!, p[1]!, p[2]!); return `${v.x},${v.y}`; }).join(' ');
const mesh = computed(() => {
  const faces: { points: string; color: string; depth: number }[] = []; const step = limit.value / 8;
  for (let a = 0; a < 8; a++) for (let b = 0; b < 8; b++) {
    const vertices = [[a * step, b * step], [(a + 1) * step, b * step], [(a + 1) * step, (b + 1) * step], [a * step, (b + 1) * step]];
    for (const indices of [[0, 1, 2], [0, 2, 3]]) {
      const triangle = indices.map(i => { const p = vertices[i]!; return [p[0]!, p[1]!, Math.max(p[0]!, p[1]!)]; });
      faces.push({ points: pointString(triangle), color: a >= b ? '#b9d7d2' : '#c9c6e2', depth: triangle.reduce((n, p) => n + project(p[0]!, p[1]!, p[2]!).depth, 0) });
    }
  }
  return faces.sort((a, b) => a.depth - b.depth);
});
const marker = computed(() => project(x.value, y.value, lower.value));
let drag: { x: number; y: number; yaw: number; pitch: number } | null = null;
function pointerDown(e: PointerEvent) { if (!e.isPrimary) return; drag = { x: e.clientX, y: e.clientY, yaw: yaw.value, pitch: pitch.value }; (e.currentTarget as SVGElement).setPointerCapture(e.pointerId); }
function pointerMove(e: PointerEvent) { if (!drag) return; yaw.value = drag.yaw + (e.clientX - drag.x) / 140; pitch.value = Math.min(1.15, Math.max(.2, drag.pitch + (e.clientY - drag.y) / 200)); }
function rotate(e: KeyboardEvent) { if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)) return; e.preventDefault(); yaw.value += e.key === 'ArrowLeft' ? -.12 : e.key === 'ArrowRight' ? .12 : 0; pitch.value = Math.max(.2, Math.min(1.15, pitch.value + (e.key === 'ArrowUp' ? .08 : e.key === 'ArrowDown' ? -.08 : 0))); }
</script>

<template>
  <div class="decision-lab" :class="{ review }">
    <p v-show="!review">{{ experiment.instruction }}</p>
    <div v-show="!review || adjusting" class="presets" :aria-label="uiText('比较三种配置')">
      <button v-for="item in presets" :key="item[0]" :disabled="disabled" @click="preset(item[0])">{{ observed.includes(item[0]) ? '✓ ' : '' }}{{ uiText(item[1]) }}</button>
      <button class="challenge" :disabled="disabled" @click="state = { ...lab }">{{ uiText('恢复本轮实验起点') }}</button>
    </div>
    <div class="readouts" aria-live="polite">
      <div><small>{{ uiText('计算时间') }}</small><strong>{{ digits(x) }} <span>ms</span></strong><span>{{ state.work_gflops }} ÷ {{ state.compute_tflops }}</span></div>
      <div><small>{{ uiText('读取时间') }}</small><strong>{{ digits(y) }} <span>ms</span></strong><span>{{ state.read_gb }} ÷ {{ state.bandwidth_tb_s }}</span></div>
      <div class="bound"><small>{{ uiText('时间下界') }}</small><strong>{{ digits(lower) }} <span>ms</span></strong><span>{{ x > y ? uiText('当前计算更慢') : x < y ? uiText('当前读取更慢') : uiText('两项相等') }}</span></div>
    </div>
    <div class="plot-switch" :aria-label="uiText('查看实验图')">
      <button :aria-pressed="plot === 'objects'" @click="plot = 'objects'">{{ uiText('3D 知识对象') }}</button>
      <button :aria-pressed="plot === '2d'" @click="plot = '2d'">{{ uiText('2D 决策图') }}</button>
      <button :aria-pressed="plot === '3d'" @click="plot = '3d'">{{ uiText('3D 下界模型') }}</button>
      <button v-if="review" :aria-expanded="adjusting" @click="adjusting = !adjusting">{{ adjusting ? uiText('收起参数') : uiText('调整实验参数') }}</button>
    </div>
    <div class="plots" :class="{ objects: plot==='objects' }">
      <CourseResourceObjects v-show="plot==='objects'" :state="state" :compact="review" :scene-key="sceneKey" :presented="presented!==false && plot==='objects'" @question="text=>emit('question',text)" />
      <figure v-show="plot === '2d'"><figcaption><strong>{{ uiText('2D · 能否进入目标区域？') }}</strong><small>{{ uiText('横轴计算时间 · 纵轴读取时间') }}</small></figcaption>
        <svg viewBox="0 0 380 330" role="img" :aria-label="uiText('二维决策图：计算 {0} ms，读取 {1} ms，目标 {2} ms', [digits(x), digits(y), state.target_ms])">
          <rect :x="px(0)" :y="py(state.target_ms)" :width="px(state.target_ms) - px(0)" :height="py(0) - py(state.target_ms)" fill="#dcece4" />
          <g v-for="n in [0,1,2,3,4]" :key="n" class="grid"><line :x1="px(n * limit / 4)" y1="48" :x2="px(n * limit / 4)" y2="280" /><line x1="56" :y1="py(n * limit / 4)" x2="324" :y2="py(n * limit / 4)" /><text :x="px(n * limit / 4)" y="299" text-anchor="middle">{{ +(n * limit / 4).toFixed(1) }}</text><text x="45" :y="py(n * limit / 4) + 4" text-anchor="end">{{ +(n * limit / 4).toFixed(1) }}</text></g>
          <path d="M56 48 V280 H324" class="axis" /><line x1="56" y1="280" x2="324" y2="48" class="diagonal" />
          <text x="75" y="70" class="region">{{ uiText('读取更慢') }}</text><text x="255" y="263" class="region">{{ uiText('计算更慢') }}</text>
          <text x="65" :y="py(state.target_ms) + 18" class="target-label">{{ uiText('两项均 ≤ {0} ms', [state.target_ms]) }}</text>
          <line :x1="px(x)" :y1="py(0)" :x2="px(x)" :y2="py(y)" class="guide"/><line :x1="px(0)" :y1="py(y)" :x2="px(x)" :y2="py(y)" class="guide"/>
          <circle :cx="px(x)" :cy="py(y)" r="7" fill="#b26a38" stroke="white" stroke-width="2" />
          <text x="55" y="29">{{ uiText('读取 / ms') }}</text><text x="266" y="323">{{ uiText('计算 / ms') }}</text>
        </svg>
      </figure>
      <figure v-show="plot === '3d'"><figcaption><strong>{{ uiText('3D · 沿哪一方向才会降低？') }}</strong><small>z = max(x, y) · {{ uiText('拖动旋转，方向键也可操作') }}</small></figcaption>
        <svg class="surface" viewBox="0 0 420 330" role="img" tabindex="0" :aria-label="uiText('可旋转的三维时间下界曲面')" @pointerdown="pointerDown" @pointermove="pointerMove" @pointerup="drag = null" @pointercancel="drag = null" @keydown="rotate">
          <polygon :points="pointString([[0,0,0],[limit,0,0],[limit,limit,0],[0,limit,0]])" fill="#f3f4f1" stroke="#c8ceca"/>
          <polygon v-for="(face,i) in mesh" :key="i" :points="face.points" :fill="face.color" stroke="#fff" stroke-opacity=".35" stroke-width=".6" />
          <polyline :points="pointString([[0,0,0],[limit,limit,limit]])" fill="none" stroke="#666581" stroke-width="2"/>
          <polygon :points="pointString([[0,0,state.target_ms],[state.target_ms,0,state.target_ms],[state.target_ms,state.target_ms,state.target_ms],[0,state.target_ms,state.target_ms]])" fill="#74b08e" fill-opacity=".18" stroke="#4d8261" stroke-dasharray="4 3"/>
          <polyline :points="pointString([[x,y,0],[x,y,lower]])" stroke="#b26a38" stroke-dasharray="4 3" fill="none"/>
          <circle :cx="marker.x" :cy="marker.y" r="6" fill="#b26a38" stroke="white" stroke-width="2"/><text :x="marker.x + 10" :y="marker.y - 8" style="fill:#8d4c25">{{ digits(lower) }} ms</text>
          <g v-for="axis in [{p:[limit,0,0],t:uiText('计算 / ms')},{p:[0,limit,0],t:uiText('读取 / ms')},{p:[0,0,limit],t:uiText('下界 / ms')}]" :key="axis.t"><polyline :points="pointString([[0,0,0],axis.p])" class="axis" fill="none"/><text :x="project(axis.p[0]!,axis.p[1]!,axis.p[2]!).x" :y="project(axis.p[0]!,axis.p[1]!,axis.p[2]!).y + 18" text-anchor="middle">{{ axis.t }}</text></g>
        </svg>
        <button class="reset-view" @click="yaw=.7; pitch=.58">{{ uiText('恢复视角') }}</button>
      </figure>
    </div>
    <div v-if="batchMode" class="batch-readouts"><div><small>{{ uiText('本批输出数') }}</small><strong>{{ state.batch_size }}</strong></div><div><small>{{ uiText('每输出分摊下界') }}</small><strong>{{ digits(model.perOutput) }} ms</strong></div><div><small>{{ uiText('理想吞吐上界') }}</small><strong>{{ Number(model.throughput.toFixed(1)) }} token/s</strong></div><p>{{ uiText('每条请求仍需等待本批步骤完成；分摊量用于换算吞吐。') }}</p></div>
    <div class="target-result" :class="{ feasible }">{{ feasible ? uiText('两条必要条件已满足，下一步需要实测验证。') : uiText('至少一项超过目标，这个配置可以直接排除。') }}</div>
    <fieldset v-show="!review || adjusting" :disabled="disabled" class="controls"><legend>{{ uiText('改变条件，对照对象与图表') }}</legend><label v-if="batchMode" class="batch-control">{{ uiText('B · 本批请求数') }}<input type="number" :value="state.batch_size" min="1" max="256" step="1" @change="changeBatch" :aria-label="uiText('本批请求数')" /><small>{{ uiText('改变 B 时，计算量按当前每请求工作量同比变化，权重读取量保持不变。') }}</small></label><div v-for="control in controls" :key="control.key" class="control"><label :for="(inputPrefix ?? 'course-value') + '-' + control.key">{{ control.symbol }} · {{ uiText(control.title) }} <span>{{ control.unit }}</span></label><input :id="(inputPrefix ?? 'course-value') + '-' + control.key" type="number" :value="state[control.key]" :min="control.min" :max="control.key==='work_gflops' ? control.max * (state.batch_size ?? 1) : control.max" step="any" @change="setValue(control, $event)" /><input v-model.number="state[control.key]" type="range" :min="control.min" :max="control.key==='work_gflops' ? control.max * (state.batch_size ?? 1) : control.max" :step="control.step" :aria-label="uiText('{0}滑块', [uiText(control.title)])" /></div></fieldset>
    <div v-show="!review || adjusting" class="lab-actions"><button :disabled="disabled" @click="state.compute_tflops = Math.min(2000, state.compute_tflops * 2)">{{ uiText('当前算力 ×2') }}</button><button :disabled="disabled" @click="state.bandwidth_tb_s = Math.min(16, state.bandwidth_tb_s * 2)">{{ uiText('当前带宽 ×2') }}</button><button :disabled="disabled" @click="state.work_gflops = Math.min(4800 * (state.batch_size ?? 1), state.work_gflops * 2)">{{ uiText('当前计算量 ×2') }}</button><button class="save" :disabled="disabled || !dirty" @click="emit('save', { ...state })">{{ dirty ? uiText('保存这个实验配置') : uiText('实验配置已保存 ✓') }}</button></div>
    <p v-show="!review" class="note">{{ uiText('保存配置会保留这次观察的条件；拖动参数本身不代表你已理解。三维模型与二维图使用同一组计算，均表示充分重叠时的下界。') }}</p>
  </div>
</template>

<style scoped>
.plots{grid-template-columns:1fr!important}.plots svg{max-height:380px}.plot-switch{flex-wrap:wrap}.batch-readouts{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px;margin:14px 0;padding:12px;background:var(--surface-soft);border-radius:10px}.batch-readouts>div{display:grid;gap:4px}.batch-readouts small,.batch-readouts p,.batch-control small{font-size:10px;color:var(--muted)}.batch-readouts strong{font-size:13px}.batch-readouts p{grid-column:1/-1;margin:0}.batch-control{grid-column:1/-1}.batch-control input{max-width:180px}
.review.decision-lab{margin:0}.review .readouts{margin:0 0 10px}.review .readouts>div{padding:10px}.review .readouts strong{font-size:21px}.plot-switch{display:flex;gap:6px;margin-bottom:8px}.plot-switch button{flex:1;min-height:34px;padding:5px 8px;font-size:12px}.plot-switch button[aria-pressed=true]{color:var(--brand-accent);border-color:var(--brand-accent);background:var(--surface-soft)}.review .plots{grid-template-columns:1fr}.review .plots figcaption{padding:10px 12px 0;font-size:12px}.review .plots svg{height:260px}.review svg text{font-size:14px}.review .target-result{margin:8px 0;padding:8px 10px;font-size:11px}.review .controls{grid-template-columns:1fr}.review .presets{margin:10px 0}.review .reset-view{position:absolute;right:6px;bottom:4px;margin:0}.review .plots figure{padding-bottom:6px}

.controls .control{display:grid;grid-template-columns:1fr 110px;gap:8px;align-items:center}.controls .control label{display:flex;justify-content:space-between;gap:8px;font-size:12px}.controls .control label span{font-size:10px;color:var(--muted)}.controls input[type=number]{box-sizing:border-box;width:100%;min-height:36px;border:1px solid var(--line);border-radius:7px;background:var(--canvas);color:var(--ink);padding:5px 8px;font:inherit;font-size:12px}.controls input[type=range]{grid-column:1/-1}
.decision-lab{margin:20px 0;font-size:13px}.presets,.lab-actions{display:flex;flex-wrap:wrap;gap:8px}button{font:inherit;min-height:40px;border:1px solid var(--line);border-radius:8px;padding:8px 12px;background:var(--canvas);color:var(--ink);cursor:pointer}button:disabled{opacity:.45;cursor:default}.challenge,.save{border-color:var(--brand-accent);color:var(--brand-accent)}.readouts{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));margin:22px 0;border:1px solid var(--line);border-radius:12px;overflow:hidden}.readouts>div{display:grid;gap:4px;padding:14px 18px}.readouts>div+div{border-left:1px solid var(--line)}.readouts strong{font-size:25px;font-weight:550;line-height:1.4}.readouts span,.readouts small{font-size:11px;color:var(--muted)}.readouts .bound{background:var(--surface-soft)}.plots{display:grid;grid-template-columns:1fr 1fr;gap:12px}.plots figure{min-width:0;margin:0;border:1px solid var(--line);border-radius:12px;overflow:hidden;background:var(--canvas);position:relative}.plots figcaption{padding:16px 16px 0;display:grid;gap:5px}.plots small{font-size:10px;color:var(--muted)}svg{display:block;width:100%;height:auto;overflow:visible}svg text{font-size:11px;fill:var(--muted);font-family:inherit}.grid line{stroke:var(--line);stroke-width:.6}.grid text{font-size:10px}.axis{stroke:#7d8985;stroke-width:1.2;fill:none}.diagonal{stroke:#8b88a3;stroke-dasharray:5 4}.guide{stroke:#b26a38;stroke-dasharray:3 3;opacity:.7}svg .region{font-size:11px;fill:#757183}svg .target-label{font-size:10px;fill:#47765b}.surface{cursor:grab;touch-action:none}.surface:active{cursor:grabbing}.reset-view{display:block;margin:0 8px 8px auto;min-height:28px;padding:3px 6px;font-size:10px;border:0;background:transparent;color:var(--muted)}.target-result{padding:12px 16px;margin:12px 0 20px;border-left:3px solid #b26a38;background:var(--surface-soft);font-size:12px}.target-result.feasible{border-color:#5b8c70}.controls{padding:0;border:0;display:grid;grid-template-columns:1fr 1fr;gap:18px 25px;margin:22px 0}.controls legend{font-size:13px;font-weight:600;margin-bottom:16px}.controls label{display:grid;gap:8px;font-size:12px}.controls label>span{display:flex;justify-content:space-between;gap:8px}.controls output{font-variant-numeric:tabular-nums;color:var(--muted)}input[type=range]{width:100%;accent-color:var(--brand-accent);min-height:24px;margin:0}.note{font-size:11px;color:var(--muted);margin:14px 0}button:focus-visible,.surface:focus-visible{outline:2px solid var(--brand-accent);outline-offset:2px}@media(max-width:650px){.plots{grid-template-columns:1fr}.readouts>div{padding:12px 9px}.readouts strong{font-size:20px}.controls{grid-template-columns:1fr}.plots svg{max-height:340px}.lab-actions .save{width:100%}}
@container (max-width:779px){.review .plots figcaption{display:none}.review .readouts>div{padding:7px}.review .readouts>div>span{display:none}.review .readouts strong{font-size:20px}.review .plots svg{height:200px}.review .plot-switch button{font-size:11px;padding-inline:4px}.review .target-result{padding:6px 8px;margin:6px 0 0}.review .plots figure{padding-top:6px}}
</style>
