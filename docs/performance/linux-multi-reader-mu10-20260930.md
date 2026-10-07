# MU10 隔离、故障、跨平台与容量验收记录

验收始于 2026-09-30，基线为 `5e10516` 上包含 MU0–MU9、RE1–RE6 与 ADR-0150 的实时工作树。MU10 测试设施、实际故障实验、跨平台回归和逐项矩阵已落地；**完整候选的发布门槛未通过**。Core / DeepSeek 预构建回归有失败，指定容量机器与真实 Provider 冒烟尚缺条件。当前不能据此进入 MU11 对外开放。

合同见 [MU 方案](../切片方案-Linux原生多人阅读与无Redis首版.md)、[MU0 冻结指标](linux-multi-reader-mu0-20260930.md)；全部 I01–I15 / T01–T79 见 [逐项矩阵](linux-multi-reader-mu10-matrix.md)。测试入口和复现方式见 [MU10 操作说明](../Linux多人阅读-MU10验收.md)。

## 实现与修复

新增 `crates/server/src/tests/mu10_tests.rs`：请求经过真实 HTTP Host、Cookie / CSRF / Origin 和对象授权，使用真实私人文件、SQLite 和独立服务根。模型替身控制延迟及提交位置。真实进程终止、Linux 文件系统错误和慢 socket 与原有 MU6 的函数级故障注入互补。

提交屏障、短锁 / 控制事务耗时、资源使用计数均受 `cfg(test)` 限制，生产入口不读取 MU10 环境变量。`scripts/linux/measure-mu10.py` 收集负载子进程的 `/proc` 指标，并保存机器条件及退出状态；本轮实机结果由同一采样方式的临时驱动产生，收口后的 CLI 未另跑一轮五分钟负载。

本轮修复三个可达问题：

1. **Linux 普通用户无法发布书包。** 发布目录先被封存为 0555，再跨父目录 rename，普通用户因此收到 EACCES。现在先移动到最终位置，再封存，最后提交发布登记。非 root 权限实验从准备阶段失败转为通过，root 测试曾掩盖这一问题；[失败记录](linux-multi-reader-mu10-20260930/linux-permission-first.log)保留。
2. **首次聊天恢复清掉已有选区。** MU8 给现场 key 增加 local / network 前缀后，首次历史返回的比较仍使用旧格式。补齐前缀，原 `App.startup.test.ts` 断言通过，完整 Web 恢复为 328 项通过。
3. **网络阅读初始化读取不存在的来源复核字段。** 网络模式构造的是精简阅读状态，`sourceReviewEvidenceKey` 读取 `source_review.unresolved` 触发 TypeError。允许精简状态没有该字段，PDF 重载及账号切换在 WebKit 复验通过。浏览器测试同时捕获 Vue 输出的 TypeError / ReferenceError，并在失败时释放测试持有的 PDF 响应，避免清理阶段继续等待。

## 故障恢复

