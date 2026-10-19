import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { buildExplanationCase, collectExplanationCase, publicCaseIndex } from './explanation-case.mjs';

function fixture(t) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'se6-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const owner = { book_id: 'private-book', session_id: 'private-session' };
  const reference = { presentation_id: 'private-page', revision: 1 };
  const read = (id, file) => ({ tool_call_id: id, method_id: 'update', revision: 1, file, offset: 0, characters: 5, end_offset: 5, total_characters: 100 });
  const use = { method_id: 'update', revision: 1, scope: 'private-scope', basis: { kind: 'read', turn_id: 't1', tool_call_ids: ['card', 'code'] } };
  const original = { owner, reference, created_by_turn_id: 't1', candidate_id: 'c1', based_on: null,
    content: { title: 'private-title', entrypoint: 'index.html', content_files: { 'index.html': '<p>before</p>', 'style.css': 'p{color:red}' },
      method_refs: [use], method_observations: { catalog: { methods: [{ method_id: 'update', revision: 1 }] }, reads: [read('card', 'card.md'), read('code', 'example/index.html')] } } };
  const repaired = structuredClone(original);
  repaired.reference.revision = 2; repaired.based_on = reference; repaired.created_by_turn_id = 't2'; repaired.candidate_id = 'c2';
  repaired.content.content_files['index.html'] = '<p>after</p>';
  repaired.content.method_refs[0].basis = { kind: 'version', reference };
  repaired.content.method_observations = { catalog: null, reads: [] };
  const receipt = { session_id: owner.session_id, turn_id: 't1', reference, state_revision: 2, saved_state_ref: 'exact-snapshot' };
  const feedback = { session_id: owner.session_id, turn_id: 't1', reference, feedback_revision: 1,
    selection: { choice: 'dislike', reasons: ['hard_to_operate'], comment: 'PRIVATE COMMENT' }, saved_state: receipt };
  const history = { sessions: [{ id: owner.session_id, book_id: owner.book_id, turns: [original, repaired].map(v => ({ turn_id: v.created_by_turn_id,
    status: 'completed', outcome: { answer_view: { parts: [{ kind: 'presentation', ...v.reference }] } } })) }] };
  const manifest = { schema_version: 'explanation-case-input.v1', case_id: 'PRIVATE CASE', origin: 'engineering_fixture', obstacle: 'PRIVATE OBSTACLE',
    materials: {}, observations: [{ fact: 'PRIVATE FACT', evidence: ['check'] }], interpretations: [{ explanation: 'PRIVATE CAUSE', evidence: ['check'] }] };
  const put = (name, data) => {
    const file = `${name}.json`; fs.writeFileSync(path.join(directory, file), JSON.stringify(data));
    manifest.materials[name] = { path: file }; return data;
  };
  put('original', original); put('repaired', repaired); put('history', history);
  put('feedback_current', feedback); put('feedback_scene', { owner, receipt, state: { values: { x: 7.999 }, observed_result: 'PRIVATE SCENE' } });
  put('task', { question: 'PRIVATE QUESTION' }); put('source', { version: 'PRIVATE SOURCE VERSION' }); put('check', { passed: true });
  const build = () => buildExplanationCase(manifest, directory, '2026-10-16T04:00:00Z');
  return { directory, manifest, put, build, original, repaired, feedback, receipt, owner, history };
}

test('exact delivery, observed ranges, inherited method, feedback scene and content difference stay linked', t => {
  const f = fixture(t), result = f.build();
  assert.equal(result.original_delivery.status, 'confirmed');
  assert.equal(result.repaired_delivery.status, 'confirmed');
  assert.equal(result.methods.declarations[0].evidence.code_read, true);
  assert.equal(result.methods.declarations[0].evidence.reads[1].total_characters, 100);
  assert.equal(result.repaired_methods.declarations[0].evidence.chain.length, 2);
  assert.equal(result.repaired_methods.declarations[0].evidence.status, 'observed');
  assert.equal(result.feedback.scene.state.values.x, 7.999);
  assert.deepEqual(result.difference.files, [{ file: 'index.html', status: 'changed' }, { file: 'style.css', status: 'unchanged' }]);
  assert.deepEqual(result.attribution.method_credits, []);
  assert.equal(result.status, 'pending_diagnosis');
});

