# Understand Book DSH 构建适配器

在 DeepSeek Harness 中确认并执行 Understand Book 的标准预构建计划。DSH 提供模型和原生 child；Build Engine 负责计划、预算、候选接纳、质量门和发布。

当前提供 Windows x64 预发布包 `0.1.0-rc.2`，固定宿主为官方 DSH Desktop `0.1.7-rc.2`。插件与 Engine 分开交付，运行时不需要源码仓库或全局 Node/Bun/Cargo。真实模型完整构建与语义质量验收仍属于 DH6 后续工作。

## Windows Desktop 安装

发布目录包含插件 `understand-book-dsh-plugin-0.1.0-rc.2.tgz`、`engine/understand-book-build.exe`、相邻的 `engine/resources/`、本说明和 `release.json`。整个 `engine` 目录可保存在任意固定位置，支持中文和空格；升级时保留原 Engine 和 registry。音乐构建需要随附的可替换谱面库与许可资料，部署时保留整个目录。

1. 配置 Engine：已安装的兼容 Reader 可通过 `HKCU\Software\UnderstandBook\InstallDir` 自动发现；独立 Engine 可通过环境变量 `UNDERSTAND_BOOK_DSH_ENGINE` 指定完整 exe 路径，然后启动 Desktop。也可以在 Desktop 的插件配置中设置 `executable`。
2. 打开 Desktop 的「插件 → 添加插件」，输入 tarball 的完整路径，点击「安装」，安装成功后点击「立即启用」。插件详情应显示 `understand-book-build` 正在运行。
3. 插件启动先验证控制协议和 executor MCP 协议。`build_engine_missing` 表示路径缺失；`build_engine_incompatible` 表示无法协商本包所需的控制/执行协议。修正 Engine 后重新启用。该验证不创建构建 invocation。
4. 升级时通过同一插件页安装新版本 tarball，再完整退出并重新打开 Desktop，使宿主加载新模块。通过插件详情可独立禁用或卸载；书籍工作区、构建记录和已接纳产物保留。

例如在 PowerShell 中启动独立 Engine 组合：

```powershell
$env:UNDERSTAND_BOOK_DSH_ENGINE = 'E:/Understand Book/engine/understand-book-build.exe'
$env:UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = 'E:/Understand Book/build-registry'
& 'E:/DeepSeekHarness/DeepSeek Harness.exe'
```

首次选择 registry 后保持它不变，控制调用与全部 executor 连接共用该目录；省略时沿用 Engine 默认临时目录下的 `understand-book-automatic-build-driver-v1`。改变目录不会迁移已有 invocation。

Windows 上宿主自带 pnpm 11.7.0 可能在共享 store 的 `projects` 目录创建符号链接时出现 `EPERM`。若安装详情明确是这一错误，可在当前 Desktop profile 的 `pnpm-workspace.yaml` 中添加 `storeDir: .pnpm-store`，保留其余内容，再从插件页重试。验收使用该设置；无需提升权限或更换宿主包管理器。

## 入口与配置

Cordis 发布入口为 `dist/index.js`，导出 `name`、`inject`、`Config`、`apply`；bundle 入口为 `cordis.patch.yml`。角色与四工具合同编入同一入口，所有 DSH 包保留为宿主提供的 peer。配置：

| 配置 | 含义 |
|---|---|
| `executable` | Build Engine 的绝对路径；空值依次使用环境变量、Reader 安装位置。 |
| `driverRoot` | 驱动记录目录绝对路径；空值依次使用环境变量、Engine 默认目录。 |
| `maxOutputTokens` | 每个 child 的明确输出上限，例如本次探针使用 4096。 |
| `safetyMarginTokens` | 模型上下文安全余量，例如本次探针使用 4096。 |
| `handoffsPerCall` | 一次前台调用的 handoff 启动上限；达到后等待本次 owned child 全部清理，再返回原 invocation。 |

模型 provider、model、reasoning 与上下文窗口来自宿主当前实际模型资料。批准时保存配置，后续执行读取同一快照。`prefixArgs` 仅供源码测试启动测试 Engine，不是模型可指定的命令参数。

## 使用流程

