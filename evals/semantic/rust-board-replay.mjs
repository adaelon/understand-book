// Real-provider replay of the recorded Rust conversation in isolated local state.
// No responses are scripted. This measures delivery and inspects generated content,
// not whether the original learner would now understand it.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { startServer } from './server.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (key, fallback) => args.includes(key) ? args[args.indexOf(key) + 1] : fallback;
const out = path.resolve(option('--out', 'tmp/rust-board-improvement-20261028/replay-01'));
const historical = path.resolve(option('--history', 'tmp/visitor-rust-review-20261028/session-events.private.json'));
const binary = path.resolve(option('--server', 'target/debug/server.exe'));
const webDist = path.resolve(option('--web-dist', 'packages/web/dist'));
const book = path.resolve(option('--book', '.understand-book/mastering-rust'));
const turnTimeout = Number(option('--turn-timeout-ms', '600000'));
const stopAfter = Number(option('--stop-after', '3'));
const resume = args.includes('--resume');
assert(fs.existsSync(binary), 'Build the intended server before replay.');
assert(fs.existsSync(path.join(webDist, 'board-preview/renderer.js')), 'Build the intended web renderer.');
assert(!fs.existsSync(out) || resume, 'Use a fresh output directory, or --resume its completed turns.');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
assert(process.env.OPENCODE_API_KEY && process.env.OPENCODE_BASE_URL && process.env.FLUID_LLM_MODEL,
  'The existing provider configuration is required.');
process.env.SEMANTIC_EVAL_SERVER = binary;
fs.mkdirSync(out, { recursive: true });
const save = (file, value) => fs.writeFileSync(path.join(out, file), JSON.stringify(value, null, 2) + '\n');
const recorded = JSON.parse(fs.readFileSync(historical, 'utf8'));
const originalTurns = recorded.events.filter(event => event.kind === 'turn.accepted').map(event => event.payload.turn);
assert.equal(originalTurns.length, 3);
const report = resume ? JSON.parse(fs.readFileSync(path.join(out, 'run.json'), 'utf8')) : {
  protocol: 'rust-board-live-replay.v1', started_at: new Date().toISOString(), status: 'running',
  model: process.env.FLUID_LLM_MODEL, binary, web_dist: webDist, book,
  historical_session: recorded.session_id,
  source_revision: originalTurns[0].domain.interaction.source_bindings[0].source_revision,
  interpretation: 'Same three recorded requests and same source; new model-generated history. The third turn targets the generated mutability explanation. This is not new human learning evidence.',
  turns: [], browser_errors: [],
};
const checkpoint = () => save('run.json', report);
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
let server, browser, activeTurn;

async function runTurn(index, body) {
  const old = report.turns.find(turn => turn.index === index && turn.status === 'completed');
  if (old) return old;
  const row = { index, request: body, started_at: new Date().toISOString(), status: 'running' };
  report.turns.push(row); checkpoint();
  const started = performance.now();
  row.descriptor = await server.api('agent/runs', { ...body, client_request_id: randomUUID(),
    session_id: report.session_id, response_language: 'zh-CN' }, 'POST');
  activeTurn = row.descriptor.turn_id;
  checkpoint();
  let lastProgress = '';
  const deadline = Date.now() + turnTimeout;
  while (Date.now() < deadline) {
    const view = await server.api(`agent/runs/${activeTurn}`);
    row.snapshot = view.snapshot ?? view;
    const activities = row.snapshot.activities ?? [];
    const progress = activities.map(item => `${item.step_id}:${item.status}`).join('|');
    if (progress !== lastProgress) {
      lastProgress = progress;
      const last = activities.at(-1);
      console.log(JSON.stringify({ turn: index, elapsed_s: Math.round((performance.now() - started) / 1000),
        activity: last?.name, status: last?.status, model_calls: activities.filter(item => item.kind === 'model').length }));
      checkpoint();
    }
    const history = await server.api('agent/history');
    const current = history.current;
    const turn = current?.turns?.find(item => item.turn_id === activeTurn);
    if (turn && turn.status !== 'pending_assistant') {
      row.turn = turn; row.status = turn.status; row.elapsed_ms = performance.now() - started;
      row.completed_at = new Date().toISOString();
      const models = activities.filter(item => item.kind === 'model');
      const tokens = models.map(item => item.usage?.total_tokens ?? item.usage_total_tokens);
      row.metrics = { model_calls: models.length,
        model_purposes: Object.fromEntries([...new Set(models.map(item => item.name))].map(name => [name, models.filter(item => item.name === name).length])),
        total_tokens: tokens.every(value => value != null) ? tokens.reduce((sum, value) => sum + value, 0) : null,
        usage_reported_calls: tokens.filter(value => value != null).length,
        tool_calls: activities.filter(item => item.kind === 'tool').length,
        errors: activities.filter(item => item.status === 'failed').map(item => ({ name: item.name, code: item.error_code })) };
      save(`turn-${index}.json`, row);
      fs.writeFileSync(path.join(out, `answer-${index}.md`), turn.outcome?.answer ?? '', 'utf8');
      activeTurn = null; checkpoint();
      assert.equal(turn.status, 'completed', `Turn ${index}: ${turn.error?.error_code ?? turn.status}`);
      assert(turn.outcome?.answer?.trim() && !turn.outcome.incomplete, `Turn ${index} did not deliver a full answer.`);
      return row;
    }
    await pause(1000);
  }
  throw new Error(`Turn ${index} exceeded the replay timeout; preserve this attempt as incomplete.`);
}

