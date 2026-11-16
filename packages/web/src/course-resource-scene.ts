import threeUrl from '../../../assets/presentation/three-0.180.0/three.min.js?url';
import type { ResourceObject } from './course-resource-model';

// The same pinned, offline Three build already used by Reader presentations.
type Three = Record<string, any>;
let loading: Promise<Three> | undefined;
function loadThree(): Promise<Three> {
  const global=window as typeof window & {THREE?:Three};
  if(global.THREE)return Promise.resolve(global.THREE);
  return loading ??= new Promise<Three>((resolve,reject)=>{
    const script=document.createElement('script');script.src=threeUrl;
    script.onload=()=>global.THREE ? resolve(global.THREE) : reject(new Error('3D library unavailable'));
    script.onerror=()=>{loading=undefined;script.remove();reject(new Error('3D library unavailable'));};
    document.head.append(script);
  });
}
export interface ResourceSceneState { yaw:number; tilt:number; explode:number; selected:ResourceObject; batch:number; readProgress:number; computeProgress:number; finished:boolean }
export async function createResourceScene(canvas:HTMLCanvasElement) {
  const T=await loadThree();
  const renderer=new T.WebGLRenderer({canvas,antialias:true,alpha:true});
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.toneMapping=T.ACESFilmicToneMapping;renderer.toneMappingExposure=.95;
  const scene=new T.Scene(),camera=new T.OrthographicCamera(-6,6,4,-4,.1,100);
  scene.add(new T.HemisphereLight(0xfffaf2,0x73887f,1.8));
  const key=new T.DirectionalLight(0xffffff,2.5);key.position.set(-4,9,7);scene.add(key);
  const fill=new T.DirectionalLight(0xdceaff,1.2);fill.position.set(5,2,-6);scene.add(fill);
  const group=new T.Group();scene.add(group);
  type Point = [number,number,number];
  const meshes:any[]=[],geometry=new T.BoxGeometry(1,1,1),matrix=new T.Matrix4(),color=new T.Color();
  function place(mesh:any,points:Point[],size:Point) {
    matrix.makeScale(...size);
    points.forEach((p,i)=>{matrix.setPosition(...p);mesh.setMatrixAt(i,matrix);});
    mesh.instanceMatrix.needsUpdate=true;
    // Exploding layers changes the bounds used for both culling and picking.
    mesh.computeBoundingSphere();
  }
  function paint(mesh:any,count:number,value:(i:number)=>number) {
    for(let i=0;i<count;i++)mesh.setColorAt(i,color.setHex(value(i)));
    mesh.instanceColor.needsUpdate=true;
  }
  function boxes(points:Point[],size:Point,hex:number,object?:ResourceObject) {
    const mesh=new T.InstancedMesh(geometry,new T.MeshStandardMaterial({roughness:.48,metalness:.16}),points.length);
    mesh.userData.object=object;place(mesh,points,size);paint(mesh,points.length,()=>hex);
    group.add(mesh);meshes.push(mesh);return mesh;
  }
  boxes([[0,-.55,0]],[9,.24,4.6],0xd5ded7);
  boxes([[-2.85,-.29,0]],[2.25,.28,2.5],0x4b6173,'weights');
  const weightPoints:Point[]=Array.from({length:36},(_,i)=>[-3.48+(i%3)*.63,.04+Math.floor(i/9)*.27,-.63+Math.floor(i%9/3)*.63]);
  const weights=boxes(weightPoints,[.56,.2,.56],0x6188a3,'weights');
  boxes([[2,-.21,0]],[2.6,.36,2.6],0x455d55,'compute');
  const tilePoints:Point[]=Array.from({length:16},(_,i)=>[1.13+(i%4)*.58,.13,-.87+Math.floor(i/4)*.58]);
  const tiles=boxes(tilePoints,[.49,.23,.49],0x5d8e7b,'compute');
  const bus=boxes(Array.from({length:4},(_,i)=>[-.35,-.12,-.69+i*.46]),[2.5,.08,.15],0xb88c4f,'channel');
  const requests=boxes(Array.from({length:16},(_,i)=>[-3.5+(i%8)*.38,.1+Math.floor(i/8)*.32,1.91]),[.29,.22,.58],0xd9a575,'requests');
  const outputs=boxes(Array.from({length:16},(_,i)=>[1+(i%8)*.38,.1+Math.floor(i/8)*.32,1.91]),[.25,.22,.35],0xd3dbd5,'requests');
  const packetPoints:Point[]=Array.from({length:4},(_,i)=>[-1,.1,-.69+i*.46]);
  const packets=boxes(packetPoints,[.16,.16,.16],0xf4d28c,'channel');
  for(const mesh of [weights,tiles,packets])mesh.instanceMatrix.setUsage(T.DynamicDrawUsage);
  const ray=new T.Raycaster(),pointer=new T.Vector2();let width=0,height=0,previous:ResourceSceneState|undefined;
  return {
    draw(s:ResourceSceneState) {
      const w=canvas.clientWidth,h=canvas.clientHeight;if(!w || !h)return;
      if(w!==width||h!==height){
        renderer.setSize(w,h,false);width=w;height=h;
        const half=Math.max(3.1,5.8*h/w);camera.left=-half*w/h;camera.right=half*w/h;camera.top=half;camera.bottom=-half;camera.updateProjectionMatrix();
      }
      if(!previous||s.yaw!==previous.yaw||s.tilt!==previous.tilt){
        camera.position.set(12*Math.sin(s.yaw)*Math.cos(s.tilt),12*Math.sin(s.tilt),12*Math.cos(s.yaw)*Math.cos(s.tilt));camera.lookAt(0,.5,0);
      }
      if(s.selected!==previous?.selected)for(const mesh of meshes){
        const selected=mesh.userData.object===s.selected;mesh.material.emissive.setHex(selected?0x51371d:0x000000);mesh.material.emissiveIntensity=selected?.24:0;
      }
      if(s.explode!==previous?.explode){
        weightPoints.forEach((p,i)=>{p[1]=.04+Math.floor(i/9)*(.27+s.explode*.43);});place(weights,weightPoints,[.56,.2,.56]);
        tilePoints.forEach(p=>{p[1]=.13+s.explode*.35;});place(tiles,tilePoints,[.49,.23,.49]);
      }
      if(!previous||Math.ceil(s.readProgress*36)!==Math.ceil(previous.readProgress*36))paint(weights,36,i=>i/36<s.readProgress?0xadc6d1:0x6188a3);
      if(!previous||Math.ceil(s.computeProgress*16)!==Math.ceil(previous.computeProgress*16))paint(tiles,16,i=>i/16<s.computeProgress?0xb3dac8:0x5d8e7b);
      if(!previous||Math.ceil(s.readProgress*4)!==Math.ceil(previous.readProgress*4))paint(bus,4,i=>s.readProgress>i/4?0xe5bb70:0xb88c4f);
      if(s.readProgress!==previous?.readProgress){
        packets.visible=s.readProgress>0&&s.readProgress<1;
        if(packets.visible){packetPoints.forEach((p,i)=>{p[0]=-1.55+((s.readProgress*6+i*.23)%1)*2.5;});place(packets,packetPoints,[.16,.16,.16]);}
      }
      if(s.batch!==previous?.batch)requests.count=outputs.count=Math.min(s.batch,16);
      if(s.finished!==previous?.finished)paint(outputs,16,()=>s.finished?0x71a68a:0xd3dbd5);
      previous={...s};
      renderer.render(scene,camera);
    },
    pick(x:number,y:number):ResourceObject|undefined {
      const r=canvas.getBoundingClientRect();pointer.set((x-r.left)/r.width*2-1,-(y-r.top)/r.height*2+1);ray.setFromCamera(pointer,camera);
      return ray.intersectObjects(meshes).find((hit:any)=>hit.object.visible && hit.object.userData.object)?.object.userData.object;
    },
    dispose(){for(const mesh of meshes){mesh.dispose();mesh.material.dispose();}geometry.dispose();renderer.dispose();renderer.forceContextLoss();},
  };
}
