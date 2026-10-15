// Run the existing three-question product runner in ABBA blocks. This wrapper
// adds only scheduling and citation-phase metrics; it never calls a provider.
import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (key, fallback) => { const index = args.indexOf(key); return index < 0 ? fallback : args[index + 1]; };
const output = path.join(root, 'docs/performance/source-citation-group-20261015/results');
const phases = ['baseline-r1', 'candidate-r1', 'candidate-r2', 'baseline-r2'];
const questionIds = ['cross-01', 'cross-02', 'exact-02'];
const usageKeys = ['input_tokens', 'output_tokens', 'reasoning_output_tokens', 'cached_input_tokens', 'cache_creation_input_tokens', 'total_tokens'];
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
const sumKnown = (rows, get) => rows.length && rows.every(row => Number.isFinite(get(row)))
  ? rows.reduce((sum, row) => sum + get(row), 0) : null;
const sumUsage = models => Object.fromEntries(usageKeys.map(key => [key, sumKnown(models, model => model.usage?.[key])]));

function derive(directory) {
  const run = read(path.join(directory, 'run.json'));
  return { variant: run.variant, status: run.status, started_at: run.started_at, finished_at: run.finished_at,
    binary: run.binary, model: run.model, cases: run.cases.map(row => {
      const outcomePath = path.join(directory, `${row.id}.outcome.json`);
      const historyPath = path.join(directory, `${row.id}.history.json`);
      if (!fs.existsSync(outcomePath) || !fs.existsSync(historyPath)) {
        return { id: row.id, status: row.status, error: row.error ?? 'Saved outcome/history unavailable', statistics: row.statistics ?? null };
      }
      const outcome = read(outcomePath);
      const history = read(historyPath);
      const activities = history.current.turns.at(-1).run_summary?.activities ?? [];
      const models = activities.filter(activity => activity.kind === 'model');
      const outerModels = models.filter(activity => activity.name === 'outer');
      const sourceActivities = activities.filter(activity => activity.kind === 'tool' && activity.name === 'source.present');
      const trace = outcome.trace ?? [];
      const sourceTrace = trace.filter(call => call.tool === 'source.present');
      const activityMappingComplete = sourceActivities.length === sourceTrace.length;
      const groups = new Map();
      for (const [index, call] of sourceTrace.entries()) {
        let input;
        try { input = JSON.parse(call.args); } catch { input = null; }
        const activity = activityMappingComplete ? sourceActivities[index] : null;
        if (!groups.has(call.model_tool_loop)) groups.set(call.model_tool_loop, []);
        groups.get(call.model_tool_loop).push({ source_call: index + 1, quote: input?.quote ?? null,
          arguments: call.args, status: activity?.status ?? null, error_code: activity?.error_code ?? null,
          step_id: activity?.step_id ?? null, result_digest: call.result_digest });
      }
      const sourceRounds = [...groups.keys()].sort((a, b) => a - b);
      const firstRound = sourceRounds[0] ?? null;
      const firstModel = firstRound === null ? null : outerModels[firstRound - 1];
      const citationModels = firstModel ? models.filter(model => model.step_id >= firstModel.step_id) : [];
      const precedingModels = firstModel ? models.filter(model => model.step_id < firstModel.step_id) : [];
      const toolRounds = [...new Set(trace.map(call => call.model_tool_loop))].sort((a, b) => a - b);
      return { id: row.id, status: row.status, message: row.message, answer: row.answer,
        elapsed_ms: row.elapsed_ms, statistics: row.statistics,
        model_activities_by_purpose: Object.fromEntries([...new Set(models.map(model => model.name))]
          .map(name => [name, models.filter(model => model.name === name).length])),
        outer_tool_rounds: toolRounds.map(loop => ({ model_tool_loop: loop,
          tools: trace.filter(call => call.model_tool_loop === loop).map(call => call.tool) })),
        outer_rounds_without_tools: Array.from({ length: outerModels.length }, (_, index) => index + 1)
          .filter(loop => !toolRounds.includes(loop)),
        non_outer_model_activities: models.filter(model => model.name !== 'outer')
          .map(({ step_id, name, status, usage }) => ({ step_id, name, status, usage })),
        before_citation: {
          model_activity_count: firstModel ? precedingModels.length : null,
          usage: sumUsage(precedingModels),
          model_duration_ms: sumKnown(precedingModels, model => model.duration_ms),
          activities: precedingModels.map(({ step_id, name, status, usage }) => ({ step_id, name, status, usage })),
        },
        citation: {
          source_call_count: sourceTrace.length,
          source_submission_round_count: sourceRounds.length,
          source_submission_rounds: sourceRounds,
          first_source_round: firstRound,
          citation_phase_model_activity_count: firstModel ? citationModels.length : null,
          citation_phase_usage: sumUsage(citationModels),
          citation_phase_model_duration_ms: sumKnown(citationModels, model => model.duration_ms),
          calls_after_first_source_round: sourceTrace.filter(call => call.model_tool_loop > firstRound).length,
          rejected_source_count: activityMappingComplete ? sourceActivities.filter(activity => activity.status === 'rejected').length : null,
          source_activity_mapping_complete: activityMappingComplete,
          non_source_tools_after_first_source_round: firstRound === null ? [] : trace
            .filter(call => call.model_tool_loop > firstRound && call.tool !== 'source.present')
            .map(({ model_tool_loop, tool, args }) => ({ model_tool_loop, tool, args })),
          groups: sourceRounds.map(loop => ({ model_tool_loop: loop, calls: groups.get(loop) })),
        } };
    }) };
}

