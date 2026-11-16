// Exercise a frozen multi-user release through its actual UI. The server and
// material are provisioned separately; this runner never mocks model or tools.
import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';
import { completionState, outcomeText } from './core.mjs';

const argv = process.argv.slice(2);
const arg = (name, fallback) => argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback;
const out = path.resolve(arg('--out', '.understand-book/answer-quality/production-parity-20261111'));
const base = arg('--url', 'https://localhost:18449');
const credentials = JSON.parse(fs.readFileSync(path.join(out, 'credentials.private.json'), 'utf8'));
const selected = arg('--only', 'M01,M06').split(',');
const cases = JSON.parse(fs.readFileSync(arg('--cases', 'evals/music/answer-quality-cases.json'), 'utf8')).cases.filter(c => selected.includes(c.id));
const timeout = Number(arg('--timeout-ms', '600000'));
const probe = argv.includes('--probe');
const recover = argv.includes('--recover');
const write = (directory, name, value) => fs.writeFileSync(path.join(directory, name), JSON.stringify(value, null, 2));
const browser = await chromium.launch({ headless: true,
  ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ? {
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH, args: ['--no-sandbox'],
  } : {}) });
try {
  for (const task of cases) {
    const directory = path.join(out, probe ? 'ui-probe' : 'online-samples', task.id, 'product');
    fs.mkdirSync(directory, { recursive: true });
    const oldSample = path.join(directory, 'sample.json');
    if (!probe && fs.existsSync(oldSample) && !recover) throw new Error(`Existing sample ${task.id}`);
    const previous = recover ? JSON.parse(fs.readFileSync(oldSample, 'utf8')) : null;
    const priorDescriptor = recover ? JSON.parse(fs.readFileSync(path.join(directory, 'descriptor.json'), 'utf8')) : null;
    const priorRequest = recover ? JSON.parse(fs.readFileSync(path.join(directory, 'request.json'), 'utf8')) : null;
    if (recover) {
      fs.copyFileSync(oldSample, path.join(directory, 'observation-error.json'));
      fs.copyFileSync(path.join(directory, 'browser.json'), path.join(directory, 'browser-interrupted.json'));
    }
    const context = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 1500, height: 1000 } });
    const page = await context.newPage();
    const owner = `quality-${task.id}`;
    const evidence = { errors: [], failed_requests: [], http_responses: [], music_responses: [], request: null, descriptor: null };
    const pending = [];
    let stamp, csrf, runPath, turn;
    const row = { case_id: task.id, arm: 'product', status: 'running', answer: '', elapsed_ms: null };
    let started;
    page.setDefaultTimeout(45000);
    await page.addLocatorHandler(page.getByRole('button', { name: '跳过引导', exact: true }), button => button.click());
    page.on('pageerror', error => evidence.errors.push(error.message));
    page.on('requestfailed', request => evidence.failed_requests.push({ url: request.url().split('?')[0], error: request.failure()?.errorText }));
    page.on('response', response => {
      const request = response.request(), pathname = new URL(response.url()).pathname;
      evidence.http_responses.push({ path: pathname, status: response.status(), length: response.headers()['content-length'], timing: request.timing().responseStart });
      if (probe) write(directory, 'network-progress.json', evidence.http_responses);
      if (pathname.endsWith('/api/auth/me') && response.ok()) pending.push(response.json().then(v => { csrf = v.csrf_token; }));
      if (request.method() === 'POST' && /\/workspaces\/[^/]+\//.test(pathname)) {
        const body = request.postDataJSON();
        if (body?.attachment_id) stamp = body;
      }
      if (request.method() === 'POST' && pathname.endsWith('/agent/runs')) {
        evidence.request = request.postDataJSON();
        runPath = pathname;
        pending.push(response.json().then(value => { evidence.descriptor = value; evidence.admission_status = response.status(); }));
      }
      if (request.method() === 'POST' && pathname.endsWith('/music/respond')) {
        pending.push(response.json().then(value => evidence.music_responses.push({ status: response.status(), request: request.postDataJSON(), response: value })));
      }
    });
    try {
      await page.goto(base + '/index.html', { waitUntil: 'domcontentloaded' });
      await page.getByLabel('邮箱或账号', { exact: true }).fill(owner);
      await page.getByLabel('密码', { exact: true }).fill(credentials[owner]);
      await page.getByRole('button', { name: '登录', exact: true }).click();
      await page.locator('.network-library').waitFor();
      if (recover) {
        await page.evaluate(({ owner, workspace_id }) => sessionStorage.setItem(`understand-book:workspace:${owner}`, JSON.stringify({ workspace_id, view: 'reader' })),
          { owner, workspace_id: priorDescriptor.workspace_id });
        await page.reload({ waitUntil: 'domcontentloaded' });
      } else await page.locator('.network-book-card').filter({ hasText: '第一乐章' }).click();
      await page.locator('.music-reader').waitFor({ timeout: 120000 });
      const tour = page.getByRole('button', { name: '跳过引导', exact: true });
      if (await tour.isVisible()) await tour.click();
      await page.waitForFunction(() => document.querySelector('.music-reader > audio')?.readyState > 0, null, { timeout: 60000 });
      if (probe) {
        fs.writeFileSync(path.join(directory, 'visible.txt'), await page.locator('body').innerText());
        write(directory, 'controls.json', await page.locator('button,textarea').evaluateAll(items => items.map(e => ({ tag: e.tagName, text: e.innerText, title: e.getAttribute('title'), class: e.className, visible: !!e.getClientRects().length }))));
        console.log(JSON.stringify({ case_id: task.id, status: 'probe_ready' }));
        continue;
      }
      // The ordinary question composer retains the Reader's default cursor;
      // no score range or hidden API context is fabricated by this runner.
      const input = page.locator('textarea[data-workspace-input="agent"]');
      if (recover) {
        evidence.request = priorRequest; evidence.descriptor = priorDescriptor;
        runPath = `/api/workspaces/${priorDescriptor.workspace_id}/agent/runs`;
        row.started_at = previous.started_at;
        row.recovered_observation = true;
        started = performance.now() - (Date.now() - Date.parse(previous.started_at));
      } else {
        if (!await input.isVisible()) await page.locator('.music-ask').click();
        await input.fill(task.question);
        const admitted = page.waitForResponse(r => r.request().method() === 'POST' && new URL(r.url()).pathname.endsWith('/agent/runs'));
        started = performance.now();
        row.started_at = new Date().toISOString();
        await page.getByRole('button', { name: '发送', exact: true }).click();
        const admission = await admitted;
        await Promise.allSettled(pending);
        write(directory, 'request.json', evidence.request);
        write(directory, 'descriptor.json', evidence.descriptor);
        if (!admission.ok()) throw new Error(`Admission HTTP ${admission.status()}: ${evidence.descriptor?.error_code}`);
      }
      console.log(JSON.stringify({ case_id: task.id, status: 'running', run_id: evidence.descriptor.run_id }));
      const workspacePath = runPath.replace(/\/agent\/runs$/, '');
      const deadline = Date.now() + timeout;
      let lastProgress = 0;
      while (Date.now() < deadline) {
        await new Promise(resolve => setTimeout(resolve, 1500));
        const history = await page.evaluate(async ({ workspacePath, stamp, csrf }) => {
          const workspace = await (await fetch(workspacePath)).json();
          const response = await fetch(workspacePath + '/chat/history', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': csrf },
            body: JSON.stringify({ attachment_id: stamp.attachment_id, generation: workspace.generation, expected_revision: workspace.revision }) });
          return { status: response.status, value: await response.json() };
        }, { workspacePath, stamp, csrf });
        if (history.status !== 200) throw new Error(`History HTTP ${history.status}: ${history.value.error_code}`);
        const value = history.value.result;
        turn = value.current?.turns?.find(t => t.turn_id === evidence.descriptor.turn_id);
        if (turn && turn.status !== 'pending_assistant') { write(directory, 'history.json', value); break; }
        if (Date.now() - lastProgress > 30000) {
          lastProgress = Date.now();
          console.log(JSON.stringify({ case_id: task.id, status: turn?.status ?? 'waiting', elapsed_s: Math.round((performance.now() - started) / 1000) }));
        }
      }
      if (!turn || turn.status === 'pending_assistant') {
        row.timed_out = true;
        const stop = page.locator('.stop-agent');
        if (await stop.isVisible()) await stop.click();
        throw new Error('Experiment observation limit reached; requested cancellation through Reader');
      }
      write(directory, 'turn.json', turn);
      write(directory, 'outcome.json', turn.outcome ?? {});
      row.turn_id = turn.turn_id;
      row.answer = outcomeText(turn.outcome);
      row.status = completionState({ answer: row.answer, incomplete: turn.outcome?.incomplete, error: turn.error });
      row.warning = turn.outcome?.warning ?? null;
      row.error = turn.error ?? null;
      const rendered = page.locator(`.turn[data-turn-id="${turn.turn_id}"] .ans-text`);
      if (await rendered.count()) {
        await rendered.scrollIntoViewIfNeeded();
        fs.writeFileSync(path.join(directory, 'browser-answer.txt'), await rendered.innerText());
      }
    } catch (error) {
      row.status = row.timed_out ? 'incomplete' : 'error'; row.error = error.message;
      evidence.failure = error.message;
      console.log(JSON.stringify({ case_id: task.id, status: row.status, error: row.error }));
    } finally {
      if (started) row.elapsed_ms = performance.now() - started;
      await page.screenshot({ path: path.join(directory, 'browser.png'), fullPage: true }).catch(error => evidence.errors.push(error.message));
      await Promise.allSettled(pending);
      write(directory, 'browser.json', evidence);
      if (!probe) {
        fs.writeFileSync(path.join(directory, 'answer.md'), row.answer);
        write(directory, 'sample.json', row);
        console.log(JSON.stringify(row, (key,value) => key === 'answer' ? `[${value.length} characters]` : value));
      }
      await context.close();
    }
  }
} finally { await browser.close(); }
