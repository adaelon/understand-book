import fs from 'node:fs';
import path from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { startProviderRecorder, measuredUsage } from '../evals/semantic/provider-recorder.mjs';
import { selectTrialProvider, positiveLimit, trialCostBudget } from './model-trial-config.mjs';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(name);
  if (index < 0) return fallback;
  const value = args[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value.`);
  return value;
};
const binary = option('--test-bin');
const outputArg = option('--out');
if (!binary || !outputArg) throw new Error('Usage: node scripts/verify-course-reuse.mjs --test-bin <compiled server test executable> --out <new evidence directory> [--book <published book directory>] [--model-settings <json file> --model-config-id <id>] [--max-requests <count> --max-tokens <count>] [--max-cost-cny <amount> --rates-file <model-rates.json>]');
if (fs.existsSync('.env')) process.loadEnvFile('.env');
const costBudget = trialCostBudget(option('--max-cost-cny'), option('--rates-file'));
const selection = selectTrialProvider(process.env, option('--model-settings'), option('--model-config-id'));
if (costBudget) {
  // Price selection uses the frozen real endpoint. A recording proxy would
  // change that identity before the Runtime's existing pricing boundary.
  selection.direct = true;
  selection.model = selection.configuredModel;
}
const { base, model, configuredModel } = selection;
const source = path.resolve(option('--book', '.understand-book/ai-infra-book-complete'));
const output = path.resolve(outputArg);
fs.accessSync(path.join(source, 'source.txt'));
fs.accessSync(binary);
fs.mkdirSync(output, { recursive: true });
const runPath = path.join(output, 'run.json');
if (fs.existsSync(runPath)) throw new Error('Choose a new evidence directory; earlier attempts remain unchanged.');
const save = (name, value) => fs.writeFileSync(path.join(output, name), JSON.stringify(value, null, 2) + '\n');
const started = new Date().toISOString();
const report = {
  started_at: started, status: 'running', configured_model: configuredModel, model,
  ...selection.metadata, transport: selection.direct ? 'direct_adapter' : 'legacy_recorder', source,
  source_head: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8', windowsHide: true }).trim(),
  test_executable: { source: path.resolve(binary), built_at: fs.statSync(binary).mtime.toISOString() },
  limits: { sample_count: 1, provider_requests: positiveLimit(option('--max-requests'), 24, '--max-requests'), recorded_tokens: positiveLimit(option('--max-tokens'), 1200000, '--max-tokens'), learner_turns: 7, step_seconds: 180,
    ...(costBudget ? { max_cost_micro_cny: costBudget.max_cost_micro_cny, rates_file: costBudget.rates_file } : {}) },
  scope: 'Real model course practice and return to the original course; agent-operated learner, not human first-use evidence.',
};
save('run.json', report);
const scratch = path.resolve('.tmp/course-reuse');
fs.mkdirSync(scratch, { recursive: true });
const privateDir = fs.mkdtempSync(path.join(scratch, 'run-'));
const executable = path.join(privateDir, path.basename(binary));
fs.copyFileSync(binary, executable);
let rejected = 0;
const recorder = selection.direct ? null : await startProviderRecorder(base, {
  tokenLimit: report.limits.recorded_tokens,
  intercept: (_request, records) => {
    if (records.length > report.limits.provider_requests) {
      rejected++;
      throw new Error('COURSE_TRIAL_REQUEST_LIMIT');
    }
    return null;
  },
});
const log = fs.createWriteStream(path.join(output, 'trial.log'));
try {
  const child = spawn(executable, ['closed_book_course_live_model_trial', '--ignored', '--nocapture', '--test-threads=1'], {
    windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, ...selection.environment, ...costBudget?.environment, TEMP: privateDir, TMP: privateDir,
      ...(recorder ? { OPENCODE_BASE_URL: recorder.url, FLUID_LLM_MODEL: model } : {}),
      MODEL_TRIAL_MAX_REQUESTS: String(report.limits.provider_requests), MODEL_TRIAL_MAX_TOKENS: String(report.limits.recorded_tokens),
      CLOSED_BOOK_TRIAL_SOURCE: source, CLOSED_BOOK_COURSE_TRIAL_OUTPUT: output },
  });
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => {
    log.write(bytes); process.stdout.write(bytes);
  });
  const code = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', resolve);
  });
  report.exit_code = code;
  report.status = code === 0 ? 'execution_passed_semantic_review_pending' : 'failed';
  process.exitCode = code === 0 ? 0 : 1;
} catch (error) {
  report.status = 'failed'; report.error = error.message;
  process.exitCode = 1;
} finally {
  await recorder?.stop();
  await new Promise(resolve => log.end(resolve));
  report.finished_at = new Date().toISOString();
  if (recorder) {
    save('provider-requests.json', recorder.records);
    save('provider-models.json', recorder.metadataRequests);
    report.provider_paths = [...new Set(recorder.records.map(record => record.path))];
    report.usage = measuredUsage(recorder.records);
    report.request_limit_rejections = rejected;
    report.token_limit_rejections = recorder.budgetRejections;
    if (rejected || recorder.budgetRejections.length) report.status = 'stopped_at_trial_budget';
  } else {
    const sendsPath = path.join(output, 'provider-sends.jsonl');
    const sends = fs.existsSync(sendsPath) ? fs.readFileSync(sendsPath, 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line)) : [];
    const completed = sends.filter(item => item.phase === 'after_send');
    report.actual_sends = sends.filter(item => item.phase === 'before_send').length;
    report.provider_usage = completed.map(item => item.outcome.usage);
    report.usage_complete = completed.length === report.actual_sends && completed.every(item => item.outcome.usage?.total_tokens != null);
    const rejectedSends = sends.filter(item => item.phase === 'rejected');
    report.request_limit_rejections = rejectedSends.filter(item => item.reason === 'ALLOWANCE_INSUFFICIENT').length;
    report.send_rejections = rejectedSends.map(item => ({ call_id: item.identity.call_id, reason: item.reason }));
    if (costBudget) {
      const confirmed = completed.filter(item => item.cost?.status === 'confirmed');
      const settled = new Set(confirmed.map(item => item.identity.call_id));
      const complete = completed.length === report.actual_sends && completed.every(item => item.cost?.status === 'confirmed');
      const known = confirmed.reduce((sum, item) => sum + item.cost.actual_micro_cny, 0);
      report.cost = { currency: 'CNY', max_micro_cny: costBudget.max_cost_micro_cny,
        confirmed_micro_cny: known, actual_micro_cny: complete ? known : null,
        retained_reserved_micro_cny: sends.filter(item => item.phase === 'before_send' && !settled.has(item.identity.call_id))
          .reduce((sum, item) => sum + (item.cost?.reserved_micro_cny ?? 0), 0),
        complete,
      };
    }
    if (report.request_limit_rejections || rejectedSends.some(item => item.reason === 'MODEL_CHARGE_RECONCILIATION_REQUIRED')
      || completed.some(item => item.cost?.status === 'unknown')) report.status = 'stopped_at_trial_budget';
    if (rejectedSends.some(item => item.reason === 'MODEL_RATE_UNAVAILABLE')) {
      report.status = 'failed'; report.error = 'No valid matching rate could authorize the model send.';
    }
    if (codeSucceededWithoutEvidence(report, sends)) {
      report.status = 'failed'; report.error = 'The selected test executable did not emit adapter send evidence; rebuild the server test binary.';
      process.exitCode = 1;
    }
  }
  save('run.json', report);
}

function codeSucceededWithoutEvidence(report, sends) {
  return report.exit_code === 0 && !sends.some(item => item.phase === 'after_send');
}
