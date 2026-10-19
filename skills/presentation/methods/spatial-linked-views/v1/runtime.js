// Extracted from the accepted gear study. Topic code owns the mathematics and
// geometry; this file owns observation, synchronized rendering and host state.
export const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
export const $ = id => document.getElementById(id);
export const fmt = (x, digits = 2) => Number(x.toFixed(digits)).toString();

export function canvas2D(canvas) {
  const w = canvas.clientWidth, h = canvas.clientHeight, dpr = Math.min(devicePixelRatio, 2);
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr);
  }
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, w, h);
  ctx.font = '13px system-ui'; ctx.textBaseline = 'middle'; ctx.lineWidth = 1.5;
  return { ctx, w, h };
}

// The same studio and orthographic fit work for both adapters. Motion bounds
// come from the model, not the current rotated mesh (which causes breathing).
export function createStage(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping; renderer.toneMappingExposure = 1.3;
  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-100, 100, 80, -80, 1, 1500);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x778477, 2.7));
  const key = new THREE.DirectionalLight(0xfff8e8, 3.5);
  key.position.set(-80, 90, 160); key.castShadow = true;
  key.shadow.mapSize.set(1024, 1024);
  Object.assign(key.shadow.camera, { left: -160, right: 160, top: 150, bottom: -150, near: 1, far: 500 });
  key.shadow.bias = -.0004; key.shadow.normalBias = .1; scene.add(key);
  const rim = new THREE.DirectionalLight(0xe5f2f5, 2); rim.position.set(100, -30, 70); scene.add(rim);
  const environment = new THREE.Scene();
  environment.add(new THREE.Mesh(new THREE.BoxGeometry(1000, 1000, 1000), new THREE.MeshBasicMaterial({ color: 0xa7b7b6, side: THREE.BackSide })));
  for (const [x,y,z,w,h,color,intensity] of [[-200,150,150,180,400,0xffffff,5],[100,100,-100,150,500,0xf0e2c7,3],[200,-200,250,300,180,0xffffff,4]]) {
    const light = new THREE.Mesh(new THREE.PlaneGeometry(w,h), new THREE.MeshBasicMaterial({color,toneMapped:false}));
    light.material.color.multiplyScalar(intensity); light.position.set(x,y,z); light.lookAt(0,0,0); environment.add(light);
  }
  const pmrem = new THREE.PMREMGenerator(renderer), env = pmrem.fromScene(environment, .05);
  scene.environment = env.texture; pmrem.dispose(); disposeObject(environment);
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(700, 700), new THREE.ShadowMaterial({opacity:.14,color:0x3b4d44}));
  floor.position.z = -19; floor.receiveShadow = true; scene.add(floor);
  let width = 0, height = 0;
  return {
    scene, renderer, camera,
    draw(s, bounds) {
      const w = canvas.clientWidth, h = canvas.clientHeight;
      if (!w || !h) return;
      if (w !== width || h !== height) { width = w; height = h; renderer.setSize(w,h,false); }
      camera.position.set(300*Math.sin(s.yaw)*Math.sin(s.tilt), -300*Math.cos(s.yaw)*Math.sin(s.tilt), 300*Math.cos(s.tilt));
      camera.lookAt(0,0,0); camera.updateMatrixWorld();
      let x0=Infinity,x1=-Infinity,y0=Infinity,y1=-Infinity;
      for (const box of bounds) for (const x of [box[0],box[3]]) for (const y of [box[1],box[4]]) for (const z of [box[2],box[5]]) {
        const p = new THREE.Vector3(x,y,z).applyMatrix4(camera.matrixWorldInverse);
        x0=Math.min(x0,p.x);x1=Math.max(x1,p.x);y0=Math.min(y0,p.y);y1=Math.max(y1,p.y);
      }
      const span = Math.max((y1-y0)*h/(h-56),(x1-x0)*h/(w-48),20);
      const cx=(x0+x1)/2,cy=(y0+y1)/2;
      camera.left=cx-span*w/h/2;camera.right=cx+span*w/h/2;
      camera.top=cy+span/2;camera.bottom=cy-span/2;camera.updateProjectionMatrix();renderer.render(scene,camera);
    },
    dispose() { disposeObject(scene); env.dispose(); renderer.dispose(); }
  };
}

export function disposeObject(root) {
  const geometries = new Set(), materials = new Set();
  root.traverse(o => { if(o.geometry)geometries.add(o.geometry); for(const m of (o.material ? Array.isArray(o.material)?o.material:[o.material] : []))materials.add(m); });
  geometries.forEach(g=>g.dispose()); materials.forEach(m=>m.dispose());
}

