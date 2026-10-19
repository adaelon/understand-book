import {createLinkedDemo,canvas2D,$,fmt,clamp} from './runtime.js';
import {springModel} from './spring-model.js';
const SCALE=40,TAU=2*Math.PI;
const topic={
  name:'弹簧振子',defaults:{mass:1,stiffness:12,amplitude:.6},derive:springModel,
  period:s=>TAU*Math.sqrt(s.mass/s.stiffness),
  createObjects(scene){
    const metal=new THREE.MeshStandardMaterial({color:0x9daaa4,metalness:.8,roughness:.28});
    const body=new THREE.Mesh(new THREE.BoxGeometry(19,24,20),new THREE.MeshStandardMaterial({color:0x4f9298,metalness:.5,roughness:.3}));body.castShadow=true;body.receiveShadow=true;scene.add(body);
    const top=new THREE.Mesh(new THREE.BoxGeometry(19.5,24.5,1),new THREE.MeshStandardMaterial({color:0x72a4a4,metalness:.65,roughness:.3}));top.position.z=10.5;scene.add(top);
    const wall=new THREE.Mesh(new THREE.BoxGeometry(5,39,32),metal);wall.position.x=-70;wall.castShadow=true;scene.add(wall);
    const rails=[];for(const y of [-10,10]){const rail=new THREE.Mesh(new THREE.CylinderGeometry(1.2,1.2,150,32),metal);rail.rotation.z=TAU/4;rail.position.set(0,y,-12);scene.add(rail);rails.push(rail);}
    // Fixed topology: stretch the local coil along x; no geometry is rebuilt per frame.
    class Helix extends THREE.Curve {getPoint(t,target=new THREE.Vector3()){return target.set(60*t,6*Math.cos(t*TAU*10),6*Math.sin(t*TAU*10));}}
    const spring=new THREE.Mesh(new THREE.TubeGeometry(new Helix(),320,.65,10,false),new THREE.MeshStandardMaterial({color:0xd1a04c,metalness:.7,roughness:.3}));spring.castShadow=true;scene.add(spring);
    const guide=new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(0,0,-15),new THREE.Vector3(0,0,45)]),new THREE.LineDashedMaterial({color:0x788a80,dashSize:2,gapSize:2}));guide.computeLineDistances();scene.add(guide);
    return {body,top,wall,spring,rails,guide};
  },
  updateObjects(o,m,s){
    const x=m.x*SCALE,length=x+57.5;o.body.position.set(x,0,20*s.explode);o.top.position.set(x,0,10.5+38*s.explode);o.spring.position.set(-67.5,0,20*s.explode);
    // Keep circular wire thickness while changing helix pitch. Reuse buffers.
    const position=o.spring.geometry.attributes.position,normal=o.spring.geometry.attributes.normal;
    for(let i=0;i<=320;i++){
      const t=i/320,a=t*TAU*10,c=Math.cos(a),sn=Math.sin(a),speed=Math.hypot(length,6*TAU*10);
      const bx=-6*TAU*10/speed,by=-length*sn/speed,bz=length*c/speed;
      for(let j=0;j<=10;j++){const b=j/10*TAU,co=-Math.cos(b),si=Math.sin(b),nx=si*bx,ny=co*c+si*by,nz=co*sn+si*bz,index=i*11+j;
        position.setXYZ(index,length*t+.65*nx,6*c+.65*ny,6*sn+.65*nz);normal.setXYZ(index,nx,ny,nz);}
    }
    position.needsUpdate=true;normal.needsUpdate=true;o.spring.geometry.computeBoundingSphere();o.length=length;o.guide.position.x=x;o.guide.visible=s.explode>0;
  },
  bounds:(m,s)=>[[-76,-23,-19,76,23,Math.max(18,12+38*s.explode)]],
  inspect:o=>({massX:o.body.position.x,coilStart:o.spring.position.x,coilEnd:o.spring.position.x+o.length,coverZ:o.top.position.z}),
  drawDiagrams(m,s){
    const {ctx,w,h}=canvas2D($('diagram')),x0=38,x1=w-16,y0=27,y1=h-38,mid=(y0+y1)/2,amp=(y1-y0)/2;
    ctx.strokeStyle='#c5d2c9';ctx.beginPath();ctx.moveTo(x0,y0);ctx.lineTo(x0,y1);ctx.moveTo(x0,mid);ctx.lineTo(x1,mid);ctx.stroke();
    ctx.strokeStyle='#32858c';ctx.lineWidth=2;ctx.beginPath();for(let i=0;i<=160;i++){const x=x0+(x1-x0)*i/160,y=mid-s.amplitude*Math.cos(TAU*i/160)*amp/.8;i?ctx.lineTo(x,y):ctx.moveTo(x,y);}ctx.stroke();
    const phase=(s.semantic_state+s.transition_progress)%1,cx=x0+(x1-x0)*phase,cy=mid-m.x*amp/.8;
    ctx.setLineDash([3,4]);ctx.strokeStyle='#b78535';ctx.beginPath();ctx.moveTo(cx,y0);ctx.lineTo(cx,y1);ctx.stroke();ctx.setLineDash([]);ctx.fillStyle='#32858c';ctx.beginPath();ctx.arc(cx,cy,5,0,TAU);ctx.fill();
    ctx.fillStyle='#657572';ctx.textAlign='left';ctx.fillText('x / m',3,13);ctx.fillText('+0.8',1,y0);ctx.fillText('0',15,mid);ctx.fillText('−0.8',1,y1);ctx.fillText('0',x0,h-16);ctx.textAlign='right';ctx.fillText(`T = ${fmt(m.period,3)} s`,x1,h-16);
    const d=canvas2D($('detail')),c=d.ctx,left=26,right=d.w-26,bottom=196,height=145,maxEnergy=6.4,barW=Math.min(72,(d.w-90)/3),xs=[left,(left+right-barW)/2,right-barW];
    for(const [i,value,color,label] of [[0,m.potential,'#b78535','势能 U'],[1,m.kinetic,'#32858c','动能 K'],[2,m.total,'#95a69c','总能 E']]){c.fillStyle=color;c.fillRect(xs[i],bottom-value/maxEnergy*height,barW,value/maxEnergy*height);c.fillStyle='#203b3e';c.textAlign='center';c.fillText(`${fmt(value,3)} J`,xs[i]+barW/2,bottom-value/maxEnergy*height-16);c.fillText(label,xs[i]+barW/2,bottom+25);}
    c.strokeStyle='#c5d2c9';c.beginPath();c.moveTo(left-4,bottom);c.lineTo(right+4,bottom);c.stroke();c.fillStyle='#657572';c.textAlign='left';c.fillText('固定尺度：满高 6.4 J',left,18);
  },
  label:(k,v)=>k==='mass'?`${fmt(v)} kg`:k==='stiffness'?`${fmt(v)} N/m`:`${fmt(v)} m`,
  describe:(m,s)=>`<strong>位置 ${fmt(m.x,3)} m，回复力 ${fmt(m.force,3)} N</strong><p>速度 <output>${fmt(m.velocity,3)} m/s</output> · 周期 <output>${fmt(m.period,3)} s</output> · 已过 <output>${fmt(m.time,3)} s</output>。</p><p>势能 ${fmt(m.potential,3)} J + 动能 ${fmt(m.kinetic,3)} J = 总能 ${fmt(m.total,3)} J。${s.explode>0?'拆解状态暂停运动，盖板与导轨沿轴分离。':'通过平衡位置时最快；在两个端点，速度为零。'}</p>`
};
const demo=createLinkedDemo(topic);
let dragging=false;
function point(e){const r=$('diagram').getBoundingClientRect(),s=demo.snapshot(),p=clamp((e.clientX-r.left-38)/(r.width-54),0,.999);demo.seek({semantic_state:Math.min(s.semantic_state,3),transition_progress:p});}
$('diagram').addEventListener('pointerdown',e=>{dragging=true;$('diagram').setPointerCapture(e.pointerId);point(e);});
$('diagram').addEventListener('pointermove',e=>{if(dragging)point(e);});
for(const event of ['pointerup','pointercancel'])$('diagram').addEventListener(event,()=>{dragging=false;demo.commit();});
$('diagram').addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight'].includes(e.key))return;e.preventDefault();const s=demo.snapshot();demo.seek({semantic_state:0,transition_progress:s.semantic_state+s.transition_progress+(e.key==='ArrowRight'?.02:-.02)});demo.commit();});
