# LearningMemory-owned replayable TutorSession

Status: Accepted, 2026-09-07. Design decision; implementation pending.

**决策**: LearningMemory 独立持有可回放教学会话。

**否决**:
- 由聊天持有：跨聊天学习会受到 transcript 生命周期影响。
- 写成长期 ProfileFact：临时目标与教法会混入长期偏好。
- 由 Reader 界面持有：布局与阅读位置不能拥有教学合同。

**命门**: TutorSession 保留用户的会话学习意图与约束，Agent 在其范围内维护当前学习焦点，材料范围单独表达，并引用路径与进度；生命周期由用户显式事件决定，当前状态按 revision 重建，聊天、Reader 与 ProfileFact 只引用它。重启或压缩保持会话，当前回合教法覆盖不改写默认，范围外请求不继承默认，模型判断不能自动切换或结束会话；最终目标、要求达到的能力深度或持续学习任务的变化由用户表达或接受。
**何时回头**: 课程生命周期出现无法由该学习会话表达的独立所有权需求。
**展开**: [grill.md Q87/Q92/Q93](../grill.md)、[领域术语](../../CONTEXT.md)。学习入口允许从可修正的暂定目标开始，完整画像与细化目标不作前置条件。2026-09-29 由 [ADR-0141](0141-global-tutor-control-and-session-ownership.md) 与 [Tutor 切片方案 §3](../切片方案-Tutor全局模式与教学闭环.md#3-全局控制与教学会话) 补全应用级控制、单一当前会话、范围和回合关联及 learning.db 持久化；设计已接受，实现待 T2/T6/T7。
