# Reader 教学回合误判无进展修复

日期：2026-10-11。范围：原文覆盖、教学目标来源绑定、有限校验恢复和 LangSmith 根记录。

## 真实运行依据

`understand-book-dev` 中，11:53、11:57、16:28（香港时间）的三个回合返回 `incomplete=true / AGENT_NO_PROGRESS`，LangSmith 根记录却显示成功。

- `01a12918-07d7-7380-a109-87029b9c944b`：19 次原文读取的输出与当前原文逐项一致；SmartNIC 目标的区间并集只缺 UTF-16 `[31273,31275)` 的两个换行符。旧覆盖函数要求连续字符区间，导致已经读完正文的教学选择仍被拒绝。
- `01a12a14-50c1-7911-b037-250f3456a477`：教学目标复制了检索结果的 `source_revision`。检索版本包含叶子划分，教学版本使用原文身份；旧顺序先检查理解解释与目标的交集，将版本错误报告成“范围不相交”。后续缺少提示内容的修正又触发参数校验和无进展停止。
- `01a1291c-24ee-79e0-b8dc-15f021d3e06e`：存在实际不相交的理解解释与教学范围。该范围约束继续保留；本次提供有限修正机会，不将任意历史解释视为当前目标的依据。

## 实现

1. `server::teaching::require_target_read_this_turn` 合并实际读取区间，仅允许未覆盖区间中的空白；漏读正文返回 LID 和 UTF-16 位置。目标选择、理解假设、评分来源共用该规则。
2. `runtime::tutor::target_input_schema` 允许省略目标来源的 `source_id/source_revision`。`server::teaching::bind_target` 从本轮冻结绑定补齐；显式错误身份仍拒绝，并先于理解解释交集检查报出具体原因。持久化的 `TeachingTarget` 继续保存完整版本和范围。
3. `ProgressPhaseGuard` 为 `tutor.step` 的教学或参数校验失败开放一个恢复批次，与定位恢复共用一次额度。重复同参和持续无效改参仍受限制。成功接纳教学选择或范围外处理的 `tutor_resolved` 纳入现有进展签名。
4. `ObservationRun::finish` 将未完成原因导出到 LangSmith 根记录 `error` 和 `metadata.stop_reason`。具体工具错误保留在 `last_tool_error_name/last_tool_error_code`，后续停止器错误不覆盖它。已交付部分回答仍如实记录为 delivered。

对应决策：[ADR-0150](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md)、[ADR-0158](../adr/0158-book-structure-ready-tutor-and-optional-teaching-assets.md)、[ADR-0134](../adr/0134-optional-langsmith-observability-and-evaluation.md)。

## 验证

原始源码和本次新增回归在独立的 D 盘副本中进行对照，不回退共享工作目录中的其他修改；旧版与修复版使用不同构建目录。

- 独立旧版对照：新增 5 项回归全部失败。校验恢复用例中，第三次合法修正被旧版提前进入的禁用工具收尾阶段拒绝，返回 `FINALIZATION_TOOL_PROTOCOL_VIOLATION`；修复版同一脚本能够完成。
- 修复版 Server 教学循环与观测链路：61 通过、0 失败、6 按原设置跳过。
- Runtime 进度、重复调用、定位恢复、Tutor 契约和观测元数据：34 通过、0 失败。
- 新增回归覆盖分段空白与漏读正文、运行时补齐来源版本、错误版本先于解释范围校验、两次真实校验失败后的成功修正及持续无效改参停止、未完成根状态及错误优先级。
- 校验恢复用例先成功回读教学记录，确保通用能力已经出现；随后确实触发 `TEACHING_INVALID` 和 `TOOL_ARGUMENTS_INVALID`。第三次修正成功时能够交付；第三次仍无效时保留未完成状态。

测试日志位于本机 `tmp/agent-no-progress-20261011/fix/`。Windows 默认 Rust 1.96 的依赖构建遇到执行文件访问拒绝，使用已安装的 Rust 1.95 和 D 盘独立构建目录完成验证；没有变更项目工具链要求。

后续固定输入实模复验：TC2A 原模型、原文、原回答与空补充配置下 **3／3 完整教学交付**，无 `AGENT_NO_PROGRESS`。第 1 次连续两次教学校验失败后纠正成功，但首次报错同批还有原文引用进展，没有单独触发新增恢复额度；该分支的因果证据仍是上面的隔离回归。持久目标或正文的条件概括错误继续保留，见[修复后复验](tutor-continuation-tc2a/recovery-20261011/README.md)。

## 已知问题与发布状态

已完成本地确定性测试及上述三次真实模型接续，尚未发布到线上，也未验证云端 LangSmith 写入。原始 trace 保持不变；新的停止状态在部署后对新回合生效。现有理解解释与教学目标必须属于同一来源版本且范围相交的约束保留。