1. 根会话调用 `ub_build_prepare_and_confirm`，指定 `target_input`、`root_dir`；`pass2` 可省略，默认 `disabled`，明确选择 `enabled` 才纳入跨段关系复核。可附计划预算与 `max_parallel`（1–3，默认 1）。
2. 完整计划显示材料类型、阶段闭包、生成/复用/排除项、预算与模型配置。明确批准后返回持久 `invocation_ref`。此调用不启动抽取 child。
3. 调用 `ub_build_run`，只提交上述引用。使用批准时保存的并发上限，最多同时运行三个 child。
4. 正常让出后继续调用同一引用。取消后资源清理完成，仍保留同一引用供恢复。遇到 Engine 用户边界时由根问答服务请求决定。
5. 只有 Engine `DONE` 代表计划完成。child final 不作为完成凭据。

拒绝、空回答、批准附带修改文字、无 answerer 或取消都不创建 invocation；批准后计划身份或源内容改变也会拒绝创建。运行引用与批准它的根会话绑定。首版仅接受标准公共计划，私有产物和自然语言目标计划在启动前拒绝。

预算示例：`{"on_exceed":"needs_user","max_total_tokens":100000,"max_wall_clock_minutes":60}`。省略预算时沿用 Engine 标准计划规则；超预算不会自动扩大授权。

源码的 SR5 已增加可选检索配置（既有发布包尚未更新）。在宿主启动前设置
`UNDERSTAND_BOOK_EMBEDDING_CONFIG` 为本地 MiniLM 安装配置文件的绝对路径，然后给
`ub_build_prepare_and_confirm` 增加例如
`"retrieval":{"retrieval_mode":"semantic_required","budget":{"max_documents":12,"max_queries":4,"max_calls":12}}`。
确认框会展示实际模型、local 数据去向、一跳对象投影/query 范围、未知工作量与明确额度；批准后才执行。
失败和重试均计入额度。`lexical_only` 无需模型配置，可将这些额度设为 0；默认仍使用词法路径。
安装文件格式、当前本地 adapter 和 SR6 验收限制见 [SR5 说明](../../docs/performance/semantic-retrieval-sr5-20261001.md)。

## 并发与恢复

### 运营后台任务

源码中的 B1 将运营后台「运营资料构建」接到同一 Build Engine。宿主启动前设置
`UNDERSTAND_BOOK_OPERATOR_BUILD_CONFIG` 为本机 JSON 配置的绝对路径，内容为：

```json
{"rootSessionId":"运营根 Agent 的原 id","connection":{"base_url":"https://reader.example","origin":"https://reader.example","cookie":"运营登录 Cookie","csrf_token":"该登录的 CSRF token"}}
```

配置和凭据仅由运行中的运营宿主读取，不写进计划、invocation 或公开发布。根 Agent 必须已存在并具备所选模型；恢复时保持原根 Agent id 和 driverRoot。插件启用后串行领取任务，无可用根 Agent 时保留排队状态。服务和宿主需访问相同的资料及成果路径。

后台先选择已有确切成果、缺失资料、平台预算与成果授权账号，点击「准备构建计划」。宿主沿原引擎准备实际计划、模型快照及复用信息，不调用抽取模型。运营者在后台检查并明确确认计划后启动；缺失资料的各 invocation 共用一个运营父任务预算与 Q3 回执，已存在的公开成果不生成模型调用。暂停会收尾当前执行，继续沿原 invocation 重读已接纳单元；提高预算须在「继续构建」中明确确认。

新视频任务可在填写预算后直接点击「创建并开始构建」；已保存但未启动的任务可从详情继续启动。运营配置增加 `video`：

```json
{"codexExecutable":"E:/tools/codex.exe","publisherExecutable":"E:/UnderstandBook/publish_book.exe","publicationServiceRoot":"E:/build-data/video-staging","pythonExecutable":"E:/video-python/python.exe","whisperModelDir":"E:/models/small.en"}
```

`video` 是上方运营配置的一个字段；所有路径均为本机绝对路径。`publicationServiceRoot` 可省略，默认使用 `driverRoot/video-publications/<task_id>`。它是构建专用的暂存发布目录，不能指向正在运行的 Reader 服务目录；视频语义执行需要阶段性不可变证据，最终成果由 Reader 服务自己的导入接口接纳。FFmpeg/FFprobe 默认按现有安装规则定位，也可在 `video` 中指定 `ffmpegExecutable`、`ffprobeExecutable`。

