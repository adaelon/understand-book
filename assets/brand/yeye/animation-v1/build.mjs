import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Hybrid Lottie: approved v7 raster artwork, native vector lids and paper.
// Original PNG bytes are copied as-is. Coordinates use the original 1254px artboard.
const here = path.dirname(fileURLToPath(import.meta.url));
const source = path.resolve(here, '../v7');
const authoring = process.argv[2] && path.resolve(process.argv[2]);
const W = 1254;
const FR = 30;
const still = k => ({ a: 0, k });
const bezier = { o: { x: [0.24], y: [0.1] }, i: { x: [0.32], y: [1] } };
function keys(entries) {
  return { a: 1, k: entries.map(([t, value], n) => ({
    t, s: Array.isArray(value) ? value : [value],
    ...(n < entries.length - 1 ? {
      e: Array.isArray(entries[n + 1][1]) ? entries[n + 1][1] : [entries[n + 1][1]],
      ...bezier,
    } : {}),
  })) };
}
function contour(v, i = [], o = [], closed = true) {
  return { v, i: v.map((_, n) => i[n] || [0, 0]), o: v.map((_, n) => o[n] || [0, 0]), c: closed };
}
const rgb = hex => [...hex.matchAll(/[0-9a-f]{2}/gi)].map(m => parseInt(m[0], 16) / 255).concat(1);
const tx = (extra = {}) => ({ o: still(100), r: still(0), p: still([0, 0, 0]), a: still([0, 0, 0]), s: still([100, 100, 100]), ...extra });
const groupTx = { ty: 'tr', ...tx() };
function layer(name, op, extra = {}) {
  return { ddd: 0, nm: name, sr: 1, ip: 0, op, st: 0, bm: 0, ks: tx(), ...extra };
}
const mask = (name, shape) => ({ inv: false, mode: 'a', nm: name, pt: still(shape), o: still(100), x: still(0) });
const stroke = (color, width) => ({ ty: 'st', c: still(rgb(color)), o: still(100), w: still(width), lc: 2, lj: 2, ml: 4 });
const fill = color => ({ ty: 'fl', c: still(rgb(color)), o: still(100), r: 1 });
function shapes(name, op, items, extra = {}) {
  return layer(name, op, { ty: 4, parent: 100, shapes: [{ ty: 'gr', nm: name, it: [...items, groupTx] }], ...extra });
}
function base(name, op) {
  return layer(name, op, { ty: 2, parent: 100, refId: 'approved-art' });
}
function root(op, extra = {}) {
  return layer('Character pivot · grounded at feet', op, {
    ty: 3, ind: 100, ks: tx({ a: still([627, 1192, 0]), p: still([627, 1192, 0]), ...extra }),
  });
}
function doc(name, op, asset, layers) {
  const reserved = new Set(layers.filter(l => l.ind).map(l => l.ind));
  let index = 1;
  for (const l of layers) if (!l.ind) { while (reserved.has(index)) index++; l.ind = index++; }
  return { v: '5.13.0', fr: FR, ip: 0, op, w: W, h: W, nm: name, ddd: 0,
    assets: [{ id: 'approved-art', w: W, h: W, u: '', p: asset, e: 0 }], layers,
    markers: [],
  };
}

// Masks stay inside the dark fur gap between each eye and the cream cheek.
const leftEye = contour(
  [[465, 379], [534, 387], [579, 416], [579, 461], [550, 506], [504, 524], [462, 517], [428, 480], [425, 441], [442, 401]],
  [[-12, 8], [-25, -7], [-9, -18], [4, -15], [13, -9], [18, 0], [16, 4], [6, 14], [-1, 14], [-9, 10]],
  [[22, 0], [20, 7], [7, 16], [-4, 15], [-15, 13], [-18, 0], [-18, -7], [-7, -16], [1, -18], [8, -10]],
);
const rightEye = contour(
  [[822, 483], [881, 492], [933, 522], [936, 573], [909, 611], [863, 630], [817, 620], [785, 583], [781, 544], [798, 505]],
  [[-10, 8], [-20, -7], [-12, -16], [4, -18], [13, -9], [17, 0], [15, 5], [6, 16], [-1, 14], [-9, 10]],
  [[20, 0], [20, 6], [7, 18], [-4, 18], [-15, 12], [-17, 0], [-18, -8], [-7, -17], [1, -17], [8, -9]],
);
const leftArc = contour([[444, 480], [488, 508], [552, 487]],
  [[0, 0], [-22, -1], [-12, 18]], [[12, 18], [25, 3], [0, 0]], false);
const rightArc = contour([[798, 582], [850, 614], [918, 588]],
  [[0, 0], [-25, -2], [-15, 19]], [[16, 19], [25, 2], [0, 0]], false);
