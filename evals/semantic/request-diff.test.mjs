import test from 'node:test';
import assert from 'node:assert/strict';
import { compareProviderRequests, createRequestDiffTracker } from './request-diff.mjs';

test('unchanged history plus appended messages retains the complete old-message prefix', () => {
  const previous = { model: 'test', messages: [{ role: 'system', content: '固定规则' }, { role: 'user', content: '问题😀' }] };
  const current = { ...previous, messages: [...previous.messages, { role: 'assistant', content: '回答' }] };
  const diff = compareProviderRequests(previous, current);
  assert.equal(diff.messages.only_appended, true);
  assert.equal(diff.messages.common_prefix_messages, 2);
  assert.equal(diff.messages.first_old_difference, null);
  const serializedPrefix = JSON.stringify(previous.messages).slice(0, -1);
  assert.equal(diff.messages.common_prefix_serialized_chars, [...serializedPrefix].length);
  assert.equal(diff.messages.common_prefix_serialized_utf8_bytes, Buffer.byteLength(serializedPrefix));
  assert.equal(diff.tools_changed, false);
  assert.equal(compareProviderRequests(previous, previous).messages.identical, true);
});

test('tool visibility and same-name schema changes are distinguished from message changes', () => {
  const tool = (name, description = 'fixed') => ({ type: 'function', function: { name, description } });
  const previous = { messages: [{ role: 'user', content: 'question' }], tools: [tool('book_text')] };
  const added = compareProviderRequests(previous, { ...previous, tools: [...previous.tools, tool('web_search')] });
  assert.equal(added.tools_changed, true);
  assert.deepEqual(added.tool_names_added, ['web_search']);
  assert.deepEqual(added.tool_names_removed, []);
  assert.equal(added.messages.identical, true);
  const schemaChanged = compareProviderRequests(previous, { ...previous, tools: [tool('book_text', 'PRIVATE_DYNAMIC_EXAMPLE')] });
  assert.equal(schemaChanged.tools_changed, true);
  assert.deepEqual(schemaChanged.tool_names_added, []);
  assert.equal(schemaChanged.first_tool_difference, 'tools[0].function.description');
  assert(!JSON.stringify(schemaChanged).includes('PRIVATE_'));
});

test('front state mutation reports the first old system position without leaking text', () => {
  const messages = [{ role: 'system', content: 'fixed' }, { role: 'system', content: 'PRIVATE_OLD_STATE' }, { role: 'user', content: 'same history' }];
  const changed = messages.map((message, index) => index === 1 ? { ...message, content: 'PRIVATE_NEW_STATE' } : message);
  const diff = compareProviderRequests({ messages }, { messages: changed });
  assert.equal(diff.messages.only_appended, false);
  assert.equal(diff.messages.common_prefix_messages, 1);
  assert.deepEqual(diff.messages.first_old_difference, { index: 1, path: 'messages[1].content', category: 'system', previous_role: 'system', current_role: 'system' });
  assert(!JSON.stringify(diff).includes('PRIVATE_'));
});

test('a prepended tool identifies both array occupants instead of implying a same-tool schema change', () => {
  const book = { type: 'function', function: { description: 'book lookup', name: 'book_query' } };
  const web = { type: 'function', function: { description: 'web lookup', name: 'web_search' } };
  const diff = compareProviderRequests({ tools: [book] }, { tools: [web, book] });
  assert.equal(diff.first_tool_difference, 'tools[0].function.description');
  assert.equal(diff.previous_tool_name, 'book_query');
  assert.equal(diff.current_tool_name, 'web_search');
  const noTools = compareProviderRequests({}, { tools: [web] });
  assert.equal(noTools.previous_tool_name, null);
  assert.equal(noTools.current_tool_name, null);
});

test('reasoning, historical tool aliases, tool bodies and removed messages have separate categories', () => {
  const previous = { messages: [{ role: 'assistant', content: '', reasoning_content: 'PRIVATE_OLD_REASONING',
    tool_calls: [{ id: 'call-1', function: { name: 'book.text', arguments: '{"lid":"10"}' } }] }] };
  const changedReasoning = structuredClone(previous);
  changedReasoning.messages[0].reasoning_content = 'PRIVATE_NEW_REASONING';
  const reasoningDiff = compareProviderRequests(previous, changedReasoning);
  assert.equal(reasoningDiff.messages.first_old_difference.category, 'reasoning');
  assert(!JSON.stringify(reasoningDiff).includes('PRIVATE_'));
  const removedReasoning = structuredClone(previous);
  delete removedReasoning.messages[0].reasoning_content;
  assert.equal(compareProviderRequests(previous, removedReasoning).messages.first_old_difference.category, 'reasoning');
  const changedAlias = structuredClone(previous);
  changedAlias.messages[0].tool_calls[0].function.name = 'book_text';
  assert.equal(compareProviderRequests(previous, changedAlias).messages.first_old_difference.category, 'tool_name');
  const toolBody = { messages: [{ role: 'tool', content: 'PRIVATE_BODY', tool_call_id: 'call-1' }] };
  assert.equal(compareProviderRequests(toolBody, { messages: [{ ...toolBody.messages[0], content: 'receipt' }] }).messages.first_old_difference.category, 'body');
  assert.equal(compareProviderRequests(previous, { messages: [] }).messages.first_old_difference.category, 'message_removed');
});

test('background purposes do not become the prior business request', () => {
  const tracker = createRequestDiffTracker();
  const first = { messages: [{ role: 'system', content: 'business' }] };
  assert.deepEqual(tracker.observe(first, 'business_loop', 1), { previous_ordinal: null, purpose_sequence: 1 });
  assert.deepEqual(tracker.observe({ messages: [{ role: 'system', content: 'background' }] }, 'background_profile_review', 2), { previous_ordinal: null, purpose_sequence: 1 });
  const third = tracker.observe({ messages: [...first.messages, { role: 'user', content: 'followup' }] }, 'business_loop', 3);
  assert.equal(third.previous_ordinal, 1);
  assert.equal(third.purpose_sequence, 2);
  assert.equal(third.messages.only_appended, true);
  tracker.clear();
  assert.equal(tracker.observe(first, 'business_loop', 4).previous_ordinal, null);
});
