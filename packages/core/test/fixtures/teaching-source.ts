import type { TeachingSource, FormalObjectProposal } from "../../src/teaching-map";
import type { BookStructureKeyStop } from "../../src/book-structure";

// Complete small authored source. Paragraphs are deliberately separate: the necessary
// definition in 1.1 has no candidate graph edge to the argument in 2.1.
export const source: TeachingSource = { source_id: "rates", source_revision: "source-v1", passages: [
  { unit_lid: "1", lid: "1.1", text: "平均速率（亦称平均速度大小）是路程除以所用时间。这里的速度大小不带方向；平均速度则是位移除以时间，带方向。口语中二者常简称速度，应以定义区分。" },
  { unit_lid: "1", lid: "1.2", text: "往返运动包含去程和回程。等距离的去程和回程，平均速率为两段速率的调和平均；等时间的两段运动，平均速率为算术平均。" },
  { unit_lid: "2", lid: "2.1", text: "等距离往返时，每段路程为 d，速率分别为 u 和 v，总时间 d/u+d/v，故平均速率为 2uv/(u+v)。假定 d、u、v 都为正数。" },
  { unit_lid: "2", lid: "2.2", text: "测量方法：先记录各段路程和时间，再分别求和，最后用总路程除以总时间。计时期间不得漏记停留。" },
  { unit_lid: "2", lid: "2.3", text: "某次观测的平均速率增加，作者据此断言每段运动都变快，但没有给出各段的测量记录。" },
] };
export const binding = (lid: string) => ({ source_id: source.source_id, source_revision: source.source_revision, lid });
export const targets: BookStructureKeyStop[] = [
  { id: "harmonic", lid: "2.1", type: "formula", reason: { text: "等距离平均速率", evidence_lids: ["2.1"] } },
  { id: "measurement", lid: "2.2", type: "example", reason: { text: "测量方法", evidence_lids: ["2.2"] } },
  { id: "gap", lid: "2.3", type: "claim", reason: { text: "区分平均与各段", evidence_lids: ["2.3"] } },
];
const object = (key: string, meaning: string, lid: string, extra = {}) => ({ key, meaning, kind: "concept" as const,
  aliases: [], source_bindings: [binding(lid)], conditions: [], candidate_refs: [], participants: [], component_keys: [], ...extra });
export const proposal: FormalObjectProposal = { objects: [
  object("speed", "总路程除以总时间的平均速率", "1.1", { aliases: ["平均速率", "平均速度大小", "速度"], candidate_refs: ["concept:平均速率", "concept:平均速度大小"] }),
  object("velocity", "位移除以时间的平均速度", "1.1", { aliases: ["平均速度", "速度"] }),
  object("out", "去程", "1.2"), object("back", "回程", "1.2"),
  object("trip", "往返运动", "1.2", { kind: "composite", component_keys: ["out", "back"] }),
  object("equal-distance", "等距离时平均速率为调和平均", "2.1", { kind: "relation", conditions: ["等距离", "速率为正"],
    participants: [{ object_key: "out", role: "去程速率" }, { object_key: "back", role: "回程速率" }] }),
  object("equal-time", "等时间时平均速率为算术平均", "1.2", { kind: "relation", conditions: ["等时间"],
    participants: [{ object_key: "out", role: "去程速率" }, { object_key: "back", role: "回程速率" }] }),
], prerequisites: [{ target_key: "equal-distance", target_capability: "independent_derivation", prerequisite_key: "speed", prerequisite_capability: "explanation",
  conditions: ["从定义推导"], source_bindings: [binding("1.1"), binding("2.1")] }], correspondences: [], coverage: [
  { unit_lid: "1", object_keys: ["speed", "velocity", "trip", "out", "back", "equal-time"], explanation: "定义与条件分别整理", source_bindings: [binding("1.1"), binding("1.2")] },
  { unit_lid: "2", object_keys: ["equal-distance"], explanation: "公式作为正式关系；测量与缺口由重点素材保留", source_bindings: [binding("2.1"), binding("2.2"), binding("2.3")] },
] };
