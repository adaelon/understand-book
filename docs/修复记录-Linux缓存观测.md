# Linux 缓存观测补齐（2026-10-03）

目标：Linux 阅读器每次模型请求的缓存用量与请求变化出现在同一个 LangSmith span，支持下一轮真实问答定位缓存复用下降的位置。

修复前证据：Linux 服务的 `UB_OBSERVABILITY_MODE=off`，未配置 LangSmith 凭据；本地项目为 `understand-book-dev`。上一轮 20 次请求的输入缓存命中率为 67.9134%，尚无完整请求对比数据，不能倒推具体输入变化原因。

工作切片：

1. 已完成：在发送到 provider 的最终 JSON 处采集请求，按同一 run、同一用途直接比较前一次请求；只输出固定类型的差异元数据，沿用现有 usage 和 span。
2. 已完成：验证追加消息、改写历史、工具定义、推理、工具参数及图片变化的区分，验证 metadata 合同和真实 HTTP 路径。
3. 已完成：Linux 环境与正式多用户执行路径均已接通 LangSmith。首次真实问答暴露的漏接已在 16:24 修复上线，并通过正式 Host 入口的隔离云端验收。见下方真实问答分析与后续修复切片。

约定：不上传正文、推理原文、工具参数、图片或密钥；请求体只在当前 run 的内存里保留各用途的前一次请求，不新增摘要或指纹。索引从 0 开始，比较结果说明 JSON 消息变化，不声称等于 provider 的 KV token 前缀或缓存失效原因。历史记录缺少字段表示未采集，不能解释为未变化。

本次工作基线存于 `tmp/cache-observation-20261003/before`，用于只部署此次增量，保留工作区已有改动。

## 记录位置与解读

