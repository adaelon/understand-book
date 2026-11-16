import type { CourseMathState } from './generated/CourseMathState';
export type Point = { x: number; y: number };
export const objective = (p: Point) => p.x * p.x + 10 * p.y * p.y;
export function descentPath(step: number, count: number): Point[] {
  const points = [{ x: 10, y: 1 }];
  for (let i = 0; i < count; i++) {
    const p = points.at(-1)!;
    points.push({ x: (1 - 2 * step) * p.x, y: (1 - 20 * step) * p.y });
  }
  return points;
}
export function chord(s: Extract<CourseMathState, {kind:'convex_chord'}>) {
  const f = (x: number) => s.function === 'square' ? x*x : x*x*x;
  const x = (1-s.t)*s.a + s.t*s.b, curve = f(x), line = (1-s.t)*f(s.a) + s.t*f(s.b);
  return { f, x, curve, line, gap: line-curve };
}
export function feasible(s: Extract<CourseMathState, {kind:'feasible_directions'}>) {
  const p={x:s.position,y:2-s.position}, r=s.angle*Math.PI/180, d={x:Math.cos(r),y:Math.sin(r)};
  const g={x:2*(p.x-2),y:2*(p.y-1)}, derivative=g.x*d.x+g.y*d.y, constraint=d.x+d.y;
  return { p,d,g,derivative,constraint,value:(p.x-2)**2+(p.y-1)**2, descending:derivative < -1e-8,allowed:constraint<=1e-8 };
}