export function createLinkedDemo(topic) {
  const defaults = {...topic.defaults, yaw:-.28, tilt:.62, explode:0, semantic_state:0, transition_progress:0};
  const initial = window.presentation?.initialState ?? {};
  const state = {...defaults, playing:false};
  for (const key of Object.keys(defaults)) if (initial[key] !== undefined) state[key]=initial[key];
  let stage, objects, derived, frame=0, previous=0, visible=true, drag;
  const canvas=$('spatial');
  try { stage=createStage(canvas); objects=topic.createObjects(stage.scene); }
  catch { $('fallback').hidden=false; canvas.hidden=true; $('front').disabled=true; $('view-reset').disabled=true; $('explode').disabled=true; }
  const values = () => Object.fromEntries(Object.keys(defaults).map(k=>[k,state[k]]));
  const snapshot = () => ({...values(),playing:state.playing});
  const commit = () => window.presentation?.commitState();
  function position(q) { q=clamp(q,0,4); state.semantic_state=Math.floor(q);state.transition_progress=q-state.semantic_state; }
  function render() {
    derived=topic.derive(state); // Exactly once: every visible representation consumes this result.
    if(stage){topic.updateObjects(objects,derived,state);stage.draw(state,topic.bounds(derived,state));}
    // The host can briefly hide/resize the iframe while restoring its layout.
    // ResizeObserver renders the latest model when the canvases become visible.
    if($('diagram').clientWidth && $('detail').clientWidth)topic.drawDiagrams(derived,state);
    $('readout').innerHTML=topic.describe(derived,state);
    for (const control of document.querySelectorAll('[data-param]')) {
      const key=control.dataset.param;control.value=state[key];
      const output=$(key+'-value');if(output)output.textContent=topic.label?.(key,state[key]) ?? fmt(state[key]);
    }
    $('position').value=state.semantic_state+state.transition_progress;
    $('position-value').textContent=`${fmt(state.semantic_state+state.transition_progress,3)} 周期`;
    $('explode').value=state.explode;$('explode-value').textContent=`${Math.round(state.explode*100)}%`;
    $('play').textContent=state.playing?'暂停':'播放';$('play').setAttribute('aria-pressed',String(state.playing));
    $('play').disabled=state.explode>0 || !Number.isFinite(topic.period(state));
  }
  function pause() { cancelAnimationFrame(frame);frame=0;state.playing=false; }
  function seek({semantic_state,transition_progress=0}) { pause();position(semantic_state+transition_progress);render();return snapshot(); }
  function schedule() { if(state.playing && visible && !document.hidden && !frame){previous=performance.now();frame=requestAnimationFrame(tick);} }
  function tick(now) {
    frame=0;position(state.semantic_state+state.transition_progress+Math.min((now-previous)/1000,.08)/topic.period(state));previous=now;
    if(state.semantic_state===4)state.playing=false;render();
    if(state.playing && visible && !document.hidden)frame=requestAnimationFrame(tick);else if(!state.playing)commit();
  }
  $('play').onclick=()=>{if(state.playing)pause();else{if(state.semantic_state===4)position(0);state.playing=true;schedule();}render();commit();};
  $('reset').onclick=()=>{seek({semantic_state:0});commit();};
  $('position').oninput=e=>seek({semantic_state:0,transition_progress:Number(e.target.value)});
  $('position').onchange=commit;
  for(const control of document.querySelectorAll('[data-param]')) {
    control.addEventListener('input',()=>{state[control.dataset.param]=Number(control.value);seek({semantic_state:0});});
    control.addEventListener('change',commit);
  }
  $('explode').oninput=e=>{pause();state.explode=Number(e.target.value);if(state.explode>0&&state.tilt<.35)state.tilt=.75;render();};
  $('explode').onchange=commit;
  $('front').onclick=()=>{state.yaw=0;state.tilt=0;render();commit();};
  $('view-reset').onclick=()=>{state.yaw=defaults.yaw;state.tilt=defaults.tilt;render();commit();};
  canvas.addEventListener('pointerdown',e=>{drag={x:e.clientX,y:e.clientY,yaw:state.yaw,tilt:state.tilt};canvas.setPointerCapture(e.pointerId);});
  canvas.addEventListener('pointermove',e=>{if(!drag)return;state.yaw=drag.yaw+(e.clientX-drag.x)*.009;state.tilt=clamp(drag.tilt+(e.clientY-drag.y)*.006,0,1.22);render();});
  const release=()=>{if(drag){drag=null;commit();}};
  canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);
  canvas.addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();state.yaw+=e.key==='ArrowLeft'?-.12:e.key==='ArrowRight'?.12:0;state.tilt=clamp(state.tilt+(e.key==='ArrowUp'?-.1:e.key==='ArrowDown'?.1:0),0,1.22);render();commit();});
  const resize=new ResizeObserver(render);for(const c of document.querySelectorAll('canvas'))resize.observe(c);
  const visibility=new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;if(visible){render();schedule();}else{cancelAnimationFrame(frame);frame=0;}});
  visibility.observe($('experiment'));
  document.addEventListener('visibilitychange',()=>{if(document.hidden){cancelAnimationFrame(frame);frame=0;}else{render();schedule();}});
  window.addEventListener('pagehide',()=>{pause();resize.disconnect();visibility.disconnect();stage?.dispose();},{once:true});
  window.presentationScene={seek,snapshot};
  window.presentation?.registerStateReader(()=>{render();return {values:values(),visible_step:`${topic.name} · ${fmt(state.semantic_state+state.transition_progress,3)} 周期`};});
  window.presentation?.registerStateRestorer(saved=>{const restored=saved.values.page;for(const key of Object.keys(defaults))if(restored?.[key]!==undefined)state[key]=restored[key];seek(state);});
  // Read-only inspection for checking actual geometry against independent equations.
  window.linkedDemo={inspect:()=>({model:{...derived},geometry:stage?topic.inspect(objects):null,webgl:!!stage})};
  seek(state);
  return {seek,commit,snapshot};
}
