import test from 'node:test';
import assert from 'node:assert/strict';
import { AGENT, CHUNK } from './agent-core.mjs';
import { actualCanonicalEvidence, commonAnswerMessages, commonAnswerSources, diagnosticJudgmentConflict,
  evidenceIntervention } from './diagnostic-core.mjs';

const corpus = { source: 'alpha beta gamma delta',
  base: { lid_nodes: [
    { lid: '1.1', span: { start: 0, end: 11 } },
    { lid: '1.2', span: { start: 11, end: 22 } },
  ] }, leaves: [
    { lid: '1.1', start: 0, end: 11 },
    { lid: '1.2', start: 11, end: 22 },
  ] };

test('EV6 extracts only canonical text actually present in model requests', () => {
  const envelope = { model_body: { text: 'alpha beta ' },
    receipt: { accepted_evidence: [{ start_lid: '1.1', end_lid: '1.1' }] } };
  const request = { messages: [{ role: 'tool', tool_call_id: 'a', content: JSON.stringify(envelope) },
    { role: 'tool', tool_call_id: 'a', content: JSON.stringify(envelope) },
    { role: 'tool', tool_call_id: 'b', content: JSON.stringify({ model_body: { text: 'gamma' } }) }] };
  const agent = actualCanonicalEvidence({ system: AGENT, requests: [{ request }] }, corpus, 7);
  assert.equal(agent.selected_utf16, 7);
  assert.equal(agent.omitted_utf16, 4);
  assert.equal(agent.evidence[0].text, 'alpha b');
  const chunk = actualCanonicalEvidence({ system: CHUNK,
    requests: [{ request: { messages: [{ role: 'user', content: JSON.stringify({ evidence: [{ text: 'gamma delta' }] }) }] } }] }, corpus, 7);
  assert.equal(chunk.evidence[0].text, 'gamma d');
  assert.equal(commonAnswerMessages('question', agent.evidence)[0].content,
    commonAnswerMessages('question', chunk.evidence)[0].content);
  assert.deepEqual(commonAnswerSources('x [[source:E1]] [[source:E99]]', agent.evidence).map(s => s.valid), [true, false]);
});

test('EV5 only reads LIDs returned by the real locator results', () => {
  const tools = ['book_search_text', 'book_text'].map(name => ({ function: { name } }));
  const first = { tools, messages: [] };
  const search = evidenceIntervention(first, [{ purpose: 'business_loop' }]);
  assert.deepEqual(search.choices[0].message.tool_calls.map(c => c.function.name),
    ['book_search_text', 'book_search_text']);
  const messages = ['1.4.14', '1.4.25'].map((lid, i) => ({ role: 'tool',
    tool_call_id: `call_diag_search_${i}`,
    content: JSON.stringify({ status: 'ok', receipt: { tool: 'book.search_text' },
      model_body: { occurrences: [{ start_lid: lid, end_lid: lid }] } }) }));
  const second = evidenceIntervention({ tools, messages },
    [{ purpose: 'business_loop' }, { purpose: 'business_loop' }]);
  assert.deepEqual(JSON.parse(second.choices[0].message.tool_calls[0].function.arguments),
    { lid: '1.4.14', end_lid: '1.4.25' });
  assert.throws(() => evidenceIntervention({ tools, messages: messages.slice(0, 1) },
    [{ purpose: 'business_loop' }, { purpose: 'business_loop' }]), /Missing diagnostic search/);
});

test('free text fact values cannot turn an all-met judgment into a product failure', () => {
  const job = { kind: 'absolute', input: { required_facts: [{ key: 'ratio', acceptable: ['denominator_falls_more'] }] } };
  const judgment = { evaluations: [{ facts: [{ key: 'ratio', value: '分母下降更多', support_ids: ['C1'] }],
    dimensions: { accuracy: { verdict: 'met' }, completeness: { verdict: 'met' }, grounding: { verdict: 'met' } } }] };
  assert.equal(diagnosticJudgmentConflict(job, judgment), true);
  judgment.evaluations[0].facts[0].value = 'denominator_falls_more';
  assert.equal(diagnosticJudgmentConflict(job, judgment), false);
});
