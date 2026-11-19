// Exercise the public native controller against an isolated, authored book.
// Usage: node scripts/plugin/smoke-maintenance-host.mjs <host executable> [report.json]
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const executable = path.resolve(process.argv[2] ?? process.env.UNDERSTAND_BOOK_DESKTOP_EXE ?? '');
assert(process.argv[2] || process.env.UNDERSTAND_BOOK_DESKTOP_EXE, 'Pass the built maintenance executable');
assert(existsSync(executable), `maintenance executable is missing: ${executable}`);
const root = mkdtempSync(path.join(tmpdir(), 'understand-book-maintenance-smoke-'));
const dataHome = path.join(root, 'local-data-home');
const dataDir = path.join(root, 'selected-data');
const library = path.join(dataDir, 'library');
const workspace = path.join(library, 'fixture-book');
const source = 'A shared reference borrows access to a value. Ownership and read access are separate.';
const writeJson = (file, value) => writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
mkdirSync(workspace, { recursive: true });
mkdirSync(dataHome, { recursive: true });
writeJson(path.join(dataHome, 'data-location.json'), dataDir);
const settingsPath = path.join(dataDir, 'settings.json');
writeJson(settingsPath, {
  schema: 'understand_book.desktop_settings.v1', library_root: library,
  model_settings: { fixture: 'preserve the existing model catalog without conversion' },
});
const settingsBefore = readFileSync(settingsPath, 'utf8');
writeFileSync(path.join(workspace, 'source.txt'), source);
writeJson(path.join(workspace, 'base.json'), {
  book_id: 'public-maintenance-fixture',
  lid_nodes: [{ lid: '1.1', path: [1, 1], kind: 'paragraph', span: { start: 0, end: source.length }, children: [] }],
  graph_nodes: [], graph_edges: [],
});
const env = {
  ...process.env,
  UNDERSTAND_BOOK_LOCAL_DATA_DIR: dataHome,
  UNDERSTAND_BOOK_PROVIDER: 'native', OPENCODE_API_KEY: 'fixture-no-provider',
  OPENCODE_BASE_URL: 'http://127.0.0.1:9/v1', FLUID_LLM_MODEL: 'fixture-no-provider',
};
// The selected local Reader directory owns this test's private plans.
delete env.UNDERSTAND_BOOK_PRIVATE_DIR;
const observations = [];
function call(operation, input, options = {}) {
  const request = { version: options.version ?? 'codex_build_intent_command.v3', operation,
    target: { workspace_dir: options.workspace ?? workspace }, input };
  const result = spawnSync(executable, ['--codex-build-intent'], {
    input: JSON.stringify(request), encoding: 'utf8', env: options.env ?? env,
    timeout: 60_000, maxBuffer: 4 * 1024 * 1024, windowsHide: true,
  });
  assert.ifError(result.error);
  assert.equal(result.status, options.exit ?? 0, result.stdout + result.stderr);
  assert.equal(result.stderr, '', result.stderr);
  assert.equal(result.stdout.trim().split(/\r?\n/u).length, 1);
  const envelope = JSON.parse(result.stdout);
  if (request.version !== 'codex_build_intent_command.v1') {
    assert.equal(envelope.version, 'codex_build_intent_result.v2');
    assert.equal(envelope.status, options.exit ? 'error' : 'ok');
  }
  assert(!result.stdout.includes('PRIVATE_GOAL_FIXTURE'), 'controller exposed the raw goal');
  assert(!result.stdout.includes('PRIVATE_CANDIDATE_FIXTURE'), 'controller exposed candidate content');
  observations.push({ operation, version: request.version, exit_code: result.status,
    response_version: envelope.response?.version ?? envelope.version, error_code: envelope.error?.error_code });
  return envelope;
}

