import { allSpans } from './core.mjs';

export const readingDatasetVersion = 'quantification-essence-reading-v1';
export const readingTaskSpecVersionV2 = 'quantification-essence-reading-task-spec-v2';
const slot = (key, description, accept, ...evidence) => ({ key, description, accept, evidence });
const item = (id, family_id, split, reading_level, question, slots, explanation = null) => ({
  id, category: 'reading', family_id, split, reading_level, interaction_condition: 'single_turn', question, slots, explanation,
});

export const readingTasks = [
  item('holdout-l1', 'holdout-integrity', 'development', 'L1', '定位防过拟合总纲：用来评判因子的数据与搜因子的数据应是什么关系？', [
    slot('relation', '应彻底隔离', ['isolated'], ['让用来评判因子的数据，和用来搜因子的数据彻底隔离']),
  ]),
  item('holdout-l2', 'holdout-integrity', 'development', 'L2', '解释组合净化交叉验证中 purging 与 embargo 各删除哪一类训练样本，以及分别防什么泄漏。', [
    slot('purging', '删除标签区间与测试集重叠的训练样本', ['overlapping_label_intervals'], ['把训练集里那些标签区间和测试集重叠的样本删掉']),
    slot('embargo', '测试段之后禁用一段训练样本以阻断序列自相关', ['post_test_serial_correlation'], ['在测试段之后再禁用一小段训练样本，掐断序列自相关造成的渗漏']),
  ], '分别说明标签时间重叠与测试后自相关是两种不同的泄漏路径'),
  item('holdout-l3', 'holdout-integrity', 'development', 'L3', '为什么反复用同一段留出集筛因子不能被 purging/embargo 单独修复？联系两处原文区分搜索污染与时间泄漏。', [
    slot('relation', '反复筛选使留出集参与搜索；purging/embargo 只处理标签重叠和序列自相关，需独立未参与搜索的数据', ['search_contamination_distinct_from_temporal_leakage'],
      ['反复拿同一段样本外去筛因子、再回头调，这段样本外也就被你搜过了'],
      ['purging，把训练集里那些标签区间和测试集重叠的样本删掉'],
      ['embargo，在测试段之后再禁用一小段训练样本']),
  ], '建立搜索污染与时间泄漏的区别，并指出只做净化交叉验证为何不足以恢复已被反复使用的留出集'),
  item('holdout-l4', 'holdout-integrity', 'development', 'L4', '团队在同一留出集上反复筛因子并调参；部分训练样本的三重屏障标签跨入测试期，测试段后紧邻训练样本。诊断至少两类不同失效，并给出对应验证修正。', [
    slot('diagnosis', '已用留出集参与搜索，同时存在标签重叠和测试后自相关泄漏', ['search_and_temporal_leakage'],
      ['反复拿同一段样本外去筛因子、再回头调，这段样本外也就被你搜过了'],
      ['标签区间和测试集重叠的样本删掉'],
      ['测试段之后再禁用一小段训练样本']),
    slot('repair', '另取未参与搜索的验证数据，并对时间切分执行 purging 与 embargo', ['fresh_holdout_plus_purge_embargo'],
      ['让用来评判因子的数据，和用来搜因子的数据彻底隔离'],
      ['purging，把训练集里那些标签区间和测试集重叠的样本删掉'],
      ['embargo，在测试段之后再禁用一小段训练样本']),
  ], '把三种具体失效映射到相应修正，不能把 purging/embargo 说成能恢复已被搜索污染的留出集'),
  item('target-l1', 'target-loss-alignment', 'holdout', 'L1', '定位原文：从预测具体价格改为预测方向后，目标标签熵 H(Y) 约为多少 bit？', [
    slot('entropy', '方向标签约 1 bit', [1], ['H(Y)\\approx 1\\ \\text{bit}']),
  ]),
  item('target-l2', 'target-loss-alignment', 'holdout', 'L2', '解释为什么把价格目标改成方向，虽然损失一部分互信息，可恢复信息占目标熵的比例仍提高。', [
    slot('ratio', '目标熵降幅大于互信息损失，可恢复比例上升', ['denominator_falls_more'],
      ['分母直接塌掉了将近八倍'], ['粗粒化的代价。但关键不在绝对值，在比例']),
  ], '说明分母 H(Y) 大幅下降与互信息小幅损失共同决定比值，而非宣称方向标签创造信息'),
  item('target-l3', 'target-loss-alignment', 'holdout', 'L3', '联系三重屏障打标与指增的 IC/rank loss：二者怎样体现“目标对齐真正要的结构”，又分别改了什么？', [
    slot('relationship', '三重屏障将固定终点改为止盈止损首达；指增损失将数值拟合改为相对排序；共同减少对无意义细节的索取', ['first_passage_and_ranking_alignment'],
      ['三重屏障把持有期变成了内生的首达时间'],
      ['让损失函数直接对齐你赚钱的那个量——也就是 IC / 排序'],
      ['把目标从”复述数值”换成”只问你真正要的那个相对结构”']),
  ], '同时建立共同原则和机制差异；分别罗列两节事实不足以满足关系要求'),
  item('target-l4', 'target-loss-alignment', 'holdout', 'L4', '团队 A 做止盈止损择时，却用固定第十天精确价格打标；团队 B 做股票截面指增，只按相对排序交易，却用 MSE 拟合次日精确收益。分别诊断目标错位，并给出与书中原则一致的替代。', [
    slot('timing', 'A 应考虑按首达结果的三重屏障标签，减少固定终点噪声', ['triple_barrier_first_passage'],
      ['固定期收益里塞满了恰好那一刻价格在哪的终点噪声'],
      ['三重屏障把持有期变成了内生的首达时间']),
    slot('ranking', 'B 应用 IC 或 rank/listwise loss 对齐相对排序', ['ic_or_rank_loss'],
      ['MSE 会逼模型把容量浪费在精确拟合那些在交易上毫无意义'],
      ['正确的做法是让损失函数直接对齐你赚钱的那个量——也就是 IC / 排序']),
  ], '分别将 A 的标签错位和 B 的损失错位映射到不同修正，不把三重屏障直接当作排序损失'),
];

