// Deterministic connection check only; never used by a real trial.
import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const [directory, step] = process.argv.slice(2);
const calls = [
  { name: 'tool.search', arguments: JSON.stringify({ task: 'Save the requested note on this passage', required_capabilities: ['reader_write'], scope: 'passage', operation: 'mutate_reader', effect_mode: 'reader_mutation_explicitly_requested', max_results: 2 }) },
  { name: 'book.text', arguments: JSON.stringify({ lid: '1.1' }) },
  { name: 'reader.note', arguments: JSON.stringify({ lid: '1.1', text: '平均速度＝总路程／总时间' }) },
];
await writeFile(resolve(directory, `response-${step.padStart(2, '0')}.json`), JSON.stringify({
  text: calls[Number(step)] ? null : '已保存笔记。', tool_calls: calls[Number(step)] ? [calls[Number(step)]] : [], usage_total_tokens: 0,
}));
