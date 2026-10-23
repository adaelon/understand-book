// One real Reader/Provider answer; verify the saved source, not only generated prose.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { startServer } from '../semantic/server.mjs';
import { startProviderRecorder, measuredUsage } from '../semantic/provider-recorder.mjs';
import { citationText } from '../semantic/agent-core.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
assert(option('--book') && option('--out'), 'Use --book exact-published-directory --out fresh-evidence-directory');
const book = path.resolve(option('--book')), output = path.resolve(option('--out'));
const publication = JSON.parse(fs.readFileSync(path.join(book, 'publication.json'), 'utf8')).reference;
assert(!fs.existsSync(output), 'Use a fresh evidence directory.');
fs.mkdirSync(output, { recursive: true });
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(['OPENCODE_BASE_URL', 'OPENCODE_API_KEY', 'FLUID_LLM_MODEL'].every(key => process.env[key]), 'Existing provider configuration is required.');
if (option('--server')) process.env.SEMANTIC_EVAL_SERVER = path.resolve(option('--server'));
const message = '请查阅这份舒伯特《音乐瞬间第3首》的已发布材料，说明作品调性、开头拍号和谱面速度标记，并引用可打开的来源。如果原谱的调式编码与作品调性标注不一致，请核对材料里的依据并说明区别。请直接回答这些事实，不用播放音频或开始连续教学。';
const report = { status: 'running', publication, message, model: process.env.FLUID_LLM_MODEL, started_at: new Date().toISOString() };
const save = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
let recorder, server;
try {
  recorder = await startProviderRecorder(process.env.OPENCODE_BASE_URL, { tokenLimit: 450000 });
  server = await startServer(book, path.join(output, 'memory'), root, { OPENCODE_BASE_URL: recorder.url, UNDERSTAND_BOOK_PRIVATE_DIR: path.join(output, 'private') });
  report.reader_url = server.url;
  const interval = setInterval(() => {
    report.calls = recorder.records.length; report.usage = measuredUsage(recorder.records);
    save('report.json', report);
    console.log(JSON.stringify({ status: report.status, calls: report.calls, usage: report.usage }));
  }, 15000);
  let outcome;
  try { outcome = await server.api('agent/chat', { message }, 'POST', 300000); }
  finally { clearInterval(interval); }
  save('outcome.json', outcome);
  const history = await server.api('agent/history');
  const turn_id = history.current.turns.at(-1).turn_id;
  report.turn_id = turn_id;
  report.answer = citationText(outcome);
  fs.writeFileSync(path.join(output, 'answer.md'), report.answer + '\n');
  const sources = [];
  for (const source of outcome.answer_view?.sources ?? []) {
    const resolved = await server.api('agent/source.resolve', { turn_id, source_ref_id: source.source_ref_id }, 'POST');
    sources.push(resolved);
  }
  save('sources.json', sources);
  assert(sources.length > 0, 'Answer must cite a source.');
  for (const source of sources) {
    assert.equal(source.stale, false); assert.equal(source.can_open_in_reader, true);
    assert.deepEqual(source.music.target.publication, publication);
  }
  const notation = sources.find(source => source.music.material.read.score_marks?.some(mark => mark.value.kind === 'direction' && mark.value.words.includes('Allegro moderato')));
  assert(notation, 'Cited source must retain the actual tempo declaration.');
  const read = notation.music.material.read;
  assert(read.score_marks.some(mark => mark.value.kind === 'time_signature' && mark.value.components.some(c => c.beats === '2' && c.beat_type === 4)));
  assert(read.score_marks.some(mark => mark.value.kind === 'key_signature' && mark.value.fifths === -4 && mark.value.encoded_mode === 'major'));
  assert.equal(read.work_tonality.tonic, 'F'); assert.equal(read.work_tonality.mode, 'minor');
  assert(notation.music.material.provenance.some(p => p.provenance_id === 'schubert-tonality-review-20261023'));
  assert(notation.music_score_pages.length > 0, 'Citation opens the original score page.');
  assert.match(report.answer, /F\s*小调|F\s*minor/i); assert.match(report.answer, /2\s*[\/／]\s*4|二四拍|四分之二/);
  assert.match(report.answer, /Allegro\s+moderato/i);
  report.citation_count = sources.length;
  report.score_source_ref_id = notation.source_ref_id;
  report.status = 'passed';
} catch (error) {
  report.status = 'failed'; report.error = error.stack ?? error.message; process.exitCode = 1;
} finally {
  if (server) await server.stop();
  if (recorder) { await recorder.stop(1000); save('provider.json', recorder.records); report.calls = recorder.records.length; report.usage = measuredUsage(recorder.records); report.budget_rejections = recorder.budgetRejections; }
  report.finished_at = new Date().toISOString(); save('report.json', report);
  console.log(JSON.stringify({ status: report.status, usage: report.usage, error: report.error, output }));
}
