<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { t as uiText } from '../i18n';
import { createResourceScene } from '../course-resource-scene';
import { resourceModel, type ResourceObject } from '../course-resource-model';
import type { CourseResourceState } from '../generated/CourseResourceState';

const props=withDefaults(defineProps<{state?:CourseResourceState; compact?:boolean; presented?:boolean; sceneKey?:string}>(),{presented:true});
const emit=defineEmits<{question:[text:string]}>();
const canvas=ref<HTMLCanvasElement>(),failed=ref(false),ready=ref(false),playing=ref(false);
const selected=ref<ResourceObject>('weights'),yaw=ref(.18),tilt=ref(.62),explode=ref(.3),position=ref(0);
const model=computed(()=>props.state?resourceModel(props.state):undefined);
const fmt=(n:number)=>Number(n.toFixed(n<1?3:2));
const objects:{id:ResourceObject;name:string;color:string;meaning:string;relation:string}[]=[
  {id:'weights',name:'权重数据',color:'#6188a3',meaning:'模型的参数存放在设备存储中。读取量 R 表示这一步要搬运多少数据；它和存储容量是两个不同的量。',relation:'读取时间 = 读取量 R ÷ 带宽 W'},
  {id:'channel',name:'读取通路',color:'#ba8a40',meaning:'数据通过读取通路送往计算单元。带宽 W 表示每秒能送多少数据；提高算力不会自动把这条通路变快。',relation:'读取时间 = R / W'},
  {id:'compute',name:'计算阵列',color:'#5d8e7b',meaning:'计算单元对数据执行运算。计算量 F 是本步要完成的运算总量，算力 C 是处理速度。数据不足时，更多算力也会等待。',relation:'计算时间 = 计算量 F ÷ 算力 C'},
  {id:'requests',name:'请求与输出',color:'#c58a56',meaning:'每条请求在这一步产生一个输出。批内请求可以共享读取的权重，但都要完成本步计算，不能把分摊时间当成各自的等待时间。',relation:'每输出分摊量 = 整批时间 / 输出数'},
];
const current=computed(()=>objects.find(o=>o.id===selected.value)!);
const objectValues=computed(()=>{
  const s=props.state,m=model.value;if(!s||!m)return '';
  if(selected.value==='weights')return uiText('本批读取 {0} GB；共享一次读取。',[s.read_gb]);
  if(selected.value==='channel')return `${s.read_gb} GB ÷ ${s.bandwidth_tb_s} TB/s = ${fmt(m.read)} ms`;
  if(selected.value==='compute')return `${s.work_gflops} GFLOPs ÷ ${s.compute_tflops} TFLOP/s = ${fmt(m.compute)} ms`;
  return uiText('{0} 条请求 · {0} 个输出；每输出分摊下界 {1} ms。',[m.batch,fmt(m.perOutput)]);
});
let scene:Awaited<ReturnType<typeof createResourceScene>>|undefined,disposed=false,visible=false,starting=false;
let resize:ResizeObserver|undefined,observer:IntersectionObserver|undefined,frame=0,last=0;
let drag:{x:number;y:number;yaw:number;tilt:number;moved:boolean}|undefined;
const key=computed(()=>props.sceneKey?`understand-book:course-objects:${props.sceneKey}`:null);
function save(){if(key.value)sessionStorage.setItem(key.value,JSON.stringify({selected:selected.value,yaw:yaw.value,tilt:tilt.value,explode:explode.value,position:position.value}));}
function draw(){
  if(disposed||!visible||document.hidden||!props.presented)return;
  const m=model.value,t=(m?.lower??1)*position.value;
  scene?.draw({yaw:yaw.value,tilt:tilt.value,explode:explode.value,selected:selected.value,batch:m?.batch??1,
    readProgress:m?Math.min(t/m.read,1):0,computeProgress:m?Math.min(t/m.compute,1):0,finished:!!m&&position.value>=1});
}
// Pointer, range, animation and resize updates share one render of the latest state.
function requestDraw(){
  if(frame||disposed||!scene||!visible||document.hidden||!props.presented)return;
  frame=requestAnimationFrame(tick);
}
function cancelDraw(){cancelAnimationFrame(frame);frame=0;}
async function start(){
  if(!canvas.value||starting||scene||disposed||!visible||props.presented===false)return;
  starting=true;failed.value=false;
  try{const result=await createResourceScene(canvas.value);if(disposed){result.dispose();return;}scene=result;ready.value=true;requestDraw();}
  catch{if(!disposed)failed.value=true;}finally{starting=false;}
}
function stop(){const wasPlaying=playing.value;playing.value=false;if(wasPlaying)save();}
function tick(now:number){
  if(disposed||!visible||document.hidden||!props.presented){stop();cancelDraw();return;}
  // Advance and render in the same frame; a second animation loop would add a frame of lag.
  if(playing.value){position.value=Math.min(1,position.value+Math.min(now-last,80)/5000);last=now;if(position.value===1)stop();}
  draw();frame=0;
  if(playing.value)requestDraw();
}
function togglePlay(){if(playing.value){stop();return;}if(position.value===1)position.value=0;playing.value=true;last=performance.now();requestDraw();}
function seek(event:Event){stop();position.value=Number((event.target as HTMLInputElement).value);save();}
function select(id:ResourceObject){selected.value=id;save();}
function down(e:PointerEvent){if(!e.isPrimary)return;drag={x:e.clientX,y:e.clientY,yaw:yaw.value,tilt:tilt.value,moved:false};canvas.value?.setPointerCapture(e.pointerId);}
function move(e:PointerEvent){if(!drag)return;drag.moved ||= Math.hypot(e.clientX-drag.x,e.clientY-drag.y)>5;if(!drag.moved)return;yaw.value=drag.yaw+(e.clientX-drag.x)/160;tilt.value=Math.max(.16,Math.min(1.35,drag.tilt+(e.clientY-drag.y)/180));}
function up(e:PointerEvent){if(drag&&!drag.moved){const id=scene?.pick(e.clientX,e.clientY);if(id)selected.value=id;}drag=undefined;save();}
function rotate(e:KeyboardEvent){if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();yaw.value+=e.key==='ArrowLeft'?-.12:e.key==='ArrowRight'?.12:0;tilt.value=Math.max(.16,Math.min(1.35,tilt.value+(e.key==='ArrowUp'?.1:e.key==='ArrowDown'?-.1:0)));save();}
function ask(){emit('question',uiText('我想弄清“{0}”：{1}',[uiText(current.value.name),objectValues.value||uiText(current.value.relation)]));}
function visibility(){if(document.hidden){stop();cancelDraw();}else{void start();requestDraw();}}
watch([yaw,tilt,explode,selected,position,model],requestDraw,{flush:'sync'});
watch(()=>props.state,()=>{position.value=0;stop();save();},{deep:true});
watch(()=>props.presented,value=>{if(value===false){stop();cancelDraw();}else{void start();void nextTick(requestDraw);}});
onMounted(()=>{
  if(key.value){try{const s=JSON.parse(sessionStorage.getItem(key.value)??'null');if(s){if(objects.some(o=>o.id===s.selected))selected.value=s.selected;for(const [name,target] of [['yaw',yaw],['tilt',tilt],['explode',explode],['position',position]] as const)if(Number.isFinite(s[name]))target.value=s[name];}}catch{/* A removed browser snapshot simply starts with the default view. */}}
  observer=new IntersectionObserver(entries=>{visible=!!entries[0]?.isIntersecting;if(visible){void start();requestDraw();}else{stop();cancelDraw();}});if(canvas.value)observer.observe(canvas.value);
  resize=new ResizeObserver(requestDraw);if(canvas.value)resize.observe(canvas.value);document.addEventListener('visibilitychange',visibility);
});
onBeforeUnmount(()=>{stop();save();disposed=true;cancelDraw();observer?.disconnect();resize?.disconnect();document.removeEventListener('visibilitychange',visibility);scene?.dispose();});
</script>