每个 LLM span 的 `extra.metadata.ub_observation.metadata.request_diagnostics` 与该次 `usage` 放在一起；本地活动记录也保存相同的诊断。LangSmith 标准用量放在 `extra.metadata.usage_metadata`，其中 `input_token_details.cache_read` 保留 provider 报告的缓存 token。这是 [LangSmith 官方成本统计文档](https://docs.langchain.com/langsmith/cost-tracking) 指定的元数据入口。

Linux 真正上报后发现旧实现把 `usage_metadata` 放在 REST 请求顶层：自定义 metadata 中用量完整，但云端标准 `prompt_tokens=0`、`prompt_token_details={}`。本次同时修正 create/update 的字段位置，新增回归，并以云端读回的标准计数验收。

Linux 验证还出现一次后台上报超过原有 2 秒时限：模型 span 已成功到达，最后一条根 span 更新超时，记录保留在持久化队列。将 HTTP 上报时限改为 10 秒，允许跨区域连接与 API 响应；上报仍在后台执行，关闭服务的等待上限仍为 2 秒。

| 字段 | 用途 |
| --- | --- |
| `request_index` / `step_id` / `previous_step_id` | 本 run 的请求序号、活动编号、同用途的前一次活动编号 |
| `message_count` / `unchanged_prefix_messages` | 请求消息数、保持完全相同的前导消息数 |
| `first_changed_message.index/role/fields` | 首个变化消息的位置、角色和变化字段；可区分正文、推理、工具参数、图片等 |
| `first_changed_message.previous_bytes/current_bytes/unchanged_prefix_bytes` | 该消息变化前后的 JSON 字节数，以及相同的字节前缀长度 |
| `messages_append_only` | 原消息全部保持，只在末尾新增消息；不代表 tools/settings 也保持 |
| `tools_changed` / `first_changed_tool` | 工具定义是否变化及首个变化位置 |
| `settings_changed` | messages/tools 以外的请求设置是否变化，包括 model、tool_choice、输出限制等 |
| `image_count` / `reasoning_message_count` 及 `previous_*` | 当前与前一次请求的图片数、带非空推理内容的消息数 |

同用途首次请求的比较字段为 null。HTTP 自动重试使用同一请求体，只记一个逻辑请求。主问答、压缩、修复、检索等用途各自保留比较基线；新一轮 run 重新开始。

后续统计使用 `sum(cached_input_tokens) / sum(input_tokens)`，不平均每次调用的百分比；先定位缓存率骤降的 LLM span，再查同 span 的首个变化位置、工具/设置变化、图片/推理数量。主问答与压缩调用分别统计，并给出全轮合计。

## 验证

- Runtime：差异分类 3 项、活动观测 6 项、流式与真实 HTTP 适配 8 项、观测合同 2 项通过。
- Server：观测、映射、队列、持久化与传输回归 29 项通过；真实 Native/ReAct HTTP 夹具经过多用户用量限制层，8 个请求的诊断与 usage 同 span，私有内容标记没有进入导出。标准 usage 的 create/update 字段位置有独立回归。
- TypeScript：合同 4 项通过，类型检查通过。已有不带新字段的记录仍可读取，新增类型由 Rust 导出。
- 真实 LangSmith canary 使用独立项目和模拟响应，其 token 数是测试值，不进入生产缓存率统计。

## Linux 发布与云端验收

2026-10-03 12:22:30（UTC+8）完成正式服务重启，运行目录为 `/opt/understand-book/releases/ex13-jl-20261002`。重启前活跃或尚未保存的运行数为 0；重启后的进程确认已加载 `UB_OBSERVABILITY_MODE=metadata`、LangSmith 凭据与项目 `understand-book-dev`。持久化目录为 `/opt/understand-book/data/multi-reader/observability-spool`，由服务账户拥有、权限 700。服务状态 active，本机和公网 HTTPS 的 `/api/auth/me` 均返回预期的 401。

最终 Linux canary 在独立项目 `understand-book-linux-observation-check` 上报 6 条记录，`sent=6`、`queued=0`、`spool_pending=0`、`dropped=0`、`last_error_code=null`。云端两个模型 span 均成功，标准输入 1000、输出 20、缓存输入 800，与各 span 内的 usage 完全一致；第二次请求识别为纯追加，原有 1 条消息保持不变。云端 inputs/outputs 为空，没有上传问答原文。这些数字来自模拟响应，不是生产缓存率。

[查看第二次模型调用的云端验收记录](https://smith.langchain.com/o/491b6259-365d-4169-b798-04d6e92643d7/projects/p/362592fe-5a23-49a3-bf32-d6e568f7710b/r/01a0ffff-2b4b-7b80-a553-2a38683e620a?trace_id=01a0ffff-2b4a-7b81-b1ea-0d768e6f5a5f&start_time=2026-10-03T04:21:47.979101)。安全回执保存在 `tmp/cache-observation-20261003/deployment.json`、`canary-result.json`、`canary-cloud.json`，远端同名回执与原程序、环境文件备份位于 `/opt/understand-book/backups/cache-observation-20261003`。临时凭据副本已删除。

生产机仅构建正式程序与小型验收程序：单编译任务、一个 CPU 配额、内存上限 1600 MB；最终构建用时 3 分 5 秒，期间阅读器保持正常。此次验证没有调用付费模型。新增的采集用于观测缓存行为，本身不改变模型请求内容或提高缓存命中率。

## 首次真实问答分析（2026-10-03）

分析对象为 15:09:21（UTC+8）开始、约 15:15:43 完成的“这部分给我演示一下吧，富文本演示，没有理解”，选区为“1.3.3 用实测检验估算”。会话 `chat-01a10097-4988-7cc2-87fa-3cc5d47ce675`，回合 `turn_01a10098-9194-7180-899b-0fb5bbeeb180`，模型 `deepseek-v4-flash`。

本地持久化日志包含全部 18 次模型调用的 usage 和请求差异，与最终 run summary 中逐次 provider 用量一致；全部为主问答调用，没有摘要压缩模型调用或 checkpoint 安装。

| 指标 | 结果 |
| --- | ---: |
| 累计输入 token | 1,277,397 |
| 缓存命中 token | 925,568 |
| 未命中 token | 351,829 |
| 整轮输入缓存命中率 | **72.4573%** |
| 输出 token | 80,335 |
| 上一轮缓存率 | 67.9134% |

命中率按累计缓存输入除以累计输入计算。两个回合内容不同，差值不代表观测代码带来的性能提升。

| 请求 | 命中率 | 未命中 token | 首个变化（索引从 0 开始） |
| --- | ---: | ---: | --- |
| 12 | 39.07% | 58,300 | 消息 27 的 assistant 工具参数变化，消息从 21,230 缩至 18,376 字节 |
| 13 | 11.47% | 88,937 | 消息 5 的 tool 正文变化，消息从 527 缩至 465 字节 |
| 17 | 11.43% | 100,206 | 消息 12 的 tool 正文变化，消息从 1,538 缩至 467 字节 |

上述 3 次合计贡献 247,443 个未命中 token，占本轮未命中的 **70.33%**；其余 15 次合计命中率为 **89.22%**。严格只追加的第 3、4、5、6、11 次合计 **94.95%**，其中第 11 次 **97.98%**。这些是观测子集，不是修复后整轮命中率预测。

已定位的客户端变化来源：

- 工具正文共用 48 KiB 的当前回合预算；`ActiveToolResultLedger::make_room_for` 淘汰最早已观察正文，`project_messages` 把其 `model_body` 改成 null。第 13、17 次的首个变化与该路径对应，且消息前缀明显退回。
- `maybe_auto_compact` 超过高水位后，先调用 `prepare_persisted_messages` 清理活动消息，再判断是否有可压缩历史。服务端该实现调用 `session_runtime::clean`，原地缩短 presentation 工具参数；即使本轮没有可压缩的旧回合，改写也已经发生。第 12 次变化后的 18,376 字节，与本轮持久化 `presentation.author.prepare` 消息转换后的请求 JSON 大小完全一致。需要修正这种“没有实际压缩却已改写活动历史”的行为。
- 图片按次注入、随后移除，以及保存后的源码参数折叠，造成其他较晚位置的变化。第 8 次图片从 5 张变成 0 张时命中率为 66.68%。不能只凭图片数量把全部未命中归因于图片。
- 第 2 次由工具发现启用 presentation 能力：system 内容、工具列表和设置同时变化，命中率 18.24%。此后工具列表与设置保持稳定；第 12、13、17 次的严重下降均无工具定义或设置变化。

正式上报缺口：读取服务进程确认 metadata 环境仍在，但 LangSmith 正式项目在部署后没有任何 run，生产 spool 也为空。线上 `run_admission.rs` 的多用户路径直接调用 `execute_model`，只传入 `RunStream` 作为 event sink，没有创建 `ObservationRun` 或转发给 LangSmith；`multi_user_host` 也未初始化 `ObservabilityRuntime`。此前独立 canary 证明传输与映射可用，不能证明该正式入口已接通。此次分析基于本地完整记录，未修改生产代码或重放模型调用。

证据存于 `tmp/cache-latest-20261003/session.jsonl`、`analysis.json` 与精简的 `summary.json`；线上相关源码快照位于该目录的 `online/`。后续优化优先处理过早改写历史，再评估工具正文预算、源码参数折叠和图片生命周期的总输入成本。

## 后续修复切片

用户已确认先完成正式上报与未压缩历史稳定性两项，再观测工具正文预算。Resident Run、压缩检查点、活动上下文预算均沿用现有 CONTEXT 定义，属于既有路径的技术修复；验收同时覆盖用量采集和历史持久化行为。

- [x] A：正式多用户入口初始化并持有观测运行，转发模型/工具事件，在执行及保存完成后上报根状态；正式 admission + HTTP 模型夹具覆盖成功、模型失败、保存失败，新增用例先红后绿。46 项受理/取消/恢复测试、6 项持久历史测试、29 项观测回归通过。
- [x] B：压缩预检查在副本上清理消息，仅在成功安装检查点后替换活动历史；新增用例先红后绿，覆盖不适用、生成失败、保存失败、成功安装、已有检查点覆盖。6 项自动压缩回归通过。
- [x] C：仅把两项增量部署到现网，受限构建耗时 4 分 43 秒；隔离测试实例走生产 Host 入口并向独立 LangSmith 项目上报，正式服务在活跃及未保存运行数为 0 时重启。

2026-10-03 16:24:55（UTC+8）完成两项修复上线，进程 PID 23583，服务 active，本机与公网 HTTPS 的 `/api/auth/me` 均返回预期 401。生产进程加载 metadata 模式与 `understand-book-dev`，正式宿主已创建 spool manifest。此次重启不修改模型或用量配置。

隔离验收使用 `formal_observability_canary`，经过实际 `multi_user_host::start`、登录、工作区、问答受理、两次模型请求、一次 `book.structure` 与历史保存。最终发送 8 条创建/更新，队列、spool 待发、丢弃均为 0；云端根状态 success、执行 completed、保存 saved，工具状态 success，两次模型的标准输入 1000、缓存输入 800 与自定义 usage 一致，第二次请求保持原有 4 条消息并纯追加。inputs/outputs 为空。这些为模拟用量，不代表修复后的生产缓存率。

[正式多用户入口的云端验收记录](https://smith.langchain.com/o/491b6259-365d-4169-b798-04d6e92643d7/projects/p/362592fe-5a23-49a3-bf32-d6e568f7710b/r/01a100db-98a6-70c1-9526-fc9863bed4d5?trace_id=01a100db-98a6-70c1-9526-fc9863bed4d5&start_time=2026-10-03T08:22:33.894308)。本次 87 项相关回归通过；最终夹具改用可直接执行的结构读取后，新增正式入口用例再次通过。早期验收夹具调用了没有当轮定位来源的原文读取，随后又用了不存在的目录工具名，均被既有门禁拒绝；修正夹具后工具成功执行，无业务门禁改动。

本地安全回执位于 `tmp/cache-repair-20261003/{deployment.json,formal-canary-result.json,formal-canary-cloud.json}`，远端同名回执、修复前源码与 `server.before` 位于 `/opt/understand-book/backups/cache-repair-20261003`。压缩修复与正式观测接线的验证均未调用付费模型。下一轮真实问答需确认整轮命中率、历史首个变化位置以及工具预算淘汰造成的未命中量。

本切片不修改 48 KiB 工具正文预算、图片生命周期或模型配置；这些策略后续按真实未命中 token 与总输入量评估。

## 已知限制

只在同一 run、同一用途内比较，不比较跨问答的首个请求。字节前缀不是模型 tokenizer 或服务端 KV 前缀；诊断能定位客户端输入变化，不能直接证明服务端缓存未命中的原因。上一轮没有采集的请求差异不能补算。

初次在 Linux 编译整个 server release 测试库期间，SSH 与网页失去响应，用户重启后恢复。主机内存约 3.7 GB、无交换空间；资源压力是可能原因，但缺少上次启动的内核记录，不能确认为 OOM。此后不再在生产机编译整个测试库，改用上述资源受限构建。原 2 秒上报超时下的失败测试记录保留在备份目录 `canary-spool`，最终验收使用独立的 `canary-spool-final`，生产队列另设目录。
