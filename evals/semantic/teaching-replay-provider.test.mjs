import test from 'node:test';
import assert from 'node:assert/strict';
import { teachingPurpose, teachingUsage } from './teaching-replay-provider.mjs';
import { createToolUseCapture } from './teaching-patch-stream.mjs';

test('native usage is cumulative and attributes foreground and background coaches separately', () => {
  const capture = createToolUseCapture('text/event-stream');
  for (const event of [
    { type: 'message_start', message: { usage: { input_tokens: 20, cache_read_input_tokens: 80, cache_creation_input_tokens: 0, output_tokens: 1 } } },
    { type: 'message_delta', usage: { output_tokens: 8 } },
    { type: 'message_delta', usage: { output_tokens: 12 } },
  ]) capture.write(Buffer.from(`data: ${JSON.stringify(event)}\n\n`));
  const usage = capture.finish().usage;
  assert.equal(usage.output_tokens, 12);
  assert.equal(teachingPurpose({ system: '你是 Understand Book 的教学教练。本次在演示页或原生画板制作前' }), 'expression_coach');
  assert.equal(teachingPurpose({ system: '你是 Understand Book 的教学教练。宿主在每个完成的教学回合后' }), 'tutor_coach');
  assert.equal(teachingPurpose({ messages: [{ role: 'system', content: 'Evaluate only the frozen atomic rubric' }] }), 'tutor_assessment');
  assert.equal(teachingPurpose({ system: 'ordinary', messages: [{ role: 'user', content: 'Teaching delivery acceptance review.' }] }), 'other');
  assert.deepEqual(teachingUsage([{ purpose: 'outer', native_usage: usage, elapsed_ms: 1500 }]).outer,
    { calls: 1, usage_complete: true, input_tokens: 100, cached_input_tokens: 80, cache_creation_input_tokens: 0,
      uncached_input_tokens: 20, output_tokens: 12, provider_seconds: 1.5, failed_requests: 0 });
  assert.equal(teachingUsage([{ purpose: 'outer', http_status: 500 }]).outer.input_tokens, null);
});
