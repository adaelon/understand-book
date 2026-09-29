# DSH 预构建兼容与实施记录

更新：2026-09-26。实施输入：切片方案 DH0–DH5，ADR-0138。

## 基线

- Understand Book HEAD：`c2ff3e1`。既有工作树修改保留。
- DSH 源码：`477b4f420553e8a52c2fbccc464d7561b239c443`，包版本 `0.1.7-rc.2`。
- 原桌面封装：`E:/DSH Desktop`，第三方 DSH Desktop `2.0.11`；旧 profile runtime `0.1.2-alpha.1`。
- 官方 Windows x64 `0.1.7-rc.2` 已安装在 `E:/DeepSeekHarness`；探针由该目录的 Electron 执行，并将全部 `@deepseek-ai/*` 导入解析到安装包内的 `resources/app.asar/dsh`。
- 2026-09-26 真实模型探针：`deepseek-official / deepseek-v4-pro / reasoningEffort=off / maxTokens=4096`。4 次 Native 工具调用、69 字节合成材料逐字回传通过；约 7.7 秒。使用宿主官方 Provider 和凭据服务，插件实现没有读取密钥。

## 切片进度与交付判据

- [ ] DH0：最小插件、真实宿主探针与兼容报告；安装态、模型往返和问答证据分别记录。
- [x] DH1：中立合同、公共/宿主 doctor、版本矩阵与旧 Codex golden。
- [x] DH2a：现有 Codex 配置显式传递，旧行为不变。
- [x] DH2b：新 DSH 公共执行配置、记录与全链路容量路由。
- [x] DH3：可信 child、独占 stdio、四工具与隔离验收（L1）。
- [x] DH4：单执行器标准阶段闭包、确认关联、让出与恢复（L1）。
- [x] DH5：2/3 并发补位、故障/取消、Engine 所有权与受控接手；[验收报告](dsh-dh5-20260926.md)。

## 当前结论

DH1–DH4 的源码实现与确定性闭环完成。官方 DSH 安装态依赖加真实源码 Engine stdio 已将技术书标准计划执行至 Engine DONE，覆盖分次继续、输入期硬取消恢复与候选字段修正。DH0 剩一次人工“要求修改”；DH5 确定性验收已完成，DH6 尚未验收，不作首版发布支持声明。最新实现与验收汇总见本文末节；以下早期证据按实施阶段保留。

## 本次已取得的证据

