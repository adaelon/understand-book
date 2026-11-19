<script setup lang="ts">
import { computed, useId } from 'vue';
import type { CourseMathState } from '../generated/CourseMathState';
import { chord, descentPath, feasible, objective, type Point } from '../course-math';
const props=defineProps<{modelValue:CourseMathState;disabled?:boolean}>();
const emit=defineEmits<{'update:modelValue':[state:CourseMathState]}>();
const id=useId().replace(/[^a-z0-9]/gi,'');
const fmt=(v:number)=>Math.abs(v)<1e-9?'0':Number(v.toPrecision(5)).toString();
function change(key:string,event:Event) { emit('update:modelValue',{...props.modelValue,[key]:Number((event.target as HTMLInputElement).value)} as CourseMathState); }
function preset(value:Partial<CourseMathState>) { emit('update:modelValue',{...props.modelValue,...value} as CourseMathState); }
const c=computed(()=>props.modelValue.kind==='convex_chord'?chord(props.modelValue):null);
const points=computed(()=>props.modelValue.kind==='gradient_descent'?descentPath(props.modelValue.step_size,props.modelValue.steps):[]);
const f=computed(()=>props.modelValue.kind==='feasible_directions'?feasible(props.modelValue):null);
const prev=computed(()=>points.value.at(-2)!);
const last=computed(()=>points.value.at(-1)!);
// Plot geometry is generated only from numeric state and fixed labels.
function plot(bounds:number[],draw:(m:(p:Point)=>Point)=>string,equal=false,labels=['x','y']) {
  let [xmin,xmax,ymin,ymax]=bounds;
  if(equal){const scale=Math.min(470/(xmax-xmin),280/(ymax-ymin)),cx=(xmin+xmax)/2,cy=(ymin+ymax)/2;xmin=cx-235/scale;xmax=cx+235/scale;ymin=cy-140/scale;ymax=cy+140/scale;}
  const m=(p:Point)=>({x:55+(p.x-xmin)/(xmax-xmin)*470,y:305-(p.y-ymin)/(ymax-ymin)*280});
  let grid='';
  for(let i=0;i<=4;i++){const x=xmin+(xmax-xmin)*i/4,y=ymin+(ymax-ymin)*i/4;grid+=`<path class="grid" d="M${55+i*117.5} 25V305 M55 ${305-i*70}H525"/><text x="${55+i*117.5}" y="328" text-anchor="middle">${Number(x.toPrecision(3))}</text><text x="47" y="${309-i*70}" text-anchor="end">${Number(y.toPrecision(3))}</text>`;}
  grid+=`<text x="536" y="328">${labels[0]}</text><text x="55" y="16">${labels[1]}</text>`;
  const o=m({x:0,y:0});
  return `<defs><clipPath id="clip${id}"><rect x="55" y="25" width="470" height="280"/></clipPath></defs>${grid}<g clip-path="url(#clip${id})"><path class="axis" d="M55 ${o.y}H525 M${o.x} 25V305"/>${draw(m)}</g>`;
}
const line=(a:Point,b:Point,cls:string,dash=false)=>`<line class="${cls}" x1="${a.x}" y1="${a.y}" x2="${b.x}" y2="${b.y}"${dash?' stroke-dasharray="5 5"':''}/>`;
const dot=(p:Point,cls:string,r=5)=>`<circle class="${cls}" cx="${p.x}" cy="${p.y}" r="${r}"/>`;
const path=(p:Point[],cls:string,close=false)=>`<path class="${cls}" d="${p.map((q,i)=>`${i?'L':'M'}${q.x},${q.y}`).join(' ')}${close?'Z':''}"/>`;
function arrow(a:Point,b:Point,cls:string){const r=Math.atan2(b.y-a.y,b.x-a.x),h=10;return line(a,b,cls)+path([{x:b.x-h*Math.cos(r-.4),y:b.y-h*Math.sin(r-.4)},b,{x:b.x-h*Math.cos(r+.4),y:b.y-h*Math.sin(r+.4)}],cls);}
const graphic=computed(()=>{
  const s=props.modelValue;
  if(s.kind==='convex_chord'){
    const v=c.value!,lim=s.function==='square'?[-.8,4.8]:[-9,9];
    return plot([-2.2,2.2,...lim],m=>path(Array.from({length:161},(_,i)=>{const x=-2+i/40;return m({x,y:v.f(x)});}),'curve')+line(m({x:s.a,y:v.f(s.a)}),m({x:s.b,y:v.f(s.b)}),'chord')+line(m({x:v.x,y:v.curve}),m({x:v.x,y:v.line}),'gap')+dot(m({x:s.a,y:v.f(s.a)}),'chord-fill')+dot(m({x:s.b,y:v.f(s.b)}),'chord-fill')+dot(m({x:v.x,y:v.curve}),'curve-fill')+dot(m({x:v.x,y:v.line}),'chord-fill'),false,['x','f(x)']);
  }
  if(s.kind==='gradient_descent'){
    const xmax=Math.max(11,...points.value.map(p=>Math.abs(p.x)*1.15));
    const ymax=Math.max(2,...points.value.map(p=>Math.abs(p.y)*1.15));
    const maxF=Math.max(110,...points.value.map(objective));
    return plot([-2,xmax,-ymax,ymax],m=>[.1,.3,.6,1].map(k=>path(Array.from({length:101},(_,i)=>{const t=i*Math.PI/50;return m({x:Math.sqrt(maxF*k)*Math.cos(t),y:Math.sqrt(maxF*k/10)*Math.sin(t)});}),'contour')).join('')+path(points.value.map(m),'curve')+points.value.map((p,i)=>dot(m(p),i===points.value.length-1?'chord-fill':'curve-fill',i===points.value.length-1?6:3.5)).join('')+dot(m({x:0,y:0}),'optimum',5),true);
  }
  const v=f.value!;
  return plot([-1,3.4,-1,3.4],m=>{
    const lower=-10,upper=10;
    let result=path([{x:lower,y:lower},{x:upper,y:lower},{x:upper,y:2-upper},{x:2-upper,y:upper},{x:lower,y:upper}].map(m),'feasible-fill',true);
    result+=[.5,1.5,4].map(level=>path(Array.from({length:101},(_,i)=>{const t=i*Math.PI/50;return m({x:2+Math.sqrt(level)*Math.cos(t),y:1+Math.sqrt(level)*Math.sin(t)});}),'contour')).join('');
    result+=line(m({x:-10,y:12}),m({x:12,y:-10}),'boundary');
    for(let angle=-180;angle<180;angle+=2){const r=(angle+1)*Math.PI/180,dx=Math.cos(r),dy=Math.sin(r);if(dx+dy<=1e-8 && v.g.x*dx+v.g.y*dy < -1e-8){result+=path([m(v.p),m({x:v.p.x+.7*Math.cos(angle*Math.PI/180),y:v.p.y+.7*Math.sin(angle*Math.PI/180)}),m({x:v.p.x+.7*Math.cos((angle+2)*Math.PI/180),y:v.p.y+.7*Math.sin((angle+2)*Math.PI/180)})],'wedge',true);}}
    const n=Math.hypot(v.g.x,v.g.y);
    result+=arrow(m(v.p),m({x:v.p.x-v.g.x/n*.85,y:v.p.y-v.g.y/n*.85}),'chord');
    result+=arrow(m(v.p),m({x:v.p.x+v.d.x*.95,y:v.p.y+v.d.y*.95}),'curve');
    return result+dot(m({x:2,y:1}),'target',5)+dot(m(v.p),'curve-fill',6);
  },true);
});
const lineSearch=computed(()=>{
  if(!prev.value || props.modelValue.kind!=='gradient_descent')return '';
  const s=props.modelValue,p=prev.value,phi=(a:number)=>objective({x:p.x*(1-2*a),y:p.y*(1-20*a)}),max=Math.max(phi(0),phi(.16),1);
  return plot([0,.16,-max*.08,max*1.12],m=>path(Array.from({length:101},(_,i)=>m({x:i*.0016,y:phi(i*.0016)})),'curve')+line(m({x:s.step_size,y:0}),m({x:s.step_size,y:phi(s.step_size)}),'chord',true)+dot(m({x:s.step_size,y:phi(s.step_size)}),'chord-fill'),false,['α','φ(α)']);
});
</script>

