// Opt-in, live Reader authoring exercise. Uses a fresh local book and isolated private state per case.
// node evals/semantic/presentation-scenes.mjs <fresh output directory> [norm|torch|proof ...]
import fs from 'node:fs';
import path from 'node:path';
import {
  fileURLToPath
} from 'node:url';
import {
  startServer
} from './server.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url)),
  [output, ...ids] = process.argv.slice(2);
if (!output) throw Error('Supply a fresh output directory; this uses the configured model.');
const out = path.resolve(output);
fs.mkdirSync(out, {
  recursive: false
});
const cases = {
  norm: {
    message: '甲是 (3,4)，乙是 (0,5)。为什么换一把尺，谁更长就会变？让我自己试一下。',
    source: '甲、乙从同一原点出发，甲的两个分量是 3 和 4，乙是 0 和 5。二维实向量的 L1 范数为两分量绝对值之和，L2 为两分量平方和的平方根，L∞ 为最大绝对分量。甲、乙的三组读数依次是 7 与 5、5 与 5、4 与 5。L1 可以用只能横纵移动的路程解释，L2 可以用两点间的直线距离解释。L∞ 可以把两轴同时各走不超过一个单位算成一步。'
  },
  torch: {
    message: '一支手电筒里面各部件怎么配合？我想看清它们装在什么位置，拆开后还能认出电流和光各往哪里走。',
    source: '本例是单节电池、尾部开关的简化手电筒。从尾端向前依次为尾盖与开关、压住电池负极的弹簧、电池、带 LED 的驱动板、反光杯与前端透明保护片。电池正极朝灯头，连接驱动板；尾部开关闭合后，电池、驱动板上的 LED 和经导电筒身、开关与弹簧回到负极的通路闭合。LED 位于反光杯底孔处；前行光与被杯壁反射的部分光线从前方保护片射出。保护片保护灯头；它不是聚焦透镜。分解时用于辨认部件，并不表示断开的电路仍能点亮 LED。'
  },
  proof: {
    message: '请从条件开始，逐步证明欧氏范数的三角不等式，并说清楚什么时候取等号。',
    source: '对实内积空间中的向量 x、y，欧氏范数定义为 ‖x‖₂=√〈x,x〉。展开有 ‖x+y‖₂²=‖x‖₂²+2〈x,y〉+‖y‖₂²。柯西–施瓦茨不等式给出 〈x,y〉≤‖x‖₂‖y‖₂，所以右边至多为 (‖x‖₂+‖y‖₂)²。两边非负，取平方根得三角不等式。任一向量为零时取等；非零时当且仅当两个向量同方向，即一个是另一个的正数倍。反方向的非零向量并不满足此处的等号条件。'
  }
};
async function run(id) {
  const item = cases[id];
  if (!item) throw Error('Unknown case ' + id);
  const base = path.join(out, id),
    book = path.join(base, 'book');
  fs.mkdirSync(book, {
    recursive: true
  });
  const text = item.source;
  fs.writeFileSync(path.join(book, 'source.txt'), text);
  fs.writeFileSync(path.join(book, 'base.json'), JSON.stringify({
    book_id: 'scene-' + id,
    lid_nodes: [{
      lid: '1',
      path: [1],
      kind: 'chapter',
      span: {
        start: 0,
        end: text.length
      },
      children: ['1.1']
    }, {
      lid: '1.1',
      path: [1, 1],
      kind: 'paragraph',
      span: {
        start: 0,
        end: text.length
      },
      children: []
    }],
    graph_nodes: [],
    graph_edges: []
  }));
  const memory = path.join(base, 'memory'),
    priv = path.join(base, 'private');
  fs.mkdirSync(memory);
  fs.mkdirSync(priv);
  const save = (file, value) => fs.writeFileSync(path.join(base, file), JSON.stringify(value, null, 2) + '\n');
  save('input.json', {
    message: item.message,
    presentation_requested: true,
    source: text
  });
  let server;
  const started = Date.now();
  try {
    server = await startServer(book, memory, root, {
      UNDERSTAND_BOOK_PRIVATE_DIR: priv,
      UB_OBSERVABILITY_MODE: 'off'
    });
    console.log(id, 'running', server.url);
    const accepted = await server.api('agent/runs', {
      message: item.message,
      presentation_requested: true,
      response_language: 'zh-CN'
    }, 'POST');
    save('admission.json', accepted);
    if (!accepted.turn_id) throw Error('Reader did not admit a run');
    let seq = -1,
      lastProgress = Date.now(),
      run;
    while (true) {
      run = await server.api('agent/runs/' + encodeURIComponent(accepted.turn_id));
      if (run.last_seq !== seq) {
        seq = run.last_seq;
        lastProgress = Date.now();
        save('progress.json', {
          state: run.execution_state,
          seq,
          activities: run.activities.map(a => ({
            name: a.name,
            status: a.status,
            error: a.error_code
          }))
        });
      }
      if (['completed', 'failed', 'cancelled', 'ended', 'interrupted'].includes(run.execution_state) && run
        .persistence_state !== 'pending') break;
      if (Date.now() - lastProgress > 15 * 60 * 1000) {
        await server.api('agent/runs/' + encodeURIComponent(accepted.turn_id) + '/cancel', {}, 'POST');
        throw Error('No Reader progress for 15 minutes; cancelled owned run')
      }
      await new Promise(r => setTimeout(r, 2500));
    }
    const hist = await server.api('agent/history');
    save('history.json', hist);
    const turn = run.final_view ?? hist.current?.turns?.at(-1),
      result = turn?.outcome;
    save('outcome.json', result ?? {
      warning: run.error
    });
    const data = result?.answer_view?.parts ?? [];
    const deliveries = data.filter(x => x.kind === 'presentation');
    const versions = turn?.presentations ?? [];
    const summary = {
      state: run.execution_state,
      incomplete: result?.incomplete,
      turns: result?.turns,
      parts: data.map(x => x.kind),
      references: deliveries.length ? deliveries : versions,
      warning: result?.warning ?? run.error?.error_code,
      elapsed_s: Math.round((Date.now() - started) / 1000)
    };
    save('summary.json', summary);
    console.log(id, JSON.stringify(summary));
    if (!result || result.incomplete || (!deliveries.length && !versions.length)) {
      process.exitCode = 1;
    }
  } catch (e) {
    save('failure.json', {
      message: e.message
    });
    console.log(id, 'failed', e.message);
    process.exitCode = 1
  } finally {
    if (server) await server.stop()
  }
}
for (const id of ids.length ? ids : Object.keys(cases)) await run(id);
