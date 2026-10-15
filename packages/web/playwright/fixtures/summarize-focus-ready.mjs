import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('../../../../', import.meta.url));
const output = path.resolve(process.env.FP3_OUTPUT ?? path.join(root, 'docs/performance/focus-reading-fp3-20261014/results'));
const read = name => readFile(path.join(output, `${name}.json`), 'utf8').then(JSON.parse);
const peak = states => Object.fromEntries(Object.keys(states[0].peak).map(key => [key, Math.max(...states.flatMap(s => [s.peak[key], s.resources[key]]))]));
const normal = [];
for (const width of [1440, 390]) for (const network of ['local', 'weak']) {
  const result = await read(`normal-${width}-${network}`);
  const samples = result.samples;
  const durations = samples.map(s => s.elapsedMs).sort((a, b) => a - b);
  normal.push({ width, network, samples: samples.length, effectiveTurns: samples.filter(s => s.before.current.id !== s.after.current.id).length,
    notReady: samples.filter(s => !s.ready).length, elapsedMs: result.elapsedMs,
    synchronousSwitchMs: { p95: durations[Math.ceil(durations.length * .95) - 1], max: durations.at(-1) },
    preparationDuringSwitch: samples.filter(s => ['alignments', 'preparations', 'geometryProbes'].some(key => s.before.work[key] !== s.after.work[key])).length,
    peak: peak(samples.map(s => s.after)),
    longTasksAfterFirstInput: result.longTasks.filter(t => t.start >= samples[0].at),
  });
}
const long = await read('long-reading');
const summary = { generatedAt: new Date().toISOString(), normal,
  longReading: { elapsedMs: long.elapsedMs, turns: long.turns, metrics: long.final.metrics, peak: peak([...long.samples.map(s => s.state), long.final]),
    resourceSamples: long.samples.map(s => ({ turns: s.turns, elapsedMs: s.elapsedMs, current: s.state.current, resources: s.state.resources, boundaryCount: s.state.boundaryCount })),
    longTasks: long.longTasks },
  physicalPresentation: null,
};
await writeFile(path.join(output, 'summary.json'), JSON.stringify(summary, null, 2));
console.log(JSON.stringify({ normal: normal.map(({ width, network, samples, effectiveTurns, notReady, preparationDuringSwitch }) => ({ width, network, samples, effectiveTurns, notReady, preparationDuringSwitch })),
  longReading: { elapsedMs: long.elapsedMs, turns: long.turns, peak: long.final.peak } }, null, 2));
