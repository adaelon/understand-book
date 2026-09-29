# Agent-native learning environment and teaching agency

Status: Accepted, 2026-09-07. Extends ADR-0007 and ADR-0113; guides the teaching design.

**决策**: 系统提供原生学习环境，Agent 作教学选择。

**否决**:
- 系统穷举用户表达并编排固定教学流程：开放意图与现场差异不能穷尽。
- 只交付工具目录或静态角色说明：缺少当前事实、对象关系与真实行动反馈。

**命门**: 系统提供和维护真实状态、对象语义、可发现的操作能力与执行反馈；Agent 结合用户目标、已观察状态和可用素材决定具体教学动作。环境由 TeachingMap、LearningMemory、Reader 与交互状态的既有权威来源构成，事实与判断分开，不复制为新的可编辑真相；既有来源、学习证据及用户控制的所有权保持成立。
**何时回头**: 真实交互证明某类选择需要明确策略支持时，限定补充该策略。
**展开**: [grill.md Q90/Q91/Q94/Q95](../grill.md)、[领域术语](../../CONTEXT.md)。每回合自动提供稳定环境说明与精简当前现场，相关资产和历史细节按需读取；表现判断必须取得实际呈现、帮助条件与用户回应。环境分别表达阅读可用与正式学习就绪，正式 TutorLoop 须满足 [ADR-0120](0120-whole-source-prebuild-gate-for-formal-learning.md) 的整份材料预构建要求；自动观察的具体字段、体积预算，以及能力发现、动作与反馈接口继续 Grill。
