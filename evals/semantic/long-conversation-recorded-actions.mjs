import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const json = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const candidates = memory => path.join(memory, 'agent-history.presentations', 'candidates');

// Replay the observed model/tool trajectory into a fresh run. Candidate
// handles are allocated anew; every new candidate must match the original
// complete stored content before subsequent actions use its new handle.
export function recordedActions(dir, through) {
  const memory = path.join(dir, 'memory');
  const forwarded = json(path.join(dir, 'forwarded-provider.json'));
  const events = json(path.join(dir,'replay.json')).events.slice(3);
  const count = through ?? (events.length - 1);
  assert.ok(Number.isInteger(count) && count > 0 && count < events.length, 'Need the subsequent real sampling to compare receipts');
  if (through === undefined) assert.equal(forwarded.at(-1).status,402);
  const choices = events.slice(0,count).map(event =>
    event.response ?? forwarded[event.forwarded_ordinal-1]?.response?.choices?.[0]);
  assert.ok(choices.length && choices.every(c => c.finish_reason === 'tool_calls'));
  const continuations = new Map(), handles = new Map();
  const sessionDir = path.join(memory, 'agent-sessions');
  for (const name of fs.readdirSync(sessionDir).filter(n => n.endsWith('.jsonl'))) {
    for (const line of fs.readFileSync(path.join(sessionDir,name),'utf8').trim().split('\n')) {
      for (const m of JSON.parse(line).payload?.messages ?? []) {
        if (m.provider_continuation?.reasoning_content && m.tool_calls?.length)
          continuations.set(m.tool_calls[0].id, m.provider_continuation.reasoning_content);
      }
    }
  }
  const proof = { source: dir, model_samplings: choices.length, candidate_mapping: {}, restored_continuations: 0 };
  function rebind(raw) {
    const choice = structuredClone(raw);
    for (const call of choice.message.tool_calls ?? []) {
      for (const [oldId,newId] of handles) call.function.arguments = call.function.arguments.replaceAll(oldId, newId);
    }
    return choice;
  }
  return { length: choices.length, proof, rebind, sample(index, body, currentMemory, toolReceipt) {
    if (index > 0 && index <= choices.length) {
      const original = json(path.join(dir,events[index].request));
      for (const call of choices[index-1].message.tool_calls) {
        const before = toolReceipt(original, call.id), now = toolReceipt(body, call.id);
        assert.equal(now.status, before.status);
        assert.equal(now.error_code, before.error_code);
        if (before.status !== 'candidate_saved') continue;
        const oldId = before.candidate_id, newId = now.candidate_id;
        const oldPage = json(path.join(candidates(memory), oldId + '.json'));
        const newPage = json(path.join(candidates(currentMemory), newId + '.json'));
        assert.deepEqual(newPage.content, oldPage.content, 'Rebuilt candidate content differs: ' + oldId);
        handles.set(oldId, newId);
        proof.candidate_mapping[oldId] = { current: newId, same_content: true };
      }
    }
    if (index >= choices.length) return null;
    const choice = rebind(choices[index]);
    // Protocol continuation stays opaque and transient outside the Reader's
    // own store. The public provider log contains only visible output.
    const token = continuations.get(choice.message.tool_calls[0].id);
    if (token) {
      choice.message.reasoning_content = token;
      proof.restored_continuations++;
    }
    return choice;
  } };
}
