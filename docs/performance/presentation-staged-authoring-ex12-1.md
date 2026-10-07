# EX12.1 共同原则、阶段职责与技术资产分层

2026-10-02。状态：**已完成；下一入口 EX12.2。** 合同见[切片方案](../切片方案-演示页全局框架与分阶段制作.md)与 [ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)。工作基于 `5e10516` 的现有工作树，保留其他任务修改，未创建提交。

## 实际改动

`skills/presentation/SKILL.md` 更新为 `ex12.v1`，只保留已接受的七段共同正文。全局、局部、串读职责分别进入 `phases/`；方案中的正文改为指向这些文件，避免维护两份规范。`engineering.md` 保持来源、确切版本、自包含资源、数值校验、真实三视口预览及后续观察／交付合同；`capabilities.md` 提供短能力目录。

六项技术参考为 editing、static_plot、state、continuous_scene、konva、manim。原固定开场编排已移除；学习率公式、具体 seek 位置和边界算例移至不自动注入的 `examples/continuous-scene.md`。通用连续场景保留前进、回退、重复定位、数学状态与视觉插值区分、零位移箭头和保存小数位置。媒体参考保留解码帧、异步定位、回调取消、短面板操作和暂停恢复合同。

`agent_prompt::presentation` 编译指导资源；`policy_modules_for_tools_with_presentation(tools, guidance)` 输出实际 `InstructionModule`，由现有 `AgentRequestPlan::for_agent_turn_with_modules` 消费。模块按稳定顺序去重，所有呈现模块携带 `ex12.v1` 版本。工具描述保留操作和参数语义，长篇工程指导已移到对应资产。

| 输入 | 呈现指导 |
| --- | --- |
| 无 author 工具，包括意外传入制作上下文 | 不加载呈现模块 |
| global | 共同正文、共同工程合同、目录、全局职责；无技术参考 |
| local / review | 共同正文、共同工程合同、目录、当前职责、所选参考及依赖 |
| continuous_scene | 同时加载 state，保持保存与恢复合同 |
| manim | 同时加载 continuous_scene 与 state |
| konva | 不隐含时间轴；自定义交互需要保存时另选 state |
| 原 Resident 无阶段调用 | 共同资产与全部技术参考，无阶段职责 |

原调用尚未提供运行内阶段上下文，因此最后一行是当前真实运行行为。显式选择器可供 EX12.2 直接接入；此次未新增 `prepare`、框架存储、持久 Brief 或交付门槛。

## 验证与结果

运行前明确的失败目标：共同正文被多次注入；全局或静态请求夹带完整媒体细节或强制 seek；能力拆分导致现场、来源或交付合同丢失；工具描述重新注入已移出的长篇指导。出现时修正模块归属或能力依赖。测试使用实际 author schema 与 `AgentRequestPlan`，检查正文和 `instruction_assets`，不锁定全文快照。

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test -p runtime agent_prompt:: -- --nocapture` | 15 通过，0 失败；其中新增 EX12.1 测试 10 项 | [运行记录](presentation-staged-authoring-ex12-1/agent-prompt-tests.log) |
| `cargo test -p runtime presentation_author:: -- --nocapture` | 8 通过，0 失败；覆盖既有读写／补丁、schema、历史句柄、发现及 Native/ReAct 图片投影 | [运行记录](presentation-staged-authoring-ex12-1/author-contract-tests.log) |

定向测试覆盖普通回答、全局设计、静态局部／局部编辑、连续场景、媒体局部、Konva 与显式现场、整篇串读、能力重复及次序、Native/ReAct 指导一致性和无阶段原入口。静态局部没有 `presentationScene` 或保存现场实现要求；全局请求连同工具参数描述也不包含 `requestVideoFrameCallback`、`HAVE_CURRENT_DATA`、`hitFunc` 或 restorer 细节。来源与三视口／后续采样合同在各制作职责中持续存在。

原测试将学习率公式、固定位置滑块和完整媒体细节锁在共同正文中，与 EX12.1 分层合同冲突；已替换为对应能力存在、无关能力缺席的请求断言。

## 已知限制

本切片验证指导资产与请求组装，未运行真实模型生成、浏览器页面实验或部署。EX12.0 的原页面和材料继续作为后续同底座对照输入。本地新资产需重新编译后才能进入服务；线上版本未更新。

编译仍报告现有 ts-rs serde 属性警告；两组定向测试均通过。

运行内 `prepare` 和全局框架投影属于 EX12.2，长页 `scroll` 属于 EX12.3；当前选择器不持有用户任务或框架数据。真实复杂制作、完整交付及学习效果不由本次单元测试证明。
