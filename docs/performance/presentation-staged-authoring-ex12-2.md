# EX12.2 同一运行的阶段切换与框架交接

2026-10-02。状态：**已完成；下一入口 EX12.3。** 合同见[切片方案](../切片方案-演示页全局框架与分阶段制作.md)与 [ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)。基于 `5e10516` 的现有工作树，保留其他任务修改，未创建提交。

## 运行行为

`presentation.author` 新增 `prepare`，phase 和 needs 必填，支持 global、local、review 及已有六项技术指导；framework、focus 为可选文本。一次提交完整替换 `RunContext.presentation_authoring`，省略文本即清除，needs 按固定次序去重。工具回执只返回状态、phase、needs 和 changed，框架不复制到结果正文。

制作能力首次可见时，`build_sample_request` 加载共同原则、工程合同、能力目录及 global 职责。prepare 后，下一次采样使用当前阶段／能力选择器；global 不带技术参考，local/review 带所选参考及依赖。当前框架与焦点由 `PresentationAuthoringContext::fragment` 生成 `presentation.authoring_context`，以 user 角色任务数据附在已投影的历史之后，纳入实际请求预算。设计文本不进入 instruction modules，也不与动态系统规则快照混写。

历史、工具结果裁剪和压缩仍走原路径；每次请求都从当前运行记录重投影框架。旧 prepare 调用／回执不恢复运行状态，不覆盖新框架。成功交付或取消清除记录，新 Run 从空记录开始。取消清理覆盖模型调用中途返回错误的路径。

prepare 必须独立成批。含 prepare 的多工具批中，author 操作返回 `PRESENTATION_PREPARE_REQUIRES_NEXT_SAMPLING`；prepare 与 write 无论先后均不改变设计或候选，下一次采样再继续。普通制作仍可直接 write；简单制作／小修订也可直接 prepare(local) 且不提供 framework。

既有进展签名包含当前设计值，不增加调用计数作为进展。prepare 不记录候选／观察／交付能力；等价重复提交（包括 needs 重排或重复项）不产生进展，沿用现有无进展收口。真实 write、preview、deliver 仍由宿主和原候选／图片观察检查处理。

方法资产版本更新为 `ex12.v2`；共同工程指导和工具参数说明包含调用与替换语义。来源、候选、图片、Goal 完成资格继续来自原运行事实。

## 验证与结果

运行前确定的失败目标是：下一次采样仍使用旧职责；框架被投成系统指令；历史或压缩覆盖当前框架；换作品继承旧焦点；重复 prepare 重置无进展判断；仅 prepare 被视为目标完成；阶段切换赋予来源或旧截图资格；取消后残留当前设计。出现时修正相应状态生命周期、投影或进展路径。

| 验证 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test -p runtime --lib ex12_ -- --nocapture` | 9 通过，0 失败 | [定向日志](presentation-staged-authoring-ex12-2/staged-runtime-tests.log) |
| `cargo test -p runtime --lib -- --nocapture` | 433 通过，3 忽略，0 失败 | [Runtime 回归](presentation-staged-authoring-ex12-2/runtime-regression.log) |
| `cargo check -p server --tests` | 编译通过 | [Server 编译](presentation-staged-authoring-ex12-2/server-check.log) |

Runtime 全库回归覆盖当时的 8 项新增测试及原来源、工具路由、图片观察、交付、目标完成、上下文预算和压缩路径。随后只新增“交付前换作品省略字段”的独立用例，未再改生产代码；最后 9 项定向测试全部通过。

受控 adapter 驱动原循环，实际捕获 `AgentRequestPlan`：global → local(A) → 写候选并观察 → global(B) → local(B) → 新候选 → review → 新候选未观察时拒绝交付 → 新候选观察 → 交付 → 空设计。断言唯一共同正文、唯一阶段、恰当参考、当前任务片段及候选绑定图片。另有简单制作与普通短答、等价重复 prepare、同批两种调用顺序、框架假来源／假交付资格、取消与历史新 Run 的测试。

压缩用例在 prepare 之后制造当前回合压力，实际安装 MidTurn checkpoint：旧已完成历史退出后续请求，当前框架／焦点仍以 user 片段存在，阶段为 local。交付前换作品用例确认省略 framework/focus 和清空 needs 后旧内容不继承。

## 已知限制

这是 Runtime 请求链路与合同验收。受控宿主提供候选和图片回执，没有运行真实模型生成、浏览器成品实验或 Linux 部署；Server 本次仅做编译验证。真实画面与三视口工程执行沿用既有能力，长页 scroll 待 EX12.3，真实模型同底座对照与 Reader 连续使用待 EX12.4/5。

运行内框架没有跨轮持久化；后续修订先读取实际交付版本。EX12.0 的两轮原请求、上一版交付与相同材料继续作为比较输入。本次不据工程测试宣称表达质量或学习效果改善。

编译保留既有 ts-rs serde 属性警告及 Server 未使用函数警告；Runtime 的 3 项既有忽略测试未运行。
