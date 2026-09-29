// Opt-in human UI acceptance, using an isolated home and synthetic plan only.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { test } from 'node:test';
import { appendFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, resolve } from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { boot, loadOverlayPatches } from '@deepseek-ai/dsh-app-boot';
import { provideCmdline } from '@deepseek-ai/dsh-cmdline';
import { SessionId } from '@deepseek-ai/dsh-session';
import { createUserMessage } from '@deepseek-ai/dsh-llm';
import { ScriptedModel, textResponse } from './harness.ts';

test('actual human answers the installed root plan-review UI', { timeout: 600000 }, async () => {
  const dir = mkdtempSync(resolve('../../tmp/dh0/human-'));
  process.env.DSH_HOME = join(dir, 'home');
  const config = join(dir, 'cordis.yml');
  writeFileSync(config, '[]\n');
  const patches = [
    '@deepseek-ai/dsh-base/cordis.patch.yml', '@deepseek-ai/dsh-web-app/cordis.patch.yml',
    '@deepseek-ai/dsh-web-app/presets/standard.patch.yml',
  ].flatMap(name => loadOverlayPatches('dh0-human', fileURLToPath(import.meta.resolve(name))));
  const ctx = await boot('dh0-human', config, patches, ctx => {
    provideCmdline(ctx, { args: ['--no-open', '--port', '0'], exit: code => { throw new Error(`startup exit ${code}`); } });
  });
  try {
    await ctx.workspaceRegistry.create(dir, 'DH0 合成确认测试');
    ctx.llm.registerAdapter(['probe'], new ScriptedModel(async function* () {
      yield* textResponse('DH0 人机确认验收。请阅读下面的合成计划，并按说明选择。该测试不会启动书籍构建。');
    }));
    const parent = (await ctx.agents.create({ sessionId: SessionId(randomUUID()), meta: { cwd: dir },
      agentOptions: { provider: 'probe', model: 'scripted' },
      setup: agentCtx => ctx.agentPresets.mount(agentCtx, 'standard').then(() => {}),
    })).agent;
    parent.session.append('turn/start', { turn: 1 });
    parent.session.append('user/message', createUserMessage({ content: [{ type: 'text', text: 'DH0 合成确认测试（零构建任务）' }], source: { kind: 'user' } }), { surfaceOp: 'append' });
    parent.session.append('turn/end', { turn: 1, reason: { kind: 'completed' } });
    const url = ctx.connection.authenticatedUrl(`http://127.0.0.1:${ctx.webServer.port}`);
    writeFileSync(resolve('../../tmp/dh0/human-confirmation-ready.json'), JSON.stringify({ url, session_id: parent.id }));
    const signal = AbortSignal.timeout(540000);
    const observations = [];
    const actions = process.env.DH0_HUMAN_ACTION === 'request_changes' ? ['要求修改'] : ['同意执行', '要求修改'];
    for (const action of actions) {
      const question = { id: `dh0-human-${action}`, question: `DH0 测试：请点击「${action}」`,
        detail: `# DH0 合成计划\n\n目标：一次隔离问答界面验收\n\nPass2：关闭\n\n预算：0 个构建任务，0 次语义生成\n\n阶段：仅显示与记录本题的实际回答\n\n不会创建构建 invocation 或执行书籍任务。\n\n完整计划尾标记：DH0-PLAN-END`,
        options: [{ label: '批准' }, { label: '拒绝' }], intent: { kind: 'plan-review', approve: '批准' } };
      let outcome;
      while (!outcome) {
        signal.throwIfAborted();
        try { outcome = { response: await ctx.userQuestions.ask({ agent: parent, signal, questions: [question] }) }; }
        catch (error) {
          if (error.code === 'ASK_CANCELLED') outcome = { error_code: error.code };
          else if (error.code === 'NO_PROVIDER') await setTimeout(500, undefined, { signal });
          else throw error;
        }
      }
      const observation = { action_requested: action, ...outcome };
      observations.push(observation);
      appendFileSync(resolve('../../tmp/dh0/human-confirmation-observations.jsonl'), JSON.stringify(observation) + '\n');
      if (action === '同意执行') assert.deepEqual(outcome.response?.answers, [{ id: question.id, selected: ['批准'] }]);
      else assert.equal(outcome.error_code, 'ASK_CANCELLED');
    }
    const evidence = { status: 'passed', runtime: '0.1.7-rc.2',
      ...(process.env.UNDERSTAND_BOOK_DSH_SIDEBAR_PATCH ? { ui_sidebar_patch: process.env.UNDERSTAND_BOOK_DSH_SIDEBAR_PATCH } : {}),
      root_question_service: true, observations, tasks_started: 0 };
    writeFileSync(resolve('../../tmp/dh0/human-confirmation-result.json'), JSON.stringify(evidence, null, 2));
    console.log(JSON.stringify(evidence));
  } finally { await ctx.fiber.dispose(); }
});