const multi = (id, family_id, split, reading_level, steps, slots, explanation) => ({ id, category: 'reading',
  family_id, split, reading_level, interaction_condition: 'multi_turn', steps, slots, explanation });
export const readingMultiTurn = [
  multi('holdout-multi', 'holdout-integrity', 'development', 'L4', [
    '我们在做时序交叉验证。purging 和 embargo 分别是做什么的？',
    '补充并纠正我的前提：同一段留出集已经被反复拿来筛因子、调参五轮。请修正刚才的诊断，说明哪些问题能由 purging/embargo 处理，哪些不能。',
  ], [slot('correction', '承接新信息，指出留出集搜索污染与时间泄漏不同，需独立验证数据', ['fresh_holdout_not_repaired_by_purge'],
    ['反复拿同一段样本外去筛因子、再回头调，这段样本外也就被你搜过了'],
    ['purging，把训练集里那些标签区间和测试集重叠的样本删掉'])],
  '终答承接第二轮纠正，不要求重复第一轮全部定义；不能声称 purging/embargo 会清除重复筛选污染'),
  multi('target-multi', 'target-loss-alignment', 'holdout', 'L4', [
    '若只想准确预测股票次日收益的小数值，MSE 对应的优化目标是什么？',
    '我修正目标：实际做的是股票截面指增，只按相对排序买卖，并不需要精确收益小数。请据此调整损失建议，联系打标的目标选择解释原因。',
  ], [slot('correction', '改为 IC 或排序损失并说明目标从数值复述转向相对结构', ['ranking_after_correction'],
    ['正确的做法是让损失函数直接对齐你赚钱的那个量——也就是 IC / 排序'],
    ['把目标从”复述数值”换成”只问你真正要的那个相对结构”'])],
  '终答针对修正后的交易目标调整建议，不把第一轮数值目标继续当作权威'),
];

export const allReadingTasks = [...readingTasks, ...readingMultiTurn];
export const readingSpecs = allReadingTasks.map(t => ({
  case_id: t.id, task_revision: readingDatasetVersion, family_id: t.family_id, split: t.split,
  category: t.category, reading_level: t.reading_level, interaction_condition: t.interaction_condition,
  user_input: t.steps ? { steps: t.steps } : { question: t.question },
  initial_state: { isolated_memory: true, isolated_chat: true, book_ref: 'quantification-essence' },
  requirements: [...t.slots.map(s => ({ requirement_id: `${t.id}:${s.key}`, kind: 'content', description: s.description,
    necessary: true, acceptable: s.accept, evidence_alternatives: s.evidence })),
  { requirement_id: `${t.id}:accuracy`, kind: 'accuracy', description: '结论及实质性额外说法准确', necessary: true },
  { requirement_id: `${t.id}:completeness`, kind: 'completeness', description: '当前用户要求完整兑现', necessary: true },
  ...(t.explanation ? [{ requirement_id: `${t.id}:explanation`, kind: 'explanation', description: t.explanation, necessary: true }] : []),
  { requirement_id: `${t.id}:sources`, kind: 'citation', description: '必要结论由实际交付的有效原文来源支持', necessary: true }],
  verification: { slots: t.slots.map(s => ({ key: s.key, acceptable: s.accept, evidence_alternatives: s.evidence })) },
}));

export function readingSpec(id) {
  const spec = readingSpecs.find(s => s.case_id === id);
  if (!spec) throw new Error(`Unknown reading task: ${id}`);
  return spec;
}
export function readingProductInput(spec, step = 0) {
  if (spec.interaction_condition === 'single_turn') {
    if (step !== 0) throw new Error('Single-turn task has one step');
    return { message: spec.user_input.question };
  }
  if (!Number.isInteger(step) || step < 0 || step >= spec.user_input.steps.length) throw new Error('Unknown user step');
  return { message: spec.user_input.steps[step] };
}
export function validateReadingDataset(corpus) {
  const ids = new Set();
  for (const t of allReadingTasks) {
    if (ids.has(t.id)) throw new Error(`Duplicate reading task: ${t.id}`);
    ids.add(t.id);
    for (const slot of t.slots) for (const group of slot.evidence)
      if (!group.length || !group.some(text => allSpans(corpus.source, text).length))
        throw new Error(`Missing reading evidence: ${t.id}/${slot.key}: ${JSON.stringify(group)}`);
  }
  for (const family of new Set(allReadingTasks.map(t => t.family_id)))
    if (new Set(allReadingTasks.filter(t => t.family_id === family).map(t => t.split)).size !== 1)
      throw new Error(`Family crosses splits: ${family}`);
  for (const level of ['L1', 'L2', 'L3', 'L4'])
    if (readingTasks.filter(t => t.reading_level === level).length !== 2) throw new Error(`Missing level pair: ${level}`);
  return { tasks: allReadingTasks.length, single_turn: readingTasks.length, multi_turn: readingMultiTurn.length,
    families: [...new Set(allReadingTasks.map(t => t.family_id))], splits: [...new Set(allReadingTasks.map(t => t.split))] };
}