// Pure reconstruction: inspect previous artifacts without starting a Reader.
if (args.includes('--derive')) {
  const directory = option('--records');
  if (!directory) throw new Error('Use --derive --records DIRECTORY');
  console.log(JSON.stringify(derive(path.resolve(directory)), null, 2));
  process.exit(0);
}

// Rebuild a comparison exclusively from the twelve saved question outcomes.
if (args.includes('--summarize')) {
  const protocol = read(path.join(output, 'protocol.json'));
  if (!protocol.phases.every(phase => phase.status === 'completed')) throw new Error('All four ABBA phases must finish before summarizing');
  const runs = phases.map(phase => {
    const directory = path.join(output, phase);
    const metrics = derive(directory);
    write(path.join(directory, 'citation-metrics.json'), metrics);
    return { phase, ...metrics };
  });
  const manualPath = path.join(output, 'manual-review.json');
  const manual = fs.existsSync(manualPath) ? read(manualPath) : null;
  const samples = runs.flatMap(run => run.cases.map(row => ({ phase: run.phase, variant: run.variant,
    id: row.id, category: row.id.startsWith('cross') ? 'complex' : 'single_passage',
    status: row.status, message: row.message, usage: row.statistics.usage,
    citation_delivery_ok: row.statistics.source_count > 0 && row.statistics.valid_source_count === row.statistics.source_count,
    manual_review: manual?.phases.find(phase => phase.phase === run.phase)?.cases.find(item => item.id === row.id) ?? null,
    eligible_for_equal_quality_comparison: manual?.phases.find(phase => phase.phase === run.phase)?.cases
      .find(item => item.id === row.id)?.eligible_for_equal_quality_comparison ?? null,
    before_citation: row.before_citation, citation: row.citation,
    metrics: {
      ...row.statistics.usage,
      first_input_tokens: row.statistics.model_calls[0]?.usage?.input_tokens ?? null,
      first_cached_input_tokens: row.statistics.model_calls[0]?.usage?.cached_input_tokens ?? null,
      model_activities: row.statistics.model_activity_count, outcome_turns: row.statistics.outcome_turns,
      outer_model_activities: row.model_activities_by_purpose.outer ?? 0,
      before_citation_models: row.before_citation.model_activity_count,
      before_citation_total_tokens: row.before_citation.usage.total_tokens,
      before_citation_input_tokens: row.before_citation.usage.input_tokens,
      before_citation_output_tokens: row.before_citation.usage.output_tokens,
      before_citation_cached_input_tokens: row.before_citation.usage.cached_input_tokens,
      citation_phase_models: row.citation.citation_phase_model_activity_count,
      citation_phase_total_tokens: row.citation.citation_phase_usage.total_tokens,
      citation_phase_input_tokens: row.citation.citation_phase_usage.input_tokens,
      citation_phase_output_tokens: row.citation.citation_phase_usage.output_tokens,
      citation_phase_cached_input_tokens: row.citation.citation_phase_usage.cached_input_tokens,
      source_submission_rounds: row.citation.source_submission_round_count,
      source_calls: row.citation.source_call_count, rejected_sources: row.citation.rejected_source_count,
      source_count: row.statistics.source_count, valid_source_count: row.statistics.valid_source_count,
      elapsed_ms: row.elapsed_ms,
    },
    model_activities_by_purpose: row.model_activities_by_purpose,
    outer_tool_rounds: row.outer_tool_rounds, outer_rounds_without_tools: row.outer_rounds_without_tools,
    non_outer_model_activities: row.non_outer_model_activities,
  })));
  const summarizeRows = rows => ({ samples: rows.length,
    measurements: Object.fromEntries(Object.keys(rows[0]?.metrics ?? {}).map(key => {
      const values = rows.map(row => row.metrics[key]);
      const complete = values.length > 0 && values.every(Number.isFinite);
      return [key, { values, observed: values.filter(Number.isFinite).length,
        mean: complete ? values.reduce((sum, value) => sum + value, 0) / values.length : null,
        min: complete ? Math.min(...values) : null, max: complete ? Math.max(...values) : null }];
    })),
    usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.usage[key])])),
    before_citation_usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.before_citation.usage[key])])),
    citation_phase_usage_totals: Object.fromEntries(usageKeys.map(key => [key, sumKnown(rows, row => row.citation.citation_phase_usage[key])])),
    count_totals: Object.fromEntries(['model_activities', 'outer_model_activities', 'before_citation_models',
      'citation_phase_models', 'source_submission_rounds', 'source_calls', 'rejected_sources', 'source_count', 'valid_source_count']
      .map(key => [key, sumKnown(rows, row => row.metrics[key])])),
    citation_delivery_successes: rows.filter(row => row.citation_delivery_ok).length,
    citation_delivery_failures: rows.filter(row => !row.citation_delivery_ok).map(row => ({ phase: row.phase, id: row.id })),
  });
  const comparison = { version: 'source-citation-group-comparison.v1', order: phases, product_questions: samples.length,
    rebuilt_at: new Date().toISOString(), samples,
    by_variant: ['baseline', 'candidate'].map(variant => ({ variant,
      ...summarizeRows(samples.filter(row => row.variant === variant)) })),
    by_question: questionIds.map(id => {
      const rows = samples.filter(row => row.id === id);
      const baseline = summarizeRows(rows.filter(row => row.variant === 'baseline'));
      const candidate = summarizeRows(rows.filter(row => row.variant === 'candidate'));
      const a = baseline.measurements.total_tokens.mean, b = candidate.measurements.total_tokens.mean;
      const equalQuality = rows.every(row => row.eligible_for_equal_quality_comparison === true);
      const change = Number.isFinite(a) && Number.isFinite(b) && a > 0 ? (b - a) / a * 100 : null;
      return { id, category: rows[0].category, identical_user_messages: new Set(rows.map(row => row.message)).size === 1,
        baseline, candidate, all_samples_equal_quality: equalQuality,
        observed_mean_total_token_change_percent: change,
        equal_quality_observed_change_percent: equalQuality ? change : null };
    }),
    by_category: ['complex', 'single_passage'].map(category => ({ category,
      baseline: summarizeRows(samples.filter(row => row.category === category && row.variant === 'baseline')),
      candidate: summarizeRows(samples.filter(row => row.category === category && row.variant === 'candidate')) })),
    limitations: [
      'Two samples per question and variant; no significance or population-level saving is inferred.',
      'Autonomous retrieval, nested query calls, answer scope, and cache state may differ. Total usage changes are not assigned entirely to citation guidance.',
      'Reasoning tokens are included in output; cache tokens are included in input. Missing provider fields remain null.',
      'Zero delivered citations is a citation failure for these tasks. Such samples retain actual usage but are not eligible for an equal-quality saving claim.',
    ],
  };
  write(path.join(output, 'comparison.json'), comparison);
  console.log(JSON.stringify({ product_questions: samples.length, questions: comparison.by_question.map(question => ({
    id: question.id, identical_user_messages: question.identical_user_messages,
    baseline_tokens: question.baseline.measurements.total_tokens,
    candidate_tokens: question.candidate.measurements.total_tokens,
    baseline_source_rounds: question.baseline.measurements.source_submission_rounds.values,
    candidate_source_rounds: question.candidate.measurements.source_submission_rounds.values,
  })) }, null, 2));
  process.exit(0);
}