- L1 安装态实际 standard preset：`test/standard-preset.probe.mjs` 从已安装的 base、web、standard patch 启动独立测试 home。标准 composition 的 compaction/pruner 已激活，父工具模式为 both；child 切至专用空 preset 后，连续 5 次模型请求恰好四工具，12518 字节合成输入仍可见，父 preset 保持 standard，父历史无 child final 哨兵，清理通过。此处使用脚本化模型。
- L2 最小真实模型往返：`test/live-model.probe.mjs` 通过。该探针只证明四工具和小输入往返，不是标准计划完成或最大容量证明。
- DH1 合同拆分：四工具 schema/参数白名单归 `build-executor-tool-contract.ts`，Codex 注册事实归 `codex-build-executor-contract.ts`，V3 ref/phase/ordinal 归 `build-executor-connection-state.ts`。旧导出入口保留，旧响应与记录版本未改变。
- DH1 公共 doctor：`automaticBuildEngineContract` 不读取 Codex TOML/MCP 资产；`automaticBuildProtocolContract` 组合公共检查与 Codex 边界检查。实际文件夹夹具验证：无 Codex 资产时公共检查通过且 Codex 分支失败，删除公共 prompt 后二者均失败。
- 拆分前的 35 项旧回归通过；schema/连接拆分后同样 35 项通过；公共 doctor 拆分后新增文件夹测试与 14 项旧 doctor 回归通过。
- 安装态确定性测试 8/8 通过；新增精确四工具 schema 转发/多字段拒绝和创建期间 dispose 等待测试。后者先失败再修复：关闭 host 会取消在途创建并等候其清理。Core 与 DSH 插件类型检查通过。
- 扩展 Codex 回归 7 文件、114 项断言通过，但 Vitest worker 的 `onTaskUpdate` 通信超时导致整次进程退出 1；单 worker、文件串行复验仍然 114 项通过并报同一错误，因此不能归因为文件并行，也不能记作全绿。该 runner 的 RPC 默认超时为 60 秒，原因仍待定位；未修改测试断言或放宽超时。
- 人工审批初次探针收到两次真实「批准」。第一题批准通过，第二题预期「拒绝」失败。固定版本 UI 的「要求修改」以 `ASK_CANCELLED` 拒绝等待，并不会返回「拒绝」选项；探针已按此实际合同修正，新增逐题落盘以保留部分证据。取消路径待补验。
- 取消单题补验运行未取得实际回答，9 分钟后 `ASK_ABORTED` 并清理退出；浏览器会话定位未成功，因此不归因为业务取消失败，也不记为用户拒绝。测试页已关闭。
- 官方模型元数据探针声明 `deepseek-v4-pro` 的 contextWindow 为 1000000、defaultMaxTokens 为 256000；这是 Provider 元数据，不是实测容量。当前真实模型探针显式冻结 maxTokens=4096。
- 后续定位：构建驱动测试在用例之间增加一次 `setImmediate`，让同步密集工作之间处理 runner IPC 回执；断言与超时不变。50 项驱动回归正常退出 0，未再出现 `onTaskUpdate` 超时。
- 新生命周期负例通过：两个重叠创建中一个失败，成功 sibling 独立完成并清理。binding 现为 4 项通过。
- 正式 standard 探针补验 42111 字节候选工具参数（嵌套 JSON、引号、反斜杠、CRLF），逐字比对通过；连续请求仍保留 12518 字节历史输入。
- `build-execution-profile.ts` 发布固定身份 `codex_mcp_v3@1` 与 `dsh_native_v4@1`，容量由 registry 所有。DSH 保留 8192 字节/2048 估算 token 单结果、32768 字节/2048 token 候选请求；每批 1 chunk、16384 MCP 序列化字节、最多 64 批。身份可解析不等于当前 Engine 已启用该宿主。
- `executor_transport_profile.v3` 明确绑定 `dsh_native_mcp` 与 Session V4；旧 V2 decoder 拒绝 V3。传输/容量/registry 13 项回归通过。
- invocation create/record V2 的显式 Codex 配置写入与续跑通过，V1 新字段和 V1/V2 混合记录被拒绝；DSH 在全链路未接通时拒绝创建，无静默回落。
- 公共/私有 delivery record V4 decoder、input manifest V4 decoder 与连接 Session V4 已实现封闭字段及 profile 匹配；旧 V3 入口明确拒绝新版本。版本边界 3 项通过，初次私有夹具使用了非支持 artifact_type，改成既有 `concept_map` 后通过。

- 两个重测试文件让出事件循环后，12 文件 175 项首跑 173 通过、2 项并行负载超时；仅串行重跑失败项均通过，RPC 超时不再出现。后续采用有限并行。
- 安装态确定性测试现为 11/11；新增累计历史越界与 provider/model 配置漂移测试，均在下一次 provider 请求前终止，child 清理完成。
- DH2a 已将显式 Codex 配置接入 driver → plan/snapshot → preflight → dispatch/lease，session packing/batch/candidate、BookStructure unit/fragment/reduce/stitch/关系分析、generation task、迁移与质量检查。旧 wire/落盘版本保持；旧入口在边界选择 Codex registry。
- DH2a 本轮相关回归 187/187：预算/分发/lease 30；BookStructure 路由/完整发布、orchestrator/dispatch runtime 48；driver/session/policy/quality 109。Core typecheck 通过。容量收紧的单测证明预检与分发实际使用传入配置；显式原配置与旧入口输出相等。
- 2026-09-26 再次进入安装态待审会话，完整计划预览实际失败：`conversation.plan-review.actions` 报 `sidebarRight: no session surface is mounted`，卡片只有标题与首段。安装包包含完整预览代码，故不是缺组件。DSH 源码修复在途；不能将该 UI 记通过。
- 上述取消单题没有收到人类回答，等待 540 秒后 `ASK_ABORTED`、进程退出 1；本次没有新的取消通过证据，无活跃问答。

