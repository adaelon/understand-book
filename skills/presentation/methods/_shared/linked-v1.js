// Author adaptation seam: change the topic's pure derive/draw functions and
// content. This clock, native controls and Reader bridge stay shared.
export const $ = id => document.getElementById(id);
export const clamp = (x,a,b) => Math.min(b,Math.max(a,x));
export const mix = (a,b,u) => a+(b-a)*u;
export const fmt = (x,d=3) => Number(x.toFixed(d)).toString();
export const C = {ink:'#233e40',muted:'#768683',teal:'#18857e',gold:'#c38a35',coral:'#cf6851',blue:'#5b83ae',line:'#d4ded7',paper:'#f5f4ed'};
export const line=(x1,y1,x2,y2,color=C.line,width=1,extra='')=>`<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${color}" stroke-width="${width}" ${extra}/>`;
export const rect=(x,y,w,h,fill,extra='')=>`<rect x="${x}" y="${y}" width="${Math.max(0,w)}" height="${Math.max(0,h)}" fill="${fill}" ${extra}/>`;
export const circle=(x,y,r,fill,extra='')=>`<circle cx="${x}" cy="${y}" r="${r}" fill="${fill}" ${extra}/>`;
export const text=(x,y,value,color=C.muted,size=20,extra='')=>`<text x="${x}" y="${y}" fill="${color}" font-size="${size}" ${extra}>${value}</text>`;
export const path=(d,color=C.teal,width=3,extra='')=>`<path d="${d}" fill="none" stroke="${color}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round" ${extra}/>`;
export const points=pts=>pts.map(p=>p.join(',')).join(' ');
export const plot=pts=>pts.map((p,i)=>(i?'L':'M')+p.join(',')).join(' ');
export const label=(x,y,v,c=C.ink)=>text(x,y,v,c,22,'text-anchor="middle"');

