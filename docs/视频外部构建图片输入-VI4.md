# 外部构建图片输入（VI4）

## 合同与入口

Build Engine 在确定语义工作单元后调用 `freezeBuildExecutorMediaInput`，传入该单元的 descriptor、视频来源清单、转写映射及媒体解码器已经产生的 PNG、实际帧位置和实际裁剪区域。输入文件保存在工作区 `.build/executor-media/<stage>/<work_unit_id>/<input_hash>.json`，包含真实图片字节。相同准备可以重入，改变已冻结的图片须使用新工作单元输入。

`executor_media_input.v1` 为现有 Codex session v3 的可选媒体输入合同。无媒体时保留原有 manifest、文本分块、候选请求和会话协议。DSH session v4 的当前发布能力为纯文本；媒体任务返回 `executor_media_capability_unavailable`。

媒体清单列出工作单元、媒体和转写版本、图片顺序、请求时刻、实际 PTS、原帧尺寸、裁剪、PNG 尺寸及字节数。相关英文依照工作单元的 evidence LID 选择。已有语义输入保持原字节及预算依据；图片提供同一单元的视觉上下文，候选仍满足该阶段的原输出合同。

## 数据流与恢复

`Engine 媒体准备 → 冻结媒体输入 → 正式 opaque handoff → executor.open → DELIVER_INPUT → executor.input.next → 文本分块 + 原生图片 → 最后一批确认 → generation.start → submit_candidate → 既有候选门与 writer`。

媒体身份进入现有 generation input 身份计算。全部图片作为不可拆分的图片集合附在最后一个输入块，MCP 返回一个文本控制块和按声明顺序排列的顶层 `image/png` 块。文本控制块只含位置与转写，不包含 Base64。图片不转成文本分片。

最终确认沿现有 ordinal 回执和生成授权执行。断开后新连接从已确认的位置继续；未确认的最后一批重发相同 PNG、原位置和转写。原解码文件移动或删除不影响重放。任务绑定改变或冻结媒体缺失使原 generation input 不匹配，旧会话不能继续。

父代理仍只持有 handoff 与生命周期控制投影。媒体、转写与候选留在专用执行器，最终返回既有 lifecycle JSON。执行器角色的 code-mode helper 会逐张发出原生 image block。

## 预算与安装资产

Codex 当前媒体传输档位：最多 8 张、PNG 总量 4 MiB、总像素 16,588,800、MCP result 8 MiB、完整 JSON-RPC 行 8 MiB + 1 KiB。该档位以 VI0c 实测八帧为依据，表示本项目采用的传输预算。

图像上下文规划按每张 `256 + ceil(width/512) × ceil(height/512) × 256` 预留 tokens；这是工程预留值，实际 Provider 用量另记。原文本估算、图片预留与新增元数据/转写文本共同受工作单元的 `effective_body_limit_tokens` 约束，超限返回 `executor_media_context_limit`。因此传输容纳八张不保证任何文本工作单元都能加入八张。候选输出预算不变。

能力由 `BuildExecutionProfile.media_input`、宿主合同及 MCP initialize 返回。安装时同步源码角色、项目 `.codex/agents`、根与插件中的角色模板、executor skill；新代码由现有 Build sidecar 入口的导入关系打包。源代码变更后需重建 sidecar 才能在安装态使用。

## 验证

2026-10-10 完成原生图片、预算、能力声明、角色接线及验收。Core 108 个不重复定向用例、DSH 8 个相邻回归通过，Core 类型检查通过。原片正式 Codex 任务读取完整整帧及裁剪，将机器转写的 “beef float 16” 与代码中的 `bfloat16` 对齐，生成的实体和 2018 年开发断言通过既有候选门；原英文未被修改。

本次正式任务调用四个工具各一次，图片批次的完整 JSON-RPC 行为 708,484 字节，耗时 72.516 秒；最终仅返回 `committed` 生命周期。凭据见[验收记录](验收-视频理解.md#vi4外部-harness-多模态构建输入)及 [verification.json](../evals/video/results/vi4-20261010/verification.json)。

`packages/core/test/video-executor-media.test.ts` 覆盖原生字节与顺序、文本无 Base64、缺最终确认拒绝、重连重放、候选接纳、输入改绑和媒体预算。原片复现：`node --import tsx evals/video/vi4-executor.ts live <Codex 可执行文件> [模型名]`。脚本创建隔离的原转写摘录任务，使用 VI0c 的 650 秒整帧及同帧代码裁剪，经正式四工具会话生成并接纳候选。

## 已知限制

- DSH 真模型/图片承载按既有确认待配置；当前显式拒绝媒体任务。读时真模型另沿 VI5 待验。
- 纯视觉图谱节点、联合证据字段及其持久化属于 VI13；VI4 的联合输入仍使用既有阶段的文本候选合同。
- Linux 和安装态媒体路径在 VI18 验收；本次原片运行使用源代码 MCP。预留图像 tokens 不代表宿主实际计费或硬上限。
