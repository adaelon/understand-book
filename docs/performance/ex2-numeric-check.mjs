// EX2 evidence check: annotated claims come from saved outputs, never model input.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function expected(eta, step, quantity = 'w') {
  const error = -2 * (1 - 2 * eta) ** step;
  if (quantity === 'e') return error;
  if (quantity === 'abs_e') return Math.abs(error);
  if (quantity === 'loss') return 4 * (1 - 2 * eta) ** (2 * step);
  assert.equal(quantity, 'w');
  return 2 + error;
}

export function checkClaim(claim, root) {
  const source = fs.readFileSync(path.resolve(root, claim.source), 'utf8');
  // Evidence may be JSON; serialize-decoding preserves the literal human text.
  const text = claim.field ? claim.field.split('.').reduce((v, k) => v[k], JSON.parse(source)) : source;
  assert.ok(text.includes(claim.quote), `missing source quote: ${claim.source}`);
  const actual = Number(claim.value.replaceAll('−', '-'));
  assert.ok(claim.quote.includes(claim.value), 'value must occur in the source quote');
  const value = expected(claim.eta, claim.step, claim.quantity);
  const [mantissa, exponent = '0'] = claim.value.split(/[eE]/);
  const decimals = mantissa.split('.')[1]?.length ?? 0;
  const tolerance = 0.5 * 10 ** (Number(exponent) - decimals) + 1e-10;
  return { ...claim, expected: value, tolerance, pass: Math.abs(actual - value) <= tolerance };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const manifest = path.resolve(process.argv[2]);
  const claims = JSON.parse(fs.readFileSync(manifest, 'utf8'));
  const results = claims.map(claim => checkClaim(claim, path.dirname(manifest)));
  console.log(JSON.stringify({ pass: results.every(r => r.pass), results }, null, 2));
  if (results.some(r => !r.pass)) process.exitCode = 1;
}