function eyelid(op, side, timings, arcTimes, success = false) {
  const isLeft = side === 'left';
  const x0 = isLeft ? 408 : 768;
  const y0 = isLeft ? 354 : 460;
  const area = structuredClone(isLeft ? leftEye : rightEye);
  if (success && !isLeft) {
    // Success has a rounder, higher eye than idle: cover that extra white cap.
    area.v[0] = [828, 473];
    area.v[1] = [884, 472];
  }
  const sweep = amount => contour([[x0, y0], [x0 + 193, y0], [x0 + 193, y0 + 10 + 193 * amount], [x0, y0 - 15 + 193 * amount]]);
  // The closed eye is a soft U, matching the existing cream eye language.
  const arc = shapes(`${side} eye · closed smile`, op, [
    { ty: 'sh', ks: still(isLeft ? leftArc : rightArc) }, stroke('F7F3EB', 8),
  ], { ks: tx({ o: keys(arcTimes) }) });
  const c0 = success ? [0.2235, 0.1451, 0.1059] : [0.204, 0.141, 0.106];
  const c1 = success ? [0.2314, 0.149, 0.1098] : [0.216, 0.149, 0.11];
  const lid = shapes(`${side} eye · upper lid`, op, [
    { ty: 'sh', ks: { a: 1, k: timings.map(([t, v], n) => ({ t, s: [sweep(v)], ...(n < timings.length - 1 ? { e: [sweep(timings[n + 1][1])], ...bezier } : {}) })) } },
    { ty: 'gf', o: still(100), r: 1, t: 1, s: still([x0 + 90, y0]), e: still([x0 + 90, y0 + 190]),
      g: { p: 2, k: still([0, ...c0, 1, ...c1]) } },
  ], { hasMask: true, masksProperties: [mask('Eye boundary', area)] });
  return [arc, lid];
}

function makeIdle() {
  const op = 120;
  const blink = [[0, 0], [42, 0], [45, 1], [47, 1], [53, 0], [119, 0]];
  const arc = [[0, 0], [44, 0], [45, 100], [47, 100], [50, 0], [119, 0]];
  const d = doc('页页 · 日常眨眼', op, 'yeye-idle.png', [
    ...eyelid(op, 'left', blink, arc),
    ...eyelid(op, 'right', blink.map(([t, v], n) => [n === 0 || n === blink.length - 1 ? t : t + 0.7, v]), arc),
    base('Approved idle artwork', op),
    root(op, { s: keys([[0, [100, 100, 100]], [35, [100.2, 100.45, 100]], [75, [100, 100, 100]], [119, [100, 100, 100]]]) }),
  ]);
  d.markers = [{ tm: 45, cm: 'blink-closed', dr: 2 }];
  return d;
}

function paperShape(top, outer, bottom) {
  return contour([[650, 810], top, outer, bottom, [639, 1040], [650, 810]],
    [[0, 0], [-18, 16], [0, -30], [30, 0], [0, 0], [0, 0]],
    [[0, 0], [12, 12], [0, 50], [-30, 0], [0, 0], [0, 0]]);
}
function makeReading() {
  const op = 150;
  // One sheet peels from the right and lands on the left. The hinge stays put.
  const poses = [
    [0, paperShape([878, 750], [895, 792], [881, 1002])],
    [49, paperShape([878, 750], [895, 792], [881, 1002])],
    [56, paperShape([831, 679], [867, 715], [848, 996])],
    [64, paperShape([679, 652], [696, 690], [687, 1022])],
    [72, paperShape([491, 669], [472, 712], [460, 972])],
    [84, paperShape([453, 710], [430, 757], [428, 988])],
    [91, paperShape([455, 718], [430, 757], [428, 988])],
    [149, paperShape([455, 718], [430, 757], [428, 988])],
  ];
  const page = shapes('Paper · right to left turn', op, [
    { ty: 'sh', ks: { a: 1, k: poses.map(([t, value], n) => ({ t, s: [value], ...(n < poses.length - 1 ? { e: [poses[n + 1][1]], ...bezier } : {}) })) } },
    fill('F8EDDC'), stroke('DDC4A7', 3),
  ], { ks: tx({ o: keys([[0, 0], [46, 0], [49, 100], [85, 100], [93, 0], [149, 0]]) }) });
  const cover = base('Original front cover and paws · occlude paper', op);
  cover.hasMask = true;
  cover.masksProperties = [mask('Front of the book', contour(
    [[327, 715], [396, 716], [541, 761], [617, 800], [650, 813], [782, 793], [934, 777], [987, 863], [980, 1195], [327, 1195]],
    [[0, 0], [0, 0], [-45, -16], [-25, -18], [-11, 0], [-55, 8], [-48, 7]],
    [[0, 0], [45, 15], [32, 12], [11, 8], [45, -6], [55, -6]],
  ))];
  const d = doc('页页 · 阅读翻页', op, 'yeye-reading-cream.png', [
    cover, page, base('Approved cream-cover reading artwork', op),
    root(op, { r: keys([[0, 0], [44, 0], [61, -0.45], [90, 0], [149, 0]]) }),
  ]);
  d.markers = [{ tm: 49, cm: 'page-lift', dr: 0 }, { tm: 64, cm: 'page-upright', dr: 0 }, { tm: 91, cm: 'page-settle', dr: 0 }];
  return d;
}

