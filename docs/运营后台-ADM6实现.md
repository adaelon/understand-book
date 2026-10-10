# ADM6 费用停止与显式继续

日期：2026-10-08。实现位于当前工作树，专项与受影响回归已通过；未提交、未部署。依据 [ADR-0156 §6](adr/0156-reader-admin-and-account-allowance.md#6-持续执行与额度停止) 和[切片方案第 5 节](切片方案-运营后台与账号额度.md#5-停止恢复与读者体验)。

## 费用原因传递

`ModelSpendPort` → `AdapterError.spend_stop: Option<SpendStop>` → Runtime → 既有 `ToolError` / 工具 JSON → 宿主历史与 Goal。适配器保留原枚举；跨工具 JSON 和历史边界只匹配结构化 `category=model_spend` 与固定 `error_code`，不检查自由文本。普通 Provider 错误保持原分类。

| SpendStop | error_code |
| --- | --- |
| MissingScope | MODEL_SPEND_SCOPE_MISSING |
| RateUnavailable | MODEL_RATE_UNAVAILABLE |
| InsufficientAllowance | ALLOWANCE_INSUFFICIENT |
| AllowanceExpired | ALLOWANCE_EXPIRED |
| StorageUnavailable | MODEL_SPEND_STORAGE_UNAVAILABLE |
| PermissionRevoked | RUN_PERMISSION_REVOKED |
| ReconciliationRequired | MODEL_CHARGE_RECONCILIATION_REQUIRED |

外层采样、query 的消歧/支持判断、synthesize 的单批/多批综合、回答来源修复、最终回答、压缩及 Tutor 判定均保留费用原因。压缩的生成失败直接返回，不进入格式修订；来源修复遇到费用拒绝直接停止，不降格成普通交付失败。图片观察和呈现生成经过原外层发送入口。模型活动记录使用实际费用错误码。

内部工具收到费用停止后记录当前工具结果，立即结束 Run；同批后续工具不执行，也不触发下一轮修复或采样。工具消息缺少的尾部由宿主补充明确的未执行回执，保持 Provider 消息配对。

## 保存与继续

费用停止保留本轮已有工具回执、效果和轨迹，沿既有私人历史入口保存。压缩在提问入上下文前停止时补回已接单的用户问题。终态为 `failed`，Goal 仍为 `open`，`last_stop_reason` 保存真实费用原因。既有完成判定继续要求实际交付成功。

保存成功的终态错误带“本次已完成内容已保存”。终态保存失败时保持 `TURN_UNSAVED` 和 `persistence_state=failed`，并在快照错误的 `execution_error` 中另存原停止原因；提示不声称保存成功。`retry-save` 使用内存中的原结果，不调用模型或工具。

补充额度本身不触发运行。读者通过原聊天/Goal 继续入口提交新的 `client_request_id`，可指定原 `goal_id`；新 Run 和新调用身份依据已保存历史推进。重发原请求号只返回原接单记录。恢复费用状态不重发请求，已保存笔记及其他私人成果仍由原存储持有。

## 验证

专项 **7 通过、0 失败**（Runtime 5 项、Server 2 项）：

- Runtime：七类费用原因 × 外层、query、synthesize、repair、finalization；内部拒绝后同批笔记不执行；压缩停止不改原历史/检查点；Tutor 停止不提交评估；模型活动保留原原因；普通错误文本中的费用码不触发费用分类。
- Native/ReAct：chat（含呈现预览图片）、complete、complete_structured 的费用拒绝保留原枚举，监听端口没有收到 HTTP。
- Server：真实 SQLite 与本地 HTTP 覆盖余额不足、无有效期、缺失费率和费用写入失败；过量实际用量完整落账，已完成笔记保留，Goal 保持 open；成功保存提示与 TURN_UNSAVED 分离；重试保存不调用模型，原请求不重放，补额不自动运行，显式继续采用新 Run/call，已有笔记内容和身份不变。

受影响回归：Runtime **69 通过、0 失败**；Server **95 通过、0 失败、1 忽略**。忽略项为既有 `jl4_kill_child` 子进程入口。覆盖 ADM4–ADM6、传输解析、活动记录、回答来源修复、自动压缩、Tutor、费用并发/恢复、Resident 生命周期、Goal 继续/完成、多人接单、取消与历史保存恢复。

本次相关修改空白检查和新合同本地链接检查通过。可复跑命令见 [checkpoint_ADM](../checkpoint_ADM.md)。

## 已知限制

- 本轮使用真实 SQLite 与本地可控 HTTP；真实 Provider、Vue 额度页面和后台发布由后续切片验收。
- 进程在终态保存失败后退出，沿原恢复合同记录 `INTERRUPTED`；已落库费用和工具效果保留，尚未持久化的内存停止原因不承诺恢复。