<template>
  <section class="resource-objects" :class="{compact}" :aria-label="uiText('可操作的知识对象')">
    <header><strong>{{ uiText('数据怎样成为一次输出？') }}</strong><small>{{ uiText('GPU 资源结构示意 · 非比例布局') }}</small></header>
    <div class="object-tabs" :aria-label="uiText('选择知识对象')"><button v-for="item in objects" :key="item.id" :aria-pressed="selected===item.id" @click="select(item.id)"><i :style="{background:item.color}" />{{ uiText(item.name) }}</button></div>
    <div class="object-stage">
      <canvas ref="canvas" tabindex="0" :aria-label="uiText('三维对象模型，可拖动旋转、点击部件，方向键也可旋转')" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="drag=undefined" @keydown="rotate" />
      <div v-if="compact" class="compact-caption"><span>{{ objectValues || uiText(current.relation) }}</span><button @click="ask">{{ uiText('提问') }}</button></div>
      <p v-if="failed" class="scene-status">{{ uiText('三维画面暂不可用，仍可通过下方对象说明和图表学习。') }} <button @click="start">{{ uiText('重试') }}</button></p>
      <span v-else-if="!ready" class="scene-status">{{ uiText('正在打开对象模型…') }}</span>
      <span class="scene-hint">{{ uiText('拖动旋转 · 点击对象') }}</span>
      <span v-if="model && model.batch>16" class="scene-count">{{ uiText('共 {0} 条请求，图中显示前 16 条',[model.batch]) }}</span>
    </div>
    <details class="object-inspector" :open="!compact"><summary>{{ uiText('对象说明与播放') }}</summary>
    <div class="object-detail" aria-live="polite"><strong>{{ uiText(current.relation) }}</strong><p>{{ uiText(current.meaning) }}</p><p v-if="objectValues" class="object-values">{{ objectValues }}</p><button class="ask-object" @click="ask">{{ uiText('我想问这个对象 →') }}</button></div>
    <div class="view-controls"><label>{{ uiText('拆开数据层') }}<input v-model.number="explode" type="range" min="0" max="1" step=".01" @input="stop" @change="save" /></label><button @click="yaw=.18;tilt=.62;explode=.3;save()">{{ uiText('恢复视角') }}</button></div>
    <div v-if="model" class="object-time"><div><button :disabled="!ready" @click="togglePlay">{{ playing ? uiText('暂停观察') : uiText('播放资源进度') }}</button><span>{{ fmt(position*model.lower) }} / {{ fmt(model.lower) }} ms</span></div><input :value="position" @input="seek" type="range" min="0" max="1" step=".001" :aria-label="uiText('定位资源完成进度')" /><small>{{ uiText('两项资源按充分重叠的理想预算展示；真实执行还受依赖和开销影响。') }}</small></div>
    </details>
  </section>