export function createDemo(topic){
  const defaults={...topic.defaults,semantic_state:0,transition_progress:0};
  const state={...defaults,...window.presentation?.initialState,playing:false};
  const fields=Object.keys(topic.defaults);
  const control=p=>`<label class="parameter" for="${p.id}"><span>${p.label}</span><output id="${p.id}-value"></output>${p.options?`<select id="${p.id}" data-param="${p.id}">${p.options.map(([v,t])=>`<option value="${v}">${t}</option>`).join('')}</select>`:p.type==='checkbox'?`<input id="${p.id}" data-param="${p.id}" type="checkbox">`:`<input id="${p.id}" data-param="${p.id}" type="range" min="${p.min}" max="${p.max}" step="${p.step}">`}</label>`;
  document.body.innerHTML=`<main><header><div class="eyebrow">EXPLORABLE EXPLANATIONS <span>${topic.number} / 09</span></div><p class="method-name">${topic.method}</p><h1>${topic.title}</h1><p class="lead">${topic.lead}</p></header>
  <section class="experiment" aria-label="${topic.mainLabel}"><div class="figure-heading"><span>01 / ${topic.mainLabel}</span><span class="live-tag" id="status">可操作</span></div><svg id="scene" viewBox="0 0 720 380" role="img" tabindex="0" aria-label="${topic.directHint}"></svg><p class="gesture">↔ ${topic.directHint}</p><div id="readout" aria-live="off"><div id="metrics" class="metrics"></div><p id="reading"></p></div></section>
  <section class="controls" aria-label="观察控制"><div class="transport"><button id="play" aria-pressed="false">播放</button><button id="back" aria-label="前一步">←</button><input id="position" aria-label="观察位置" type="range" min="0" max="${topic.max}" step="0.001"><button id="next" aria-label="后一步">→</button><button id="reset">重置</button></div><p class="position-row"><span>${topic.timelineLabel}</span><output id="position-label"></output></p><div class="parameters">${topic.parameters.map(control).join('')}</div><div class="presets">${topic.presets.map((p,i)=>`<button data-preset="${i}">${p.label}</button>`).join('')}</div></section>
  <section class="linked"><div class="essay"><p class="eyebrow">02 / ${topic.detailLabel}</p><h2>${topic.question}</h2><p>${topic.answer}</p><div class="formula">${topic.formula}</div><p class="small">${topic.detailCaption}</p></div><div class="detail-figure"><svg id="detail" viewBox="0 0 640 300" role="img" tabindex="0" aria-label="${topic.detailHint||'联动细节图；左右方向键移动观察位置'}"></svg><p id="detail-reading" class="small"></p></div></section>
  <section class="walkthrough"><p class="eyebrow">03 / 亲手试一试</p><div class="chapters">${topic.chapters.map((c,i)=>`<button data-chapter="${i}"><span class="chapter-number">0${i+1}</span><strong>${c.title}</strong><span>${c.body}</span><em>观察这一刻 ↗</em></button>`).join('')}</div></section><footer><p>${topic.takeaway}</p><details><summary>模型与适用范围</summary><p>${topic.limits}</p></details></footer></main>`;
  let frame=0,last=0,visible=true,disposed=false;
  const snapshot=()=>({...state});
  const commit=()=>window.presentation?.commitState();
  function position(q){q=clamp(q,0,topic.max);state.semantic_state=Math.floor(q);state.transition_progress=q-state.semantic_state;}
  function pause(){cancelAnimationFrame(frame);frame=0;state.playing=false;last=0;}
  function render(){
    const m=topic.derive(state);$('scene').innerHTML=topic.drawMain(m,state);$('detail').innerHTML=topic.drawDetail(m,state);
    $('metrics').innerHTML=topic.metrics(m,state).map(([label,value,unit=''])=>`<div><span>${label}</span><output>${value}<small>${unit}</small></output></div>`).join('');
    $('reading').textContent=topic.explain(m,state);$('detail-reading').textContent=topic.detailReading(m,state);
    $('position').value=state.semantic_state+state.transition_progress;$('position-label').textContent=topic.positionLabel(m,state);
    for(const p of topic.parameters){const el=$(p.id);if(p.type==='checkbox')el.checked=state[p.id];else el.value=state[p.id];$(p.id+'-value').textContent=p.options?'':p.format?p.format(state[p.id]):fmt(state[p.id]);}
    $('play').textContent=state.playing?'暂停':'播放';$('play').setAttribute('aria-pressed',String(state.playing));$('status').textContent=state.playing?'正在演示':'停下来，观察';
    $('play').disabled=state.semantic_state===topic.max;$('back').disabled=state.semantic_state===0&&state.transition_progress===0;$('next').disabled=state.semantic_state===topic.max;
    return m;
  }
  function seek({semantic_state,transition_progress=0}){pause();position(semantic_state+transition_progress);render();return snapshot();}
  function apply({values={},q=0}={}){pause();for(const key of fields)if(key in values)state[key]=values[key];position(q);render();}
  const current=()=>state.semantic_state+state.transition_progress;
  function tick(t){frame=0;if(!state.playing||!visible||document.hidden||disposed){last=0;return;}if(last)position(current()+Math.min((t-last)/1000,.06)/(topic.secondsPerStep||2));last=t;if(current()===topic.max)pause();render();if(state.playing)frame=requestAnimationFrame(tick);}
  function resume(){if(state.playing&&!frame&&!document.hidden&&visible&&!disposed){last=0;frame=requestAnimationFrame(tick);}}
  $('play').onclick=()=>{if(state.playing)pause();else{state.playing=true;resume();}render();commit();};
  $('back').onclick=()=>{seek({semantic_state:state.transition_progress>0?state.semantic_state:state.semantic_state-1});commit();};
  $('next').onclick=()=>{seek({semantic_state:state.semantic_state+1});commit();};
  $('reset').onclick=()=>{seek({semantic_state:0});commit();};
  $('position').oninput=e=>seek({semantic_state:Number(e.target.value)});$('position').onchange=commit;
  for(const p of topic.parameters){$(p.id).oninput=e=>apply({values:{[p.id]:p.type==='checkbox'?e.target.checked:typeof defaults[p.id]==='number'?Number(e.target.value):e.target.value}});$(p.id).onchange=commit;}
  document.querySelectorAll('[data-preset]').forEach(b=>b.onclick=()=>{apply(topic.presets[Number(b.dataset.preset)]);commit();});
  document.querySelectorAll('[data-chapter]').forEach(b=>b.onclick=()=>{apply(topic.chapters[Number(b.dataset.chapter)]);commit();});
  function direct(id,handler){const el=$(id);let down=false;
    const update=e=>{const p=new DOMPoint(e.clientX,e.clientY).matrixTransform(el.getScreenCTM().inverse());const change=handler?.(p,state,e);if(change)apply(change);};
    el.onpointerdown=e=>{if(e.button!==0)return;down=true;el.setPointerCapture(e.pointerId);update(e);};el.onpointermove=e=>{if(down)update(e);};
    el.onpointerup=el.onpointercancel=()=>{if(down){down=false;commit();}};
    el.onkeydown=e=>{if(!['ArrowRight','ArrowLeft','Home','End'].includes(e.key))return;e.preventDefault();const change=topic.key?.(id,e.key,state);if(change)apply(change);else seek({semantic_state:e.key==='Home'?0:e.key==='End'?topic.max:current()+(e.key==='ArrowRight'?.05:-.05)});commit();};
  }
  direct('scene',topic.pointer);direct('detail',topic.detailPointer);
  const visibility=()=>{if(document.hidden){cancelAnimationFrame(frame);frame=0;last=0;}else resume();};document.addEventListener('visibilitychange',visibility);
  const observer=new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;if(!visible){cancelAnimationFrame(frame);frame=0;last=0;}else resume();});observer.observe(document.querySelector('main'));
  window.addEventListener('pagehide',()=>{disposed=true;pause();observer.disconnect();document.removeEventListener('visibilitychange',visibility);},{once:true});
  window.presentationScene={seek,snapshot};window.explanationDemo={inspect:()=>structuredClone(topic.derive(state))};
  window.presentation?.registerStateReader(()=>({values:Object.fromEntries([...fields,'semantic_state','transition_progress'].map(k=>[k,state[k]])),visible_step:topic.positionLabel(topic.derive(state),state)}));
  window.presentation?.registerStateRestorer(saved=>{const values=saved.values.page;apply({values,q:(values.semantic_state??0)+(values.transition_progress??0)});});
  position(state.semantic_state+state.transition_progress);render();
}
