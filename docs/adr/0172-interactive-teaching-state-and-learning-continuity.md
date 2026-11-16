# ADR-0172 交互教学现场与学习接续

日期：2026-10-20。状态：已接受设计；OM0–OM6 已完成。实施合同见[切片方案](../切片方案-交互教学与学习接续.md)，语义现场、原证据接续、支撑选择、单动作闭环、连续演示及完整场景对照分别见 [OM1](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om1/README.md)、[OM2](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om2/README.md)、[OM3](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om3/README.md)、[OM4](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om4/README.md)、[OM5](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om5/README.md)、[OM6 实施记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/interactive-teaching-continuity/om6/README.md)，外部依据见[OpenMAIC 源码索引](../参考实现-OpenMAIC交互教学与学习记忆.md)。

现有 Reader 已有教具状态回执、教学设计、帮助记录和私人学习证据。本次把这些能力接成“观察现场 → 提供支撑或演示 → 接收真实回应 → 带着依据继续教学”的闭环，采用以下五项决定。

1. **现场使用教学语义。** 在既有 `PresentationState` 上表达当前参数、实际呈现结果及两者是否一致；页面复用自身计算状态产生观察，宿主继续保留实际可见文本和确切保存回执。
2. **教师操作同一教具并回读结果。** 内容版本保持不可变；教师通过当前版本声明的动作改变运行现场，取得浏览器执行及保存回执后继续讲解。动作记为教师行为，实际展示的支撑另记帮助，学习者表达独立记录。
3. **支撑与判断按当前任务选择。** 沿用 TeachingDesign／TeachingMove：先提供可上手的关系、方法或类比例子，再判断是否需要学习者作答；开放任务保留合理的多种答案，提问由其实际教学价值决定。
4. **学习接续由原始证据重建。** 下一轮从当前目标、既有作品、评价、帮助条件、理解修订和待解决关系组织紧凑上下文；原事件及引用为权威，教师工作记忆与学习者理解继续分别表达。
5. **记忆改变支撑，当前明确请求优先。** 通过任务范围内的 `teaching_implication` 和设计修订调整讲法；用户要求直接解释、降低门槛或深入时立即采用，历史推断不能覆盖当前请求。

这保留了现有证据与工作区归属。OM4 已沿本地／网络活跃运行、页面回调和原状态保存实现真实执行回执；OM5 将教师动作、可见帮助、输入时原表达与原页面交付关联，取消后的已见帮助及重开接续沿原 LearningStore 保存。`compare-one-factor@3` 派生并保留 @2，用户可在同页接手。OM6 完成速度、借用／移动、开放设计三组场景，覆盖本地与网络 Reader、同版重开和已选入历史。真实模型暴露的总量／额外收益混淆通过共享 `plain-explanation.v3` 修正，并仅重跑受影响案例。继承 [ADR-0164](0164-tutor-mission-and-cognitive-building-blocks.md)、[ADR-0168](0168-executable-explanation-methods-and-evidence-based-evolution.md)、[ADR-0169](0169-cross-book-reading-and-private-understanding.md) 和 [ADR-0170](0170-goal-scoped-learning-workspaces-and-shared-releases.md) 的边界。

## 验收边界

OM6 的真实模型对照在同一当前上下文中替换指导；模型生成设计和正文，冻结回放沿原工具校验、存储和浏览器交付。语义审阅使用已有确定性接受夹具。15 次 provider 调用及失败、修正、调用量与耗时均保留，改前开放设计的内部 LID 拒绝没有隐藏。结论限于三组场景及所述工程路径，真人学习收益尚需实际使用依据。
