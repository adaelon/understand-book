// Both studies redraw on input or resize. The camera changes only the view.
function inspectionStage(canvas,state,vertical,redraw,commit){
  if(!window.THREE)return null;
  let renderer;try{renderer=new THREE.WebGLRenderer({canvas,antialias:true,preserveDrawingBuffer:true})}catch{return null}
  const scene=new THREE.Scene(),camera=new THREE.OrthographicCamera(-1,1,1,-1,.01,150);
  camera.up.set(0,vertical==='z'?0:1,vertical==='z'?1:0);
  const clamp=(x,a,b)=>Math.max(a,Math.min(b,x));
  function orbit(dx,dy){state.yaw+=dx;state.pitch=clamp(state.pitch+dy,-1.15,1.15);redraw()}
  let drag=null;
  canvas.onpointerdown=e=>{if(drag||e.button!==0)return;canvas.setPointerCapture(e.pointerId);drag=[e.pointerId,e.clientX,e.clientY]};
  canvas.onpointermove=e=>{if(drag?.[0]!==e.pointerId)return;const dx=e.clientX-drag[1],dy=e.clientY-drag[2];drag=[e.pointerId,e.clientX,e.clientY];orbit(-dx*.009,dy*.009)};
  canvas.onlostpointercapture=()=>{if(drag){drag=null;commit()}};
  canvas.onkeydown=e=>{const delta={ArrowLeft:[-.14,0],ArrowRight:[.14,0],ArrowUp:[0,.1],ArrowDown:[0,-.1]}[e.key];if(delta){e.preventDefault();orbit(...delta);commit()}};
  let w=0,h=0;
  function draw(bounds){
    const width=canvas.clientWidth,height=canvas.clientHeight;if(!width||!height)return;
    if(width!==w||height!==h){w=width;h=height;renderer.setPixelRatio(Math.min(window.devicePixelRatio||1,2));renderer.setSize(w,h,false)}
    const center=bounds.getCenter(new THREE.Vector3()),c=Math.cos(state.pitch),s=Math.sin(state.pitch),a=state.yaw;
    const dir=vertical==='z'?new THREE.Vector3(c*Math.sin(a),-c*Math.cos(a),s):new THREE.Vector3(c*Math.sin(a),s,c*Math.cos(a));
    const right=new THREE.Vector3().crossVectors(camera.up,dir).normalize(),up=new THREE.Vector3().crossVectors(dir,right),v=new THREE.Vector3();
    let xr=0,yr=0;
    for(const x of [bounds.min.x,bounds.max.x])for(const y of [bounds.min.y,bounds.max.y])for(const z of [bounds.min.z,bounds.max.z]){
      v.set(x,y,z).sub(center);xr=Math.max(xr,Math.abs(v.dot(right)));yr=Math.max(yr,Math.abs(v.dot(up)));
    }
    const half=Math.max(yr,xr*h/w,.1)*1.1;camera.top=half;camera.bottom=-half;camera.right=half*w/h;camera.left=-camera.right;
    camera.position.copy(center).addScaledVector(dir,42);camera.lookAt(center);camera.updateProjectionMatrix();renderer.render(scene,camera)
  }
  const resize=new ResizeObserver(redraw);resize.observe(canvas);
  window.addEventListener('pagehide',e=>{if(!e.persisted){resize.disconnect();renderer.dispose()}});
  return {scene,camera,draw};
}