test('missing scene, missing feedback and withdrawn feedback remain distinct', t => {
  const f = fixture(t);
  f.feedback.saved_state = null; f.put('feedback_current', f.feedback);
  assert.equal(f.build().feedback.scene.status, 'missing');
  f.feedback.selection = null; f.feedback.feedback_revision = 2; f.put('feedback_current', f.feedback);
  assert.equal(f.build().feedback.status, 'withdrawn');
  f.feedback.feedback_revision = 0; f.put('feedback_current', f.feedback);
  assert.equal(f.build().feedback.status, 'missing');
  delete f.manifest.materials.feedback_current;
  assert.equal(f.build().feedback.status, 'missing');
});

test('current selection replaces a prior vote; withdrawal and unavailable current state never reuse it', t => {
  const f = fixture(t); f.put('feedback_previous', f.feedback);
  f.feedback.feedback_revision = 2; f.feedback.selection = { choice: 'like', reasons: [], comment: '' }; f.feedback.saved_state = null;
  f.put('feedback_current', f.feedback);
  f.manifest.materials.feedback_current.observed_at = '2026-10-15T00:00:00Z';
  assert.equal(f.build().feedback.transition, 'changed_selection');
  assert.equal(f.build().feedback.as_of, '2026-10-15T00:00:00Z');
  assert.equal(f.build().feedback.freshness, 'export_snapshot');
  assert.equal(f.build().feedback.current.data.selection.choice, 'like');
  assert.equal(f.build().feedback.scene.status, 'missing');
  f.feedback.feedback_revision = 3; f.feedback.selection = null; f.put('feedback_current', f.feedback);
  assert.equal(f.build().feedback.transition, 'withdrawn');
  f.manifest.materials.feedback_current = { status: 'unavailable', reason: 'access_revoked', path: 'feedback_previous.json' };
  assert.equal(f.build().feedback.status, 'unavailable');
  assert.equal(f.build().feedback.current.data, undefined);
});

test('wrong version, session, turn or snapshot is not usable feedback evidence', t => {
  const f = fixture(t);
  for (const change of [{ reference: { ...f.feedback.reference, revision: 2 } }, { session_id: 'other' }, { turn_id: 't2' }]) {
    f.put('feedback_current', { ...f.feedback, ...change });
    assert.equal(f.build().feedback.status, 'identity_mismatch');
  }
  f.put('feedback_current', f.feedback);
  f.put('feedback_scene', { owner: f.owner, receipt: { ...f.receipt, state_revision: 3 }, state: {} });
  assert.equal(f.build().feedback.scene.status, 'identity_mismatch');
  f.feedback.saved_state = { ...f.receipt, turn_id: 't2' }; f.put('feedback_current', f.feedback);
  delete f.manifest.materials.feedback_scene;
  assert.equal(f.build().feedback.scene.status, 'identity_mismatch');
});

test('unrecorded use, declared none, catalog-only and card-only reads are distinguished', t => {
  const f = fixture(t);
  f.original.content.method_observations.reads = [];
  f.put('original', f.original);
  assert.equal(f.build().methods.declarations[0].evidence.status, 'unobserved');
  f.original.content.method_refs = []; f.put('original', f.original);
  assert.equal(f.build().methods.status, 'none_declared');
  delete f.original.content.method_refs; f.put('original', f.original);
  assert.equal(f.build().methods.status, 'unrecorded');
  f.original.content.method_refs = [{ method_id: 'update', revision: 1, basis: { kind: 'read', turn_id: 't1', tool_call_ids: ['card'] } }];
  f.original.content.method_observations.reads = [{ method_id: 'update', revision: 1, tool_call_id: 'card', file: 'card.md', offset: 5, characters: 10, end_offset: 15, total_characters: 100 }];
  f.put('original', f.original);
  const evidence = f.build().methods.declarations[0].evidence;
  assert.equal(evidence.status, 'observed'); assert.equal(evidence.card_read, true); assert.equal(evidence.code_read, false);
});

