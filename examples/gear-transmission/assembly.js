import * as THREE from './vendor/three.module.min.js';
import { gearOutline, gearSpec, drivenAngle, clamp } from './model.js';

export function createAssembly(canvas, state, change) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.3;
  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-100, 100, 80, -80, 1, 1500);
  camera.up.set(0, 1, 0);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x778477, 2.7));
  const key = new THREE.DirectionalLight(0xfff8e8, 3.5);
  key.position.set(-80, 90, 160); key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048); key.shadow.camera.left = -140; key.shadow.camera.right = 140;
  key.shadow.camera.top = 120; key.shadow.camera.bottom = -120;
  key.shadow.camera.near = 1; key.shadow.camera.far = 500;
  key.shadow.bias = -.0004; key.shadow.normalBias = .1; key.shadow.radius = 3;
  scene.add(key);
  const rim = new THREE.DirectionalLight(0xe5f2f5, 2);
  rim.position.set(100, -30, 70); scene.add(rim);
  const environment = new THREE.Scene();
  environment.add(new THREE.Mesh(new THREE.BoxGeometry(1000, 1000, 1000), new THREE.MeshBasicMaterial({ color: 0xa7b7b6, side: THREE.BackSide })));
  for (const [x, y, z, w, h, color, intensity] of [[-200,150,150,180,400,0xffffff,5],[100,100,-100,150,500,0xf0e2c7,3],[200,-200,250,300,180,0xffffff,4]]) {
    const light = new THREE.Mesh(new THREE.PlaneGeometry(w,h), new THREE.MeshBasicMaterial({color, toneMapped:false}));
    light.material.color.multiplyScalar(intensity); light.position.set(x,y,z); light.lookAt(0,0,0); environment.add(light);
  }
  const pmrem = new THREE.PMREMGenerator(renderer), env = pmrem.fromScene(environment, .05);
  scene.environment = env.texture; pmrem.dispose();
  environment.traverse(o => { o.geometry?.dispose(); o.material?.dispose(); });
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(700, 700), new THREE.ShadowMaterial({ opacity: .14, color: 0x3b4d44 }));
  floor.position.z = -19; floor.receiveShadow = true; scene.add(floor);
  const assembly = new THREE.Group(); scene.add(assembly);
  let gears = [], lastTeeth = 0, width = 0, height = 0;

  function cylinder(radius, length, material) {
    const mesh = new THREE.Mesh(new THREE.CylinderGeometry(radius, radius, length, 48), material);
    mesh.rotation.x = Math.PI / 2; mesh.castShadow = true; mesh.receiveShadow = true; return mesh;
  }
  function makeGear(teeth, color, x) {
    const spec = gearSpec(teeth), carrier = new THREE.Group(), rotor = new THREE.Group();
    carrier.position.x = x; carrier.add(rotor);
    const outline = gearOutline(teeth, 12), shape = new THREE.Shape(outline.map(p => new THREE.Vector2(...p)));
    const bore = new THREE.Path(); bore.absarc(0,0,3.3,0,Math.PI*2,true); shape.holes.push(bore);
    for (let i = 0; i < 6; i++) {
      const a = i * Math.PI / 3, hole = new THREE.Path();
      hole.absarc(Math.cos(a) * spec.pitch * .53, Math.sin(a) * spec.pitch * .53, spec.pitch * .13, 0,Math.PI*2,true);
      shape.holes.push(hole);
    }
    const geometry = new THREE.ExtrudeGeometry(shape, { depth: 6, bevelEnabled: true, bevelSize: .16, bevelOffset: -.16, bevelThickness: .2, bevelSegments: 2, steps: 1, curveSegments: 20 });
    geometry.translate(0,0,-3);
    const face = new THREE.MeshStandardMaterial({color, metalness:.65,roughness:.32, envMapIntensity:.65});
    const side = new THREE.MeshStandardMaterial({color, metalness:.6,roughness:.4, envMapIntensity:.5});
    const wheel = new THREE.Mesh(geometry,[face,side]); wheel.castShadow=true;wheel.receiveShadow=true;rotor.add(wheel);
    const hubMat = new THREE.MeshStandardMaterial({color:0xb6c0ba, metalness:.85, roughness:.26, envMapIntensity:.8});
    const hub = cylinder(spec.pitch*.235,9,hubMat); hub.position.z=1;rotor.add(hub);
    const shaft = cylinder(2.9,32,new THREE.MeshStandardMaterial({color:0x99a4a0,metalness:.85,roughness:.3}));
    shaft.position.z=-3;carrier.add(shaft);
    const guide = new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(0,0,-19),new THREE.Vector3(0,0,62)]),new THREE.LineDashedMaterial({color:0x788a80,dashSize:1.3,gapSize:1.3,transparent:true,opacity:.5}));
    guide.computeLineDistances();carrier.add(guide);
    const cap = cylinder(spec.pitch*.15, .6, new THREE.MeshStandardMaterial({color:0x697976,metalness:.8,roughness:.4}));
    cap.position.z=5.8;rotor.add(cap);
    const slot = new THREE.Mesh(new THREE.BoxGeometry(spec.pitch*.19,.65,.2),new THREE.MeshStandardMaterial({color:0x273d3f,roughness:.8}));
    slot.position.z=6.2;rotor.add(slot);
    const marker = new THREE.Mesh(new THREE.CylinderGeometry(1.1,1.1,.35,24),new THREE.MeshStandardMaterial({color:0xfff9e5,roughness:.5}));
    marker.rotation.x=Math.PI/2;marker.position.set(spec.pitch*.8,0,3.35);rotor.add(marker);
    const circlePoints = Array.from({length:181},(_,i)=>new THREE.Vector3(spec.pitch*Math.cos(i*Math.PI/90),spec.pitch*Math.sin(i*Math.PI/90),3.7));
    const pitch = new THREE.Line(new THREE.BufferGeometry().setFromPoints(circlePoints),new THREE.LineDashedMaterial({color:0xffffff,dashSize:1,gapSize:.8,transparent:true,opacity:.75}));
    pitch.computeLineDistances();rotor.add(pitch);
    assembly.add(carrier);return {carrier,rotor,hub,cap,slot,pitch,spec,shaft,guide};
  }
  function rebuild() {
    assembly.traverse(o => { o.geometry?.dispose(); for(const m of (o.material ? Array.isArray(o.material) ? o.material : [o.material] : []))m.dispose(); });
    assembly.clear();
    const r1=gearSpec(20).pitch,r2=gearSpec(state.teeth).pitch;
    gears=[makeGear(20,0xd1a04c,-r2),makeGear(state.teeth,0x4f9298,r1)];lastTeeth=state.teeth;
  }
  function draw() {
    if(lastTeeth!==state.teeth)rebuild();
    const w=canvas.clientWidth,h=canvas.clientHeight;
    if(w!==width||h!==height){width=w;height=h;renderer.setSize(w,h,false);}
    const {yaw,tilt}=state.camera;
    camera.position.set(300*Math.sin(yaw)*Math.sin(tilt),-300*Math.cos(yaw)*Math.sin(tilt),300*Math.cos(tilt));
    camera.lookAt(0,0,0);camera.updateMatrixWorld();
    gears[0].rotor.rotation.z=state.angle;gears[1].rotor.rotation.z=drivenAngle(state.angle,state.teeth);
    for(const [i,g] of gears.entries()){
      g.rotor.position.z=state.explode*(i===0?15:32);
      g.hub.position.z=1+state.explode*9;g.cap.position.z=5.8+state.explode*16;g.slot.position.z=6.2+state.explode*16;
      g.pitch.visible=state.pitch;g.guide.visible=state.explode>0;
    }
    // Stable circular envelopes avoid camera breathing as the teeth rotate.
    // Project the two envelopes so every orbit and exploded state fits between the labels.
    let minX=Infinity,maxX=-Infinity,minY=Infinity,maxY=-Infinity;
    for(const g of gears)for(const x of [-g.spec.tip,g.spec.tip])for(const y of [-g.spec.tip,g.spec.tip])for(const z of [-19,Math.max(13,g.rotor.position.z+6.5+state.explode*16)]){
      const p=new THREE.Vector3(g.carrier.position.x+x,y,z).applyMatrix4(camera.matrixWorldInverse);
      minX=Math.min(minX,p.x);maxX=Math.max(maxX,p.x);minY=Math.min(minY,p.y);maxY=Math.max(maxY,p.y);
    }
    const vertical=Math.max((maxY-minY)*h/(h-150),(maxX-minX)*h/(w-40));
    const centerX=(minX+maxX)/2,centerY=(minY+maxY)/2-vertical*25/h;
    camera.left=centerX-vertical*w/h/2;camera.right=centerX+vertical*w/h/2;
    camera.top=centerY+vertical/2;camera.bottom=centerY-vertical/2;camera.updateProjectionMatrix();
    renderer.render(scene,camera);
  }
  let drag;
  canvas.addEventListener('pointerdown',e=>{drag={x:e.clientX,y:e.clientY,yaw:state.camera.yaw,tilt:state.camera.tilt};canvas.setPointerCapture(e.pointerId);});
  canvas.addEventListener('pointermove',e=>{if(!drag)return;state.camera.yaw=drag.yaw+(e.clientX-drag.x)*.009;state.camera.tilt=clamp(drag.tilt+(e.clientY-drag.y)*.006,0,1.22);change('camera');draw();});
  const release=()=>{drag=null;};canvas.addEventListener('pointerup',release);canvas.addEventListener('pointercancel',release);
  canvas.addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();state.camera.yaw+=(e.key==='ArrowLeft'?-.12:e.key==='ArrowRight'?.12:0);state.camera.tilt=clamp(state.camera.tilt+(e.key==='ArrowUp'?-.1:e.key==='ArrowDown'?.1:0),0,1.22);change('camera');draw();});
  new ResizeObserver(draw).observe(canvas);
  draw();return {draw,renderer};
}