## 早期限制记录（最新边界见末节）

- 初始 `harness.ts` 的 standard 是空测试 preset，其通过结果不能单独证明正式 standard 组合；真实 standard 的证据使用上面的独立安装探针。
- standard 探针首次断言错误地从父 Agent 读取隔离组服务；改为核对真实 composition 激活状态。随后因测试根缺少 `meta.cwd` 导致 persona 模板失败，补齐正常会话工作目录后通过。
- `questions.test.ts` 是真实问答服务加脚本化 answerer；实际人类批准已收到，完整计划阅读与「要求修改」取消路径尚未全部验收。
- 尚未启用 DSH Engine 执行入口。确认引用、运行时冻结模型预算、handoff 新记录/协商入口与执行配置全链路仍待实现；不能将 decoder 测试解释成 DSH 完整构建通过。

## DH2a 旧常量保留范围

`build-execution-profile.ts` 保留 Codex 原始常量作为发布配置；`executor-transport.ts` 保留协议定义。`codex-build-doctor.ts` 与 `skills/build/build-executor-mcp.ts` 当前只服务旧 Codex bootstrap/session，继续校验旧 profile 和原始批次上限。`automatic-build-executor-session.ts` 的旧 stdin 上限及 V3 record 类型属于旧版本入口；内部 packing、batch 与 candidate 校验已接收显式配置。`book-structure.ts` 与 relation-routing 的导出旧 API 保留缺省 Codex，生产调用均传入配置。没有扩大旧 V1/V3 记录 decoder 的接受集合；DSH 需要新入口和新记录，留给 DH2b。

## DH0 计划全文显示修复

DSH 官方安装态在切到待审会话时，`conversation.plan-review.actions` 的 passive effect 早于 SidebarRight 的重新绑定运行；浏览器日志实际报 `sidebarRight: no session surface is mounted`，全文链接与预览均未显示。DSH 仓库 `packages/client/ui-sidebar-right/src/client/shell/SidebarRight.tsx` 将公共导航绑定移到 layout effect。新增 session 切换回归在原实现上产生同一错误，修复后通过；侧栏 seat/service 与 plan-preview 共 109 项通过。中英文 README/一致性记录已同步，命名文档配对检查及 diff whitespace 检查通过。

组件 bundle 构建成功。安装态 loader 仅在显式设置 `UNDERSTAND_BOOK_DSH_SIDEBAR_PATCH` 时以本地构建的 SidebarRight 替换该包，其他 DSH 包仍使用官方安装；没有覆盖 app.asar。实际浏览器验证显示 Pass2、预算、阶段和 `DH0-PLAN-END` 全文尾标记，并且没有 console error。此项通过的组合明确为 `0.1.7-rc.2 + 本地 SidebarRight 修复`，不覆盖原官方包失败记录。人工要求修改仍待实际回答。

DSH Host 全量构建的 TypeScript 阶段生成了所需 remote 类型，后续批量 bundle 因 settings 旧生成文件引用缺失导出而失败；没有修改该无关模块。SidebarRight 定向类型构建与组件 bundle 均通过。修复组合的取消单题仍未取得人类回答，540 秒后 `ASK_ABORTED`；测试进程已退出，测试页已关闭。

## DSH 模型配置快照（DH1 在途）

新增封闭的 `build_executor_model_runtime.v1`，包含 provider/model、reasoning、输出上限、上下文窗口、安全余量与 Provider 元数据来源。DSH 解析器要求明确输出上限，读取宿主精确模型资料并物化默认 reasoning；没有上下文资料时拒绝。Host 在异步创建前复制冻结该快照，自动给每个 child 安装累计上下文/路由检查，连续启动也从同一快照派生参数。此处不改变 Codex 入口。

