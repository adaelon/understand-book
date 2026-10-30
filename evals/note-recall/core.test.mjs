import test from 'node:test';
import assert from 'node:assert/strict';
import { aggregate, compareFusion, ranksFor, scoreCase, validateCases } from './core.mjs';

const hit = (id, lexical, semantic, structural) => ({ reference: { note_id: id }, lexical_rank: lexical, semantic_rank: semantic, structural_rank: structural });

test('case files need unique ids, a query and note ids', () => {
  assert.equal(validateCases({ cases: [{ id: 'a', query: 'q', relevant: ['n1'] }] }).length, 1);
  for (const bad of [{}, { cases: [] }, { cases: [{ id: 'a', query: '', relevant: [] }] },
    { cases: [{ id: 'a', query: 'q', relevant: [] }, { id: 'a', query: 'q', relevant: [] }] }]) {
    assert.throws(() => validateCases(bad));
  }
});

test('each ranker keeps its own rank; fused rank is the position', () => {
  const hits = [hit('n2', null, 1), hit('n1', 3, 7)];
  assert.deepEqual(ranksFor(hits, 'n1'), { lexical: 3, semantic: 7, fused: 2, structural: null });
  assert.deepEqual(ranksFor(hits, 'gone'), { lexical: null, semantic: null, fused: null, structural: null });
});

test('the target set is what both baselines miss, and structural finds are counted against it', () => {
  const item = { id: 'c', query: 'q', relevant: ['n1', 'n2', 'n3'] };
  const result = scoreCase(item, [hit('n1', 1, 2), hit('n2', 12, 15, 2), hit('n9', 2, 1)]);
  assert.deepEqual(result.recall.lexical, { 5: 1 / 3, 10: 1 / 3 });
  assert.deepEqual(result.missed_by_baselines, ['n2', 'n3']);
  assert.deepEqual(result.structural_found, ['n2']);
  const report = aggregate([result, scoreCase({ id: 'd', query: 'q', relevant: [] }, [])]);
  assert.equal(report.scored_cases, 1);
  assert.equal(report.target_set.length, 2);
  assert.match(report.gate, /^continue/);
  assert.match(aggregate([scoreCase({ id: 'e', query: 'q', relevant: ['n1'] }, [hit('n1', 1, 1)])]).gate, /^stop/);
});

test('three-way fusion reports new and false recall separately', () => {
  const item = { id: 'c', query: 'q', relevant: ['n1', 'n2'] };
  const two = [hit('n1', 1, 1), hit('n8', 2, 2)];
  const three = [hit('n1', 1, 1), hit('n2', null, null, 1), hit('n7', null, null, 2)];
  assert.deepEqual(compareFusion(item, two, three, 3), { k: 3, new_recall: ['n2'], false_recall: ['n7'], displaced_relevant: [] });
  const result = { ...scoreCase(item, three), fusion: compareFusion(item, two, three, 3) };
  const report = aggregate([result]);
  assert.deepEqual(report.fusion, { cases: 1, new_recall: 1, false_recall: 1, displaced_relevant: 0 });
  assert.match(report.m3_gate, /^continue/);
  assert.equal(aggregate([scoreCase(item, two)]).m3_gate, undefined, 'no M3 verdict without a structural run');
});