function makeSuccess() {
  const op = 75;
  // The raised-paw source pose is stable; a single wink supplies the character beat.
  const blink = [[0, 0], [22, 0], [26, 1], [30, 1], [36, 0], [74, 0]];
  const arc = [[0, 0], [25, 0], [26, 100], [30, 100], [34, 0], [74, 0]];
  const accents = [
    shapes('Paw response · short ray', op, [
      { ty: 'sh', ks: still(contour([[1020, 850], [1052, 839]], [], [], false)) }, stroke('B65A17', 9),
    ], { ks: tx({ o: keys([[0, 0], [17, 0], [21, 100], [30, 100], [39, 0], [74, 0]]),
      p: keys([[0, [0, 0, 0]], [17, [-6, 3, 0]], [27, [4, -2, 0]], [39, [8, -4, 0]], [74, [8, -4, 0]]]) }) }),
    shapes('Paw response · lower ray', op, [
      { ty: 'sh', ks: still(contour([[1006, 890], [1033, 904]], [], [], false)) }, stroke('B65A17', 9),
    ], { ks: tx({ o: keys([[0, 0], [20, 0], [24, 100], [32, 100], [41, 0], [74, 0]]),
      p: keys([[0, [0, 0, 0]], [20, [-4, -2, 0]], [31, [3, 2, 0]], [41, [7, 4, 0]], [74, [7, 4, 0]]]) }) }),
  ];
  const d = doc('页页 · 完成反馈', op, 'yeye-success.png', [
    ...eyelid(op, 'right', blink, arc, true), ...accents, base('Approved success artwork', op),
    root(op, {
      s: keys([[0, [100, 100, 100]], [8, [101.4, 98.9, 100]], [18, [99.6, 101.4, 100]], [29, [100.4, 99.8, 100]], [42, [100, 100, 100]], [74, [100, 100, 100]]]),
      r: keys([[0, 0], [8, 0.6], [19, -1.1], [31, 0.35], [43, 0], [74, 0]]),
    }),
  ]);
  d.markers = [{ tm: 8, cm: 'anticipation', dr: 0 }, { tm: 26, cm: 'wink', dr: 4 }, { tm: 43, cm: 'settled', dr: 31 }];
  return d;
}

const scenes = [
  { slug: 'idle', scene: 'scene-1', file: 'yeye-idle.png', loop: true, animation: makeIdle() },
  { slug: 'reading', scene: 'scene-2', file: 'yeye-reading-cream.png', loop: true, animation: makeReading() },
  { slug: 'success', scene: 'scene-3', file: 'yeye-success.png', loop: false, animation: makeSuccess() },
];
for (const item of scenes) {
  const targets = [path.join(here, item.slug)];
  if (authoring) targets.push(path.join(authoring, 'public/projects/yeye', item.scene));
  for (const target of targets) {
    fs.mkdirSync(target, { recursive: true });
    fs.copyFileSync(path.join(source, item.file), path.join(target, item.file));
    fs.writeFileSync(path.join(target, 'lottie.json'), JSON.stringify(item.animation, null, 2) + '\n');
  }
}
fs.writeFileSync(path.join(here, 'manifest.json'), JSON.stringify({
  character: '页页', version: 1, source: '../v7', format: 'Lottie with original PNG assets and vector animation layers',
  dimensions: [W, W], fps: FR,
  authoring: { project: 'https://github.com/diffusionstudio/lottie', skill: 'Text-to-Lottie', verifier: 'official Skia Skottie player' },
  runtime: { project: 'https://github.com/airbnb/lottie-web', version: '5.13.0', renderer: 'svg' },
  scenes: scenes.map(({ slug, scene, file, loop, animation }) => ({
    id: slug, title: animation.nm, file: `${slug}/lottie.json`, asset: `${slug}/${file}`, loop,
    frames: animation.op, seconds: animation.op / FR, officialScene: `yeye/${scene}`,
  })),
  notes: [
    'Artwork is unchanged from the approved v7 PNGs. Eyelids, page and response marks are local vector layers.',
    'Success is a one-shot animation. The raised paw is part of the source pose, not a separately articulated limb.',
    'Keep each lottie.json together with its referenced PNG when distributing. Background is transparent.',
    'MIT code notices are bundled in licenses. Character artwork provenance is recorded in ../v7/design-spec.json.',
  ],
}, null, 2) + '\n');
console.log(`Wrote ${scenes.length} Yeye animations${authoring ? ' and synced the official player' : ''}.`);
