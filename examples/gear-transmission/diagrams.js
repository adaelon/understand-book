import { gearOutline, gearSpec, drivenAngle, transmission, contactGeometry, unwrapPoint, TAU } from './model.js';
const INK='#253b3d',MUTED='#72817a',GOLD='#d3a34f',TEAL='#6a9fa3',CORAL='#c65c44',PAPER='#efefea';
const paths=new Map();
function gearPath(teeth){
  if(paths.has(teeth))return paths.get(teeth);
  const points=gearOutline(teeth,14),path=new Path2D();
  points.forEach(([x,y],i)=>i?path.lineTo(x,y):path.moveTo(x,y));path.closePath();
  const r=gearSpec(teeth).pitch;
  path.moveTo(3.1,0);path.arc(0,0,3.1,0,TAU);path.closePath();
  for(let i=0;i<6;i++){const a=i*TAU/6,x=r*.53*Math.cos(a),y=r*.53*Math.sin(a),hr=r*.13;path.moveTo(x+hr,y);path.arc(x,y,hr,0,TAU);path.closePath();}
  paths.set(teeth,path);return path;
}
function setup(canvas){const dpr=Math.min(devicePixelRatio,2),w=canvas.clientWidth,h=canvas.clientHeight;if(canvas.width!==Math.round(w*dpr)||canvas.height!==Math.round(h*dpr)){canvas.width=Math.round(w*dpr);canvas.height=Math.round(h*dpr);}const ctx=canvas.getContext('2d');ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,w,h);return{ctx,w,h};}
function text(ctx,value,x,y,size=12,color=INK,align='center'){ctx.save();ctx.fillStyle=color;ctx.textAlign=align;ctx.textBaseline='middle';ctx.font=`${size}px "Segoe UI","Microsoft YaHei",sans-serif`;ctx.fillText(value,x,y);ctx.restore();}
function line(ctx,a,b,color=INK,width=1,dash=[]){ctx.save();ctx.beginPath();ctx.moveTo(...a);ctx.lineTo(...b);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.setLineDash(dash);ctx.stroke();ctx.restore();}
function dot(ctx,p,r,color,stroke){ctx.beginPath();ctx.arc(...p,r,0,TAU);ctx.fillStyle=color;ctx.fill();if(stroke){ctx.strokeStyle=stroke;ctx.lineWidth=1.5;ctx.stroke();}}
function arrow(ctx,a,b,color,width=1.5){line(ctx,a,b,color,width);const angle=Math.atan2(b[1]-a[1],b[0]-a[0]),len=7;line(ctx,b,[b[0]-len*Math.cos(angle-.45),b[1]-len*Math.sin(angle-.45)],color,width);line(ctx,b,[b[0]-len*Math.cos(angle+.45),b[1]-len*Math.sin(angle+.45)],color,width);}
function circle(ctx,p,r,color,width=1,dash=[]){ctx.save();ctx.beginPath();ctx.arc(...p,r,0,TAU);ctx.setLineDash(dash);ctx.strokeStyle=color;ctx.lineWidth=width;ctx.stroke();ctx.restore();}
function drawGear(ctx,teeth,cx,cy,scale,angle,color,pitch=true,marker=true){
  const g=gearSpec(teeth);ctx.save();ctx.translate(cx,cy);ctx.scale(scale,-scale);ctx.rotate(angle);
  ctx.fillStyle=color;ctx.strokeStyle=color===GOLD?'#a2752f':'#427a7f';ctx.lineWidth=.8/scale;ctx.fill(gearPath(teeth),'evenodd');ctx.stroke(gearPath(teeth));
  ctx.beginPath();ctx.arc(0,0,g.pitch*.235,0,TAU);ctx.arc(0,0,3.1,0,TAU,true);ctx.fillStyle='#aeb9b2';ctx.fill('evenodd');ctx.restore();
  if(pitch)circle(ctx,[cx,cy],g.pitch*scale,'#344d4d80',1,[4,4]);
  if(marker)dot(ctx,[cx+Math.cos(angle)*g.pitch*.8*scale,cy-Math.sin(angle)*g.pitch*.8*scale],3.2,'#fff9e9');
  dot(ctx,[cx,cy],2,INK);
}

