import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lexicalLanes, fuse, metrics } from './ranking.mjs';

test('fusion retains exact aliases and fills past duplicates without counting them toward semantic quota', () => {
  const record = (key, meaning, aliases = []) => ({ key, meaning, aliases, lexical_fields: [meaning, ...aliases] });
  const lanes = lexicalLanes([record('self', '速率'), record('alias', '路程/时间', ['速率']), record('lexical', '平均速率')], '速率', 'self');
  assert.deepEqual(lanes, { exact: ['alias'], lexical: ['lexical'] });
  assert.deepEqual(fuse(lanes, ['alias', 'lexical', 's1', 's2', 's3'], 2), ['alias', 'lexical', 's1', 's2']);
  assert.deepEqual(metrics(['self', 'relevant'], ['relevant', 'missing'], 6), {
    hits: 1, relevant: 2, returned: 2, recall: 0.5, precision_at_k: 1 / 6, precision_returned: 0.5,
  });
});