视频在已安装 Build Engine 内运行原有媒体执行链路。宿主把临时费用连接通过进程输入交给 Engine，收到进度后先保存到后台再确认推进；取消同样通过进程输入传递，等待当前执行器收尾和费用回执。进度来自实际复制字节、转写分段、解码位置和已接纳候选；阶段未知总量时不显示伪造百分比。默认生成转写、双语字幕、画面索引、图谱及原讲解结构，不生成额外 Tutor 教学材料。书籍的默认准备计划也不生成额外的正式教学对象、认知材料和教学发布，仍保留原文理解与全书结构；明确设置 `build_tutor:true` 才纳入额外教学阶段。

只有原引擎返回 DONE，宿主才请求服务使用既有发布导入器接纳成果；随后后台选择本任务的确切成果、填写公共编排并发布共享工作区。费用观察仍是部分或未知，完整供应商费用缺失时持续显示待核算。原引擎遇到需要额外决定的边界时，后台显示等待决定的原因；运营者在已连接的 DSH 根会话原问答中选择后继续。换用新的根 Agent id 不会静默接管旧 invocation。

容量取已确认并发、当前 owned child、宿主实时 Agent 注册表和在途创建的交集。固定宿主已验证三并发；原生 spawn 没有实时配额 API，因此按所有当前子代理占用保守扣减，返回 `agent_registry_conservative` 来源。先完成的 child 清理后立即推进 Engine，正常让出和用户边界会收尾已有 child。

同一实例中的重叠继续请求返回 `build_already_running`；另一插件实例或进程遇到 Engine 保存的活动 controller 返回 `build_owner_active`。Engine 只有证明原宿主进程已退出才允许重新取得控制权；无法证明时保持阻塞。不同 invocation 的工作区竞争继续使用已有 dispatch/lease 所有权门。

每次启动的有界观察保存在 Engine registry 的 `dsh-observations/<invocation_ref>/<launch_id>.json`。bootstrap 零调用、精确 open 拒绝、连接中断、运行时故障、候选回执和取消分别保存；没有调用证据时为 `null`。已提交但响应丢失时继续原引用，由 Engine 重读持久成果；未知故障不会自动变成语义重试。跨宿主接手需停止旧 owner，重新评估并确认新 invocation，已有有效产物继续复用。

### 精选音乐预构建

后台的「精选音乐」使用同一运营队列、预算、暂停继续、在线导入和共享发布。来源位置填写 `music_curation.v1` 清单的绝对路径；清单内音频、MusicXML 和对齐文件可相对清单目录定位。示例见 `evals/music/schubert-curation.json`。真实录音、谱本、素材许可、演奏出现、校准覆盖和有来源的音乐事件分别保存，纯音乐不创建语音转写。首期接纳已审核的精选标注与本地计算结果，不启动音乐理解模型。

音乐任务不需要配置 Codex、Whisper 或视频暂存发布器。FFmpeg / FFprobe 按现有媒体工具规则读取；已有运营 `video.ffmpegExecutable`、`video.ffprobeExecutable` 也可作为明确的媒体工具路径。任务保留统一平台预算，本地导入、谱面渲染、接纳标注不会创建供应商模型调用回执；用户实际伴听时的模型费用继续走既有运行时计费。

`node apps/desktop/scripts/build-sidecar.mjs` 同时准备 Engine 相邻 `resources/music/`。Verovio 6.3.0 的 ESM/WASM 模块以可替换文件分发，附原始许可、确切对应源码及构建/替换说明；`resources/music/NOTICE.txt` 是谱面使用的 Leipzig 字体版权与 OFL 许可。部署时保留整个目录，不能只复制 `.exe`。首次构建会从官方来源取得固定版本源码与许可并缓存到 `target/dependencies/verovio-6.3.0`，后续构建复用缓存。

本地音乐校准工作台通过同一 Engine 的 `music-calibrate --book-dir <workspace> --revision-file <revision.json>` 保存修订，保留前后记录、已有来源和候选状态；已发布目录禁止原地校准。该本地命令不创建新队列或模型费用，校准后的共享版本仍沿原发布流程生成。

每份资料默认要求「音乐播放、可查乐谱、音乐事件」；逐音符跟谱是独立能力，并按发布中的覆盖区间说明范围。准备完成后服务端再次检查原始资产、素材分发权及版本绑定，再授予选定账号并发布共享工作区。对齐未接纳的范围不变成已完成能力，缺少所需能力时保留已有成果，中断任务等待补齐清单后继续。