export function createDiagrams(state,change){
  const ratio=document.querySelector('#ratio-scene'),torque=document.querySelector('#torque-scene'),contact=document.querySelector('#contact-scene'),involute=document.querySelector('#involute-scene');
  let ratioLayout;
  function ratioDraw(){
    const{ctx,w,h}=setup(ratio),m=transmission(state.teeth,state.rpm,state.torque);
    const scale=Math.min((w-34)/(2*m.center+8),(h-90)/(2*m.b.tip)),cy=h*.5;
    const x1=w/2-m.b.pitch*scale,x2=w/2+m.a.pitch*scale;
    ratioLayout={x1,x2,cy,scale,r1:m.a.pitch,r2:m.b.pitch};
    line(ctx,[x1-10,cy],[x2+10,cy],'#c5cdc5',1,[2,5]);
    drawGear(ctx,20,x1,cy,scale,state.angle,GOLD);drawGear(ctx,state.teeth,x2,cy,scale,drivenAngle(state.angle,state.teeth),TEAL);
    // The two traces cover the same input turn. A full small-circle turn corresponds
    // to only a fraction of the larger circle, without changing either marker's identity.
    let progress=((state.angle%TAU)+TAU)%TAU;if(Math.abs(progress)<1e-7&&Math.abs(state.angle)>1e-7)progress=TAU;
    const bAngle=drivenAngle(state.angle,state.teeth);
    for(const[x,r,from,to,reverse]of [[x1,m.a.pitch*.8*scale,-state.angle+progress,-state.angle,true],[x2,m.b.pitch*.8*scale,-bAngle-progress/m.ratio,-bAngle,false]]){
      ctx.save();ctx.strokeStyle='#fff9e9bb';ctx.lineWidth=2;ctx.beginPath();ctx.arc(x,cy,r,from,to,reverse);ctx.stroke();ctx.restore();
    }
    const p=[x1+m.a.pitch*scale,cy];dot(ctx,p,3.8,PAPER,INK);
    text(ctx,'20 齿',x1,cy-m.a.tip*scale-20,12,'#966d2b');
    text(ctx,`${state.teeth} 齿`,x2,cy-m.b.tip*scale-20,12,'#36777b');
    text(ctx,'A 转 1 圈',x1,h-20,12,'#966d2b');
    text(ctx,`B 转 ${state.teeth===20?'1':state.teeth===30?'⅔':state.teeth===40?'½':'⅓'} 圈`,x2,h-20,12,'#36777b');
    circle(ctx,[x1,cy],m.a.pitch*.32*scale,'#fff8',1);
  }
  function torqueDraw(){
    const{ctx,w,h}=setup(torque),m=transmission(state.teeth,state.rpm,state.torque);
    const scale=Math.min((w-40)/(2*m.center+8),(h-75)/(2*m.b.tip)),cy=h*.48;
    const x1=w/2-m.b.pitch*scale,x2=w/2+m.a.pitch*scale,px=x1+m.a.pitch*scale;
    ctx.globalAlpha=.45;drawGear(ctx,20,x1,cy,scale,state.angle,GOLD,false,false);drawGear(ctx,state.teeth,x2,cy,scale,drivenAngle(state.angle,state.teeth),TEAL,false,false);ctx.globalAlpha=1;
    line(ctx,[x1,cy],[px,cy],'#9b6f2b',3);line(ctx,[px,cy],[x2,cy],'#34777d',3);dot(ctx,[px,cy],4,PAPER,INK);
    const forceLength=24+state.torque*7;
    arrow(ctx,[px-4,cy-5],[px-4,cy-forceLength],'#34777d',2.2);arrow(ctx,[px+4,cy+5],[px+4,cy+forceLength],'#9b6f2b',2.2);
    text(ctx,`Fₜ = ${m.tangentForce.toFixed(0)} N`,px+12,cy-forceLength-12,11,'#36777b','left');
    text(ctx,'反作用力',px-10,cy+forceLength+14,10,'#966d2b','right');
    text(ctx,'rA', (x1+px)/2,cy+15,12,'#966d2b');text(ctx,'rB',(x2+px)/2,cy+15,12,'#36777b');
    text(ctx,`${state.torque.toFixed(1)} N·m`,x1,h-20,13,'#966d2b');text(ctx,`${m.outputTorque.toFixed(1)} N·m`,x2,h-20,13,'#36777b');
  }
  function contactDraw(){
    const{ctx,w,h}=setup(contact),c=contactGeometry(state.teeth,state.contact),scale=Math.min(w/24,h/19),px=w*.5,py=h*.52;
    const map=([x,y])=>[px+(x-c.a.pitch)*scale,py-y*scale];
    const a=map([0,0]),b=map([c.a.pitch+c.b.pitch,0]);
    ctx.save();ctx.beginPath();ctx.rect(0,0,w,h);ctx.clip();
    drawGear(ctx,20,...a,scale,c.angle,GOLD,true,false);drawGear(ctx,state.teeth,...b,scale,c.outputAngle,TEAL,true,false);
    line(ctx,map([c.a.pitch,-15]),map([c.a.pitch,15]),'#7c8b7e',1,[4,5]);
    line(ctx,map([c.a.pitch-20*c.normal[0],-20*c.normal[1]]),map([c.a.pitch+20*c.normal[0],20*c.normal[1]]),CORAL,1.6);
    for(const p of c.contacts)dot(ctx,map(p),3,'#fff8',CORAL);
    for(const end of [c.start,c.end]){const p=map(end);line(ctx,[p[0]-4,p[1]],[p[0]+4,p[1]],CORAL,1);}
    const q=map(c.point);dot(ctx,q,6,'#fffefa',CORAL);dot(ctx,q,2.5,CORAL);
    dot(ctx,[px,py],3.2,INK);
    ctx.restore();
    // Label plaques are kept in screen space so they stay readable at any width.
    const plaque=(value,x,y,color)=>{ctx.font='11px "Segoe UI","Microsoft YaHei",sans-serif';const width=ctx.measureText(value).width+14;ctx.fillStyle='#f8f7f3eb';ctx.fillRect(x-width/2,y-11,width,22);text(ctx,value,x,y,11,color);};
    plaque('P',px-17,py+18,INK);plaque('接触点',Math.min(w-40,q[0]+48),q[1]-13,CORAL);
    text(ctx,'A 的齿面',22,22,11,'#966d2b','left');text(ctx,'B 的齿面',w-22,h-18,11,'#36777b','right');
    const arcR=42;ctx.save();ctx.strokeStyle='#a95740';ctx.lineWidth=1;ctx.beginPath();ctx.arc(px,py,arcR,-Math.PI/2,-Math.PI/2+Math.PI/9);ctx.stroke();ctx.restore();
    plaque('20°',px+17,py-61,CORAL);
  }
  function involuteDraw(){
    const{ctx,w,h}=setup(involute),r=Math.min(w*.22,(h-45)/3.4),cx=w*.4,cy=h-r-20,t=state.unwrap;
    const map=([x,y])=>[cx+x*r,cy-y*r];
    ctx.fillStyle='#e0e5dc';ctx.beginPath();ctx.arc(cx,cy,r,0,TAU);ctx.fill();circle(ctx,[cx,cy],r,'#9daea0',1.3);
    const {point,tangent}=unwrapPoint(t),p=map(point),touch=map(tangent);
    line(ctx,[cx,cy],touch,'#a1aea1',1,[3,4]);
    ctx.save();ctx.strokeStyle='#aebcaf';ctx.lineWidth=2;ctx.setLineDash([4,5]);ctx.beginPath();for(let i=0;i<=150;i++){const q=map(unwrapPoint(2.3*i/150).point);i?ctx.lineTo(...q):ctx.moveTo(...q);}ctx.stroke();ctx.restore();
    ctx.beginPath();for(let i=0;i<=120;i++){const q=map(unwrapPoint(t*i/120).point);i?ctx.lineTo(...q):ctx.moveTo(...q);}ctx.strokeStyle=CORAL;ctx.lineWidth=2.5;ctx.stroke();
    line(ctx,touch,p,INK,1.6);dot(ctx,touch,3.2,PAPER,INK);dot(ctx,p,5.5,CORAL);dot(ctx,[cx,cy],2,MUTED);
    text(ctx,'基圆',cx,cy+20,12,MUTED);
    if(t>.25){const mid=[(touch[0]+p[0])/2,(touch[1]+p[1])/2];text(ctx,'绷紧的线',mid[0]+12,mid[1]-14,11,INK,'left');}
    text(ctx,'线端描出渐开线',w-18,h-20,11,CORAL,'right');
  }
  const drawAll=()=>{ratioDraw();torqueDraw();contactDraw();involuteDraw();};
  for(const canvas of [ratio,torque,contact,involute])new ResizeObserver(drawAll).observe(canvas);
  let drag;
  ratio.addEventListener('pointerdown',e=>{const rect=ratio.getBoundingClientRect(),x=e.clientX-rect.left,y=e.clientY-rect.top,l=ratioLayout;
    const first=Math.hypot(x-l.x1,y-l.cy)<l.r1*l.scale+10,second=Math.hypot(x-l.x2,y-l.cy)<l.r2*l.scale+10;if(!first&&!second)return;
    drag={x:first?l.x1:l.x2,y:l.cy,previous:Math.atan2(l.cy-y,x-(first?l.x1:l.x2)),factor:first?1:-state.teeth/20};state.playing=false;change('drag');ratio.setPointerCapture(e.pointerId);});
  ratio.addEventListener('pointermove',e=>{if(!drag)return;const rect=ratio.getBoundingClientRect(),a=Math.atan2(drag.y-(e.clientY-rect.top),e.clientX-rect.left-drag.x);let d=a-drag.previous;if(d>Math.PI)d-=TAU;if(d<-Math.PI)d+=TAU;state.angle+=d*drag.factor;drag.previous=a;change('angle');});
  ratio.addEventListener('pointerup',()=>drag=null);ratio.addEventListener('pointercancel',()=>drag=null);
  ratio.addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight'].includes(e.key))return;e.preventDefault();state.playing=false;state.angle+=(e.key==='ArrowRight'?1:-1)*Math.PI/30;change('angle');});
  return {drawAll,ratioDraw,torqueDraw,contactDraw,involuteDraw};
}