const phase = option('--phase');
const phaseIndex = phases.indexOf(phase);
if (phaseIndex < 0) throw new Error(`Choose --phase ${phases.join('|')}`);
const variant = phase.startsWith('baseline') ? 'baseline' : 'candidate';
const binary = path.resolve(option('--binary', path.join(root, `tmp/source-citation-group-trial-20261015/${variant}-server.exe`)));
if (!fs.existsSync(binary)) throw new Error('Frozen Reader binary not found');
const phaseOutput = path.join(output, phase);
const protocolPath = path.join(output, 'protocol.json');
fs.mkdirSync(output, { recursive: true });
const protocol = fs.existsSync(protocolPath) ? read(protocolPath) : {
  version: 'source-citation-group-trial.v1', created_at: new Date().toISOString(),
  intent: 'Compare the existing batch-reading runtime with and without grouped source.present guidance.',
  order: 'ABBA blocks; each block runs cross-01, cross-02, exact-02 in that order.',
  maximum_product_questions: 12, grading_provider_calls: 0, retries: 0,
  isolation: 'Each product question starts a new local Reader with isolated memory/private directories; external telemetry is off.',
  inputs: 'The old runner supplies identical questions and explicit LID positions. No source body or gold labels are put in user messages.',
  measurement: {
    usage: 'Final provider-reported activity fields; missing values stay null. Reasoning is included in output, cached input in input.',
    source_submission_round_count: 'Distinct outcome.trace model_tool_loop values containing source.present.',
    citation_phase: 'All model activities from the first outer source.present submission through final delivery; later non-source tools are separately listed.',
    validation: 'Resolve every final citation to canonical source text, then manually check core facts without a paid judge.',
    limitations: 'Two samples per case/variant; provider cache and autonomous extra reading can differ. No guaranteed saving is assumed.',
  },
  phases: phases.map((name, index) => ({ name, order: index + 1, status: 'pending', question_ids: questionIds })),
};
if (protocol.phases[phaseIndex].status !== 'pending') throw new Error('This phase was already started; do not retry or overwrite it');
if (phaseIndex && protocol.phases[phaseIndex - 1].status !== 'completed') throw new Error('The previous ABBA phase must finish first');
if (fs.existsSync(path.join(phaseOutput, 'run.json'))) throw new Error('Phase output already exists');
Object.assign(protocol.phases[phaseIndex], { status: 'running', started_at: new Date().toISOString(), binary });
write(protocolPath, protocol);

