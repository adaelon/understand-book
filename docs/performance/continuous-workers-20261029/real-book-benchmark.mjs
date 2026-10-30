import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync, appendFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

// Maintenance-only harness. Semantic inputs and candidates stay in the Engine/executor.
const [command, ...argv] = process.argv.slice(2);
const arg = (name) => argv[argv.indexOf(name) + 1];
const required = (name) => { const value = argv.includes(name) && arg(name); if (!value) throw new Error(`Missing ${name}`); return path.resolve(value); };
const save = (file, value) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, JSON.stringify(value, null, 2) + '\n'); };
const read = (file) => JSON.parse(readFileSync(file, 'utf8'));
const log = (dir, value) => appendFileSync(path.join(dir, 'control-events.jsonl'), JSON.stringify({ at: new Date().toISOString(), ...value }) + '\n');
const codeImport = (source, relative) => import(pathToFileURL(path.join(source, relative)).href);

function runEngine(engine, operation, request, cwd) {
  const started = new Date().toISOString();
  const result = spawnSync(engine, [operation], { cwd, input: JSON.stringify(request), encoding: 'utf8', windowsHide: true, maxBuffer: 8 * 1024 * 1024 });
  if (result.error || result.status !== 0) throw result.error ?? new Error(`Engine ${operation} exited ${result.status}: ${result.stderr}`);
  return { started_at: started, finished_at: new Date().toISOString(), response: JSON.parse(result.stdout) };
}