<template>
  <section class="math-lab" aria-label="数学交互实验">
    <template v-if="modelValue.kind==='convex_chord' && c">
      <header><strong>把“弦在上方”变成一个可检查的量</strong><span>蓝色：函数曲线 · 橙色：两点连成的弦 · 紫色：高差</span></header>
      <div class="presets"><button :disabled="disabled" :aria-pressed="modelValue.function==='square'" @click="preset({function:'square'})">f(x) = x²</button><button :disabled="disabled" :aria-pressed="modelValue.function==='cube'" @click="preset({function:'cube'})">f(x) = x³</button><button :disabled="disabled" @click="preset({a:-1,b:1,t:.5})">对称两点</button><button :disabled="disabled" @click="preset({a:-1.5,b:-.5,t:.5})">负半轴两点</button></div>
      <svg viewBox="0 0 560 350" role="img" aria-label="函数曲线、弦及同一横坐标上的高差" v-html="graphic" />
      <div class="controls"><label>A 的横坐标 a <output>{{fmt(modelValue.a)}}</output><input type="range" min="-2" max="2" step=".05" :value="modelValue.a" :disabled="disabled" @input="change('a',$event)"></label><label>B 的横坐标 b <output>{{fmt(modelValue.b)}}</output><input type="range" min="-2" max="2" step=".05" :value="modelValue.b" :disabled="disabled" @input="change('b',$event)"></label><label>从 A 走向 B 的比例 t <output>{{fmt(modelValue.t)}}</output><input type="range" min="0" max="1" step=".01" :value="modelValue.t" :disabled="disabled" @input="change('t',$event)"></label></div>
      <dl><div><dt>横坐标 (1−t)a+tb</dt><dd>{{fmt(c.x)}}</dd></div><div><dt>曲线高度 f((1−t)a+tb)</dt><dd>{{fmt(c.curve)}}</dd></div><div><dt>弦高 (1−t)f(a)+tf(b)</dt><dd>{{fmt(c.line)}}</dd></div><div><dt>弦高 − 曲线高</dt><dd :class="{negative:c.gap< -1e-8}">{{fmt(c.gap)}}</dd></div></dl>
      <p class="observation">{{c.gap < -1e-8 ? '此处弦在曲线下方：这是检验凸性所需的一次反例。' : '这一次高差非负。换点继续检验；有限次观察不能替代对任意点的证明。'}}</p>
    </template>
    <template v-else-if="modelValue.kind==='gradient_descent'">
      <header><strong>同一个方向，迈多大一步？</strong><span>f(x,y)=x²+10y²，从 (10,1) 出发；每一步重新计算负梯度。</span></header>
      <div class="presets"><button v-for="step in [.02,.085,.1,.12]" :key="step" :disabled="disabled" :aria-pressed="modelValue.step_size===step" @click="preset({step_size:step})">α = {{step}}</button></div>
      <div class="controls"><label>固定步长 α <output>{{fmt(modelValue.step_size)}}</output><input type="range" min="0" max=".16" step=".001" :value="modelValue.step_size" :disabled="disabled" @input="change('step_size',$event)"></label><label>迭代次数 <output>{{modelValue.steps}}</output><input type="range" min="1" max="8" step="1" :value="modelValue.steps" :disabled="disabled" @input="change('steps',$event)"></label></div>
      <figure><figcaption>等高线与轨迹 · 橙点为当前点，圆环为最小点 (0,0)</figcaption><svg viewBox="0 0 560 350" role="img" aria-label="梯度下降轨迹及等高线" v-html="graphic" /></figure>
      <figure><figcaption>最后一步的直线搜索 · 横轴 α，纵轴 φ(α)=f(z−α∇f(z))</figcaption><svg viewBox="0 0 560 350" role="img" aria-label="最后一次更新对应的一维步长函数" v-html="lineSearch" /></figure>
      <dl><div><dt>更新前 z</dt><dd>({{fmt(prev.x)}}, {{fmt(prev.y)}})</dd></div><div><dt>更新后</dt><dd>({{fmt(last.x)}}, {{fmt(last.y)}})</dd></div><div><dt>更新前 f</dt><dd>{{fmt(objective(prev))}}</dd></div><div><dt>更新后 f</dt><dd>{{fmt(objective(last))}}</dd></div></dl>
      <p class="observation">{{objective(last)<objective(prev)-1e-8?'这一步函数值下降。继续比较后续各步，留意两个坐标的变化。':objective(last)>objective(prev)+1e-8?'这一步函数值上升。下降方向只保证足够小的正步长下降。':'这一步函数值没有明显变化。'}} 图会随轨迹调整尺度，请同时比较数值。</p>
    </template>
    <template v-else-if="modelValue.kind==='feasible_directions' && f">
      <header><strong>让目标下降，还得留在允许的区域</strong><span>min (x−2)²+(y−1)²，约束 x+y≤2；候选点在边界上。</span></header>
      <div class="presets"><button v-for="p in [1,1.5,2]" :key="p" :disabled="disabled" :aria-pressed="modelValue.position===p" @click="preset({position:p})">候选点 ({{p}},{{2-p}})</button></div>
      <svg viewBox="0 0 560 350" role="img" aria-label="可行半平面、等高线、负梯度和所选方向" v-html="graphic" />
      <p class="legend">浅绿色：可行域 · 深绿色扇形：可行且下降的方向 · 橙箭头：负梯度 · 蓝箭头：你的方向 · 空心点：(2,1)</p>
      <div class="controls"><label>边界位置 s，候选点 (s,2−s) <output>{{fmt(modelValue.position)}}</output><input type="range" min="-.5" max="2.5" step=".05" :value="modelValue.position" :disabled="disabled" @input="change('position',$event)"></label><label>方向角 θ（从 x 正轴逆时针） <output>{{modelValue.angle}}°</output><input type="range" min="-180" max="180" step="1" :value="modelValue.angle" :disabled="disabled" @input="change('angle',$event)"></label></div>
      <dl><div><dt>目标值</dt><dd>{{fmt(f.value)}}</dd></div><div><dt>方向导数 ∇f·d</dt><dd>{{fmt(f.derivative)}} · {{f.descending?'下降':'非严格下降'}}</dd></div><div><dt>约束变化 dₓ+dᵧ</dt><dd>{{fmt(f.constraint)}} · {{f.allowed?'可行':'不可行'}}</dd></div><div><dt>能否沿此方向改进</dt><dd>{{f.descending&&f.allowed?'可以（取足够小的步长）':'不能由这个方向改进'}}</dd></div></dl>
      <p class="observation">箭头只表示方向，长度经过归一化。这里是线性约束，dₓ+dᵧ≤0 才能从边界出发保持可行。</p>
    </template>
  </section>