</template>

<style scoped>
.scene-count{position:absolute;top:6px;left:10px;font-size:10px;color:var(--muted);pointer-events:none}
.compact-caption{position:absolute;bottom:0;left:8px;right:8px;display:flex;gap:6px;align-items:center;justify-content:space-between;background:var(--canvas);font-size:10px;padding:3px 5px}.compact-caption button{flex-shrink:0;min-height:25px;padding:3px 6px}.object-inspector summary{padding:6px 12px;font-size:11px;color:var(--muted);cursor:pointer;border-top:1px solid var(--line)}.resource-objects:not(.compact) .object-inspector summary{display:none}.resource-objects.compact header{display:none}.resource-objects.compact .object-stage{height:155px}.compact .scene-hint{display:none}.view-controls input{accent-color:var(--brand-accent)}

.resource-objects{min-width:0;border:1px solid var(--line);border-radius:12px;background:var(--canvas);overflow:hidden;font:13px/1.7 var(--sans);color:var(--ink)}header{display:flex;justify-content:space-between;gap:10px;flex-wrap:wrap;padding:15px 18px 0}header strong{font-size:14px}header small{font-size:10px;color:var(--muted)}.object-stage{height:300px;position:relative;background:radial-gradient(ellipse at 50% 60%,#edf0e9,transparent 70%)}canvas{width:100%;height:100%;display:block;touch-action:none;cursor:grab}canvas:active{cursor:grabbing}.stage-label{position:absolute;top:15px;font-size:11px;pointer-events:none}.memory-label{left:18px;color:#527c98}.compute-label{right:18px;color:#467b66}.scene-hint{position:absolute;bottom:4px;right:15px;font-size:10px;color:var(--muted);pointer-events:none}.scene-status{position:absolute;left:18px;right:18px;top:40%;padding:8px 12px;background:var(--canvas);font-size:12px}.object-tabs{display:flex;gap:5px;flex-wrap:wrap;padding:9px 14px;border-top:1px solid var(--line)}button{font:inherit;font-size:11px;min-height:34px;border:1px solid var(--line);border-radius:7px;background:var(--canvas);color:var(--ink);padding:6px 9px;cursor:pointer}button:disabled{opacity:.5;cursor:default}.object-tabs button{display:flex;gap:5px;align-items:center}.object-tabs button[aria-pressed=true]{border-color:var(--brand-accent);background:var(--surface-soft)}i{display:block;width:7px;height:7px;border-radius:2px}.object-detail{padding:5px 17px 12px}.object-detail strong{font-size:13px}.object-detail p{font-size:12px;margin:7px 0;white-space:pre-line}.object-values{color:var(--brand-accent);font-variant-numeric:tabular-nums}.ask-object{border:0;padding:3px 0;color:var(--brand-accent);background:transparent}.view-controls{display:flex;gap:14px;align-items:center;padding:10px 16px;border-top:1px solid var(--line);background:var(--surface-soft)}.view-controls label{display:flex;align-items:center;gap:8px;flex:1;min-width:0;font-size:11px}.view-controls input{min-width:30px;width:100%;flex:1}.object-time{padding:12px 16px}.object-time>div{display:flex;align-items:center;justify-content:space-between;gap:8px}.object-time input{display:block;width:100%;margin:13px 0;accent-color:var(--brand-accent)}.object-time small{font-size:10px;color:var(--muted)}.compact header{padding:9px 12px 0}.compact header small{display:none}.compact .object-stage{height:205px}.compact .object-detail p:not(.object-values){display:none}.compact .object-detail{padding:3px 12px}.compact .object-tabs{padding:6px;gap:3px}.compact .object-tabs button{padding:4px 6px;font-size:10px}.compact .object-time{padding:8px 12px}.compact .view-controls{padding:7px 12px}button:focus-visible,canvas:focus-visible,input:focus-visible{outline:2px solid var(--brand-accent);outline-offset:-2px}@media(max-width:480px){header{padding:12px 12px 0}.object-stage{height:240px}.compact .object-stage{height:180px}.object-tabs{padding:8px}.view-controls{padding:10px}.object-detail{padding:4px 12px 10px}}
@container(min-width:780px){.resource-objects.compact .object-stage{height:240px}}
</style>
