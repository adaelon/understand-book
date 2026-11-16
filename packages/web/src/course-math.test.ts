import { describe, it, expect } from 'vitest';
import { chord, descentPath, feasible, objective } from './course-math';
describe('mathematics observations used by the sample lessons',()=>{
  it('separates a successful convexity example from a counterexample',()=>{
    const square=chord({kind:'convex_chord',function:'square',a:-1,b:1,t:.25});
    expect(square).toMatchObject({x:-.5,curve:.25,line:1,gap:.75});
    expect(chord({kind:'convex_chord',function:'cube',a:-1.5,b:-.5,t:.5}).gap).toBe(-.75);
  });
  it('does not confuse the first decreasing step with convergence',()=>{
    const p=descentPath(.085,1)[1]; expect(p.x).toBeCloseTo(8.3);expect(p.y).toBeCloseTo(-.7);expect(objective(p)).toBeCloseTo(73.79);
    expect(objective(descentPath(.12,1)[1])).toBeLessThan(110);
    expect(objective(descentPath(.12,8)[8])).toBeGreaterThan(110);
    expect(Math.abs(descentPath(.1,8)[8].y)).toBe(1);
  });
  it('requires feasibility and descent simultaneously, including tangent directions',()=>{
    expect(feasible({kind:'feasible_directions',position:1,angle:0})).toMatchObject({descending:true,allowed:false});
    expect(feasible({kind:'feasible_directions',position:1,angle:-45})).toMatchObject({descending:true,allowed:true});
    expect(feasible({kind:'feasible_directions',position:1.5,angle:-45})).toMatchObject({descending:false,allowed:true,value:.5});
    for(let angle=-180;angle<=180;angle++){
      const r=feasible({kind:'feasible_directions',position:1.5,angle});expect(r.descending&&r.allowed).toBe(false);
    }
  });
});
