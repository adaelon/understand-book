import {createLinkedDemo,canvas2D,disposeObject,$,fmt,clamp} from './runtime.js';
import {gearSpec,gearOutline,transmission,drivenAngle,TAU} from './gear-model.js';

// Topic model: millimetres for geometry, rpm for speed, N·m for torque.
const derive=s=>({...transmission(s.teeth,s.rpm,s.torque),angle:TAU*(s.semantic_state+s.transition_progress),
  outputAngle:drivenAngle(TAU*(s.semantic_state+s.transition_progress),s.teeth)});
function cylinder(radius,length,material){const m=new THREE.Mesh(new THREE.CylinderGeometry(radius,radius,length,48),material);m.rotation.x=Math.PI/2;m.castShadow=true;m.receiveShadow=true;return m;}
function makeGear(teeth,color,x){
  const spec=gearSpec(teeth),carrier=new THREE.Group(),rotor=new THREE.Group();carrier.position.x=x;carrier.add(rotor);
  const shape=new THREE.Shape(gearOutline(teeth,12).map(p=>new THREE.Vector2(...p)));
  const bore=new THREE.Path();bore.absarc(0,0,3.3,0,TAU,true);shape.holes.push(bore);
  for(let i=0;i<6;i++){const a=i*TAU/6,hole=new THREE.Path();hole.absarc(Math.cos(a)*spec.pitch*.53,Math.sin(a)*spec.pitch*.53,spec.pitch*.13,0,TAU,true);shape.holes.push(hole);}
  const geometry=new THREE.ExtrudeGeometry(shape,{depth:6,bevelEnabled:true,bevelSize:.16,bevelOffset:-.16,bevelThickness:.2,bevelSegments:2,steps:1,curveSegments:20});geometry.translate(0,0,-3);
  const wheel=new THREE.Mesh(geometry,[new THREE.MeshStandardMaterial({color,metalness:.65,roughness:.32,envMapIntensity:.65}),new THREE.MeshStandardMaterial({color,metalness:.6,roughness:.4,envMapIntensity:.5})]);wheel.castShadow=true;wheel.receiveShadow=true;rotor.add(wheel);
  const metal=new THREE.MeshStandardMaterial({color:0xb6c0ba,metalness:.85,roughness:.26,envMapIntensity:.8});
  const hub=cylinder(spec.pitch*.235,9,metal);hub.position.z=1;rotor.add(hub);
  const shaft=cylinder(2.9,32,metal);shaft.position.z=-3;carrier.add(shaft);
  const cap=cylinder(spec.pitch*.15,.6,new THREE.MeshStandardMaterial({color:0x697976,metalness:.8,roughness:.4}));cap.position.z=5.8;rotor.add(cap);
  const slot=new THREE.Mesh(new THREE.BoxGeometry(spec.pitch*.19,.65,.2),new THREE.MeshStandardMaterial({color:0x273d3f,roughness:.8}));slot.position.z=6.2;rotor.add(slot);
  const marker=cylinder(1.1,.35,new THREE.MeshStandardMaterial({color:0xfff9e5,roughness:.5}));marker.position.set(spec.pitch*.8,0,3.35);rotor.add(marker);
  const guide=new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(0,0,-19),new THREE.Vector3(0,0,62)]),new THREE.LineDashedMaterial({color:0x788a80,dashSize:1.3,gapSize:1.3,transparent:true,opacity:.5}));guide.computeLineDistances();carrier.add(guide);
  return {carrier,rotor,hub,cap,slot,spec,guide};
}
const outlines=new Map();
function outline(n){if(!outlines.has(n)){const p=new Path2D(),pts=gearOutline(n);pts.forEach(([x,y],i)=>i?p.lineTo(x,y):p.moveTo(x,y));p.closePath();for(let i=0;i<6;i++){const a=i*TAU/6,r=gearSpec(n).pitch;p.moveTo(Math.cos(a)*r*.53+r*.13,Math.sin(a)*r*.53);p.arc(Math.cos(a)*r*.53,Math.sin(a)*r*.53,r*.13,0,TAU);}p.moveTo(3.3,0);p.arc(0,0,3.3,0,TAU);outlines.set(n,p);}return outlines.get(n);}
function arrow(ctx,x,y,dx,dy,color){ctx.strokeStyle=color;ctx.fillStyle=color;ctx.beginPath();ctx.moveTo(x,y);ctx.lineTo(x+dx,y+dy);ctx.stroke();if(Math.hypot(dx,dy)<.01)return;const a=Math.atan2(dy,dx);ctx.beginPath();ctx.moveTo(x+dx,y+dy);ctx.lineTo(x+dx-8*Math.cos(a-.45),y+dy-8*Math.sin(a-.45));ctx.lineTo(x+dx-8*Math.cos(a+.45),y+dy-8*Math.sin(a+.45));ctx.fill();}
const topic={
  name:'齿轮传动',defaults:{teeth:40,rpm:12,torque:2},derive,period:s=>s.rpm>0?60/s.rpm:Infinity,
  createObjects(scene){const group=new THREE.Group();scene.add(group);return {group,teeth:0,gears:[]};},
  updateObjects(o,m,s){
    if(o.teeth!==s.teeth){disposeObject(o.group);o.group.clear();o.gears=[makeGear(20,0xd1a04c,-m.b.pitch),makeGear(s.teeth,0x4f9298,m.a.pitch)];o.gears.forEach(g=>o.group.add(g.carrier));o.teeth=s.teeth;}
    o.gears[0].rotor.rotation.z=m.angle;o.gears[1].rotor.rotation.z=m.outputAngle;
    o.gears.forEach((g,i)=>{g.rotor.position.z=s.explode*(i===0?15:32);g.hub.position.z=1+s.explode*9;g.cap.position.z=5.8+s.explode*16;g.slot.position.z=6.2+s.explode*16;g.guide.visible=s.explode>0;});
  },
  bounds:(m,s)=>[[-m.b.pitch-m.a.tip,-m.a.tip,-19,-m.b.pitch+m.a.tip,m.a.tip,Math.max(13,6.5+s.explode*31)], [m.a.pitch-m.b.tip,-m.b.tip,-19,m.a.pitch+m.b.tip,m.b.tip,Math.max(13,6.5+s.explode*48)]],
  inspect:o=>({angles:o.gears.map(g=>g.rotor.rotation.z),centers:o.gears.map(g=>g.carrier.position.x),separation:o.gears.map(g=>g.rotor.position.z)}),
  drawDiagrams(m,s){
    const {ctx,w,h}=canvas2D($('diagram')),scale=Math.min((w-44)/(2*(m.a.pitch+m.b.pitch)+4),(h-65)/(2*m.b.tip));
    const cy=(h-35)/2, centers=[w/2-m.b.pitch*scale,w/2+m.a.pitch*scale];
    for(const [i,n,a,color] of [[0,20,m.angle,'#b78535'],[1,s.teeth,m.outputAngle,'#32858c']]){
      ctx.save();ctx.translate(centers[i],cy);ctx.scale(scale,-scale);ctx.rotate(a);ctx.fillStyle=color;ctx.fill(outline(n),'evenodd');ctx.restore();
      const radius=gearSpec(n).pitch*scale;ctx.strokeStyle='#e2e8df';ctx.setLineDash([3,4]);ctx.beginPath();ctx.arc(centers[i],cy,radius,0,TAU);ctx.stroke();ctx.setLineDash([]);
      ctx.fillStyle='#fff5d5';ctx.beginPath();ctx.arc(centers[i]+Math.cos(a)*radius*.8,cy-Math.sin(a)*radius*.8,3,0,TAU);ctx.fill();
      ctx.fillStyle=color;ctx.textAlign='center';ctx.fillText(`${n} 齿`,centers[i],h-24);
    }
    const d=canvas2D($('detail')),c=d.ctx, left=42,right=d.w-30, length=right-left;
    c.strokeStyle='#d4ded6';c.beginPath();c.moveTo(left,55);c.lineTo(right,55);c.moveTo(left,155);c.lineTo(right,155);c.stroke();
    const unit=length/6;for(const [y,value,color,name] of [[55,s.torque,'#b78535','输入'],[155,m.outputTorque,'#32858c','输出']]){
      arrow(c,left,y,value*unit,0,color);c.fillStyle=color;c.textAlign='left';c.fillText(name,left,y-24);c.fillText(`${fmt(value)} N·m`,left,y+27);
    }
    c.fillStyle='#657572';c.fillText(`同一切向力 ${fmt(m.tangentForce)} N`,left,d.h-20);
  },
  label:(k,v)=>k==='teeth'?`${v} 齿`:k==='rpm'?`${v} rpm`:`${fmt(v)} N·m`,
  describe:(m,s)=>`<strong>${m.ratio===1?'同样快，方向相反':`转速变为 1/${fmt(m.ratio)}，扭矩变为 ${fmt(m.ratio)} 倍`}</strong><p>输入 <output>${fmt(s.rpm)} rpm</output> → 输出 <output>${fmt(m.outputRpm)} rpm</output>；输出扭矩 <output>${fmt(m.outputTorque)} N·m</output>。负号表示反向。</p><p>同一节圆线速度 ${fmt(m.pitchSpeed,3)} m/s · 理想传递功率 ${fmt(m.inputPower)} W。${s.explode>0?'拆解状态暂停运动，沿轴分离只用于观察。':'两只轮的标记与三维模型保持同一转角。'}</p>`
};
const demo=createLinkedDemo(topic);
let dragging=false;
function drag(e){const s=demo.snapshot(),m=derive(s),r=$('diagram').getBoundingClientRect(),scale=Math.min((r.width-44)/(2*(m.a.pitch+m.b.pitch)+4),(r.height-65)/(2*m.b.tip));const cx=r.width/2-m.b.pitch*scale,cy=(r.height-35)/2;let a=Math.atan2(cy-(e.clientY-r.top),e.clientX-r.left-cx);const current=TAU*(s.semantic_state+s.transition_progress);a+=TAU*Math.round((current-a)/TAU);demo.seek({semantic_state:0,transition_progress:clamp(a/TAU,0,4)});}
$('diagram').addEventListener('pointerdown',e=>{dragging=true;$('diagram').setPointerCapture(e.pointerId);drag(e);});
$('diagram').addEventListener('pointermove',e=>{if(dragging)drag(e);});
for(const type of ['pointerup','pointercancel'])$('diagram').addEventListener(type,()=>{dragging=false;demo.commit();});
$('diagram').addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight'].includes(e.key))return;e.preventDefault();const s=demo.snapshot();demo.seek({semantic_state:0,transition_progress:s.semantic_state+s.transition_progress+(e.key==='ArrowRight'?.02:-.02)});demo.commit();});
