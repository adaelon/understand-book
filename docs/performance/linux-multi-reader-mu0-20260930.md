# Linux 多人阅读 MU0 基线与 MU1a 验收

日期：2026-09-30。基线 HEAD：`5e105166573c50ce294f2fa7d01544884f229fce`。本记录针对 Windows 实时工作树；Linux 多人发布仍按 [切片方案](../切片方案-Linux原生多人阅读与无Redis首版.md) 后续门槛验收。

## 基线与实施顺序

- [x] 读取 checkpoint 冷启动合同、E01–E26 和实际入口。
- [x] 保存既有 [相关 diff](linux-multi-reader-mu0-20260930/pre-existing.patch) 与 [工作区状态](linux-multi-reader-mu0-20260930/worktree-status.txt)，保留全部既有修改。
- [x] 冻结路由、全局状态、存储、测试维度和预算。
- [x] 完成改动前 Server 基线：338 通过、0 失败、33 原有忽略，398.93 秒。
- [x] MU1a 搬移用户权威对象与服务配置，定向回归。
- [x] 更新架构、代码链路和 checkpoint。

## 范围与权威

威胁模型沿 ADR-0147：当前本地操作者可信；将来的 Linux 受邀账号彼此不可信，模型生成代码也不可信。MU0/MU1a 在本地隔离测试根执行，不开放多人入口。

| 对象 | 基线权威 | MU1a 落点 / 后续责任 |
| --- | --- | --- |
| 身份 | 本地 Host，MCP visitor 独立工具表 | 固定本地 UserRuntime；MCP 私人存储继续不可用；登录 MU4 |
| Memory / Profile / pending reads / review jobs | 一份 AppState.store | UserRuntime.store；后台复核及已读 worker 借用同一对象 |
| History / Goal / Pending Turn / 临时确认 | AppState.agent_history + history_path | UserRuntime；保持 active_by_book 选择和 schema |
| Learning / Tutor / Trace / Evidence | Memory 相邻 learning.db | 通过 UserRuntime 的 Memory 访问，仍为原 SQLite schema |
| 私人演示 | history_path.with_extension("presentations") | 用户拥有原根，当前书 / 聊天检查保留 |
| 私人 Intent / Artifact / usage | intent_store_root | UserRuntime；默认路径注入 MU2 |
| Book / Reader / messages / session / active stream | AppState | MU1b/MU1c；本片不复制或另存 |
| Provider / library 配置 / 宿主能力 | AppState + ServerHostConfig | 独立 ServiceState；启动配置与热更新沿用 |
| 接单与任务生命周期 | RunCoordinator 单 active 槽 + History | 本片不变；持久接单 MU6a、调度 MU6c |
| 公共内容版本 / 教学 readiness | Book/source/teaching 文件 | 本片不变；不可变 publication MU3 |

## 路由与旁路清点

[routes.csv](linux-multi-reader-mu0-20260930/routes.csv) 逐行记录 128 个入口的调用者、当前权限、对象归属、读写根、耗时类别及基线源码位置。`/api` 前缀和无前缀旧别名由 `normalize_api_url` 进入同一核心；规范 Book REST/MCP 名称取自 BookToolContractRegistry。Reader 读状态也沿旧 POST 合同。

- Host 旁路：`/observability/status`、`/build_intent/usage.event`、`/agent/runs`、按 turn 的 GET/events/cancel、`/agent/chat`、选区翻译、地图本地化、Book 二进制资源。
- PDF `/book/pdf/original` 与 `/book/assets/{relative_path}` 当前只支持 GET，按当前 book_dir 解析。未来 HEAD/Range/条件请求也必须先授权。私人演示资产随受检查的 Presentation 内容返回，不存在独立公共目录。
- `route_book` 中导航规划读取用户 Memory；它们不能误标成可全用户共享缓存的纯 Book 查询。
- `/build_intent/*` 有用户私人根并可调用 Node/模型；reader-only 仅允许 status/artifacts/usage GET 和 usage.event POST。`/build_workbench/*` 及 create 被 reader-only 拒绝。
- MCP 13 个 Book 工具与 3 个 artifact 工具仅消费绑定的书、临时 visitor 会话和 active accepted artifact 只读端口；不会因 MU1a 得到 Resident 笔记、History 或演示。
- Tauri Provider/书库设置在本机命令面，调用 `RunningServer::set_provider_config/set_library_root`，不属于普通读者 HTTP 权限。