Core 快照和 profile 测试 5/5；安装态测试 15/15，含宿主元数据探针与原 11 项回归，插件类型检查通过。官方模型探针输出 `deepseek-official/deepseek-v4-pro/off`、输出 4096、窗口 1000000、余量 4096；没有调用模型，也不把元数据当压力测试结果。快照可 JSON 往返并已用于子代理执行；Engine invocation 的持久关联及人类确认关联仍待完成。

快照强制接入后的实际 standard 组合探针再次通过（12518 字节输入、42111 字节候选、5 请求四工具），Core 类型检查通过。

## 2026-09-26 DH1–DH4 完成记录

### 实现

`index.ts` 正式 Cordis 源码入口注册 `ub_build_prepare_and_confirm` 与 `ub_build_run`。准备复用 Engine 标准计划编译，问答只显示目标、阶段闭包、生成/复用/排除、预算与模型等控制信息；不显示包含 generation inputs 的 snapshot。问答由真实根调用，只接受准确问题的一项批准且无 `custom` 修改文字。

DSH V2 invocation 保存 `dsh_build_confirmation.v1` 与 `build_executor_model_runtime.v1`。确认关联 plan id/revision/digest、根会话、问题和回答；Engine 创建前重读计划与源指纹。续跑校验原根身份，读取持久模型快照，不继承父之后切换的模型。标准计划编译的 confirmed 状态不能替代真实回答。

DSH 实际发出公共 handoff V5、manifest/delivery/Session V4、transport V3、generation-start V3。初始化要求 MCP `2025-06-18` 和精确 `dsh_build_executor_contract.v1`；未初始化不能 list/call。旧入口拒绝 DSH 新记录。私有 V4 decoder 保留，DSH 私有创建与恢复仍拒绝。完整矩阵已回填 ADR §6。

执行配置由 invocation 沿 plan/snapshot、preflight、descriptor/dispatch/lease、session 分片/批次/candidate、BookStructure 各路由传递到 writer/quality/publication。旧入口仍固定 Codex。每个 child 的累计模型请求核对冻结路由并计量角色、工具与历史，保留输出和安全余量。

每个 handoff 新建原生 child、专用空 preset、Native 四工具与独占 stdio 进程。控制与 executor 使用相同 registry root；Engine 启动采用 argv 数组、`shell:false`、必要环境变量，不传模型凭据。每连接串行请求，不自动重连/重试副作用。正常让出等待资源清理后返回原 invocation；硬取消传播 signal，保留原引用。NEEDS_USER 在无 owned child 时重读并映射真实 choice；完成只认 Engine DONE。

### 实际问题与修复

1. 自由文字字段是 `custom`，原逻辑错误接受“批准并附修改意见”；red/green 测试后改为零 invocation。
2. 准备输出原含完整 snapshot，会将生成输入送入根问答；改为 Engine 控制投影，测试核对问题与根结果没有合成正文。
3. 批准期间源变化原会创建 invocation；新增源身份复核，red/green 测试证明在持久创建前拒绝。
4. 单任务失败后待办列表不变，驱动复用失败前分发时钟，误报 executor_unavailable。转换状态现纳入已有失败次数，无失败旧键保持；真实 writer 字段错误后收到 retry_feedback、修正并 DONE。

### 本轮测试

