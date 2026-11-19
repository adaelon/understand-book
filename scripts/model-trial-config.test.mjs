import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { selectTrialProvider, positiveLimit, trialCostBudget } from './model-trial-config.mjs';

test('explicit model settings preserve the real endpoint and allow no-key services', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'model-trial-'));
  try {
    const filename = path.join(dir, 'settings.json');
    fs.writeFileSync(filename, JSON.stringify({ revision: 2, services: [
      { id: 'local', base_url: 'http://localhost:11434', auth: 'none', api_key: '' },
      { id: 'azure', base_url: 'https://example.openai.azure.com/openai/deployments/d?api-version=2025-04-01-preview', auth: 'api_key', api_key: 'secret' },
    ], models: [
      { id: 'local-model', service_id: 'local', model_id: 'qwen3', api_protocol: 'ollama_chat' },
      { id: 'cloud-model', service_id: 'azure', model_id: 'deployment', api_protocol: 'openai_responses' },
    ], bindings: { default_model: 'local-model' } }));
    const local = selectTrialProvider({}, filename);
    assert.equal(local.direct, true);
    assert.equal(local.model, 'qwen3');
    assert.equal(local.environment.UNDERSTAND_BOOK_MODEL_CONFIG_ID, 'local-model');
    const cloud = selectTrialProvider({}, filename, 'cloud-model');
    assert.match(cloud.base, /\?api-version=/);
    assert.equal(cloud.metadata.api_protocol, 'openai_responses');
    assert.equal(JSON.stringify(cloud).includes('secret'), false);
    assert.throws(() => selectTrialProvider({ OPENCODE_API_KEY: 'fallback' }, filename, 'missing'), /not found/);
  } finally { fs.rmSync(dir, { recursive: true }); }
});

test('legacy aliases remain stable and an explicit invalid selection never falls back', () => {
  assert.equal(selectTrialProvider({ OPENCODE_BASE_URL: 'https://api.deepseek.com', OPENCODE_API_KEY: 'k', FLUID_LLM_MODEL: 'deepseek-v4-flash' }).model, 'deepseek-flash');
  assert.throws(() => selectTrialProvider({}, undefined, 'chosen'), /requires/);
  assert.throws(() => positiveLimit('0', 24, 'requests'), /positive integer/);
  assert.equal(positiveLimit('5', 24, 'requests'), 5);
});

test('CNY trial budget produces exact integer units and a real rates-file path', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'model-trial-cost-'));
  try {
    const filename = path.join(dir, 'model-rates.json');
    fs.writeFileSync(filename, '{"rates":[]}');
    const budget = trialCostBudget('2', filename);
    assert.equal(budget.max_cost_micro_cny, 2000000);
    assert.equal(budget.environment.MODEL_TRIAL_MAX_COST_MICRO_CNY, '2000000');
    assert.equal(budget.environment.MODEL_TRIAL_RATES_FILE, path.resolve(filename));
    assert.equal(trialCostBudget('0.000001', filename).max_cost_micro_cny, 1);
    assert.equal(trialCostBudget('1.234567', filename).max_cost_micro_cny, 1234567);
    assert.equal(trialCostBudget(), null);
  } finally { fs.rmSync(dir, { recursive: true }); }
});

test('CNY trial budget rejects incomplete, unrepresentable or unavailable configuration before a trial starts', () => {
  for (const pair of [['2', undefined], [undefined, 'rates.json']]) {
    assert.throws(() => trialCostBudget(...pair), /supplied together/);
  }
  for (const value of ['0', '-1', '0.0000001', 'NaN', '2e1', '9007199254.740992']) {
    assert.throws(() => trialCostBudget(value, 'unused.json'), /amount|range/);
  }
  assert.throws(() => trialCostBudget('2', path.join(os.tmpdir(), 'missing-model-trial-rates-file.json')), /ENOENT/);
});

test('the trial CLI rejects a cost flag with no value instead of silently disabling the budget', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'model-trial-cli-'));
  try {
    const script = fileURLToPath(new URL('./verify-course-reuse.mjs', import.meta.url));
    const result = spawnSync(process.execPath, [script, '--test-bin', 'unused', '--out', 'unused', '--max-cost-cny'], {
      cwd: dir, encoding: 'utf8', windowsHide: true,
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /--max-cost-cny requires a value/);
    assert.equal(fs.existsSync(path.join(dir, 'unused')), false);
  } finally { fs.rmSync(dir, { recursive: true }); }
});