## 持久根、隐式状态与进程

[implicit-state-and-processes.csv](linux-multi-reader-mu0-20260930/implicit-state-and-processes.csv) 保存 crates 与桌面启动代码中 107 处环境/default root/static/进程/当前会话引用。只记录源码字段，不输出运行密钥。生成入口为同目录 `inventory.py`；CSV 是修改前冻结记录，不用修改后的行号覆盖。全部 Server 源码的改动前副本另保留在 `tmp/mu0-source/crates/server/src`，含未跟踪 Tutor 源码；用于本次增量差异对照。

| 根 / 状态 | 基线读写者与归属 | 后续处理 |
| --- | --- | --- |
| UNDERSTAND_BOOK_MEMORY_DIR，否则 USERPROFILE/HOME/.understand-book/memory | MemoryStore；memory.json、辅助画像投影/回执、相邻 learning.db | MU2 显式用户路径；原快照不可每窗口各开一份 |
| memory_path.parent/session.json | Host 位置/覆盖层恢复，book/open 与导航保存 | MU1b/MU5 现场化；MU1a 不搬路径 |
| 同根 agent-history.json | Host/Run/复核输入、pending 确认仅内存 | 用户历史权威；保留原提交/恢复 |
| history_path.with_extension(presentations) | 候选/不可变版本/现场回执；AppState 权威借用 | MU1a 根归 UserRuntime，当前材料闸保持 |
| UNDERSTAND_BOOK_PRIVATE_DIR，否则用户默认 private 根 | IntentArtifactStore、Core 子进程和 MCP accepted artifact 读端口 | MU2 私人路径注入，MCP 继续只读 |
| library_root / 当前书派生库根 / registry | Host、桌面书库与构建控制器 | MU3 管理员发布目录 |
| book_dir/source、base、PDF、教学、sidecar | Book/Reader/Tutor 公共内容读 | MU3 不可变发布 |
| book_dir/.build、注册表、地图中文缓存 | 构建/就绪检查；地图本地化直接写缓存 | MU3/MU7 迁出或禁用后才可只读挂载 |
| UB_OBSERVABILITY_SPOOL_DIR | 服务级观测 spool、异步 exporter | MU7 保持隐私白名单和有界队列 |
| 临时 Python / Manim / Browser profile | presentation_plot/animation/preview | MU7 验收隔离或禁用；目前普通进程不构成沙箱 |
| Tauri LOCALAPPDATA / USERPROFILE/Documents | Provider 设置、窗口/宿主设置、插件安装与库根 | 本地运维配置，MU1a 不改文件布局 |
| PAPER_LOCALIZATION_CACHE_WRITE | 进程级缓存文件写互斥 | MU3 公共派生缓存；不能持锁等模型 |
| WRITE_NONCE / presentation NEXT | 原子文件临时名和演示 ID 序列 | 仅命名，无私人对象缓存 |
| ProfileContextCache | AppState 用户画像投影 | MU1a 同一 UserRuntime |
| RuntimeStatePort.previewed / animations / plots | 每 Run 的预览回执与生成资产临时句柄 | MU1c 固定 Run owner；MU7 隔离执行 |
| RunCoordinator.slot / unsaved / unsaved_stream | 单宿主活动运行及未保存结果；RunStream 缓存快照/事件/来源绑定 | MU6 接单恢复与有界观察；不是另一个 History |
| Observability BoundedQueue / Provider ureq Agent | 服务级有界导出队列与 HTTP 连接复用 | 配置/连接归服务；私人内容仍服从白名单 |
| active_by_book / messages / visitor_sessions | 持久聊天选择/当前对话/独立临时访客会话 | MU1b 拆当前选择；visitor 不迁入用户历史 |

