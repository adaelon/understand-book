import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual as equal } from 'node:util';
import { toolMessages } from './agent-core.mjs';
import { reportToolTrace } from './agent-report.mjs';
import { measuredUsage } from './provider-recorder.mjs';

const available = material => material?.status === 'available';
const sameRef = (a, b) => !!a && !!b && a.presentation_id === b.presentation_id && a.revision === b.revision;
const sameMethod = (a, b) => a.method_id === b.method_id && a.revision === b.revision;

// Only explicitly selected, authorized exports are read. An unavailable export
// never falls back to a live store or another revision.
export function readCaseMaterial(spec, directory) {
  if (!spec) return { status: 'missing', reason: 'not_supplied' };
  if (spec.status === 'missing' || spec.status === 'unavailable') return { status: spec.status, reason: spec.reason, observed_at: spec.observed_at ?? null };
  const filename = path.resolve(directory, spec.path);
  try {
    const text = fs.readFileSync(filename, 'utf8').replace(/^\uFEFF/, '');
    let data = spec.format === 'text' ? text : JSON.parse(text);
    for (const key of spec.select ?? []) data = data?.[key];
    if (data === undefined) return { status: 'missing', reason: 'selection_not_found', path: filename };
    return { status: 'available', path: filename, select: spec.select ?? [], observed_at: spec.observed_at ?? null, data };
  } catch (error) {
    const reason = error.code === 'ENOENT' ? 'file_not_found' : error.code === 'EACCES' || error.code === 'EPERM'
      ? 'access_denied' : error instanceof SyntaxError ? 'invalid_json' : 'read_failed';
    return { status: reason === 'file_not_found' ? 'missing' : 'unavailable', reason, path: filename };
  }
}

function delivery(version, history) {
  if (!available(version)) return { status: version.status };
  if (!available(history)) return { status: history.status };
  const v = version.data;
  const session = history.data.sessions?.find(s => s.id === v.owner?.session_id && s.book_id === v.owner?.book_id);
  const turns = session?.turns?.filter(t => t.status === 'completed' &&
    t.outcome?.answer_view?.parts?.some(p => p.kind === 'presentation' && sameRef(p, v.reference))) ?? [];
  return { status: turns.length ? 'confirmed' : 'not_delivered', turn_ids: turns.map(t => t.turn_id) };
}

export function methodEvidence(version, basisMaterials = []) {
  if (!available(version)) return { status: version.status, declarations: [] };
  const root = version.data;
  const resolve = (node, use, visited = new Set()) => {
    const identity = node.reference ? JSON.stringify(node.reference) : node.candidate_id;
    if (visited.has(identity)) return { status: 'unresolved_basis', reason: 'cyclic_basis', chain: [] };
    const chain = [{ reference: node.reference ?? null, candidate_id: node.candidate_id, basis: use.basis }];
    const basis = use.basis;
    if (basis?.kind === 'read') {
      const reads = (node.content.method_observations?.reads ?? []).filter(r => sameMethod(r, use)
        && basis.tool_call_ids.includes(r.tool_call_id) && r.characters > 0 && r.end_offset === r.offset + r.characters);
      const allCalls = basis.tool_call_ids.length > 0 && basis.tool_call_ids.every(id => reads.some(r => r.tool_call_id === id));
      if (basis.turn_id !== node.created_by_turn_id || !allCalls) return { status: 'unobserved', chain, reads };
      return { status: 'observed', chain, reads, card_read: reads.some(r => r.file === 'card.md'),
        code_read: reads.some(r => r.file.startsWith('example/') && /\.(html|js|ts|css|svg)$/.test(r.file)) };
    }
    const parent = basisMaterials.filter(available).map(m => m.data).find(v =>
      equal(v.owner, root.owner) && (basis?.kind === 'version' ? sameRef(v.reference, basis.reference)
        : basis?.kind === 'candidate' && v.candidate_id === basis.candidate_id));
    const inherited = parent?.content?.method_refs?.find(r => sameMethod(r, use));
    if (!inherited) return { status: 'unresolved_basis', chain };
    const next = resolve(parent, inherited, new Set([...visited, identity]));
    return { ...next, chain: [...chain, ...next.chain] };
  };
  const refs = root.content.method_refs;
  return { status: refs == null ? 'unrecorded' : refs.length ? 'declared' : 'none_declared',
    catalog: root.content.method_observations?.catalog ?? null,
    observed_reads: root.content.method_observations?.reads ?? [],
    declarations: (refs ?? []).map(use => ({ ...use, evidence: resolve(root, use) })) };
}

