# VI0c 真实图片通道实验

固定输入为 VI0a 的 V01、V02、V04 原始 PNG；局部裁剪只截取代码，保留原像素、原帧身份、实际 PTS 和原图归一化坐标。九项实验覆盖单帧换图、两张裁剪、正反两帧、四帧渐进说明、两帧矩阵和八帧容量。模型问题与图片标签不包含文字答案。

## 重跑

在仓库根目录使用现有 VI0a 媒体；缺失时先 `node evals/video/media.mjs replay`。需要 Node、FFmpeg、项目依赖；Runtime 记录另外需要 Rust 工具链。

```powershell
. .\.worktree-local\environment.ps1
node evals/video/image-experiment.mjs prepare
node --test evals/video/image-experiment.test.mjs
node --import tsx evals/video/image-capacity.ts
node evals/video/image-codex.mjs $env:CODEX_CLI_PATH gpt-6-astra
node evals/video/image-runtime.mjs record
node evals/video/image-report.mjs
```

`image-codex.mjs` 的第一个参数是 Codex 可执行文件或 CLI JS 入口，第二个参数为明确的模型名；后面可选择 `single_a single_b crop_a crop_b pair reverse progressive matrix capacity8`。使用当前登录凭据，忽略用户配置文件，只添加独立实验 MCP。只读工具返回顶层 `type: image` 内容块。模型每项单独运行、只调一次图片工具。原始工具返回、调用事件、原文回答、用量及错误均在 `generated/vi0c/codex/<case>/`。

重跑会覆盖选定 case 的实验输出；先复制需要保留的历史结果。脚本先清除该 case 旧回答和图片调用记录，防止失败重跑误用旧成功凭据。`prepare` 覆盖裁剪与输入清单，保持原片不变。

单次实验的墙钟预算为 300 秒，超时保存失败并继续下一项；网络重试时间包含在墙钟内。超时没有原始最终响应时，用量保留未知。通过后可运行 `node evals/video/image-report.mjs --archive` 保存本次结果快照。

## 读时通道

`image-runtime.mjs record` 通过 `video_image_probe.rs` 调用真实 `AgentRequestPlan → ProviderRegistry → chat_observed → append_preview_images`，把完整 HTTP 请求发到本地记录器。记录器只返回明确标注的占位文本；这证明请求投影及字节传递，不证明模型理解图片。

已有读时配置时，按项目现有环境变量设置 `OPENCODE_API_KEY`、`OPENCODE_BASE_URL`、`FLUID_LLM_MODEL`，可选 `UNDERSTAND_BOOK_PROVIDER=native|react`，再运行：

```powershell
node evals/video/image-runtime.mjs live single_a single_b pair reverse progressive matrix
```

`live` 只在三项配置均存在时运行，结果写入 `generated/vi0c/runtime-live/`；不会由本地记录结果推导真模型通过。Rust observer 保存实际请求正文、原始 Provider 响应及用量，认证头不写入结果。当前报告汇总 Codex 与本地 Native 投影；live 结果另行对照图片验收后更新支持结论。

## 容量与验证

`image-capacity.ts` 直接调用当前 Core 的序列化及限额函数，比较原生图片结果与生产文本包装。记录 PNG 字节、Base64 字节、完整 MCP result、JSON-RPC 行、现有 Codex/DSH profile 的拒绝原因与 Codex 批次上限。它不把 Base64 字符串计作可见图片。

`image-report.mjs` 核对实际图片块与原 PNG 字节、顺序和真实调用成功事件，并逐项匹配 VI0a 的关键 dtype、断言、结果和矩阵签名；裁剪不应猜出区域外的结果。Runtime 核对 observer 请求与实际 HTTP body、图片顺序和 `detail: high`。任何缺失、失败或反向错误使报告失败。

只声明最大实际成功输入，不推定宿主硬上限。完整封套指 MCP JSON-RPC 行或 Runtime 模型请求 JSON；Codex 服务端模型 HTTP 封套及内部图像缩放不可见。Codex 用量包含宿主指令及工具定义，不等于纯图片 token 数。

Codex 的 `--json` 调用事件会将工具结果转为文本日志，大结果还会截断。报告用事件判断调用是否成功，用服务端另存的完整 `mcp-result.json` 核对真实顶层图片块；事件中的文本展示不作为图像字节来源，也不冒称完整模型 HTTP 请求。

## 已知问题

用户于 2026-10-10 选择“先完成 Codex，其余留待验证”；DSH 真模型和读时真模型继续待配置。VI0c 当时的生产构建 MCP 与 DSH 工具将值包装为文本；VI4 已完成 Codex 正式原生图片接线，DSH 当前显式声明纯文本。见 [VI4 说明](../../docs/视频外部构建图片输入-VI4.md)。实验不创建构建任务、书库发布或媒体观察账本。

全局 CLI 0.147.0 被服务端拒绝使用当前模型，实验改用本机桌面端的 CLI 0.162.0-alpha.2。初版实验工具缺少只读声明，被审批策略拒绝；补全 `readOnlyHint` 等声明后通过，未更改宿主审批策略。注解依据见 [OpenAI 官方 MCP 工具说明](https://developers.openai.com/plugins/guides/optimize-metadata)。
