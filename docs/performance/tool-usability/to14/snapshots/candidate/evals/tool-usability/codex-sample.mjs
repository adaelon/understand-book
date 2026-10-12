import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { runCodex, recordedUsage } from './codex-client.mjs';

const [run, ordinal] = process.argv.slice(2);
const index = Number(ordinal);
const output = resolve(run, `response-${String(index).padStart(2, '0')}.json`);
try {
  if (index >= 8 || await recordedUsage(run) >= 120_000) throw new Error('TO14_TASK_BUDGET');
  const request = await readFile(resolve(run, `request-${String(index).padStart(2, '0')}.json`), 'utf8');
  const schema = { type: 'object', additionalProperties: false, required: ['text', 'tool_calls'], properties: {
    text: { type: ['string', 'null'] }, tool_calls: { type: 'array', items: {
      type: 'object', additionalProperties: false, required: ['name', 'arguments'],
      properties: { name: { type: 'string' }, arguments: { type: 'string' } },
    } },
  } };
  const prompt = `You are the model for ONE decision of a resident reading agent. The JSON below is the actual current request, including its instructions, user input, available tools and tool results. Follow those instructions and answer the user's request. Return only the required JSON AssistantTurn: text is user-facing text or null; tool_calls is an array of exact available tool names and JSON-encoded argument strings. Select tools yourself; the host executes them and supplies results in the next decision. For a final answer use an empty tool_calls array. Do not claim a tool has executed before seeing its result. Do not mention this adapter. If request.kind is completion, return the requested completion as text with no tool_calls.\n\n${request}`;
  const result = await runCodex({ directory: resolve(run, `sample-${String(index).padStart(2, '0')}`), prompt, schema });
  if (!result.completed || result.final === null) throw new Error(`CODEX_SAMPLE_FAILED: exit=${result.exit_code}, timeout=${result.timed_out}`);
  const turn = JSON.parse(result.final);
  await writeFile(output, JSON.stringify({ ...turn, usage_total_tokens: result.total_tokens }, null, 2));
} catch (error) {
  await writeFile(output, JSON.stringify({ error: error.message }, null, 2));
  process.exitCode = 1;
}
