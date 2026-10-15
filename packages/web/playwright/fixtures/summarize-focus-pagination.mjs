import { readFile, writeFile, readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const directory = process.env.FP2_OUTPUT ?? fileURLToPath(new URL('../../../../docs/performance/focus-reading-fp2-20261014/results/', import.meta.url));
const files = await readdir(directory);
const read = async name => JSON.parse(await readFile(path.join(directory, `${name}.json`), 'utf8'));
const distribution = values => {
  const sorted = [...values].sort((a, b) => a - b);
  const at = q => sorted.length ? sorted[Math.ceil(sorted.length * q) - 1] : null;
  return { count: sorted.length, p50: at(.5), p95: at(.95), p99: at(.99), max: at(1) };
};
const reflow = [];
for (const width of [1440, 390]) {
  const result = await read(`reflow-${width}`);
  for (const material of ['epub', 'complex', 'mixed']) {
    const rows = result.samples.filter(row => row.material === material);
    reflow.push({ width, material, readyMs: distribution(rows.map(row => row.readyMs)), browserFrameMs: distribution(rows.map(row => row.firstReadableBrowserFrameMs)) });
  }
}
const peak = {};
for (const name of files.filter(name => /^continuation-.*\.json$/.test(name))) {
  const result = JSON.parse(await readFile(path.join(directory, name), 'utf8'));
  for (const [key, value] of Object.entries(result.report.peak)) peak[key] = Math.max(peak[key] ?? 0, value);
}
const long = files.includes('long-reading.json') ? await read('long-reading') : null;
const tasks = await read('preparation-tasks');
const summary = {
  generatedAt: new Date().toISOString(), reflow, continuationPeak: peak,
  firstLinkAlignmentMs: tasks.linkReport.metrics.maxAlignmentMs,
  longTasksDuringIsolatedPreparation: tasks.longTasks,
  longReading: long ? { elapsedMs: long.elapsedMs, turns: long.turns, browser: long.browser, viewport: long.viewport, peak: long.report.peak, samples: long.samples.map(row => ({ turns: row.turns, elapsedMs: row.elapsedMs, start: row.screen.start, resources: row.resources })) } : null,
  physicalPresentation: null,
};
await writeFile(path.join(directory, 'summary.json'), JSON.stringify(summary, null, 2));
console.log(JSON.stringify({ reflow: reflow.map(row => ({ width: row.width, material: row.material, p95: row.browserFrameMs.p95 })), longReading: summary.longReading && { turns: long.turns, elapsedMs: long.elapsedMs, peak: long.report.peak } }, null, 2));