| 场景 | 实际动作与结论 | 证据 |
|---|---|---|
| preparing | 第一次控制事务提交后杀掉 Host；缺原 Pending 的请求键关闭，重试不创建新问题 | `mu-final.log`、`linux-mu-final.log` |
| pending / prepared / queued | 在私人 Pending、准备完成及排队提交后分别杀进程；完整未领取回合恢复原问题，模型只调用一次 | 同上 |
| teaching_receipt | 原学习回应已提交、绑定及准备标记未完成时 kill；重开后完整原回应 / attempt 不变，补齐冻结绑定与标记；一条回合保存，模型两次采样（tutor.step outside + 文本） | [Windows 八窗口](linux-multi-reader-mu10-20260930/kill-final.log)、[Linux 八窗口](linux-multi-reader-mu10-20260930/linux-kill-final.log) |
| claimed | 领取后杀进程；History 标记中断，不重放不确定模型调用 | 同上 |
| 私人笔记已写 | 工具写入后、最终回答前杀进程；私人笔记保留，回合中断，模型不重跑 | 同上 |
| terminal | History 完成后、控制库 settled 前杀进程；重开仅修控制索引，一条 History，无模型重跑 | 同上 |
| SQLite busy | 另一真实连接持有 IMMEDIATE 事务；HTTP 分别 3ms / 2ms 返回 503、零接单和模型调用，释放后原键可接单 | 同上 |
| 慢 SSE | 客户端不读 socket，发送 4000 × 8 KiB 事件；保存不被反压，旧游标得到替换快照 | 同上 |
| 100 条恢复 | 100 条完整 queued 重开对账；原输入与 attempt=0 保留、未调用模型；Windows 576ms、Linux 72ms | 同上 |
| ENOSPC | 4 MiB 独立 tmpfs 写到真实磁盘满；Pending 字节不变，unsaved / CHAT_BUSY 可见；释放空间后原结果保存，无模型重跑 | [Linux 日志](linux-multi-reader-mu10-20260930/linux-disk-full.log)，1 通过 |
| 权限失败 | 非 root 将临时 History 目录改为 0500，实际 PermissionDenied；恢复权限后原结果可保存 | [Linux 日志](linux-multi-reader-mu10-20260930/linux-permission-final.log)，1 通过 |

八个窗口最终全部通过：Windows 恢复 27–95ms，Linux 4–14ms；新增教学窗口分别 27ms / 11ms。

测试进程终止在 Windows 使用系统进程终止、Linux 使用 SIGKILL。独立 tmpfs 已卸载，验收服务已退出，生产根与入口未切换。测试使用同根重启及真实持久结果检查，未以重新提交问题代替恢复。

## Linux 五分钟容量实验

隔离根 `/opt/understand-book/acceptance/mu10`。当前可用主机为 4 CPU、3,802,132 KiB 内存（约 3.63 GiB）、1,991,196,672 字节空闲磁盘（约 1.85 GiB）；内核 `5.15.120-5.ve2.x86_64`，glibc 2.34，SQLite 3.53.2，WAL / synchronous=FULL / schema 4。

冻结机器要求是 **4 vCPU / 8 GiB RAM / 40 GiB 空闲本地 SSD**。本机未满足内存与空闲磁盘条件，以下结果是当前机器上的诊断实测，不能签署指定硬件容量验收。

负载持续 300 秒：10 用户、20 现场、两本材料、2 模型槽、每用户 1 活动回合及 2 未结接单；模型每次阻塞 10 秒，usage 未知；持续阅读，周期性私人写入和提交下一回合。随后排空未结工作，总测试 349.81 秒。全部用户在负载窗口内完成 5–6 轮，排空后共 68 次模型调用。

| 指标 | 冻结门槛 | 实测 |
|---|---:|---:|
| 阅读 / 状态 p95 | ≤250 ms | **5.53 ms**，28,283 样本 |
| 私人写入 p95 | ≤500 ms | **22.87 ms** |
| 接单响应 p95 | ≤500 ms | **76.84 ms** |
| 用户短锁持有 p95 | ≤50 ms | **12.72 ms**，34,081 样本 |
| 控制库提交 p95 | ≤100 ms | **3.15 ms**，136 样本 |
| 100 条重启对账 | ≤10 s | **72 ms**，另项恢复测试 |
| 进程 RSS 峰值 | ≤4 GiB | **188,428 KiB，约 184 MiB** |
| 存活 Book 计费 | ≤2 GiB | 2 本，64,112 字节 |
| 模型并发 | ≤2 | 2 |
| 线程 / 文件描述符峰值 | 记录实际占用 | 34 / 56 |
| 排空后模型活动 / 全部活动 / 等待 | 全部归零 | 0 / 0 / 0 |

实测满足本轮负载的数值阈值。样本书体积很小，存活 Book 数据不能外推为 2 GiB 书库压力结论；完整渲染混合压力仍缺。短锁采样覆盖 Host 现场操作、接单、终态保存，控制库采样覆盖现场检查点、preparing 与 settled 事务。