| 检查 | 结果 | 日志/范围 |
|---|---|---|
| DSH session/profile/version/plan | 11/11 | `tmp/dh0/core-final.log`；显式协商、V5/V4、重放恢复、技术书/论文闭包、零预算、私有拒绝。 |
| 旧 driver | 51/51 | `tmp/dh0/driver-final.log`；转换状态修复后整文件通过，含旧私有路径。 |
| 旧 session + model budget | 56/56 | `tmp/dh0/codex-tests.log`；同次 driver 仅旧拒绝文案预期失败，现由最终 51/51 覆盖。 |
| MCP + 版本边界 + DSH session | 18/18 | `tmp/dh0/session-tests.log`，含原 MCP 13 项。 |
| 确认负例 | 7/7 | `tmp/dh0/control-final.log`；mixed、decline、empty、no-answerer、cancel、dispose、source-drift 均零 invocation/模型。 |
| Pass2 关闭完整闭环 | 通过，4 child | `tmp/dh0/control-final.log`；Pass1/profile_sidecar/BookStructure 经质量与发布到 DONE。 |
| Pass2 开启、取消与修正 | 通过，6 child | `tmp/dh0/retry-final.log`；输入期取消后原引用恢复，structure key-stop 类型错误后下一轮修正。 |
| 正式入口/standard/身份/模型边界/schema | 9/9 | `tmp/dh0/plugin-final.log`；Cordis 根工具真实小预算边界，官方 standard 的 Native、隔离、清理。 |
| 模型解析与既有插件回归 | 16/16 | `tmp/dh0/plugin-regression.log`，含 6 项确认旧集、runtime 3、binding/model/schema。 |
| Core / 插件 typecheck | 均退出 0 | `tmp/dh0/typecheck-final.log`、`tmp/dh0/plugin-typecheck-final.log`。 |

完整闭环使用官方 DSH 安装包依赖、脚本模型与真实源码 Engine stdio。样例只有一个 claim，Pass2 开启时没有可用跨段配对，由 Engine 关闭无模型任务阶段；未要求或伪造 Pass2 模型调用。开启用例的首次失败保留于 `control-final.log`，修复后重跑失败项见 `retry-final.log`。

本轮再次运行实际 standard 探针通过：父 both、compaction/pruner 激活，child 连续 5 次恰好四工具，12518 字节输入与 42111 字节候选逐字保留，父无 final 哨兵。42111 字节是宿主能力证据，不是提高 Engine candidate 上限的依据。

### 已知限制与下一步

- 本轮人工“要求修改”仍未收到操作，540 秒后 `ASK_ABORTED`，测试进程已退出；日志 `tmp/dh0/human-confirmation-current.log`。未将超时记作取消通过，未替人点击。当前无活跃人工问答。
- 计划全文通过的组合仍是官方 `0.1.7-rc.2 + 本地 SidebarRight 修复`；原官方包的全文入口问题保持明确记录。
- 论文已验证标准阶段闭包、确认和零预算门，完整 DONE 证据来自技术书。
- DH5 已补齐并发、多故障、提交断连及受控接手，最新实现和证据见末节。
- DH6 独立分发、脱离源码目录与真实模型完整构建/恢复尚未验收。此前 L2 小型往返不替代完整构建。
- 正文/候选可能保留在 DSH child 日志中；父上下文隔离不等于宿主日志不落盘。CLI/自定义 composition 的全局裁剪策略须单独验证。

后续从 [Harness checkpoint](../../SESSION_CHECKPOINT_DSH.md) 继续。插件配置和使用流程见 [DSH README](../../packages/dsh-plugin/README.md)。


## 2026-09-26 DH5 完成记录

DSH 标准构建现支持确认后的 1–3 并发，按单个 child 终态补位；Engine 保存前台 controller 所有权与逐 launch 的有界观察。NEEDS_USER 收尾后重新读取，取消/卸载等候全部资源清理。已接纳后丢响应或 child 崩溃均从原 invocation 复用成果。跨 invocation 的实际活动 lease 现在阻止 DSH 重放外部 owner 工作，停止原 owner 后新 invocation 可受控接手。

完整实现、红测修复、分层证据和限制见 [DH5 验收报告](dsh-dh5-20260926.md)。当前交接入口为 [Harness checkpoint](../../SESSION_CHECKPOINT_DSH.md)。DH0 人工要求修改与 DH6 正式安装/真实模型验收继续保持未完成。
