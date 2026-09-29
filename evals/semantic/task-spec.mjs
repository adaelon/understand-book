import { qaTasks, sourceTasks, restartTasks, version as sourceVersion } from './agent-dataset.mjs';

export const taskSpecVersion = 'quantification-essence-task-spec-v1';
export const taskSpecVersionV2 = 'quantification-essence-task-spec-v2';
export const readingLevels = ['L1', 'L2', 'L3', 'L4'];
export const interactionConditions = ['single_turn', 'multi_turn', 'cross_session'];

// Frozen classifications describe the required operation, not the number of chapters or tools.
const levels = {
  'exact-01':'L1', 'exact-02':'L1', 'exact-03':'L1', 'exact-04':'L1',
  'concept-01':'L1', 'concept-02':'L2', 'concept-03':'L2', 'concept-04':'L2',
  'cross-01':'L1', 'cross-02':'L3', 'cross-03':'L1', 'cross-04':'L2',
  'contrast-01':'L1', 'contrast-02':'L4', 'contrast-03':'L2', 'contrast-04':'L2',
  'formula-01':'L2', 'formula-02':'L2', 'formula-03':'L2', 'formula-04':'L2',
  'source-01':'L1', 'source-02':'L1', 'source-03':'L1', 'source-04':'L1',
  'refusal-01':'L1', 'refusal-02':'L1', 'refusal-03':'L1', 'refusal-04':'L1',
  'restart-01':'L1', 'restart-02':'L1', 'restart-03':'L1', 'restart-04':'L1',
};
const explanations = {
  'concept-02':'说明非价格数据成为因子的条件',
  'concept-03':'说明 DSR 为何扣除搜索带来的运气',
  'concept-04':'解释 embargo 的做法及防止序列泄漏的目的',
  'cross-02':'联系打标的熵目标与指增损失的排序目标',
  'contrast-02':'说明反复用同一留出集筛选为何污染验证',
  'contrast-03':'说明 MWU 软权重与 FTL 全押的区别',
  'formula-01':'用平方根关系解释四倍宽度对应两倍 IR',
  'formula-02':'用极值近似式解释 N 增大时门槛的方向',
  'formula-04':'用指数更新式解释较大损失对应较小权重',
};
const families = {
  'concept-01':'trend-arb', 'contrast-01':'trend-arb', 'restart-01':'trend-arb',
  'exact-02':'holdout-isolation', 'contrast-02':'holdout-isolation', 'restart-02':'holdout-isolation',
  'concept-04':'serial-leakage', 'source-01':'serial-leakage',
  'contrast-03':'mwu-ftl', 'formula-04':'mwu-ftl', 'source-02':'mwu-ftl', 'restart-03':'mwu-ftl',
  'exact-03':'database-history', 'source-03':'database-history', 'restart-04':'database-history',
};

export const taskSpecs = [...qaTasks, ...sourceTasks, ...restartTasks].map(task => {
  const restart = task.category === 'restart', refusal = task.category === 'refusal';
  const requirements = restart ? [
    { requirement_id: `${task.id}:setup`, kind: 'action', description: '在正确原文位置保存指定内容或导航到指定位置', necessary: true },
    { requirement_id: `${task.id}:persistence`, kind: 'persistence', description: '服务重启后指定记录或位置仍可读取', necessary: true },
    { requirement_id: `${task.id}:recovery`, kind: 'content', description: '新聊天从真实持久状态恢复并复述指定信息', necessary: true },
  ] : refusal ? [
    { requirement_id: `${task.id}:refusal`, kind: 'content', description: '明确承认本书没有足够证据且不编造具体答案', necessary: true },
    { requirement_id: `${task.id}:accuracy`, kind: 'accuracy', description: '拒绝及附带说法没有实质性错误', necessary: true },
  ] : [
    ...task.slots.map(slot => ({ requirement_id: `${task.id}:${slot.key}`, kind: 'content', description: slot.description, necessary: true,
      acceptable: slot.accept, evidence_alternatives: slot.evidence })),
    { requirement_id: `${task.id}:accuracy`, kind: 'accuracy', description: '结论及实质性额外说法准确', necessary: true },
    { requirement_id: `${task.id}:completeness`, kind: 'completeness', description: '用户明确要求完整兑现', necessary: true },
    ...(explanations[task.id] ? [{ requirement_id: `${task.id}:explanation`, kind: 'explanation', description: explanations[task.id], necessary: true }] : []),
    { requirement_id: `${task.id}:sources`, kind: 'citation', description: '必要结论由实际交付的有效原文来源支持', necessary: true },
    ...(task.category === 'source' ? [{ requirement_id: `${task.id}:navigation`, kind: 'action', description: '实际跳转且最终视口覆盖目标原文', necessary: true }] : []),
  ];
  return {
    case_id: task.id, task_revision: taskSpecVersion, family_id: families[task.id] ?? task.id, split: 'development',
    legacy_dataset_version: sourceVersion, category: task.category, reading_level: levels[task.id],
    interaction_condition: restart ? 'cross_session' : 'single_turn',
    recovery_mode: restart ? 'new_chat_after_restart' : null,
    user_input: restart ? { setup: task.setup, resume: task.resume } : { question: task.question },
    initial_state: { isolated_memory: true, isolated_chat: true, book_ref: 'quantification-essence' },
    requirements, verification: restart ? { anchor: task.anchor, content: task.content ?? task.anchor, kind: task.kind }
      : { slots: task.slots.map(({ key, accept, evidence }) => ({ key, acceptable: accept, evidence_alternatives: evidence })) },
  };
});

export function taskSpec(id) {
  const spec = taskSpecs.find(s => s.case_id === id);
  if (!spec) throw new Error(`Unknown task spec: ${id}`);
  return spec;
}

// This is the only task-spec projection permitted in a product request. Gold and future script steps stay private.
export function productInput(spec, phase = 'question') {
  if (spec.interaction_condition === 'cross_session') {
    if (!['setup', 'resume'].includes(phase)) throw new Error('Choose setup or resume');
    return { message: spec.user_input[phase] };
  }
  if (phase !== 'question') throw new Error('Single-turn task has one question');
  return { message: spec.user_input.question };
}