子进程入口：`lib.rs` 的 Workbench runner 与 Node Core 命令、`build_intent_api.rs::execute_core`、plot Python、Manim Python、CDP Browser；animation/preview 使用 taskkill/kill 收尾。桌面 plugin_manager 调用 Codex CLI。Node intent/metrics/blueprint 请求显式传 `private_root`，未发现这些入口另行读取 HOME；构建侧 `automatic-build-driver`/executor-session 的 DRIVER_ROOT、sidecar-entry 的 PLUGIN_ROOT/SIDECAR_SELF、诊断工具的 CODEX_HOME 仍属于预构建宿主配置，MU4 普通读者不得触达。后台线程：Host 已读冲刷/复核/HTTP workers、Run worker、SSE writer、观测 exporter、Core 输出排空。模型旁路包括 query、synthesize、翻译、本地化、规划/来源审阅、画像复核/回填，以及 Resident 内的压缩/来源修复/教学评估；MU6c 需统一资源计量。

## 冻结测试材料与资源门槛

| 维度 | 固定输入 |
| --- | --- |
| 用户 / 现场 | A、B；A1/A2/B1；相同 `session-legacy`、`turn-legacy`、`note-legacy` 字符串，不用真实私人数据 |
| X 技术材料 | `server::tests::state_named` / sample_base；1、1.1 节点；`X.repeat(100)+尾巴` |
| X 正式教学就绪 | `tests/tutor_loop_tests.rs::fixture` 的平均速度来源、speed 对象及 v1 map |
| X 未就绪 / 旧版 | `tutor_tests.rs` 缺 receipt、缺 map、source_revision 不符、source_review failed 分支 |
| Y PDF 材料 | `lib.rs::write_pdf_runtime_artifacts/use_pdf_runtime_fixture_source/simple_pdf` 构造夹具；与 X 不同 source ID，临时目录 |
| 内容与发布维度 | 后续 MU3 使用 X-v1/X-v2 不同 book_id；同 X-v1 的 publication-1/publication-2；输入正文和能力清单直接比较 |
| 私人对象 | 真实临时 Memory JSON、History、learning.db、Presentation 版本/现场；固定 Provider 答案和可控等待，不调用付费模型 |

本机：Windows 10 Pro 10.0.19044，Ryzen 7 5800H，8 核/16 线程，约 13.9 GiB 可见内存；开始时约 0.8 GiB 空闲，性能结果不视作独占机器测量。Rust/Cargo 1.96.0 MSVC，Node 24.9.0，pnpm 10.34.2，Python 3.14.4；依赖沿当前 Cargo.lock/pnpm-lock：rusqlite 0.32.1、libsqlite3-sys 0.30.1（bundled）、tiny_http 0.12.0、ureq 2.12.1。SQLite 链接探针见下一节。

后续 Linux 容量验收冻结目标为 4 vCPU / 8 GiB RAM / 本机 SSD 40 GiB 可用；10 个账号、20 个现场，2 个全局模型槽、每用户 1 个活动 Run，全局队列 20 / 每用户 2。渲染/动画/预览分别 1 槽；服务 RSS ≤4 GiB，全部存活 Book ≤2 GiB，空闲现场卸载后应释放对应引用。此为验收负载，不是当前支持容量声明。

使用本地 fake Provider 10 秒阻塞，以 10 并发读者运行 5 分钟：纯阅读/状态 p95 ≤250ms、私人提交 p95 ≤500ms、已接单响应 p95 ≤500ms（不含模型执行）；用户短写锁 p95 ≤50ms，服务库提交 p95 ≤100ms，busy 有界 ≤5s。重启对账 ≤10s/100 条未终局记录；同账户多窗口修改无丢更新；静态负载结束后连接/进程/队列回归空闲。失败后定位锁/IO/资源占位，不能调高阈值宣告通过。

MU1a 判据为同一命令前后确定性回归等价，尤其模型阻塞期间 Reader/Memory/History 可访问、复核回写同一 Store、持久路径和 Tutor revision 不变。Linux p95/RSS/并发与故障恢复留待 MU6/MU10 实机测量。

## 验证命令与结果