### 个人资料准备（B2）

同一宿主也领取Reader工作区发起的个人准备任务，沿上述连接、根会话和共享目录运行。Reader支持EPUB、Markdown、文本和MP4上传；书籍先准备实际计划，用户确认百分比上限与当前额度期后才执行。原始PDF仍需既有工作台完成来源对齐。默认关闭Pass2和额外Tutor材料，完成后自动加入发起工作区；暂停、宿主恢复及切换工作区不改变任务归属。

个人书籍启用`ExternalBuildCostBinding.metering: provider_calls_v1`，每次实际`llm/stream`发送前经服务端准入，结束后提交用量。Reader服务的`model-rates.json`须配置与DSH实际provider路由名、model相匹配且有效的费率；服务端冻结费率，未知用量继续待核算。该模式必须配套本轮插件与Engine，旧DSH粗粒度及Codex执行能力不会因此自动支持个人扣费。费率字段沿现有服务配置合同，本插件不提供商业默认值。

个人MP4使用`personal_source_only`，配置仍沿上面的`video`字段，实际只执行原视频导入、本地Whisper和原文发布；不启动Codex语义执行。原文就绪即可在工作区播放、问答、按需取帧和原声复核。完整翻译、视觉索引、图谱和原讲解结构由运营预构建及共享成果提供。实际HTTP、计量及恢复证据见[实施记录](../../docs/个人资料准备与跨资料媒体-20261018.md)。

## 数据与生命周期

每个 handoff 使用新的原生 child、专用空 preset、Native 四工具及独占 Engine stdio 进程。四工具参数来自 Core 单源合同。Engine 进程不接收模型凭据。每个 child 的累计模型请求都核对冻结路由与上下文容量；超限时停止，不裁剪正文。

根结果只包含控制投影，候选、分片和字段修正反馈留在 child 与 Engine 路径。正文及候选经所选模型路由处理，并可能保留在 DSH child 会话日志中；日志保留由宿主管理。

## 验证与限制

维护者构建与独立发布验证：

```powershell
node scripts/build-dsh-prebuild-release.mjs 'E:/UB Release'
node scripts/verify-dsh-prebuild-release.mjs 'E:/UB Release' 'E:/DeepSeekHarness' 'E:/Previous Engine/understand-book-build.exe'
```

构建机需要 Bun 与 npm，必要时通过 `BUN_BINARY`、`NPM_CLI_JS` 指定。验证器解包到源码目录之外，使用宿主安装版与编译 Engine，并把 PATH 限制为 Windows 系统目录；验证缺失/旧 Engine 拒绝、原生 child 取消、插件卸载与重新启用。构建工具不会随插件分发。CLI、其他平台及其他 DSH 版本保持未验证。

安装态测试入口：

```powershell
node packages/dsh-plugin/test/run-installed.mjs E:/DeepSeekHarness test/control-integration.test.ts test/control-confirmation.test.ts
node packages/dsh-plugin/test/run-installed.mjs E:/DeepSeekHarness test/plugin-entry.test.ts test/standard-preset.probe.mjs
```

这些测试使用合成材料、脚本模型与真实 Engine 源码 stdio；不会构建用户书籍。Pass2 开/关的技术书闭环已通过 writer、quality 和 publication。开启样例包含硬取消恢复与一次候选修正；单 claim 没有可用跨段配对，Pass2 由 Engine 判定无模型任务后关闭。论文已验证计划闭包与预算边界，未宣称论文完整模型构建通过。

官方 `0.1.7-rc.2` 的计划全文入口存在已定位的 SidebarRight 绑定问题。全文展示通过的组合是官方包加本地 SidebarRight 修复；实际人工批准已取得，“要求修改”人工操作仍待验收。原始材料若使用纯中文文件名，现有 Engine 不能自动派生 ASCII bookId；应先按现有流程明确 bookId 并准备有效工作区。首次稳定发布还须完成 DH0 的人工要求修改补验与 DH6 的真实模型完整构建。

源码仓库中的详细证据：`docs/performance/dsh-prebuild-compatibility-20260925.md`、`docs/performance/dsh-dh6-install-20260926.md`。
