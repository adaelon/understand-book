import { readFileSync, writeFileSync, readdirSync, mkdirSync, copyFileSync } from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { publishVideoTranslationSnapshot } from '../../skills/build/video-translation';
import { estimateTokens } from '../../packages/core/src/window';
const root = path.resolve('evals/video/generated/vi10'), workspace = path.join(root, 'workspace');
const read = (file: string) => JSON.parse(readFileSync(file, 'utf8'));
const plan = read(path.join(workspace, '.build/video-translation/plan.json'));
const before = read(path.join(root, 'recovery-before.json'));
for (const [file, bytes] of Object.entries(before.accepted_files))
  assert.equal(readFileSync(path.join(workspace, '.build/video-translation/accepted', file), 'utf8'), bytes);
for (const task of before.tasks.filter((t: any) => t.status === 'accepted'))
  assert.equal(plan.tasks.find((t: any) => t.id === task.id).attempts, task.attempts);
const reused = await publishVideoTranslationSnapshot(workspace, path.resolve('target/debug/publish_book.exe'), path.join(root, 'service'));
assert(reused.reused, 'Completed resume must not copy the media again');
const link = read(path.join(workspace, 'video_publication.json')), directory = path.resolve(workspace, link.directory);
const translations = read(path.join(directory, 'transcript_translations.json'));
const map = read(path.join(directory, 'transcript_map.json'));
assert.equal(translations.length, 720);
for (const t of translations) assert.deepEqual(t.transcript, map.transcript);
const checks: Array<{ id: number; criterion: string; patterns: RegExp[] }> = [
  { id: 83, criterion: 'Both inability statements, for large and small numbers, remain negative.', patterns: [/无法|不能/, /很大|大.*数/, /很小|小.*数/] },
  { id: 84, criterion: 'Exponent width compares five bits against eight.', patterns: [/指数/, /五|5/, /八|8/] },
  { id: 85, criterion: 'FP16 training is conditional and unstable.', patterns: [/FP16/i, /如果|若|用/, /不稳定/] },
  { id: 86, criterion: 'Underflow, overflow and NaN all remain present.', patterns: [/下溢/, /上溢/, /NAN/i] },
  { id: 87, criterion: 'Uncertain machine term is retained rather than rewriting English.', patterns: [/beef float 16/, /转写存疑/] },
  { id: 90, criterion: 'Total bits stay the same; bits move from mantissa to exponent.', patterns: [/相同|一样|不变/, /尾数/, /指数/] },
  { id: 92, criterion: 'The dynamic range is the same as float32.', patterns: [/相同|一样/, /float\s*32/i, /动态范围/] },
  { id: 93, criterion: 'The resolution tradeoff remains worse.', patterns: [/分辨率/, /更差|降低|较差/] },
  { id: 189, criterion: 'Unmentioned hidden dimension is summed out, with original code identifiers.', patterns: [/hidden/, /求和/, /消去|消掉/, /C1/, /C2/] },
  { id: 191, criterion: 'Summation enumerates all values, preserving variable names.', patterns: [/所有/, /求和/, /sequence/, /hidden/, /C2/] },
];
const samples = checks.map(check => {
  const id = `segment-${check.id}`, english = map.segments.find((s: any) => s.segment_id === id);
  const chinese = translations.find((t: any) => t.segment_id === id);
  for (const pattern of check.patterns) assert(pattern.test(chinese.zh_text), `${id}: ${check.criterion}`);
  return { segment_id: id, location: english.location, english: english.text, chinese: chinese.zh_text, checked_meaning: check.criterion };
});
const runs = readdirSync(path.join(workspace, '.build/video-translation/runs'), { recursive: true }).filter(f => String(f).endsWith('run.json'))
  .map(f => read(path.join(workspace, '.build/video-translation/runs', String(f))));
const publications = readFileSync(path.join(root, 'publications.jsonl'), 'utf8').trim().split('\n').map(line => JSON.parse(line));
const candidates = readdirSync(path.join(workspace, '.build/video-translation/accepted')).map(file => {
  const rows = read(path.join(workspace, '.build/video-translation/accepted', file));
  return { file, segments: rows.length, estimated_candidate_tokens: estimateTokens(JSON.stringify({ transcript: rows[0].transcript,
    translations: rows.map(({ segment_id, zh_text }: any) => ({ segment_id, zh_text })) })) };
});
const actualCopies = publications.filter(p => !p.reused);
const firstRun = Math.min(...readdirSync(path.join(workspace, '.build/video-translation/runs/zh-001')).map(Number));
const firstChinese = publications.find(p => p.group === 'zh-001');
// UUIDv7 publication timestamp gives actual time across resumed driver invocations.
const publishedAt = (id: string) => parseInt(id.replaceAll('-', '').slice(0, 12), 16);
const usage = runs.flatMap(run => run.events.filter((event: any) => event.type === 'turn.completed').map((event: any) => event.usage));
const result = { ...read(path.join(root, 'verification.json')), execution: read(path.join(root, 'execution.json')), recovery: { preserved_tasks: Object.keys(before.accepted_files).length,
  preserved_segments: before.tasks.filter((t: any) => t.status === 'accepted').flatMap((t: any) => t.segment_ids).length,
  accepted_bytes_unchanged: true, accepted_attempts_unchanged: true, completed_publication_reused: true },
  first_chinese_wait_ms_including_interruption: publishedAt(firstChinese.reference.publication_id) - firstRun,
  total_wall_ms_including_interruption: publishedAt(link.reference.publication_id) - firstRun,
  media_bytes_per_publication: actualCopies[0].files['media/original.mp4'], actual_publications: actualCopies.length,
  actual_copied_media_bytes: actualCopies.reduce((n, p) => n + p.files['media/original.mp4'], 0),
  normal_six_batch_plus_english_media_bytes: 7 * actualCopies[0].files['media/original.mp4'],
  publications: publications.map(({ group, reference, elapsed_ms, reused }: any) => ({ group, reference, elapsed_ms, reused })),
  runs: runs.map(({ group, elapsed_ms, exit_code, complete }: any) => ({ group, elapsed_ms, exit_code, complete })), usage,
  usage_note: 'Host-reported completed-turn usage; forcibly interrupted turns have no final usage. Model was the CLI default, low reasoning; model identity and monetary charges were not returned.',
  candidates, samples, browser: read(path.join(root, 'browser-result.json')) };
const out = path.resolve('evals/video/results/vi10-20261010'); mkdirSync(out, { recursive: true });
writeFileSync(path.join(out, 'verification.json'), JSON.stringify(result, null, 2));
for (const name of ['first-batch.png', 'complete.png']) copyFileSync(path.join(result.browser.output, name), path.join(out, name));
console.log(JSON.stringify({ status: 'passed', segments: translations.length, recovery: result.recovery, samples: samples.length,
  actual_publications: actualCopies.length, output: out }));
