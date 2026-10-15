import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const output = process.env.FP1_OUTPUT ?? fileURLToPath(new URL('../../../../docs/performance/focus-reading-fp1-20261014/results/', import.meta.url));
const read = name => JSON.parse(fs.readFileSync(path.join(output, `${name}.json`), 'utf8'));
const range = values => ({ min: Math.min(...values), max: Math.max(...values) });
const summary = { baseline: [], columns: [], bounded: [], pdf: null };
for (const material of ['epub', 'complex', 'mixed']) for (const width of [1440, 390]) {
  const r = read(`continuous-${material}-${width}`);
  summary.baseline.push({ material, width, coldReadyMs: r.cold.totalMs, firstCaptureUpperBoundMs: r.cold.firstReadableCaptureUpperBoundMs,
    warmReadyMs: r.warmReadyMs, sourceGrowth: r.growth.map(x => x.sourceBytes), parsedGrowth: r.growth.map(x => x.parsedBytes), nodeGrowth: r.growth.map(x => x.domNodes),
    maxLongTask: Math.max(...r.report.longTasks.map(x => x.duration)), inputHandlerMs: range(r.report.inputs.map(x => x.handlerAt - x.inputAt)), frameEvidence: r.trace });
}
for (const width of [1440, 1024, 390, 844, 320]) {
  const r = read(`columns-${width}`);
  summary.columns.push({ width, geometry: r.measurements.map(m => m.mapping.geometry), mapMs: range(r.measurements.map(m => m.mapping.durationMs)),
    realStress: r.realStress.map(s => ({ material: s.material, firstMapMs: s.mapping.durationMs, alignmentMs: s.mapping.alignmentMs, reusedMapMs: s.reused.durationMs, mapBuildsOnReuse: s.reused.mapBuilds })),
    syntheticLongPages: new Set(r.stress.leaves.find(s => s.lid === 'stress.long').runs.map(x => x.page)).size, overflow: r.overflow });
}
for (const width of [1440, 390]) {
  const r = read(`bounded-${width}`);
  summary.bounded.push({ width, samples: r.length, selectedLeaves: range(r.map(x => x.selected)), columns: range(r.map(x => x.resident.columns)),
    maxNodes: Math.max(...r.map(x => x.resident.domNodes)), maxSourceBytes: Math.max(...r.map(x => x.resident.sourceBytes)), maxParsedBytes: Math.max(...r.map(x => x.resident.parsedBytes)),
    maxDecodedImagePixels: Math.max(...r.map(x => x.resident.decodedImagePixels)), singlePrepareMs: range(r.flatMap(x => x.attempts.map(a => a.prepareMs))) });
}
const pdf = read('pdf');
summary.pdf = { firstCanvasAndTextMs: pdf.firstCanvasAndTextMs, maxCanvasPixels: Math.max(...pdf.samples.map(x => x.canvasPixels)),
  maxActiveCanvases: Math.max(...pdf.samples.map(x => x.canvases.filter(c => c.width > 1 && c.height > 1).length)), samples: pdf.samples.length };
fs.writeFileSync(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2));
console.log(`Summarized ${summary.baseline.length} continuous baselines, ${summary.columns.length} geometries, ${summary.bounded.length} bounded-window groups and PDF.`);