原始证据：[容量结果](linux-multi-reader-mu10-20260930/capacity.json)、[RSS / 线程 / 描述符](linux-multi-reader-mu10-20260930/capacity-resources.json)、[机器条件](linux-multi-reader-mu10-20260930/linux-environment.json)、[测试日志](linux-multi-reader-mu10-20260930/linux-capacity.log)。

## 跨平台与完整回归

| 执行范围 | 本轮结果 | 原始记录 |
|---|---|---|
| Windows `cargo test -p memory -p runtime -p server` | Memory 125、Runtime 416 + 6 通过；Runtime 3 原忽略。Server 444 通过 / 1 失败 / 36 忽略；唯一失败为新 MU10 笔记夹具没有明确请求笔记，修正后在下行复验通过 | [全量首次](linux-multi-reader-mu10-20260930/rust-full.log) |
| Windows 多人相关最终回归 | 119 通过 / 0 失败 / 3 忽略，覆盖 MU2–MU10；含少量名称也匹配 `mu` 的原有测试 | [最终多人回归](linux-multi-reader-mu10-20260930/mu-final.log) |
| Linux 多人相关回归 | 120 通过 / 0 失败 / 9 忽略；独立 root / 普通用户 / 容量用例另计 | [Linux 回归](linux-multi-reader-mu10-20260930/linux-mu-final.log) |
| Book MCP / Server CLI / preview | MCP 5、CLI 2、preview 1 通过；preview 10 原有浏览器忽略 | [二进制入口](linux-multi-reader-mu10-20260930/server-bins.log)、[preview](linux-multi-reader-mu10-20260930/server-preview.log) |
| Windows Tauri | 17 单测通过；当前调试程序构建、独立用户根启动、正文、保存私人笔记及刷新恢复通过 | [单测](linux-multi-reader-mu10-20260930/tauri-tests.log)、[构建](linux-multi-reader-mu10-20260930/tauri-build.log)、[运行回执](linux-multi-reader-mu10-20260930/tauri-smoke.json)、[截图](linux-multi-reader-mu10-20260930/tauri-reader.png) |
| Web Vitest | 首次 327 通过 / 1 失败；修正首次聊天恢复后完整 328 通过；来源复核修正后启动 / 网络客户端另 11 通过，双浏览器全场景通过 | [首次](linux-multi-reader-mu10-20260930/web-tests.log)、[最终全量](linux-multi-reader-mu10-20260930/web-final.log)、[最终定向](linux-multi-reader-mu10-20260930/frontend-final.log) |
| Web / Core 构建 | `pnpm build` 通过；多人环境网络 Web 构建通过 | [根构建](linux-multi-reader-mu10-20260930/build-final.log)、[网络构建](linux-multi-reader-mu10-20260930/network-build-final.log) |
| HTTPS Chromium | 最终登录与原键恢复、多标签页、演示来源 / 附属追问、晚到 PDF 账号隔离 4 通过 | [Chromium 最终](linux-multi-reader-mu10-20260930/chromium-final.log) |
| HTTPS WebKit | 首次 3 通过 / 1 失败，单独重试仍失败；修正来源复核字段异常后最终 4 项全部通过 | [首次](linux-multi-reader-mu10-20260930/webkit.log)、[失败重试](linux-multi-reader-mu10-20260930/webkit-retry.log)、[修复复验](linux-multi-reader-mu10-20260930/webkit-fixed.log)、[最终四场景](linux-multi-reader-mu10-20260930/webkit-final.log) |
| 根 `pnpm test` | 未通过。Core 首次 1,032 通过 / 29 失败 / 4 worker 错误；单独限制并行后 1,055 通过 / 6 失败 | [根测试](linux-multi-reader-mu10-20260930/pnpm-test.log)、[Core 限并行](linux-multi-reader-mu10-20260930/core-serial.log) |
| DeepSeek Harness 源码测试 | 配置 `UNDERSTAND_BOOK_TEST_NODE` 后：35 项，29 通过 / 3 失败 / 3 因超时取消；初次缺该环境变量的结果另保留 | [正确配置后的结果](linux-multi-reader-mu10-20260930/dsh-configured.log)、[初次](linux-multi-reader-mu10-20260930/dsh-tests.log) |
| DeepSeek Harness 已安装宿主 | 文档所列 `test/run-installed.mjs E:/DeepSeekHarness`，实际宿主绑定 / 能力 7 通过 | [已安装宿主](linux-multi-reader-mu10-20260930/dsh-installed.log) |