function feedbackEvidence(current, previous, original, delivered, scene) {
  const result = { status: current.status, current, previous, transition: 'unknown', attribution: 'page',
    as_of: current.observed_at ?? null, freshness: 'export_snapshot',
    scene: { status: 'missing', reason: 'no_feedback_scene' } };
  if (!available(current)) return result;
  const f = current.data;
  const matches = value => sameRef(value.reference, original.data?.reference)
    && value.session_id === original.data?.owner?.session_id && delivered.turn_ids?.includes(value.turn_id);
  if (!matches(f)) return { ...result, status: 'identity_mismatch' };
  result.status = f.selection ? 'active' : f.feedback_revision > 0 ? 'withdrawn' : 'missing';
  if (available(previous)) {
    const old = previous.data;
    result.transition = !matches(old) || old.feedback_revision > f.feedback_revision ? 'invalid_previous'
      : old.feedback_revision === f.feedback_revision
        ? equal(old.selection, f.selection) ? 'unchanged' : 'invalid_previous'
        : !f.selection ? 'withdrawn' : old.selection?.choice !== f.selection.choice ? 'changed_selection' : 'updated';
  }
  if (!f.selection || !f.saved_state) return result;
  if (!sameRef(f.saved_state.reference, f.reference) || f.saved_state.session_id !== f.session_id || f.saved_state.turn_id !== f.turn_id) {
    return { ...result, scene: { status: 'identity_mismatch', reason: 'not_the_feedback_delivery' } };
  }
  if (!available(scene)) return { ...result, scene: { status: scene.status, reason: scene.reason, receipt: f.saved_state } };
  const saved = scene.data;
  const valid = equal(saved.receipt, f.saved_state) && matches(saved.receipt) && equal(saved.owner, original.data?.owner);
  result.scene = valid ? { status: 'available', receipt: saved.receipt, state: saved.state }
    : { status: 'identity_mismatch', reason: 'not_the_feedback_snapshot' };
  return result;
}

function contentDifference(original, repaired) {
  if (!available(original) || !available(repaired)) return { status: 'unavailable' };
  const before = original.data, after = repaired.data;
  if (!sameRef(after.based_on, before.reference) || !equal(before.owner, after.owner)) return { status: 'unrelated_revision' };
  const files = new Set([...Object.keys(before.content.content_files), ...Object.keys(after.content.content_files)]);
  return { status: 'available', original: before.reference, repaired: after.reference,
    changed_fields: [...new Set([...Object.keys(before.content), ...Object.keys(after.content)])]
      .filter(k => !equal(before.content[k], after.content[k])),
    files: [...files].map(file => ({ file, status: before.content.content_files[file] === after.content.content_files[file]
      ? 'unchanged' : !(file in before.content.content_files) ? 'added' : !(file in after.content.content_files) ? 'removed' : 'changed' })) };
}

