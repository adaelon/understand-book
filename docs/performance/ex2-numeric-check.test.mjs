import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkClaim, expected } from './ex2-numeric-check.mjs';

const root = fileURLToPath(new URL('./ex2-windows/', import.meta.url));
const historical = JSON.parse(fs.readFileSync(new URL('./ex2-windows/historical-numeric-claims.json', import.meta.url)));

test('rejects both observed prose errors, including the wrong side of the optimum', () => {
  for (const claim of historical) assert.equal(checkClaim(claim, root).pass, false);
  assert.equal(expected(0.9, 3), 3.024);
  assert.equal(expected(0.6, 4), 1.9968);
});

test('accepts the correct preceding values at their stated precision', () => {
  assert.equal(checkClaim({ ...historical[0], step: 2, value: '0.72' }, root).pass, true);
  assert.equal(checkClaim({ ...historical[1], step: 3, value: '2.016' }, root).pass, true);
});

test('a missing quotation cannot be reported as checked evidence', () => {
  assert.throws(() => checkClaim({ ...historical[0], quote: 'this is not in the artifact' }, root), /missing source quote/);
});

test('rejects v4 confusing next step with the following step, with a Unicode minus', () => {
  const claims = JSON.parse(fs.readFileSync(new URL('./ex2-windows/v4-numeric-claims.json', import.meta.url)));
  assert.equal(checkClaim(claims.at(-1), root).pass, false);
  assert.equal(checkClaim(claims.find(claim => claim.value === '−1.92'), root).pass, true);
});

test('scientific notation uses the last displayed significant digit', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ex2-precision-'));
  try {
    fs.writeFileSync(path.join(dir, 'claim.txt'), '3.36e-5; 3.40e-5');
    const claim = { source: 'claim.txt', quote: '3.36e-5; 3.40e-5', eta: 0.7, step: 12, quantity: 'abs_e' };
    assert.equal(checkClaim({ ...claim, value: '3.36e-5' }, dir).pass, true);
    assert.equal(checkClaim({ ...claim, value: '3.40e-5' }, dir).pass, false);
  } finally {
    fs.unlinkSync(path.join(dir, 'claim.txt'));
    fs.rmdirSync(dir);
  }
});