if (command === 'prepare') {
  const source = required('--source-code');
  const original = required('--book');
  const output = required('--run');
  const engine = required('--engine');
  if (existsSync(output)) throw new Error('Run output already exists; reuse its existing invocation or choose a new isolated directory');
  if (output === original || output.startsWith(original + path.sep)) throw new Error('Run must be outside the original workspace');
  const workspace = path.join(output, '.understand-book', path.basename(original));
  mkdirSync(workspace, { recursive: true });
  const copied = [];
  const foundation = ['source.txt', 'source.epub', 'source_manifest.json', 'base.json', 'profile_metadata.json', 'asset_manifest.json', 'assets',
    'discourse_index.json', 'formula_semantics.json', 'math_structure.json', 'long_range_candidates.json', '.build/input', '.build/source-reconciliation'];
  for (const stage of ['pass1', 'profile_sidecar']) {
    for (const base of ['.build/automatic-build/v2/close', '.build/automatic-build/v2/publication',
      '.build/automatic-build/v3/artifacts', '.build/automatic-build/v3/shadow', '.build/automatic-build/v4/policies']) foundation.push(`${base}/${stage}`);
    foundation.push(`.build/automatic-build/v2/quality/${stage}.json`);
  }
  for (const relative of foundation) {
    const input = path.join(original, relative);
    if (!existsSync(input)) continue;
    const destination = path.join(workspace, relative);
    mkdirSync(path.dirname(destination), { recursive: true });
    cpSync(input, destination, { recursive: true, errorOnExist: true, force: false });
    copied.push(relative);
  }
  const { prepareExplicitLegacyBuildPlan } = await codeImport(source, 'skills/build/automatic-build.ts');
  const planned = prepareExplicitLegacyBuildPlan(workspace, output, { pass2: 'disabled', build_tutor: false });
  const projection = { version: planned.version, build_plan_path: planned.build_plan_path, plan_id: planned.plan.plan_id,
    plan_digest: planned.plan.plan_digest, public_stage_closure: planned.plan.public_stage_closure,
    create: planned.plan.create, reuse: planned.plan.reuse, excluded: planned.plan.excluded, estimate: planned.plan.estimate, budget: planned.plan.budget };
  save(path.join(output, 'plan-projection.json'), projection);
  if (JSON.stringify(planned.plan.create) !== JSON.stringify(['public.book_structure'])) {
    save(path.join(output, 'preparation-status.json'), { ready: false, reason: 'accepted prerequisites not recognized', projection });
    throw new Error('Plan must reuse Pass1/profile and create only BookStructure; inspect bounded plan-projection.json');
  }
  const created = runEngine(engine, 'build.step', { version: 'automatic_build_invocation_create.v1', target_input: workspace,
    root_dir: output, build_plan_path: planned.build_plan_path, quality_profile: 'full', max_parallel: 8, created_at: new Date().toISOString() }, output);
  if (created.response.version !== 'automatic_build_invocation_ref.v1') throw new Error(`Invocation not created: ${JSON.stringify(created.response)}`);
  const state = { version: 'continuous_workers_benchmark_run.v1', source_code: source, source_workspace: original, workspace_dir: workspace,
    run_dir: output, engine, invocation_ref: created.response.invocation_ref, quality_profile: 'full', max_parallel: 8,
    scope: 'whole-book BookStructure from accepted full-book Pass1/profile', copied_prerequisites: copied, live_by_slot: {}, completed_refs: [] };
  save(path.join(output, 'control-state.json'), state);
  log(output, { kind: 'invocation_created', ...created });
  console.log(JSON.stringify({ ready: true, ...projection, invocation_ref: state.invocation_ref, run_dir: output }));
} else if (command === 'engine') {
  const dir = required('--run');
  const engine = required('--engine');
  if (!existsSync(engine)) throw new Error('Engine must exist');
  const file = path.join(dir, 'control-state.json');
  const state = read(file);
  if (Object.keys(state.live_by_slot).length) throw new Error('Cannot change engine while children are live');
  state.engine = engine;
  save(file, state);
  log(dir, { kind: 'engine_selected', engine });
  console.log(JSON.stringify({ engine }));
} else if (command === 'step' || command === 'refill') {
  const dir = required('--run');
  const stateFile = path.join(dir, 'control-state.json');
  const state = read(stateFile);
  const extra = argv.includes('--request') ? read(required('--request'))
    : argv.includes('--terminal') ? { terminal_children: arg('--terminal').split(',') } : {};
  const request = command === 'step'
    ? { version: 'automatic_build_step_request.v1', invocation_ref: state.invocation_ref, available_agent_slots: 8 - Object.keys(state.live_by_slot).length, ...extra }
    : { version: 'automatic_build_refill_request.v1', invocation_ref: state.invocation_ref, capacity_limit: 8,
      live_by_slot: state.live_by_slot, completed_refs: state.completed_refs, terminal_children: [], ...extra };
  const result = runEngine(state.engine, `build.${command}`, request, dir);
  const response = result.response;
  if (response.version === 'automatic_build_refill.v1') { state.live_by_slot = response.live_by_slot; state.completed_refs = response.completed_refs; }
  save(stateFile, state);
  save(path.join(dir, 'last-control-result.json'), response);
  log(dir, { kind: command, request, ...result });
  const step = response.step ?? response;
  console.log(JSON.stringify({ version: response.version, action: step.action.kind === 'SPAWN_EXECUTORS'
    ? { kind: step.action.kind } : step.action,
    ready_executors: response.ready_executors ?? step.action.executors ?? [],
    live: Object.keys(state.live_by_slot).length, completed: state.completed_refs.length,
    progress: step.book_structure_progress && {
      fragments: step.book_structure_progress.discovery?.fragments,
      chapters: step.book_structure_progress.organization?.chapters,
      themes: step.book_structure_progress.organization?.themes,
      reconciliation: step.book_structure_progress.organization?.reconciliation,
      publication: step.book_structure_progress.publication,
      work: step.build_progress?.stages?.find(item => item.stage === 'book_structure')?.work,
    } }));
} else if (command === 'spawned') {
  const dir = required('--run');
  const file = path.join(dir, 'control-state.json');
  const state = read(file);
  const launches = argv.includes('--start')
    ? (read(path.join(dir, 'last-control-result.json')).ready_executors
      ?? read(path.join(dir, 'last-control-result.json')).action.executors).map((item, index) => ({ ...item,
        child: `/root/${arg('--prefix')}_${String(Number(arg('--start')) + index).padStart(3, '0')}` }))
    : read(required('--launches'));
  for (const launch of launches) {
    if (state.live_by_slot[launch.dispatch_slot_ref]) throw new Error('Slot already owned');
    state.live_by_slot[launch.dispatch_slot_ref] = { child: launch.child, opaque_handoff_ref: launch.opaque_handoff_ref };
    log(dir, { kind: 'child_started', ...launch });
  }
  save(file, state);
  console.log(JSON.stringify({ live: Object.keys(state.live_by_slot).length, completed: state.completed_refs.length }));
} else if (command === 'prepare-inputs') {
  const dir = required('--run');
  const state = read(path.join(dir, 'control-state.json'));
  const result = runEngine(state.engine, 'build.prepare', { version: 'automatic_build_prepare_request.v1', invocation_ref: state.invocation_ref, stage: 'book_structure' }, dir);
  log(dir, { kind: command, ...result });
  console.log(JSON.stringify(result.response));
} else {
  throw new Error('Use prepare, engine, step, refill, spawned, or prepare-inputs');
}