let exitCode = null;
try {
  exitCode = await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [path.join(root, 'evals/semantic/book-text-batch-trial.mjs'),
      '--variant', variant, '--binary', binary, '--out', phaseOutput],
    { cwd: root, windowsHide: true, stdio: 'inherit', env: process.env });
    child.once('error', reject);
    child.once('exit', code => resolve(code));
  });
  if (fs.existsSync(path.join(phaseOutput, 'run.json'))) {
    const metrics = derive(phaseOutput);
    write(path.join(phaseOutput, 'citation-metrics.json'), metrics);
    protocol.phases[phaseIndex].status = exitCode === 0 && metrics.status === 'completed' ? 'completed' : 'failed';
    protocol.phases[phaseIndex].question_count = metrics.cases.length;
    console.log(JSON.stringify({ phase, status: protocol.phases[phaseIndex].status,
      cases: metrics.cases.map(row => ({ id: row.id, total_tokens: row.statistics?.usage.total_tokens,
        model_activities: row.statistics?.model_activity_count, source_rounds: row.citation?.source_submission_round_count,
        source_calls: row.citation?.source_call_count, rejected_sources: row.citation?.rejected_source_count })) }));
  } else protocol.phases[phaseIndex].status = 'failed';
} catch (error) {
  protocol.phases[phaseIndex].status = 'failed';
  protocol.phases[phaseIndex].error = error.message;
  process.exitCode = 1;
} finally {
  protocol.phases[phaseIndex].finished_at = new Date().toISOString();
  protocol.phases[phaseIndex].runner_exit_code = exitCode;
  write(protocolPath, protocol);
  if (exitCode !== 0) process.exitCode = 1;
}