</template>

<style scoped>
.math-lab header span,.math-lab dt,.math-lab figcaption,.math-lab .legend{color:var(--ink,#29313c);opacity:.68}
@media(max-width:600px){.math-lab svg :deep(text){font-size:20px}}
.math-lab{border:1px solid var(--line,#d6dce2);border-radius:12px;padding:20px;background:var(--canvas,#fff);color:var(--ink,#1b283b);display:grid;gap:16px;min-width:0;font:14px/1.65 var(--sans,sans-serif)}header{display:grid;gap:6px}header strong{font-size:18px}header span,.legend,figcaption{font-size:12px;color:var(--muted,#536070)}svg{display:block;width:100%;max-height:390px;overflow:visible}svg :deep(text){font:12px var(--sans,sans-serif);fill:var(--muted,#536070)}svg :deep(path),svg :deep(line){fill:none;stroke-width:2}svg :deep(.grid){stroke:var(--line,#dce2e8);stroke-width:.7}svg :deep(.axis){stroke:var(--muted,#8a94a2);stroke-width:1}svg :deep(.curve){stroke:#397dc9;stroke-width:2.7}svg :deep(.chord){stroke:#d48229;stroke-width:2.7}svg :deep(.gap){stroke:#8a5bb9;stroke-width:4}svg :deep(.curve-fill){fill:#397dc9}svg :deep(.chord-fill){fill:#d48229}svg :deep(.contour){stroke:#8c9cac;stroke-width:1.2;opacity:.65}svg :deep(.boundary){stroke:#4c8474}svg :deep(.feasible-fill){fill:#60aa8522;stroke:none}svg :deep(.wedge){fill:#28956a55;stroke:none}svg :deep(.target),svg :deep(.optimum){fill:var(--canvas,#fff);stroke:#5e6a78;stroke-width:2}.presets{display:flex;gap:8px;flex-wrap:wrap}button{font:inherit;font-size:12px;min-height:36px;border:1px solid var(--line,#ccd3df);border-radius:7px;padding:5px 12px;background:var(--surface-soft,#f5f7fa);color:inherit;cursor:pointer}button[aria-pressed=true]{border-color:#397dc9;background:#397dc91a}button:disabled{opacity:.55;cursor:default}.controls{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:16px}label{font-size:13px;display:block}output{float:right;font-variant-numeric:tabular-nums;font-weight:600}input{display:block;width:100%;margin:12px 0 0;accent-color:#397dc9}dl{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px;margin:0}dl div{padding:10px 12px;border-radius:8px;background:var(--surface-soft,#f3f6f8)}dt{font-size:12px;color:var(--muted,#526070)}dd{margin:4px 0 0;font-variant-numeric:tabular-nums;font-size:15px;overflow-wrap:anywhere}.negative{color:#b35b33}.observation{font-size:13px;border-left:3px solid #397dc9;padding:6px 12px;margin:0}.legend{margin:0}figure{margin:0}figcaption{margin-bottom:6px}@media(max-width:600px){.math-lab{padding:12px;gap:12px}.math-lab header strong{font-size:16px}dl{grid-template-columns:1fr 1fr}dd{font-size:13px}}
</style>