1. `cargo test -p server --lib -- --test-threads=1`：覆盖启动、笔记、聊天、Tutor、演示、模型等待和后台复核；失败则先记录原始断言，MU1a 必须保持原先通过的用例。日志：[server-baseline.log](linux-multi-reader-mu0-20260930/server-baseline.log)。结果：338 通过、0 失败、33 原有忽略，398.93 秒。
2. SQLite 实际链接探针：`rustc --edition 2021 docs/performance/linux-multi-reader-mu0-20260930/sqlite_probe.rs --extern rusqlite=target/debug/deps/librusqlite-fccfc0113bf48866.rlib -L dependency=target/debug/deps -o tmp/mu0-sqlite-probe.exe`，然后传入隔离 `tmp/mu0-probe.db`。结果：SQLite **3.46.0**，journal_mode=delete，synchronous=2，foreign_keys=1，busy_timeout=5000ms。见 [sqlite.txt](linux-multi-reader-mu0-20260930/sqlite.txt)；LearningStore 没有改这些 PRAGMA，因此此为其默认连接配置。
3. `cargo check -p server --all-targets`：检测所有 Server 构造器、生产入口与测试入口是否仍引用已搬走的 AppState 字段；失败则修正所属对象访问。通过，见 [mu1a-check.log](linux-multi-reader-mu0-20260930/mu1a-check.log)。
4. `cargo test -p server -- --test-threads=1`：验证状态搬移后启动、笔记、聊天、Tutor、私人演示、后台任务及模型等待行为；失败则定位相对基线的行为差异。库测试 **339 通过、0 失败、33 原有忽略，406.47 秒**；MCP 启动 5 通过，CLI 1 通过，预览集成 1 通过/10 原有浏览器忽略，doc-tests 0。合计 **346 通过、0 失败、43 忽略**。见 [mu1a-server-tests.log](linux-multi-reader-mu0-20260930/mu1a-server-tests.log)。
5. `cargo test -p server --lib visitor_dispatch_has_no_reader_or_memory_branch -- --test-threads=1`：全套运行后新增访客断言，检测身份搬移是否意外开放私人根或写文件；失败则恢复 MCP 只读边界。1 通过、0 失败，见 [mu1a-visitor-test.log](linux-multi-reader-mu0-20260930/mu1a-visitor-test.log)。该用例已包含在全套计数中，不另计新增用例。

## MU1a 交付与接手

`AppState` 组合一个 `UserRuntime` 与一个 `ServiceState`。固定本地身份 `local` 拥有原 MemoryStore、History、画像上下文缓存、History/Intent 路径；Learning、私人演示和 Intent 成果通过该用户对象访问。构造时移动原已加载对象，未复制快照，也未新增兼容字段或锁。服务配置拥有 Provider adapter、library_root 和宿主能力；热更新设置与切书继续借用同一用户权威对象。

Host、Run、复核、Tutor、Presentation、Build Intent 和 MCP 接点已改为显式所属对象访问。MCP visitor 保持无 Resident 身份、不可用 Memory 和空私人根；公开 Book 工具仍可用。文件布局、SQLite schema、聊天选择和调度保持原合同。私人演示的当前书/聊天/回合校验继续由现有入口承担。

新增 `agent_run_tests::local_user_survives_service_changes_book_switch_and_restart`：保存笔记、新聊天和 Tutor revision，更新书库与 Provider 配置，切书往返并重启，验证身份、原持久根、笔记、所选聊天和 Tutor 状态保持。补强 MCP 测试验证 Book 可读、Reader/Memory 路由被拒绝、Learning/Intent/Presentation 私人根不可用且未落盘。既有模型等待、取消、聊天切换、复核回写及停机测试继续通过。

下一步为 MU1b：把 Book、Reader、selected_chat、generation 归入 ReaderWorkspace，审计 `ensure_agent_history_for_book` 和 `AgentHistory.active_by_book` 的隐式选择；沿用单本地现场入口，再验证两个内存现场的选择互不覆盖。MU1c 才拆 Run 状态端口，MU2 才注入用户路径与服务数据库。T01–T79 多人端到端矩阵仍待执行。

## 已知限制与后续门槛

- 当前 SQLite 3.46.0 不满足方案 §3.3 的未来多连接 WAL 版本门槛。当前 Learning 使用 DELETE journal；MU2 启用服务库 WAL 前核验/升级，MU1a 不改依赖或声称已有 WAL 故障。
- 当前主路由部分 query/synthesize/构建模型调用持 AppState 借用；Host Resident、复核、翻译和本地化模型等待已在锁外。MU1a 保持该行为，旁路资源并发归 MU6c。
- 当前并无账号鉴权、多现场隔离、受限生成代码执行层；只靠部署单账号入口不构成多人服务。
- Linux 实机、HTTPS/iPhone、故障注入和容量数据未测；本次不安装或发布服务。
