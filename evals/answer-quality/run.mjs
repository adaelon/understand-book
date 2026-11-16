import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { startServer } from '../semantic/server.mjs';
import { startProviderRecorder, measuredUsage } from '../semantic/provider-recorder.mjs';
import { runCodex } from '../tool-usability/codex-client.mjs';
import { version, candidatePrompt, observedMaterials, outcomeText, completionState, comparisonJobs, reportMarkdown } from './core.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2), command = args[0] ?? 'run';
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (file, data) => fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n');

if (command === 'prepare-review') {
  const runFile = path.resolve(option('--run', ''));
  const run = read(runFile);
  const out = path.resolve(option('--out', path.join(path.dirname(runFile), 'review')));
  fs.mkdirSync(out); // A new directory preserves the original run and previous reviews.
  const materials = Object.fromEntries(run.cases.map(task => {
    const file = path.join(path.dirname(runFile), task.id, 'product/materials.json');
    return [task.id, fs.existsSync(file) ? read(file) : []];
  }));
  const jobs = comparisonJobs(run, materials);
  write(path.join(out, 'jobs.private.json'), jobs);
  // Mapping and model identity stay outside the blinded inputs.
  for (const job of jobs) write(path.join(out, `${job.id}.json`), job.input);
  console.log(JSON.stringify({ status: 'prepared_for_review', jobs: jobs.length, out }));
} else if (command === 'run') {
  const casesFile = path.resolve(option('--cases', path.join(root, 'evals/music/answer-quality-cases.json')));
  const dataset = read(casesFile);
  const arms = option('--arms', 'product,same_model,astra').split(',');
  if (arms.some(arm => !['product', 'same_model', 'astra'].includes(arm))) throw new Error('Unknown arm.');
  const selected = option('--only', '').split(',').filter(Boolean);
  const cases = dataset.cases.filter(task => selected.length ? selected.includes(task.id) : task.split !== 'holdout');
  if (!cases.length || selected.some(id => !cases.some(task => task.id === id))) throw new Error('Select existing, nonempty cases.');
  const bookOption = option('--book', null);
  const needsReader = arms.includes('product') && cases.some(task => !task.saved_product);
  if (!bookOption && needsReader) throw new Error('Specify --book, or provide saved_product for every case.');
  const book = bookOption ? path.resolve(bookOption) : null;
  const publication = book ? read(path.join(book, 'publication.json')).reference : null;
  if (publication && dataset.book_id && publication.book_id !== dataset.book_id) throw new Error('Case dataset and publication differ.');
  if (publication && dataset.publication_id && publication.publication_id !== dataset.publication_id) throw new Error('Choose the frozen case publication.');
  const out = path.resolve(option('--out', path.join(root, '.understand-book/answer-quality', new Date().toISOString().replace(/[:.]/g, '-'))));
  if (fs.existsSync(out)) throw new Error('Use a new output directory.');
  if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
  const configuredModel = process.env.FLUID_LLM_MODEL;
  const provider = { base: process.env.OPENCODE_BASE_URL, key: process.env.OPENCODE_API_KEY,
    model: option('--product-model', configuredModel) };
  if ((needsReader || arms.includes('same_model')) && (!provider.base || !provider.key || !provider.model)) throw new Error('Use the existing configured product provider.');
  const codex = option('--codex', process.env.TO14_CODEX);
  const model = option('--baseline-model', 'gpt-6-astra');
  const effort = option('--effort', 'medium');
  const baselineRunFile = option('--baseline-run', null);
  const baselineRun = baselineRunFile ? read(path.resolve(baselineRunFile)) : null;
  if (arms.includes('astra') && !codex) throw new Error('Specify the installed Codex executable using --codex.');
  if (!arms.includes('product') && cases.some(task => task.baseline_mode === 'same_material')) throw new Error('Same-material cases require a product run to collect actual materials.');
  const timeout = Number(option('--timeout-ms', '300000'));
  const tokenLimit = Number(option('--product-token-limit', '800000'));
  const directMaxTokens = Number(option('--direct-max-tokens', '32768'));
  const withBrowser = args.includes('--browser');
  const webSource = path.resolve(option('--web-dist', path.join(root, 'packages/web/dist')));
  const serverPath = path.resolve(option('--server', path.join(root, 'target/debug', process.platform === 'win32' ? 'server.exe' : 'server')));
  fs.mkdirSync(out, { recursive: true });
  const serverCopy = path.join(out, process.platform === 'win32' ? 'reader.exe' : 'reader');
  const webCopy = path.join(out, 'reader-web');
  if (arms.includes('product') && needsReader) {
    fs.copyFileSync(serverPath, serverCopy);
    process.env.SEMANTIC_EVAL_SERVER = serverCopy;
    if (withBrowser) fs.cpSync(webSource, webCopy, { recursive: true });
  }
  const git = argv => execFileSync('git', ['-c', 'core.safecrlf=false', ...argv], { cwd: root, encoding: 'utf8', windowsHide: true, maxBuffer: 16 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'] });
  const run = { version, dataset_version: dataset.version, status: 'running', started_at: new Date().toISOString(),
    cases, publication, cases_file: casesFile, samples: [],
    configuration: { configured_model: configuredModel, product_model: provider.model, product_provider_origin: provider.base ? new URL(provider.base).origin : null,
      baseline_model: model, baseline_effort: effort, baseline_transport: 'codex_limited_tools',
      reader_interface: withBrowser ? 'real_reader_browser' : 'api_only', web_dist: withBrowser ? webCopy : null,
      product_executable: arms.includes('product') && needsReader ? serverCopy : null,
      executable_source: serverPath, timeout_ms: timeout, product_token_limit: tokenLimit,
      direct_max_tokens: directMaxTokens, repeats: 1, initial_context: 'new chat, no score selection or cursor',
      git_head: git(['rev-parse', 'HEAD']).trim(), working_changes: git(['diff', '--name-only']).trim().split('\n').filter(Boolean) } };
  fs.writeFileSync(path.join(out, 'source-state.patch'), git(['diff', '--', 'crates', 'skills']));
  for (const task of cases) for (const arm of arms) run.samples.push({ case_id: task.id, arm,
    condition: arm === 'product' ? 'product_materials_and_tools' : task.baseline_mode,
    status: 'pending', answer: '', usage: null, elapsed_ms: null });
  const persist = () => {
    write(path.join(out, 'run.json'), run);
    fs.writeFileSync(path.join(out, 'report.md'), reportMarkdown(run));
  };
  persist();

  async function product(task, directory, row) {
    if (task.saved_product) {
      const file = path.resolve(path.dirname(casesFile), task.saved_product);
      const saved = read(file);
      if (typeof saved.answer !== 'string' || !Array.isArray(saved.materials)) throw new Error('Saved product requires answer and actual materials array.');
      row.answer = saved.answer;
      row.status = saved.status ?? completionState({ answer: saved.answer });
      row.original_run = file;
      row.original_usage = saved.usage ?? null;
      row.original_elapsed_ms = saved.elapsed_ms ?? null;
      write(path.join(directory, 'saved-product.json'), saved);
      write(path.join(directory, 'materials.json'), saved.materials);
      return;
    }
    const recorder = await startProviderRecorder(provider.base, { tokenLimit });
    let server;
    try {
      server = await startServer(book, path.join(directory, 'memory'), root,
        { OPENCODE_BASE_URL: recorder.url, FLUID_LLM_MODEL: provider.model, UNDERSTAND_BOOK_PRIVATE_DIR: path.join(directory, 'private'),
          ...(withBrowser ? { UNDERSTAND_BOOK_WEB_DIST: webCopy } : {}) });
      const request = { ...(task.reader_request ?? {}), message: task.context ? candidatePrompt(task) : task.question, response_language: 'zh-CN' };
      write(path.join(directory, 'request.json'), request);
      const outcome = withBrowser
        ? await (await import('./browser.mjs')).browserTurn({ server, request, directory, timeout })
        : await server.api('agent/chat', request, 'POST', timeout);
      write(path.join(directory, 'outcome.json'), outcome);
      row.answer = outcomeText(outcome);
      row.status = completionState({ answer: row.answer, incomplete: outcome.incomplete });
      row.warning = outcome.warning ?? null;
      const history = await server.api('agent/history');
      row.turn_id = history.current?.turns?.at(-1)?.turn_id ?? null;
      const sources = [];
      for (const source of outcome.answer_view?.sources ?? []) {
        try { sources.push(await server.api('agent/source.resolve', { turn_id: row.turn_id, source_ref_id: source.source_ref_id }, 'POST')); }
        catch (error) { sources.push({ source_ref_id: source.source_ref_id, error: error.message }); }
      }
      write(path.join(directory, 'sources.json'), sources);
      row.sources = sources.map(source => ({ source_ref_id: source.source_ref_id, stale: source.stale,
        can_open_in_reader: source.can_open_in_reader, error: source.error }));
    } finally {
      try { if (server) await server.stop(); }
      finally {
        await recorder.stop(5000);
        write(path.join(directory, 'provider.json'), recorder.records);
        write(path.join(directory, 'model-discovery.json'), recorder.metadataRequests);
        write(path.join(directory, 'materials.json'), observedMaterials(recorder.records));
        row.usage = measuredUsage(recorder.records);
        row.budget_rejections = recorder.budgetRejections;
      }
    }
  }

  async function direct(prompt, directory, row) {
    const request = { model: provider.model, messages: [{ role: 'user', content: prompt }], temperature: 0, max_tokens: directMaxTokens };
    write(path.join(directory, 'request.json'), request);
    const response = await fetch(provider.base.replace(/\/$/, '') + '/chat/completions', {
      method: 'POST', headers: { authorization: `Bearer ${provider.key}`, 'content-type': 'application/json' },
      body: JSON.stringify(request), signal: AbortSignal.timeout(timeout) });
    const data = await response.json();
    const choice = data.choices?.[0];
    write(path.join(directory, 'response.json'), { status: response.status, model: data.model,
      finish_reason: choice?.finish_reason, content: choice?.message?.content, usage: data.usage,
      error: data.error ? { type: data.error.type, code: data.error.code } : null });
    row.usage = data.usage ?? null;
    row.finish_reason = choice?.finish_reason ?? null;
    if (!response.ok) throw new Error(`Provider HTTP ${response.status}: ${data.error?.code ?? 'request failed'}`);
    row.answer = choice?.message?.content ?? '';
    row.status = completionState({ answer: row.answer, finishReason: choice?.finish_reason });
  }

  for (const task of cases) {
    let materials = [];
    // Material-dependent comparisons run after collection; no product answer is sent to a baseline.
    const orderedArms = task.baseline_mode === 'same_material'
      ? ['product', ...arms.filter(arm => arm !== 'product')].filter(arm => arms.includes(arm)) : arms;
    for (const arm of orderedArms) {
      const row = run.samples.find(row => row.case_id === task.id && row.arm === arm);
      const directory = path.join(out, task.id, arm);
      fs.mkdirSync(directory, { recursive: true });
      const prior = arm === 'astra' && baselineRun?.samples.find(sample => sample.case_id === task.id && sample.arm === arm && sample.status === 'delivered');
      if (prior && baselineRun.configuration.baseline_model === model && baselineRun.configuration.baseline_effort === effort) {
        const priorPrompt = path.join(path.dirname(path.resolve(baselineRunFile)), task.id, arm, 'prompt.txt');
        const prompt = candidatePrompt(task, task.baseline_mode === 'same_material' ? materials : []);
        if (fs.existsSync(priorPrompt) && fs.readFileSync(priorPrompt, 'utf8') === prompt) {
          Object.assign(row, prior, { reused_from: path.resolve(baselineRunFile) });
          fs.writeFileSync(path.join(directory, 'answer.md'), row.answer);
          write(path.join(directory, 'sample.json'), row);
          persist();
          console.log(JSON.stringify({ case_id: task.id, arm, status: 'reused_exact_input' }));
          continue;
        }
      }
      row.status = 'running'; row.started_at = new Date().toISOString(); persist();
      console.log(JSON.stringify({ case_id: task.id, arm, status: 'running' }));
      const started = performance.now();
      try {
        if (arm === 'product') {
          await product(task, directory, row);
        } else {
          if (task.baseline_mode === 'same_material' && !materials.length && !task.context) {
            row.status = 'unavailable'; row.error = 'No product-read materials were collected; no empty-context substitute was run.';
          } else {
            const evidence = task.baseline_mode === 'same_material' ? materials : [];
            const prompt = candidatePrompt(task, evidence);
            if (arm === 'same_model') await direct(prompt, directory, row);
            else {
              const result = await runCodex({ directory, prompt, model, effort, executable: codex, timeoutMs: timeout });
              row.answer = result.final ?? '';
              row.status = completionState({ answer: row.answer, incomplete: !result.completed });
              row.usage = result.usage ? { ...result.usage, total_tokens: result.total_tokens } : null;
              row.exit_code = result.exit_code; row.timed_out = result.timed_out;
              if (!result.completed) row.error = result.timed_out ? 'Codex timed out' : `Codex exit ${result.exit_code}`;
            }
          }
        }
      } catch (error) { row.status = 'error'; row.error = error.message; }
      if (arm === 'product' && fs.existsSync(path.join(directory, 'materials.json'))) materials = read(path.join(directory, 'materials.json'));
      row.elapsed_ms = performance.now() - started;
      row.finished_at = new Date().toISOString();
      fs.writeFileSync(path.join(directory, 'answer.md'), row.answer);
      write(path.join(directory, 'sample.json'), row);
      persist();
      console.log(JSON.stringify({ case_id: task.id, arm, status: row.status, elapsed_ms: row.elapsed_ms,
        usage: row.usage, error: row.error }));
    }
  }
  run.status = run.samples.every(row => row.status === 'delivered') ? 'completed' : 'completed_with_failures';
  run.finished_at = new Date().toISOString(); persist();
  console.log(JSON.stringify({ status: run.status, out, samples: run.samples.length }));
} else {
  throw new Error('Use run or prepare-review.');
}
