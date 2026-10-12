import fs from 'node:fs';
import path from 'node:path';
import { loadCorpus } from './core.mjs';
import { prepareV2DiagnosticBundle, taskJudgeJobs, taskQualityReport,
  taskQualityVersionV2, preferenceInstruction } from './task-quality.mjs';
import { v2Instruction } from './task-quality-v2.mjs';

const args = process.argv.slice(2);
const option = name => {
  const at = args.indexOf(name);
  if (at < 0 || !args[at + 1]) throw new Error(`Provide ${name}`);
  return path.resolve(args[at + 1]);
};
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const out = option('--out');
if (fs.existsSync(out)) throw new Error('Use a new diagnostic output directory');
const formal = read(option('--bundle'));
const corpus = loadCorpus(option('--book'));
const ev5 = read(option('--ev5')).row;
const ev6 = read(option('--ev6')).answers;
fs.mkdirSync(out, { recursive: true });
for (const [name, rows, wholeBlockSources] of [['ev5', [ev5], false], ['ev6', ev6, true]]) {
  const bundle = prepareV2DiagnosticBundle(formal, rows, corpus, name.toUpperCase(), { wholeBlockSources });
  const dir = path.join(out, name);
  fs.mkdirSync(dir);
  for (const [file, data] of [['bundle.json', bundle], ['jobs.json', taskJudgeJobs(bundle)],
    ['protocol.json', { version: taskQualityVersionV2, absoluteInstruction: v2Instruction, preferenceInstruction }],
    ['report.json', taskQualityReport(bundle, [])]])
    fs.writeFileSync(path.join(dir, file), JSON.stringify(data, null, 2) + '\n', { flag: 'wx' });
}
console.log(JSON.stringify({ out, ev5: 1, ev6: ev6.length, status: 'prepared_unscored' }));