async function captureBoard(index) {
  const value = await server.api('boards/read', { board_id: report.board_id }, 'POST');
  save(`board-${index}.json`, value);
  const document = value.document;
  const added = document.items.filter(item => item.target.turn_id === report.turns.find(turn => turn.index === index)?.descriptor.turn_id)
    .map(item => item.item_id);
  browser ??= await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1600, height: 1100 }, deviceScaleFactor: 1 });
  page.on('pageerror', error => report.browser_errors.push(error.message));
  await page.setContent('<!doctype html><html lang="zh-CN"><head><meta charset="UTF-8"></head><body><div id="app"></div></body></html>');
  await page.addStyleTag({ content: fs.readFileSync(path.join(webDist, 'board-preview/renderer.css'), 'utf8') });
  await page.evaluate(({ document, added }) => {
    window.__boardPreviewDocument = document; window.__boardPreviewCandidateItemIds = added;
  }, { document, added });
  await page.addScriptTag({ path: path.join(webDist, 'board-preview/renderer.js') });
  await page.waitForFunction(() => window.__boardPreviewReady === true);
  const inspection = await page.evaluate(() => window.__boardPreviewInspect());
  save(`board-${index}-inspection.json`, inspection);
  await page.screenshot({ path: path.join(out, `board-${index}-focus.png`) });
  await page.getByRole('button', { name: '显示全部内容', exact: true }).click();
  await page.screenshot({ path: path.join(out, `board-${index}-all.png`) });
  await page.close();
  return document;
}

function selectMutability(document) {
  const itemOverride = option('--selected-item');
  const blockOverride = option('--selected-block');
  if (itemOverride && !blockOverride) {
    const item = document.items.find(item => item.item_id === itemOverride);
    assert(item, 'The selected item must exist on the generated board.');
    report.selected_explanation = { item_id: item.item_id, kind: item.target.kind, title: item.title, text: item.readable_content };
    checkpoint();
    return { board_id: document.board_id, board_revision: document.revision, item_id: item.item_id };
  }
  const blocks = document.items.filter(item => item.target.kind === 'answer')
    .flatMap(item => item.explanation_blocks.map(block => ({ item, block })));
  const candidates = blocks.filter(({ item, block }) => itemOverride || blockOverride
    ? (!itemOverride || item.item_id === itemOverride) && (!blockOverride || block.block_id === blockOverride)
    : /可写|可变|读写|只读/.test((block.markdown ?? '').split('\n')[0]));
  save('follow-up-candidates.json', candidates.map(({ item, block }) => ({ item_id: item.item_id, ...block })));
  assert.equal(candidates.length, 1, 'Choose the exact generated mutability block with --selected-item/--selected-block and --resume.');
  const { item, block } = candidates[0];
  report.selected_explanation = { item_id: item.item_id, ...block }; checkpoint();
  return { board_id: document.board_id, board_revision: document.revision, item_id: item.item_id, block_id: block.block_id };
}

try {
  report.status = 'running'; delete report.error; checkpoint();
  server = await startServer(book, path.join(out, 'memory'), root, {
    UNDERSTAND_BOOK_PRIVATE_DIR: path.join(out, 'private'), UNDERSTAND_BOOK_WEB_DIST: webDist,
    UNDERSTAND_BOOK_PREVIEW_BROWSER: option('--browser', 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'),
  });
  report.reader_url = server.url;
  const history = await server.api('agent/history');
  report.session_id ??= history.current?.id ?? history.active_session_id;
  assert(report.session_id, 'No current replay session.'); checkpoint();
  const first = await runTurn(1, { message: '', expected_chat_head: null, entry_action: 'explain_selection',
    question_anchor_lid: '30.2', question_quote: originalTurns[0].question_quote,
    source_id: 'mastering-rust', source_revision: report.source_revision });
  if (stopAfter >= 2) {
    if (!report.board_id) {
      const board = await server.api('boards/create', { title: 'Rust 切片：真实对话回放' }, 'POST');
      report.board_id = board.board_id; report.initial_board_revision = board.revision; checkpoint();
    }
    const second = await runTurn(2, { message: originalTurns[1].user,
      expected_chat_head: first.descriptor.turn_id,
      board_follow_up: { board_id: report.board_id, board_revision: report.initial_board_revision } });
    const document = await captureBoard(2);
    if (stopAfter >= 3) {
      const target = selectMutability(document);
      await runTurn(3, { message: '没懂', expected_chat_head: second.descriptor.turn_id, board_follow_up: target });
      await captureBoard(3);
    }
  }
  report.status = stopAfter === 3 ? 'completed' : 'paused_after_completed_turn';
} catch (error) {
  report.status = 'failed'; report.error = error.message;
  console.error(error.message); process.exitCode = 1;
} finally {
  if (activeTurn && server) {
    try { await server.api(`agent/runs/${activeTurn}/cancel`, {}, 'POST'); } catch {}
  }
  if (browser) await browser.close();
  if (server) await server.stop();
  report.finished_at = new Date().toISOString(); checkpoint();
  console.log(JSON.stringify({ status: report.status, output: out, turns: report.turns.map(({ index, status, elapsed_ms, metrics }) => ({ index, status, elapsed_ms, metrics })) }));
}
