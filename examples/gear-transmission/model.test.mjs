import test from 'node:test';
import assert from 'node:assert/strict';
import { gearSpec, gearOutline, transmission, contactGeometry, drivenAngle, unwrapPoint } from './model.js';

const close=(actual,expected,tolerance=1e-9)=>assert.ok(Math.abs(actual-expected)<=tolerance,`${actual} ≠ ${expected}`);
const transform=([x,y],angle,dx=0)=>[x*Math.cos(angle)-y*Math.sin(angle)+dx,x*Math.sin(angle)+y*Math.cos(angle)];
function distanceToOutline(point,outline){
  let best=Infinity;
  for(let i=0;i<outline.length;i++){
    const a=outline[i],b=outline[(i+1)%outline.length],dx=b[0]-a[0],dy=b[1]-a[1],length=dx*dx+dy*dy;
    const t=length?Math.max(0,Math.min(1,((point[0]-a[0])*dx+(point[1]-a[1])*dy)/length)):0;
    best=Math.min(best,Math.hypot(point[0]-a[0]-t*dx,point[1]-a[1]-t*dy));
  }return best;
}

test('standard geometry has the requested tooth count, module and pitch thickness',()=>{
  for(const teeth of [20,30,40,60]){
    const g=gearSpec(teeth),points=gearOutline(teeth);
    close(g.pitch,teeth);close(g.tip,teeth+2);close(g.root,teeth-2.5);
    close(2*g.halfTooth*g.pitch,Math.PI); // half of the circular pitch, mπ / 2
    close(Math.max(...points.map(([x,y])=>Math.hypot(x,y))),teeth+2,1e-8);
    assert.ok(points.every(([x,y])=>Number.isFinite(x)&&Number.isFinite(y)));
  }
});

test('independent speed, force and power balances cover every selectable ratio',()=>{
  for(const teeth of [20,30,40,60])for(const rpm of [0,1,12,24])for(const torque of [.5,2,5]){
    const m=transmission(teeth,rpm,torque);
    close(m.outputRpm*teeth,-rpm*20);
    close(m.outputTorque/(teeth/1000),torque/.02);
    close(m.outputTorque*Math.abs(m.outputRpm)*2*Math.PI/60,m.inputPower);
    close(m.tangentForce,torque/.02);
    close(drivenAngle(2*Math.PI,teeth)-drivenAngle(0,teeth),-2*Math.PI*20/teeth);
  }
});

test('the moving contact point lies on both rendered tooth flanks across the full engagement',()=>{
  for(const teeth of [20,30,40,60])for(const phase of [0,.08,.24,.5,.67,.92,1]){
    const c=contactGeometry(teeth,phase),a=gearOutline(20,14).map(p=>transform(p,c.angle)),b=gearOutline(teeth,14).map(p=>transform(p,c.outputAngle,20+teeth));
    // An independent point-to-polygon distance detects incorrect meshing phase or tooth shape.
    assert.ok(distanceToOutline(c.point,a)<.012,`A flank ${teeth}/${phase}`);
    assert.ok(distanceToOutline(c.point,b)<.012,`B flank ${teeth}/${phase}`);
    for(const p of c.contacts){assert.ok(distanceToOutline(p,a)<.012);assert.ok(distanceToOutline(p,b)<.012);}
    assert.ok(c.contacts.length>=1&&c.contacts.length<=2);
    assert.ok(c.contactRatio>1&&c.contactRatio<2);
    // This normal passes through P; its distances from the centres are the base radii.
    const distanceA=Math.abs(c.a.pitch*c.normal[1]),distanceB=Math.abs(c.b.pitch*c.normal[1]);
    close(distanceA,c.a.base);close(distanceB,c.b.base);
  }
});

test('unwinding uses a tangent string of the same length as the unwrapped arc',()=>{
  for(const t of [0,.1,1.1,2,2.3]){
    const {point:p,tangent:q}=unwrapPoint(t,25),v=[p[0]-q[0],p[1]-q[1]];
    close(Math.hypot(...q),25);close(Math.hypot(...v),25*t);
    close(q[0]*v[0]+q[1]*v[1],0,1e-8);
    if(t>.01){const before=unwrapPoint(t-.00001,25).point,after=unwrapPoint(t+.00001,25).point;
      close(v[0]*(after[0]-before[0])+v[1]*(after[1]-before[1]),0,1e-7);}
  }
});
