// Detect a changed fixture or host/model setting before interpreting a paired result.
import assert from 'node:assert/strict';
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const out = resolve(process.argv[2]);
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const plan = await json(resolve(out, 'schedule.json'));
const material = new Map(), settings = [];
let sourceCount = 0;
for (const task of plan.planned) {
  const dir = resolve(out, 'runs', task.name);
  let input;
  try { input = await json(resolve(dir, 'input.json')); } catch (e) { if (e.code === 'ENOENT') continue; throw e; }
  const source = task.task === 'rainwater' ? (await json(resolve(dir, 'evidence.json'))).source_text : input.source;
  const family = task.task === 'rainwater' ? 'rainwater' : 'average-speed';
  if (material.has(family)) assert.equal(source, material.get(family), `${task.name}: fixture source drift`);
  else material.set(family, source);
  if (task.task === 'rainwater') {
    const prompt = await readFile(resolve(dir, 'agent/prompt.txt'), 'utf8');
    if (material.has('remote-prompt')) assert.equal(prompt, material.get('remote-prompt'));
    else material.set('remote-prompt', prompt);
  }
  const names = task.task === 'rainwater' ? ['agent'] : (await readdir(dir)).filter(n => /^sample-\d+$/.test(n));
  for (const name of names) {
    const client = await json(resolve(dir, name, 'client.json'));
    assert.equal(client.model, plan.model); assert.equal(client.reasoning_effort, plan.effort);
    settings.push(client.client);
  }
  sourceCount++;
}
assert.equal(new Set(settings).size, 1, 'Mixed client versions');
const result = { verified_sources: sourceCount, planned_tasks: plan.planned.length, cli: settings[0], model: plan.model,
  effort: plan.effort, source_content_equal_by_family: true, remote_prompt_equal: true,
  identity_note: 'Independent fixtures issue different publication/session/turn IDs; within-run references retain their actual identities. No hashes were used.' };
await writeFile(resolve(out, 'input-verification.json'), JSON.stringify(result, null, 2));
console.log(JSON.stringify(result));