export function buildExplanationCase(manifest, directory, collectedAt = new Date().toISOString()) {
  if (manifest.schema_version !== 'explanation-case-input.v1') throw new Error('Expected explanation-case-input.v1');
  if (!['actual_agent_revision', 'actual_reader_use', 'engineering_fixture'].includes(manifest.origin)) throw new Error('Explicit case origin required');
  const materials = Object.fromEntries(Object.entries(manifest.materials).map(([id, spec]) => [id, readCaseMaterial(spec, directory)]));
  const material = id => materials[id] ?? { status: 'missing', reason: 'not_supplied' };
  const original = material('original'), repaired = material('repaired');
  const originalDelivery = delivery(original, material('history')), repairedDelivery = delivery(repaired, material('history'));
  const feedback = feedbackEvidence(material('feedback_current'), material('feedback_previous'), original, originalDelivery, material('feedback_scene'));
  const methods = methodEvidence(original, (manifest.method_basis ?? []).map(material));
  const repairedMethods = methodEvidence(repaired, [original, ...(manifest.method_basis ?? []).map(material)]);
  const requests = material('provider_records');
  const records = available(requests) ? requests.data : [];
  const claims = list => (list ?? []).map(item => ({ ...item, evidence_status:
    item.evidence?.length && item.evidence.every(id => available(material(id))) ? 'available' : 'incomplete' }));
  const observations = claims(manifest.observations), interpretations = claims(manifest.interpretations);
  const difference = contentDifference(original, repaired);
  const gaps = [];
  if (originalDelivery.status !== 'confirmed') gaps.push('original_delivery_unconfirmed');
  if (repairedDelivery.status !== 'confirmed') gaps.push('repair_delivery_unconfirmed');
  if (difference.status !== 'available') gaps.push('revision_difference_unavailable');
  if (!observations.length || observations.some(o => o.evidence_status !== 'available')) gaps.push('observation_evidence_incomplete');
  if (!available(material('task'))) gaps.push('task_unavailable');
  if (!available(material('source'))) gaps.push('source_unavailable');
  if (feedback.status !== 'active') gaps.push(`feedback_${feedback.status}`);
  if (feedback.scene.status !== 'available') gaps.push(`feedback_scene_${feedback.scene.status}`);
  if (methods.status === 'unrecorded') gaps.push('method_use_unrecorded');
  if (methods.declarations.some(d => d.evidence.status !== 'observed')) gaps.push('method_basis_incomplete');
  return { schema_version: 'explanation-case.v1', case_id: manifest.case_id, origin: manifest.origin,
    collected_at: collectedAt, status: 'pending_diagnosis', obstacle: manifest.obstacle,
    materials, original_delivery: originalDelivery, repaired_delivery: repairedDelivery,
    methods, repaired_methods: repairedMethods, feedback, difference, observations, interpretations,
    hypothesis: manifest.hypothesis ?? null, replay: manifest.replay ?? [], gaps,
    attribution: { level: 'page', method_credits: [], candidate_basis: 'requires_diagnosis' },
    run: { status: requests.status, tool_results: toolMessages(records),
      trace: reportToolTrace({ requests: records }), usage: measuredUsage(records) } };
}

// Deliberate projection: private prompts, comments, scopes, sources, identifiers,
// file names and machine paths never enter the shareable experiment index.
export function publicCaseIndex(result) {
  const roles = ['original', 'repaired', 'history', 'task', 'source', 'feedback_current', 'feedback_previous', 'feedback_scene', 'provider_records'];
  return { schema_version: result.schema_version, collected_at: result.collected_at, origin: result.origin, status: result.status,
    material_status: Object.fromEntries(roles.map(key => [key, result.materials[key]?.status ?? 'missing'])),
    other_materials: Object.entries(result.materials).filter(([key]) => !roles.includes(key)).map(([, value]) => value.status),
    original_delivery: result.original_delivery.status, repaired_delivery: result.repaired_delivery.status,
    feedback: { status: result.feedback.status, transition: result.feedback.transition, scene: result.feedback.scene.status, attribution: 'page' },
    methods: { status: result.methods.status, declarations: result.methods.declarations.length,
      observed: result.methods.declarations.filter(d => d.evidence.status === 'observed').length },
    difference: { status: result.difference.status,
      changed_files: result.difference.files?.filter(f => f.status !== 'unchanged').length ?? null },
    observation_count: result.observations.length, gaps: result.gaps,
    attribution: result.attribution, usage: result.run.usage };
}

export function collectExplanationCase(manifestPath, outputDirectory) {
  const filename = path.resolve(manifestPath);
  const manifest = JSON.parse(fs.readFileSync(filename, 'utf8').replace(/^\uFEFF/, ''));
  const result = buildExplanationCase(manifest, path.dirname(filename));
  fs.mkdirSync(outputDirectory, { recursive: true });
  // A new output directory per collection preserves old feedback observations.
  fs.writeFileSync(path.join(outputDirectory, 'case.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  fs.writeFileSync(path.join(outputDirectory, 'index.json'), JSON.stringify(publicCaseIndex(result), null, 2) + '\n', { flag: 'wx' });
  return result;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [manifest, output] = process.argv.slice(2);
  if (!manifest || !output) throw new Error('Usage: node evals/semantic/explanation-case.mjs <private manifest.json> <new private output directory>');
  const result = collectExplanationCase(manifest, output);
  console.log(JSON.stringify(publicCaseIndex(result), null, 2));
}