try {
  const old = call('planning.context', {}, { version: 'codex_build_intent_command.v2', exit: 2 });
  assert.equal(old.error.error_code, 'CODEX_BUILD_INTENT_REQUEST_INVALID');
  const context = call('planning.context', {}).response;
  assert.equal(context.version, 'build_planning_context.v2');
  assert.equal(typeof context.context_id, 'string');
  assert.equal(context.context_revision, 1);
  assert.equal(context.context_digest, undefined);
  assert(context.scope_catalog.available_lids.includes('1.1'));
  const preset = context.blueprint_registry.find((entry) => entry.source === 'system');
  assert(preset, 'public Core must supply a system Blueprint');
  const draft = {
    user_goal: 'PRIVATE_GOAL_FIXTURE', context_id: context.context_id, context_revision: context.context_revision,
    candidate: {
      version: 'build_intent_planner_candidate.v2', goal_kind: 'learn',
      source_scope: { whole_book: true, lids: [], sections: [] },
      artifacts: [{ source: preset.source, blueprint_id: preset.blueprint_id, blueprint_version: preset.blueprint_version }],
      usage_horizon: 'project',
    },
  };
  const invalid = call('draft.candidate', { ...draft, candidate: { version: 'invalid', secret: 'PRIVATE_CANDIDATE_FIXTURE' } }, { exit: 2 });
  assert.equal(invalid.error.phase, 'candidate');
  const drift = call('draft.candidate', { ...draft, context_revision: context.context_revision + 1 }, { exit: 2 });
  assert.equal(drift.error.error_code, 'BUILD_PLANNING_CONTEXT_DRIFT');
  const candidate = call('draft.candidate', draft).response;
  assert.equal(candidate.version, 'codex_build_intent_response.v1');
  assert.equal(candidate.projection.version, 'codex_build_intent_plan.v3');
  const plan = candidate.projection.plan;
  assert.equal(typeof plan.plan_id, 'string');
  assert.equal(plan.plan_revision, 1);
  const privateRoot = path.join(dataDir, 'private');
  assert(path.resolve(candidate.build_plan_path).startsWith(`${privateRoot}${path.sep}`));
  assert(!existsSync(path.join(workspace, '.build', 'automatic-build')), 'draft started public build work');
  const stale = call('confirm', { plan_id: plan.plan_id, plan_revision: plan.plan_revision + 1 }, { exit: 2 });
  assert.equal(stale.error.category, 'conflict');
  const confirmed = call('confirm', { plan_id: plan.plan_id, plan_revision: plan.plan_revision }).response;
  assert.equal(confirmed.projection.plan.plan_id, plan.plan_id);
  assert.equal(confirmed.projection.plan.plan_revision, plan.plan_revision);
  const current = call('status', { plan_id: plan.plan_id }).response;
  assert.equal(current.build_plan_path, confirmed.build_plan_path);
  assert.equal(readFileSync(settingsPath, 'utf8'), settingsBefore, 'controller rewrote local model/settings data');

  // Keep the established cloud-operator default without coupling it to cloud code.
  const localAppData = path.join(root, 'legacy-local-app-data');
  mkdirSync(path.join(localAppData, 'UnderstandBook'), { recursive: true });
  writeJson(path.join(localAppData, 'UnderstandBook', 'settings.json'), {
    schema: 'understand_book.desktop_settings.v1', library_root: library,
  });
  const legacyEnv = { ...env, LOCALAPPDATA: localAppData, UNDERSTAND_BOOK_PRIVATE_DIR: path.join(root, 'legacy-private') };
  delete legacyEnv.UNDERSTAND_BOOK_LOCAL_DATA_DIR;
  const legacy = call('status', {}, { version: 'codex_build_intent_command.v1', env: legacyEnv });
  assert.equal(legacy.version, 'codex_build_intent_response.v1');
  assert.equal(legacy.inspection.book_id, 'public-maintenance-fixture');
  const report = { status: 'passed', executable, model_calls: 0, source: 'authored isolated fixture',
    core_execution: env.UNDERSTAND_BOOK_BUILD_EXE || env.UNDERSTAND_BOOK_BUILD_SIDECAR ? 'configured executable' : 'source development entry',
    build_engine: env.UNDERSTAND_BOOK_BUILD_SIDECAR || env.UNDERSTAND_BOOK_BUILD_EXE || null,
    checks: ['revision context/draft/confirm/status', 'stale context and plan rejected', 'private inputs redacted',
      'Reader data-location and private store', 'model settings preserved', 'legacy operator settings'], observations };
  if (process.argv[3]) writeJson(path.resolve(process.argv[3]), report);
  console.log(JSON.stringify(report, null, 2));
  assert(path.dirname(root) === path.resolve(tmpdir()) && path.basename(root).startsWith('understand-book-maintenance-smoke-'));
  rmSync(root, { recursive: true, force: true });
} catch (error) {
  console.error(`Isolated maintenance fixture retained at ${root}`);
  throw error;
}
