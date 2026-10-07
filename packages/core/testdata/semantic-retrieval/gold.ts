import { source as ratesSource, proposal as ratesProposal } from "../../test/fixtures/teaching-source";
import { acceptFormalObjects, type FormalObjectProposal, type TeachingSource } from "../../src/teaching-map";
import { newObjectAlignment } from "../../src/teaching-object-alignment";

// Authored source and labels frozen before the first provider ranking (2026-10-01).
// Additional passages make direction and distractors explicit; none are book excerpts.
export const source: TeachingSource = { ...ratesSource, passages: [...ratesSource.passages,
  { unit_lid: "3", lid: "3.1", text: "甲车牵引乙车前进。另一次实验由乙车牵引甲车前进。施力者和受力者互换，应分别描述这两种牵引关系。" },
  { unit_lid: "4", lid: "4.1", text: "温度是物体冷热程度的量度。质量表示物体惯性的大小。电流是单位时间通过截面的电荷量。电压是电势差。电阻表示导体对电流的阻碍。频率是单位时间内周期变化的次数。压强为单位面积承受的压力。密度是单位体积的质量。动能是物体因运动具有的能量。功率是单位时间做的功。加速度是速度的变化率。路程是轨迹的长度。位移是从起点指向终点的有向线段。" },
] };
const binding = (lid: string) => ({ source_id: source.source_id, source_revision: source.source_revision, lid });
const object = (key: string, meaning: string, lid: string, extra: Partial<FormalObjectProposal["objects"][number]> = {}) => ({
  key, meaning, kind: "concept" as const, aliases: [], conditions: [], candidate_refs: [], participants: [], component_keys: [], source_bindings: [binding(lid)], ...extra,
});
const distractors = [
  ["temperature", "温度是物体冷热程度的量度"], ["mass", "质量表示物体惯性的大小"],
  ["current", "电流是单位时间通过截面的电荷量"], ["voltage", "电压是电势差"],
  ["resistance", "电阻表示导体对电流的阻碍"], ["frequency", "频率是单位时间内周期变化的次数"],
  ["pressure", "压强为单位面积承受的压力"], ["density", "密度是单位体积的质量"],
  ["energy", "动能是物体因运动具有的能量"], ["power", "功率是单位时间做的功"],
  ["acceleration", "加速度是速度的变化率"], ["distance", "路程是轨迹的长度"],
  ["displacement", "位移是从起点指向终点的有向线段"],
];
const extraObjects = [
  object("measure", "逐段记录路程与计时结果，累加后求两者之比", "2.2", { kind: "method" }),
  object("car-a", "甲车", "3.1"), object("car-b", "乙车", "3.1"),
  object("a-pulls-b", "甲车牵引乙车", "3.1", { kind: "relation", participants: [{ object_key: "car-a", role: "施力者" }, { object_key: "car-b", role: "受力者" }] }),
  object("b-pulls-a", "乙车牵引甲车", "3.1", { kind: "relation", participants: [{ object_key: "car-b", role: "施力者" }, { object_key: "car-a", role: "受力者" }] }),
  ...distractors.map(([key, meaning]) => object(key, meaning, "4.1")),
];
const current: FormalObjectProposal = { ...structuredClone(ratesProposal),
  objects: [...structuredClone(ratesProposal.objects), ...extraObjects],
  coverage: [...structuredClone(ratesProposal.coverage), ...["3", "4"].map(unit => ({ unit_lid: unit,
    object_keys: extraObjects.filter(o => o.source_bindings[0].lid === `${unit}.1`).map(o => o.key),
    explanation: "独立来源定义", source_bindings: [binding(`${unit}.1`)] }))],
};
current.coverage[1].object_keys.push("measure");
const paraphrases: FormalObjectProposal = { objects: [
  object("speed-reworded", "行驶全程的距离与历时之比", "2.2"),
  object("measure-reworded", "把每一段走了多远和花了多久记下来，分别加起来再相除", "2.2", { kind: "method" }),
], prerequisites: [], correspondences: [], coverage: [{ unit_lid: "2", object_keys: ["speed-reworded", "measure-reworded"],
  explanation: "跨块复述相同定义与方法", source_bindings: [binding("2.2")] }] };
