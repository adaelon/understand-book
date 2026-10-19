import { TAU, transmission, contactGeometry } from './model.js';
import { createDiagrams } from './diagrams.js';

const $ = id => document.getElementById(id);
const defaults = () => ({teeth:40,rpm:12,torque:2,angle:0,playing:!matchMedia('(prefers-reduced-motion: reduce)').matches,pitch:false,explode:0,camera:{yaw:-.28,tilt:.62},contact:.24,unwrap:1.1});
const state=defaults(),storageKey='gear-transmission-scene-v1';
let assembly=null,diagrams=null,unwrapPlaying=false,oneTurn=null,last=performance.now();
const visible=new Set();
const snapshot=()=>structuredClone(state);
const saved=()=>{try{const value=JSON.parse(localStorage.getItem(storageKey));return value?.version===1?value.state:null;}catch{return null;}};
const previous=saved();if(previous)Object.assign(state,previous,{playing:false});
const number=(v,decimals=1)=>Number(v.toFixed(decimals)).toString();
function write(name,value){document.querySelectorAll(`[data-read="${name}"]`).forEach(n=>n.textContent=value);}
function sync(){
  const m=transmission(state.teeth,state.rpm,state.torque),turns={20:'1',30:'⅔',40:'½',60:'⅓'}[state.teeth];
  for(const[name,value]of Object.entries({teeth:state.teeth,'teeth-unit':state.teeth+' 齿','input-rpm':state.rpm,'output-rpm':number(Math.abs(m.outputRpm),2),'input-torque':state.torque.toFixed(1),'output-torque':m.outputTorque.toFixed(1),power:m.inputPower.toFixed(2),ratio:number(m.ratio),turns,'speed-fraction':state.teeth===30?'2 / 3':`1 / ${m.ratio}`}))write(name,value);
  for(const[id,value]of Object.entries({speed:state.rpm,explode:state.explode,'torque-input':state.torque,'contact-phase':state.contact,unwrap:state.unwrap}))$(id).value=value;
  $('speed-value').value=`${state.rpm} rpm`;$('explode-value').value=`${Math.round(state.explode*100)}%`;
  $('torque-value').value=`${state.torque.toFixed(1)} N·m`;$('contact-value').value=`${Math.round(state.contact*100)}%`;$('unwrap-value').value=`${Math.round(state.unwrap*180/Math.PI)}°`;
  document.querySelectorAll('[data-teeth]').forEach(b=>b.setAttribute('aria-pressed',Number(b.dataset.teeth)===state.teeth));
  document.querySelectorAll('[data-view]').forEach(b=>b.setAttribute('aria-pressed',b.dataset.view==='front'?state.camera.tilt===0:state.camera.tilt!==0));
  $('pitch-toggle').setAttribute('aria-pressed',state.pitch);
  $('play').setAttribute('aria-pressed',state.playing);$('play').disabled=state.explode>0;
  $('play-icon').textContent=state.playing?'Ⅱ':'▶';$('play-text').textContent=state.playing?'暂停':'继续';
  $('motion-label').textContent=state.explode>0?'分解观察 · 传动已暂停':oneTurn?'单圈观察':state.playing&&state.rpm>0?'实时传动':'暂停观察';
  $('ratio-observation').textContent=state.teeth===20?'同样的齿数，同样的转速。':`B 每转 1 圈，A 转 ${number(m.ratio)} 圈。`;
  $('speed-a-bar').style.width='100%';$('speed-b-bar').style.width=(100/m.ratio)+'%';
  $('torque-a-bar').style.width=(100/m.ratio)+'%';$('torque-b-bar').style.width='100%';
  $('contact-ratio').textContent=contactGeometry(state.teeth,state.contact).contactRatio.toFixed(2);
  $('unwrap-play').textContent=unwrapPlaying?'暂停展开 Ⅱ':'自动展开 ↗';$('unwrap-play').setAttribute('aria-pressed',unwrapPlaying);
  $('restore').disabled=!saved();
}
function draw(){assembly?.draw();diagrams?.drawAll();}
function change(kind){if(!['camera','unwrap-tick'].includes(kind))oneTurn=null;sync();draw();}
diagrams=createDiagrams(state,change);sync();
for(const b of document.querySelectorAll('[data-teeth]'))b.addEventListener('click',()=>{state.teeth=Number(b.dataset.teeth);change('teeth');});
for(const[id,key]of Object.entries({speed:'rpm',explode:'explode','torque-input':'torque','contact-phase':'contact',unwrap:'unwrap'}))$(id).addEventListener('input',e=>{state[key]=Number(e.target.value);if(key==='explode'&&state.explode>0){state.playing=false;if(state.camera.tilt<.35)state.camera.tilt=.75;}if(key==='unwrap')unwrapPlaying=false;change(key);});
$('play').addEventListener('click',()=>{state.playing=!state.playing;change('play');});
$('pitch-toggle').addEventListener('click',()=>{state.pitch=!state.pitch;change('pitch');});
for(const b of document.querySelectorAll('[data-view]'))b.addEventListener('click',()=>{state.camera=b.dataset.view==='front'?{yaw:0,tilt:0}:{yaw:-.28,tilt:.62};change('camera');});
$('one-turn').addEventListener('click',()=>{state.playing=false;state.explode=0;state.angle=0;oneTurn={elapsed:0,duration:60/(state.rpm||12)};sync();draw();});
$('unwrap-play').addEventListener('click',()=>{unwrapPlaying=!unwrapPlaying;if(unwrapPlaying&&state.unwrap>=2.3)state.unwrap=0;sync();});
$('save').addEventListener('click',()=>{try{localStorage.setItem(storageKey,JSON.stringify({version:1,state:{...snapshot(),playing:false}}));$('save-status').textContent='当前现场已保存。再次打开会停在这里。';$('restore').disabled=false;}catch{$('save-status').textContent='浏览器未允许本地保存，请保留此页面。';}});
$('restore').addEventListener('click',()=>{const value=saved();if(value){Object.assign(state,value,{playing:false});unwrapPlaying=false;change('restore');$('save-status').textContent='已恢复保存的参数、视角和图解位置。';}});
$('reset').addEventListener('click',()=>{Object.assign(state,defaults());unwrapPlaying=false;change('reset');$('save-status').textContent='已恢复初始设置，之前保存的现场仍可恢复。';});
$('export').addEventListener('click',()=>{
  if(!assembly){$('save-status').textContent='三维画面尚未加载完成。';return;}
  const source=$('assembly'),originalRatio=assembly.renderer.getPixelRatio();
  assembly.renderer.setPixelRatio(1600/source.clientWidth);assembly.draw();
  const image=document.createElement('canvas');image.width=1600;image.height=1040;
  const ctx=image.getContext('2d');ctx.fillStyle='#f8f7f3';ctx.fillRect(0,0,image.width,image.height);
  ctx.fillStyle='#253b3d';ctx.font='600 54px "Segoe UI","Microsoft YaHei",sans-serif';ctx.fillText('齿轮的约定',85,105);
  ctx.font='24px "Segoe UI","Microsoft YaHei",sans-serif';ctx.fillStyle='#6b7775';ctx.fillText('速度与力量，如何交换？',85,155);
  const scale=Math.min(1500/source.width,650/source.height),w=source.width*scale,h=source.height*scale;
  ctx.drawImage(source,(1600-w)/2,200+(650-h)/2,w,h);
  assembly.renderer.setPixelRatio(originalRatio);assembly.draw();
  const m=transmission(state.teeth,state.rpm,state.torque);ctx.font='26px "Segoe UI","Microsoft YaHei",sans-serif';
  ctx.fillStyle='#ab792d';ctx.fillText(`主动轮 A  ·  20 齿  ·  ${state.rpm} rpm  ·  ${state.torque.toFixed(1)} N·m`,85,905);
  ctx.fillStyle='#337b80';ctx.fillText(`从动轮 B  ·  ${state.teeth} 齿  ·  ${number(Math.abs(m.outputRpm),2)} rpm  ·  ${m.outputTorque.toFixed(1)} N·m`,85,952);
  ctx.fillStyle='#6b7775';ctx.font='18px "Segoe UI","Microsoft YaHei",sans-serif';ctx.fillText(state.explode>0?'沿轴分解视图 · 传动已暂停；读数表示重新啮合时的理想关系。':'外啮合，转向相反 · 理想无损传动 · 20° 渐开线齿形',85,1000);
  image.toBlob(blob=>{if(!blob)return;const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download='齿轮的约定.png';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);$('save-status').textContent='已导出当前三维视角与传动参数。';},'image/png');
});
const observer=new IntersectionObserver(entries=>{for(const e of entries){if(e.isIntersecting){visible.add(e.target.id);if(e.target.id==='assembly')assembly?.draw();else diagrams.drawAll();}else visible.delete(e.target.id);}},{rootMargin:'100px'});
for(const id of ['assembly','ratio-scene','torque-scene','contact-scene','involute-scene'])observer.observe($(id));
document.addEventListener('visibilitychange',()=>{last=performance.now();});
function frame(now){const dt=Math.min(.08,(now-last)/1000);last=now;
  if(!document.hidden){
    let moved=false;
    if(oneTurn){oneTurn.elapsed+=dt;state.angle=Math.min(1,oneTurn.elapsed/oneTurn.duration)*TAU;moved=true;if(oneTurn.elapsed>=oneTurn.duration){oneTurn=null;sync();}}
    else if(state.playing&&state.explode===0&&['assembly','ratio-scene','torque-scene'].some(id=>visible.has(id))){state.angle+=dt*state.rpm*TAU/60;moved=state.rpm>0;}
    if(moved){if(visible.has('assembly'))assembly?.draw();if(visible.has('ratio-scene'))diagrams.ratioDraw();if(visible.has('torque-scene'))diagrams.torqueDraw();}
    if(unwrapPlaying&&visible.has('involute-scene')){state.unwrap=Math.min(2.3,state.unwrap+dt*.35);if(state.unwrap===2.3)unwrapPlaying=false;$('unwrap').value=state.unwrap;$('unwrap-value').value=`${Math.round(state.unwrap*180/Math.PI)}°`;diagrams.involuteDraw();if(!unwrapPlaying)sync();}
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
// Read-only observation for the demonstration's acceptance runner.
window.gearDemo=Object.freeze({snapshot,model:()=>transmission(state.teeth,state.rpm,state.torque)});
try{const{createAssembly}=await import('./assembly.js');assembly=createAssembly($('assembly'),state,change);document.body.dataset.render='webgl';}
catch(error){$('render-error').hidden=false;$('assembly').hidden=true;$('export').disabled=true;document.body.dataset.render='failed';console.error(error);}
document.body.dataset.ready='true';