全量失败保留在原日志；定向复验只能证明相应失败已修复，不能把未重新执行的全量命令写成一次全绿。浏览器通过真实 TLS 代理访问当前 Rust Host；模型替身不会发送真实模型请求。

## 已知问题与发布门槛

1. **Core 六项仍失败。** `automatic-build-release.test.ts` 的 extractor 清单未含现有教学与关系抽取项；`build-intent-v2.test.ts`、`explicit-legacy-build-plan.test.ts` 的关闭 Pass2 预期未含 `formal_objects / cognitive_materials / teaching_publish`。另有 `book-structure-append-production.test.ts`（120s）、`build-orchestrator.test.ts`（5s）、`codex-executor-agent.test.ts`（20s）超时。MU10 没有更改这些计划 / 夹具合同，也没有放宽断言；需要沿当前预构建合同确认并修复后复验。
2. **DeepSeek 完整预构建未通过。** lost-response / crash-after-commit 两个恢复场景在 formal_objects 阶段停为 NEEDS_USER / 无有效回执；两 worker 场景的子进程退出断言失败；Pass2 开 / 关两条完整流程均在 240s 超时，根插件入口在 30s 超时。已安装宿主的 7 项通过不能替代这六个未成功结束的场景。
3. **正式容量验收条件未齐。** 当前 Linux 机器不符合 MU0 硬件要求；渲染混合负载、足量材料的常驻内存仍需在合适隔离环境实测。不得因当前延迟较低调低原机器或负载要求。
4. **真实 Provider 冒烟未执行。** 已请求一次现有 DeepSeek 路由、输出上限 512 token 的测试预算，尚未获得答复；所有本轮模型测试使用固定替身。待预算确认后仅做隔离账号最小真实调用，并单列回执。
5. **实体与发布运行单仍归 MU11。** 真实 Nginx HTTPS / iPhone、指定旧 release 二进制回滚、生产停写 / 切换 / 恢复点演练尚未执行。SIGKILL 证明进程崩溃恢复，不能证明宿主断电及存储设备掉电持久性。Tauri 来源与完整富呈现当前仅有 Server / Web 交叉证据，仍需同候选桌面正式验收。

MU10 的测试与报告交付完成后，下一步先收口上述候选回归和验收条件；MU11 只消费通过记录与明确的 MU9 生产数据映射。本轮未修改生产服务、Nginx 或 systemd，未迁移生产资料，保留全部原有未提交成果。

## 2026-10-01 MU11 后续收口

本页原执行结果保留。教学阶段夹具对齐后，Core 全量 1061 通过 / 1 个 20 秒注册超时；该文件单独 25 项通过，未改阈值。DSH 通过两个不重叠批次覆盖全部 35 项。真实 DeepSeek 获授权后执行 1 次、最多 512 输出 token，实际输出 23 token，结果 saved。真实 Nginx TLS、两个账号、PDF、浏览器、备份恢复、指定旧 release 读取、禁用与发布切换、强杀重启证据见 [MU11 实施记录](linux-multi-reader-mu11-20261001.md)。正式硬件与混合负载、优化发布包、可信 HTTPS / 实体 iPhone、生产迁入切换仍保留为门槛。

2026-10-01 后续发布：用户明确决定预构建相关失败可按通过处理、不挡部署。MU11 已完成优化构建、可信 IP HTTPS、生产迁入与切换；9 份材料 / 449 条记忆 / 17 段对话 / 25 回合迁入与真实浏览器通过。测试原始结果不变，正式容量与实体设备的未验范围单列。见 [现网说明](../Linux多人阅读-现网运行说明.md)。