const fragment = (fragment_id: string, proposal: FormalObjectProposal) => ({ version: "formal_object_fragment.v1" as const, fragment_id,
  proposal, source_ranges: source.passages.map(p => ({ lid: p.lid, start: 0, end: p.text.length })), auxiliary_ranges: [] });

export function fixture() {
  const previousProposal: FormalObjectProposal = { objects: [object("old-speed", "累计行程和耗时的比值", "2.2")],
    prerequisites: [], correspondences: [], coverage: ["1", "2", "3", "4"].map(unit => ({ unit_lid: unit,
      object_keys: unit === "2" ? ["old-speed"] : [], explanation: "旧版只整理速率，其余来源明确为空",
      source_bindings: [binding(source.passages.find(p => p.unit_lid === unit)!.lid)] })) };
  const previous = acceptFormalObjects({ source, proposal: previousProposal, operation_id: "sr0-previous", allocate_id: () => "old-speed" });
  return { source, previous, work: newObjectAlignment([fragment("base", structuredClone(current)), fragment("later", structuredClone(paraphrases))]) };
}

export const policy = { version: "sr0-gold.v1", frozen_on: "2026-10-01", page_size: 6, ks: [6, 12], primary: "recall@6",
  continuation: "substring misses a compare pair; real provider retrieves it at 6; report B/C as exploratory only",
  publication: "C recall@6 > B; exact/alias retained; separated-pair false merge = 0; total false merge and missed reuse no worse than B",
  decision_control: "SR6 must hold model, prompt, read gate and decision budget constant",
  max_documents: 40, max_queries: 10, batch_size: 8, max_sequence_tokens: 128,
};
export const cases = [
  { id: "low-lexical-speed", category: "低词面同义", focus: "later/speed-reworded", query: "行驶全程的距离与历时之比", compare: ["base/speed"], separate: ["base/velocity"], same: ["base/speed"], lids: ["1.1", "2.2"], reason: "两者均为总路程/总时间，位移平均速度不同" },
  { id: "low-lexical-method", category: "低词面同义", focus: "later/measure-reworded", query: "把每一段走了多远和花了多久记下来，分别加起来再相除", compare: ["base/measure"], separate: ["base/speed"], same: ["base/measure"], lids: ["2.2"], reason: "测量步骤相同，步骤方法与被测量概念分别追踪" },
  { id: "homonym", category: "同词异义", focus: "base/speed", query: "速度", compare: ["base/velocity"], separate: ["base/velocity"], same: [], lids: ["1.1"], reason: "口语同名但路程与位移定义不同，需比较并分离" },
  { id: "related-not-identical", category: "高相关非同一", focus: "base/speed", query: "测量平均速率的方法", compare: ["base/measure"], separate: ["base/measure"], same: [], lids: ["1.1", "2.2"], reason: "定义与测量程序相关但不是同一内容单位" },
  { id: "relation-conditions", category: "关系条件", focus: "base/equal-distance", query: "等距离时平均速率为调和平均", compare: ["base/equal-time"], separate: ["base/equal-time"], same: [], lids: ["1.2", "2.1"], reason: "等距离和等时间适用条件不同" },
  { id: "relation-direction", category: "关系角色方向", focus: "base/a-pulls-b", query: "甲车牵引乙车", compare: ["base/b-pulls-a"], separate: ["base/b-pulls-a"], same: [], lids: ["3.1"], reason: "参与者相同，施力者与受力者互换" },
  { id: "whole-parts", category: "整体与部件", focus: "base/trip", query: "往返运动", compare: ["base/out", "base/back"], separate: ["base/out", "base/back"], same: [], lids: ["1.2"], reason: "往返包含去程与回程，整体和部件分别追踪" },
  { id: "previous-paraphrase", category: "previous identity 改写", focus: "later/speed-reworded", query: "行驶全程的距离与历时之比", compare: ["previous/old-speed"], separate: ["base/velocity"], same: ["previous/old-speed"], lids: ["1.1", "2.2"], reason: "旧对象累计行程/耗时与新复述定义相同，应显式沿用身份" },
];
