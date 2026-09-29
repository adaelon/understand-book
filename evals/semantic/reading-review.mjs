import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCorpus } from './core.mjs';
import { reference } from './quality-core.mjs';
import { allReadingTasks, readingDatasetVersion, readingSpec, validateReadingDataset } from './reading-dataset.mjs';

const examples = {
  'holdout-l1': ['评判因子的数据必须与搜索因子的数据彻底隔离。', '可以在同一份数据上搜索和验收。'],
  'holdout-l2': ['purging 删除标签区间与测试集重叠的训练样本，防止标签跨期泄漏；embargo 在测试段后禁用一小段训练样本，阻断序列自相关。', '两者都是删除测试段前的所有训练样本。'],
  'holdout-l3': ['反复筛选让留出集参与了搜索，已不再独立；purging 只处理标签区间重叠，embargo 处理测试后的序列自相关，不能清除搜索污染，应另用未参与搜索的数据。', '只做 purging 和 embargo 就能让反复使用的留出集恢复独立。'],
  'holdout-l4': ['同一留出集反复筛选使其参与搜索，需另取未参与搜索的数据；跨入测试期的标签要 purging，测试后相邻训练样本要 embargo。', '只要增加 embargo，留出集反复调参也仍是独立的。'],
  'target-l1': ['约 1 bit。', '约 7.7 bit。'],
  'target-l2': ['方向标签粗粒化会损失部分互信息，但目标熵从约 7.7 bit 降到约 1 bit，分母下降更多，可恢复信息占比因此提高；它没有创造新信息。', '方向标签增加了互信息，因此比例提高。'],
  'target-l3': ['三重屏障把固定终点改成止盈止损的首达结果，减少终点噪声；指增用 IC/rank loss 对齐相对排序而非收益小数。二者都选择交易真正关心的结构，但分别改变标签与优化损失。', '三重屏障直接替代 IC loss 计算排序，IC loss 则给择时设置止盈止损标签。'],
  'target-l4': ['团队 A 应考虑首达结果的三重屏障标签，避免固定终点噪声；团队 B 应使用 IC 或 rank/listwise loss 来优化相对排序，而非 MSE 拟合精确收益。', '团队 A 继续用固定终点价格，团队 B 用三重屏障替代排序损失。'],
  'holdout-multi': ['补充信息改变了诊断：留出集已参与反复搜索，需另取未参与搜索的验证数据。purging 与 embargo 分别处理标签重叠和测试后自相关，不能修复留出集搜索污染。', '保持第一次建议即可，purging 和 embargo 能解决所有问题。'],
  'target-multi': ['按你修正后的目标，应改用 IC 或 rank/listwise loss 对齐股票相对排序，不再用 MSE 拟合精确收益小数；这与打标从数值复述转向有用结构是同一原则。', '既然第一轮问过数值拟合，仍坚持 MSE。'],
};

export function readingReviewPacket(corpus) {
  validateReadingDataset(corpus);
  return { version: readingDatasetVersion, status: 'candidate_not_human_reviewed', reviewer: null, reviewed_at: null,
    cases: allReadingTasks.map(t => ({ case_id: t.id, family_id: t.family_id, split: t.split,
      reading_level: t.reading_level, interaction_condition: t.interaction_condition,
      user_input: readingSpec(t.id).user_input, requirements: readingSpec(t.id).requirements,
      reference_evidence: reference(t, corpus), proposed_sufficient_answer: examples[t.id][0],
      proposed_critical_wrong_answer: examples[t.id][1], human_review: null })) };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
  const arg = key => { const i = process.argv.indexOf(key); return i < 0 ? null : process.argv[i + 1]; };
  const book = path.resolve(arg('--book') ?? path.join(root, '.understand-book/quantification-essence'));
  const out = path.resolve(arg('--out') ?? path.join(root, 'evals/semantic/quality-results/reading-review-v1.json'));
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(readingReviewPacket(loadCorpus(book)), null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ out, cases: allReadingTasks.length, status: 'candidate_not_human_reviewed' }));
}
