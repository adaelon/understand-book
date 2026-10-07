# EX13.5 摘要输入与修复前缀

2026-10-02。**实现及确定性验证完成**。Runtime 453 项通过、0 失败、3 忽略；新增真实模型调用 0。合同见 [EX13.5](../../../切片方案-EX13-Goal工作计划与演示修订上下文.md#ex135优化摘要请求保持来源覆盖合同)及 [ADR-0155 §5](../../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md#5-摘要与效果验证)。

## 请求与预算

```text
CompactionRequest（完整运行时合同）
  → generation_input（来源材料在前，phase 在后）
  → 固定 system + user 输入 → 严格校验
  → 首次拒绝：同一 system + 原 user + 尾部 runtime_repair_diagnostic
  → 再次拒绝或修复输入超预算：返回原错误，不安装摘要
```

`generation_input` 保留完整、有序的 eligible_items、required_source_ids、optional_source_ids、allowed_evidence_refs、allowed_supersession_edges 和 phase。只从模型输入移除以下四个字段；完整 `CompactionRequest` 及检查点仍持有原字段。

| 移除字段 | 仍由运行时承担的用途 |
| --- | --- |
| schema_version | 请求结构校验；模型输出只接受十节 CompactionDraft。 |
| prompt_version | 请求及检查点版本校验；生成提示正文已由 system 提供。 |
| source_history_revision | 安装、恢复时识别覆盖历史；不参与语义归纳。 |
| raw_retained_item_ids | 选择并保留当前用户原文和未完成工具链；这些正文不在 eligible_items 中。 |

来源 ID、顺序、正文、来源与证据关联保持原样；没有重写工具回执正文或补造来源。既有检查点的版本及恢复合同保持，未新增迁移。固定规则说明如何读取尾部修复诊断；诊断只带原错误码与错误说明，失败草稿不重新加入输入。

`request_input_tokens` 直接估算发送的 system/user 字符串。普通请求、完整回合分块、层级合并都使用同一 `generation_input`；修复检查使用已追加诊断的实际 user。`maybe_auto_compact` 继续先扣除摘要输出预留与 safety margin，CatalogMatch Flash 的 65,536 输出上限、low reasoning 和应用压缩水位保持原值。

## 与既有裁剪的关系

EX13.2 在成功候选首次被观察后裁剪当前回合模型投影，EX13.4 将追问全文改为确切版本读取入口。当前 Runtime 回归继续通过 EX13.2 的七次受控采样，以及包含源码裁剪、Goal 工作项和来源覆盖的中途压缩用例。EX13.4 的请求体积及 Server 验证沿用[上一片记录](../ex13-4/README.md)。

压缩触发仍由当前采样计划的活动预算决定。`prepare_history_compaction` 从原始历史生成 source ID，以确定性工具回执提供工具材料；当前回合保留为原文后缀。EX13.5 不把候选投影拿来重算 source ID。摘要前后对照固定使用 feedback5 的同一份 237 来源输入，均为首轮加一次修复，共两次结构化请求；本片不据此声称自动压缩触发次数减少。

## 离线证据

[before](before/completion-00-request.json) 在生产修改前通过原 `call_generator` 捕获；[after](after/completion-00-request.json) 使用同一份 feedback5 输入和原始失败/修复输出捕获。原 EX13.0/EX12.4 文件保持冻结。两份输出经过当前 Runtime 真实校验器：首份仍因 `item.fact.quote_receipts` 引用不存在的 source ID 被拒绝，原修复份保持完整内容并通过。

| 指标（UTF-8 字节） | 改动前 | 改动后 |
| --- | ---: | ---: |
| 首轮 user 材料 | 152,535 | 152,356 |
| 首轮 system + user | 156,803 | 156,655 |
| 修复 system + user | 157,204 | 156,818 |
| 首轮有序内容紧凑 JSON | 181,737 | 181,574 |
| 修复有序内容紧凑 JSON | 182,141 | 181,749 |

改动前首差在 system 的第 4,268 字节，位于整份来源材料之前。改动后 system 完全相同，user 的全部 152,356 字节保持为修复请求前缀，仅追加 163 字节诊断；规则与材料共有 156,655 字节可保持不变。正文体积节省较小，主要变化是修复前缀的位置。

[Native HTTP](after/wire-native.json) 和 [ReAct HTTP](after/wire-react.json) 来自本地 HTTP 服务收到的实际请求，使用另一份受控材料：真实适配器收到首份非法输出后修复，两个协议均保留 system 和完整 user 前缀；只有 system/user 两条消息，不携带工具，输出上限与 reasoning 设置保持。此录制不包含请求头或凭据。

完整分类、首差和原 usage 见 [comparison.json](comparison.json)。原 feedback5 两次真实调用的输入合计 109,641 token（缓存输入 55,296），输出 85,079 token，其中 reasoning 44,442，合计输入加输出 194,720。reasoning 是输出的子集，不再次相加。此次重放 `usage=null`，新增提供方调用为 0。

## 验证与重放

- 新真实录制回归先在“修复改变 system”处失败，原请求已冻结于 before。
- `cargo test -p runtime --lib`：453 通过、0 失败、3 忽略。包括新增的真实失败重放、精确预算边界、严格校验矩阵，以及 Native/ReAct 本地 HTTP 修复用例。
- 覆盖十节缺项、遗漏/重复来源覆盖、required source 被标 non_task、未知证据、来源未关联目标、两次非法输出后拒绝第三次调用，以及修复输入恰好可容纳/少一个预估 token 的边界。既有层级合并、旧历史恢复、失败不安装及当前原文保留用例通过。
- 既有提供方等价用例原先要求在线路上出现 prompt_version；移除该包装后，改为检查完整固定提示相同、线路不带该字段、检查点继续保存原版本。验证合同没有放宽。
- `python docs/performance/presentation-context-ex13/ex13-5/summarize.py`：确认来源字段、顺序、诊断对应关系、首差与两种 HTTP 请求，并生成 comparison.json。

日志：`tmp/ex13-5-red.log`、`tmp/ex13-5-compaction.log`、`tmp/ex13-5-runtime.log`。定向首跑检出上述旧线路断言，最终全库已通过。

重新录制时，设置 `EX13_COMPACTION_RECORDING_DIR` 为新的输出目录，运行 `cargo test -p runtime --lib ex13_compaction`；重放两项请求测试会分别写 completion 请求及 wire-native/react。该变量只用于测试输出；本目录的 before/after 作为本片冻结证据保留。

## 已知限制

预算是现有文本 token 估算，并非提供方精确 tokenizer。离线字节和本地 HTTP 前缀不能证明真实缓存命中、自然模型修复成功率或费用下降；这些仍由 EX13.6 在明确调用范围和费用边界后验证。主线与摘要仍是各自独立的请求，本片未尝试共享两者前缀。

本次未提交、未部署。EX12.4 最终原生验收仍受原 402 余额问题阻塞，EX12.5 连续使用尚未开始；主 SESSION_CHECKPOINT.md 属于其他在途任务，本轮刷新 checkpoint_ex.md。