test('candidate inheritance resolves to the exact original read and missing basis stays unresolved', t => {
  const f = fixture(t);
  const basis = structuredClone(f.original); delete basis.reference; basis.candidate_id = 'parent';
  f.original.content.method_refs[0].basis = { kind: 'candidate', candidate_id: 'parent' }; f.put('original', f.original);
  f.put('basis', basis); f.manifest.method_basis = ['basis'];
  assert.equal(f.build().methods.declarations[0].evidence.status, 'observed');
  f.manifest.materials.basis = { status: 'unavailable', reason: 'access_revoked' };
  assert.equal(f.build().methods.declarations[0].evidence.status, 'unresolved_basis');
});

test('a page vote is not distributed across declared methods', t => {
  const f = fixture(t);
  f.original.content.method_refs.push({ method_id: 'comparison', revision: 2, scope: 'another panel', basis: { kind: 'read', turn_id: 't1', tool_call_ids: ['unread'] } });
  f.put('original', f.original);
  assert.equal(f.build().methods.declarations.length, 2);
  assert.deepEqual(f.build().attribution.method_credits, []);
  assert.equal(f.build().feedback.attribution, 'page');
});

test('unavailable source, original or evidence is explicit and prevents an evidence-complete claim', t => {
  const f = fixture(t);
  f.manifest.materials.source = { status: 'unavailable', reason: 'access_revoked', path: 'source.json' };
  f.manifest.materials.check = { path: 'absent.json' };
  const result = f.build();
  assert.equal(result.materials.source.data, undefined);
  assert.equal(result.observations[0].evidence_status, 'incomplete');
  assert.ok(result.gaps.includes('source_unavailable'));
  f.manifest.materials.original = { status: 'unavailable', reason: 'chat_deleted' };
  assert.equal(f.build().original_delivery.status, 'unavailable');
  assert.equal(f.build().difference.status, 'unavailable');
});

test('orphan versions and unrelated repairs are not called confirmed deliveries', t => {
  const f = fixture(t);
  f.history.sessions[0].turns[0].outcome.answer_view.parts = []; f.put('history', f.history);
  assert.equal(f.build().original_delivery.status, 'not_delivered');
  f.repaired.based_on.revision = 9; f.put('repaired', f.repaired);
  assert.equal(f.build().difference.status, 'unrelated_revision');
});

test('reuses provider tool evidence, failed-run attempts and measured usage', t => {
  const f = fixture(t);
  f.put('provider_records', [{ request: { messages: [{ role: 'tool', tool_call_id: 'r', content: JSON.stringify({ model_body: { status: 'version_read' } }) }] },
    response: { choices: [{ message: { tool_calls: [{ function: { name: 'presentation_author', arguments: '{}' } }] } }] }, usage: { total_tokens: 123 } }]);
  const result = f.build();
  assert.equal(result.run.tool_results[0].model_body.status, 'version_read');
  assert.equal(result.run.trace[0].tool, 'presentation.author');
  assert.equal(result.run.usage.total_tokens, 123);
});

test('selects one existing run and marks missing or malformed exports without inventing evidence', t => {
  const f = fixture(t);
  f.put('provider_records', { qa: [{ requests: [{ usage: { total_tokens: 12 }, request: { messages: [] } }] }] });
  f.manifest.materials.provider_records.select = ['qa', 0, 'requests'];
  assert.equal(f.build().run.usage.total_tokens, 12);
  f.manifest.materials.provider_records.select = ['qa', 1, 'requests'];
  assert.equal(f.build().run.status, 'missing');
  assert.equal(f.build().run.usage.total_tokens, null);
  fs.writeFileSync(path.join(f.directory, 'check.json'), '{incomplete export');
  assert.equal(f.build().materials.check.reason, 'invalid_json');
  assert.equal(f.build().observations[0].evidence_status, 'incomplete');
});

test('private export round trips without overwriting; index excludes raw content and paths', t => {
  const f = fixture(t);
  f.put('PRIVATE-EVIDENCE-NAME', { comment: 'PRIVATE COMMENT' });
  const filename = path.join(f.directory, 'manifest.json'); fs.writeFileSync(filename, JSON.stringify(f.manifest));
  const output = path.join(f.directory, 'out');
  const result = collectExplanationCase(filename, output);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(output, 'case.json'))), result);
  assert.throws(() => collectExplanationCase(filename, output), /EEXIST/);
  const index = JSON.stringify(publicCaseIndex(result));
  for (const privateText of ['PRIVATE', 'private-', f.directory, 'exact-snapshot', 'index.html', 'style.css']) assert.equal(index.includes(privateText), false);
});
