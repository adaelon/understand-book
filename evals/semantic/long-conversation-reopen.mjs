// Reopen already-stopped Reader stores in a fresh process. Both generation and
// model discovery point to a loopback listener which cannot forward traffic.
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { startServer } from './server.mjs';

const [out, scratchRoot, ...batches] = process.argv.slice(2);
if (!out || !scratchRoot || !batches.length) {
  throw new Error('Expected <summary.json> <short scratch directory> <batch dirs...>');
}

const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const observations = [];
fs.mkdirSync(scratchRoot, { recursive: true });
let localRequests = 0;
const offline = http.createServer((req, res) => {
  localRequests++;
  req.resume();
  res.writeHead(503, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: { message: 'Offline reopen' } }));
});
await new Promise(resolve => offline.listen(0, '127.0.0.1', resolve));
const loopback = `http://127.0.0.1:${offline.address().port}/v1`;

async function inspect(batch, run, sample) {
  const archived = path.join(batch, sample.id);
  const scratch = fs.mkdtempSync(path.join(path.resolve(scratchRoot), 'r-'));
  const memory = path.join(scratch, 'memory');
  const privateDir = path.join(scratch, 'private');
  let server;
  try {
    fs.cpSync(path.join(archived,'memory'), memory, { recursive:true });
    fs.cpSync(path.join(archived,'private'), privateDir, { recursive:true });
    const settings = read(path.join(archived, 'model-settings.json'));
    for (const service of settings.services) {
      service.base_url = loopback;
      service.api_key = 'offline';
    }
    const config = path.join(scratch, 'settings.json');
    fs.writeFileSync(config, JSON.stringify(settings));
    server = await startServer(run.book, memory, process.cwd(), {
      UNDERSTAND_BOOK_PRIVATE_DIR: privateDir, UNDERSTAND_BOOK_MODEL_SETTINGS: config,
      UNDERSTAND_BOOK_MODEL_CONFIG_ID:'subject', OPENCODE_BASE_URL:loopback, OPENCODE_API_KEY:'offline',
      FLUID_LLM_MODEL:sample.model, UB_OBSERVABILITY_MODE:'off', LANGSMITH_TRACING:'false',
    });
    const history = await server.api('agent/history');
    const notes = await server.api('memory/recall', {type:'note'}, 'POST');
    const expected = read(path.join(archived, 'after-history.json'));
    const before = read(path.join(archived, 'before-notes.json'));
    const after = read(path.join(archived, 'after-notes.json'));
    const turn = history.current?.turns?.at(-1);
    const original = expected.current?.turns?.at(-1);
    return {batch:path.basename(batch), sample:sample.id, session_id:history.active_session_id,
      same_session:history.active_session_id === expected.active_session_id,
      reopened_turns:history.current?.turns?.length, final_user:turn?.user,
      same_final_turn:!!original && isDeepStrictEqual(turn,original),
      note_counts:{before:before.length,after:after.length,reopened:notes.length},
      notes_match_before_and_after:isDeepStrictEqual(notes,before) && isDeepStrictEqual(notes,after)};
  } finally {
    if (server) await server.stop();
    fs.rmSync(scratch, {recursive:true,force:true});
  }
}

try {
  for (const batch of batches) {
    const run = read(path.join(batch, 'run.json'));
    for (const sample of run.samples) observations.push(await inspect(batch,run,sample));
  }
} finally {
  await new Promise(resolve => offline.close(resolve));
}
const passed = observations.length > 0 && observations.every(o=>o.same_session && o.reopened_turns===8
  && o.same_final_turn && o.final_user==='按之前的约定整理。' && o.notes_match_before_and_after && o.note_counts.reopened===0);
fs.writeFileSync(out, JSON.stringify({passed,local_requests:localRequests,upstream_requests:0,observations},null,2)+'\n');
if (!passed) process.exitCode=1;
