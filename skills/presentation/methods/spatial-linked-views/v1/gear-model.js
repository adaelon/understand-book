// Standard 20° involute spur gears. Millimetres, seconds, N·m.
export const TAU = 2 * Math.PI;
export const PRESSURE = 20 * Math.PI / 180;
export const MODULE = 2;
export const INPUT_TEETH = 20;
export const OUTPUT_TEETH = [20, 30, 40, 60];
export const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

export function gearSpec(teeth, module = MODULE) {
  const pitch = teeth * module / 2;
  return { teeth, module, pitch, base: pitch * Math.cos(PRESSURE),
    tip: pitch + module, root: pitch - 1.25 * module,
    halfTooth: Math.PI / (2 * teeth), invPitch: Math.tan(PRESSURE) - PRESSURE };
}

export function involuteAngle(radius, base) {
  const t = Math.sqrt(Math.max(0, (radius / base) ** 2 - 1));
  return t - Math.atan(t);
}

// Counterclockwise outline. The flanks are sampled from the exact involute;
// the non-contact root transition is radial, not a manufacturing fillet.
export function gearOutline(teeth, samples = 9) {
  const g = gearSpec(teeth), points = [], step = TAU / teeth;
  const polar = (r, a) => points.push([r * Math.cos(a), r * Math.sin(a)]);
  const half = r => g.halfTooth + g.invPitch - involuteAngle(r, g.base);
  const start = Math.max(g.base, g.root);
  for (let k = 0; k < teeth; k++) {
    const a = k * step, h0 = half(start), ht = half(g.tip);
    polar(g.root, a - h0);
    for (let j = 0; j <= samples; j++) {
      const r = start + (g.tip - start) * j / samples;
      polar(r, a - half(r));
    }
    for (let j = 1; j <= 4; j++) polar(g.tip, a - ht + 2 * ht * j / 4);
    for (let j = samples; j >= 0; j--) {
      const r = start + (g.tip - start) * j / samples;
      polar(r, a + half(r));
    }
    polar(g.root, a + h0);
    for (let j = 1; j <= 4; j++) polar(g.root, a + h0 + (step - 2 * h0) * j / 4);
  }
  return points;
}

export function transmission(teeth, rpm, torque) {
  const a = gearSpec(INPUT_TEETH), b = gearSpec(teeth);
  return { ratio: teeth / INPUT_TEETH, inputRpm: rpm,
    outputRpm: -rpm * INPUT_TEETH / teeth,
    outputTorque: torque * teeth / INPUT_TEETH,
    inputPower: torque * rpm * TAU / 60,
    tangentForce: torque / (a.pitch / 1000),
    pitchSpeed: rpm * TAU / 60 * a.pitch / 1000,
    center: a.pitch + b.pitch, a, b };
}

export const drivenAngle = (angle, teeth) => Math.PI - Math.PI / teeth - angle * INPUT_TEETH / teeth;

export function contactGeometry(teeth, phase) {
  const a = gearSpec(INPUT_TEETH), b = gearSpec(teeth);
  const s0 = b.pitch * Math.sin(PRESSURE) - Math.sqrt(b.tip ** 2 - b.base ** 2);
  const s1 = Math.sqrt(a.tip ** 2 - a.base ** 2) - a.pitch * Math.sin(PRESSURE);
  const s = s0 + (s1 - s0) * phase;
  const basePitch = Math.PI * MODULE * Math.cos(PRESSURE);
  const angle = s / a.base - a.halfTooth;
  const normal = [Math.sin(PRESSURE), Math.cos(PRESSURE)];
  const point = offset => [a.pitch + offset * normal[0], offset * normal[1]];
  const contacts = [-1, 0, 1].map(k => s + k * basePitch)
    .filter(q => q >= s0 - 1e-9 && q <= s1 + 1e-9).map(point);
  return { a, b, s, s0, s1, angle, outputAngle: drivenAngle(angle, teeth),
    normal, point: point(s), start: point(s0), end: point(s1), contacts,
    contactRatio: (s1 - s0) / basePitch };
}

export function unwrapPoint(t, radius = 1) {
  return { tangent: [radius * Math.cos(t), radius * Math.sin(t)],
    point: [radius * (Math.cos(t) + t * Math.sin(t)), radius * (Math.sin(t) - t * Math.cos(t))] };
}
