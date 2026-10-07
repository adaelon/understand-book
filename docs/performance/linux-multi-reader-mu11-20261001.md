# MU11 Linux 发布实施记录

日期：2026-10-01。基线：`5e10516` 上包含 MU0–MU10、RE1–RE6、ADR-0150 与既有 MU11 草稿的实时工作树。**新版已完成生产迁入与入口切换：`https://115.190.121.150`。用户明确决定 MU10 预构建失败不阻挡本次部署，原始测试结果保留。** 现网参数、续期和恢复见 [运行说明](../Linux多人阅读-现网运行说明.md)。

## 实施范围

- [x] 按 checkpoint 冷启动读序确认合同、失败日志和既有实现。
- [x] 收口 Core / DSH 教学阶段夹具，执行完整覆盖的回归；保留 Core 全量中的超时事实。
- [x] 显式模式启动、同源码 Server / 网络 Web 构建、版本收据及 systemd / Nginx 配置。
- [x] Linux 普通用户隔离实例：真实 Nginx TLS、两个账号、阅读、笔记、PDF、真实问答 / SSE、停止、恢复与旧二进制读取。
- [x] 修复演练发现的备份父目录权限问题；核验禁用账号、切换默认发布、强杀重启。
- [x] 发布运行单、证据、状态索引与 checkpoint 同步。

## 实现与修正

`start-reader.sh` 按 `UNDERSTAND_BOOK_MODE=single-user|multi-user` 选择入口，默认保留原单读者模式。多人入口传独立服务根、HTTPS origin 和 loopback 地址给 Server 校验。`build-multi-reader.sh` 在最终源码目录构建五个 Rust 二进制与 `dist-multi`，避免覆盖本地模式 `dist`；成功才写 `multi-reader-build.txt`，开始重建时移除旧成功收据。收据标明源码目录、commit / 工作树或源码副本、工具链、时间和 profile。

部署配置为 `understand-book-multi.service`、`multi-reader.env.example`、`nginx-multi-reader.conf.example`；操作和参数见 [发布运行单](../Linux多人阅读-MU11发布运行单.md)。浏览器冒烟为 `smoke-multi-reader.mjs`，凭证只从 stdin 接收，不提交模型问题。

既有 Core 测试夹具对齐当前 extractor 与 `formal_objects / cognitive_materials / teaching_publish` 合同；结构追加完成后仍需教学阶段，因此返回 `NEEDS_USER`。DSH 共享模型夹具提供完整教学产物，恢复观察数量对齐 9 次；未修改生产预构建合同或延长测试超时。

隔离备份实测发现 `reader_maintenance::snapshot` 会将目标的**已有父目录**改为 0700，使同目录下的 Nginx 静态文件不可访问。修正只对新建快照施加私人权限；Unix 回归断言共享父目录仍为 0755、快照为 0700。最终实机备份也保留原父目录 0711。

发布切换复验发现材料列表的两个发布版本原来显示相同书名。`NetworkApp.vue:bookLabel` 在同名材料有多个发布时显示版本序号与默认标记；版本序号是当前授权目录的展示顺序，打开现场仍使用确切 PublishedBookRef。实际双浏览器覆盖两个同名发布的区分与默认版本选择。

## 已确定部署输入

通过实际 systemd 配置、进程目录与环境文件确认：

| 对象 | 实际值 |
|---|---|
| 旧生产服务 | `understand-book.service`，用户 `understand-book`，`127.0.0.1:8787` |
| 旧 release | `/opt/understand-book/releases/working-tree-c12cbb9-huawei-viewport-20260919-1140` |
| 旧私人数据 | `/opt/understand-book/data/lx6/memory`、`/opt/understand-book/data/lx6/private` |
| 旧书库 | `/opt/understand-book/books`，约 801 MB；session / registry 还引用两个外部材料目录 |
| 外部材料 | `/opt/understand-book/acceptance/lx2-lx6/run-1788841095/books/missing-paper`；`/opt/understand-book/acceptance/lx7/accepted-fixture/sample-book` |
| 拟定正式目标 | `/opt/understand-book/data/multi-reader`；旧资料唯一目标账号 `reader`；备份 `/opt/understand-book/backups/<operation-id>` |
| 本轮隔离根 | `/opt/understand-book/acceptance/mu11`，最终恢复根 `restored-final`，最终恢复点 `backup-final` |
| 隔离入口 | 独立 Nginx `127.0.0.1:8443` → 普通用户 Server `127.0.0.1:8788`；SSH 隧道浏览器 origin `https://127.0.0.1:18443` |

用户确认目前没有域名。隔离入口使用 IP 测试证书：HTTP 客户端显式信任该证书，Playwright 显式忽略测试证书错误。生产资料未迁入；正式发布映射须包含书库与外部目录的全部旧引用。

## 回归结果

| 范围 | 实际结果 | 证据 |
|---|---|---|
| 启动脚本 | 5 通过：默认单读者、多人参数、缺参数和非法模式 | [launcher-resume.log](linux-multi-reader-mu11-20261001/launcher-resume.log) |
| 构建脚本 | 2 通过：独立网络目录 / 五个入口 / 源码副本收据；失败移除旧收据 | [build-script.log](linux-multi-reader-mu11-20261001/build-script.log) |
| Core 全量限并行 | 1061 通过 / 1 超时，142 文件；20 秒的 Windows 注册测试超时 | [core-full.log](linux-multi-reader-mu11-20261001/core-full.log) |
| Core 注册文件单独复跑 | 25 通过；原超时测试 19.129 秒，保留原 20 秒阈值 | [core-registration.log](linux-multi-reader-mu11-20261001/core-registration.log) |
| DSH 原失败四文件 | 7 通过，含两个恢复、双 worker、Pass2 开关和插件入口 | [dsh-resume.log](linux-multi-reader-mu11-20261001/dsh-resume.log) |
| DSH 其他文件 | 28 通过；与上一行不重叠，共覆盖当前 35 项 | [dsh-remaining.log](linux-multi-reader-mu11-20261001/dsh-remaining.log) |
| 网络 Web | vue-tsc + Vite 成功；默认 dist 保持本地模式 | [web-build-final.log](linux-multi-reader-mu11-20261001/web-build-final.log) |
| Linux MU9 修正后回归 | 11 通过，含共享父目录权限与混合恢复 | [mu9-final.log](linux-multi-reader-mu11-20261001/mu9-final.log) |
| Linux 五个 Rust 入口 | 当前源码离线锁定构建成功，**dev / unoptimized** | [build-final.log](linux-multi-reader-mu11-20261001/build-final.log)、[实际依赖](linux-multi-reader-mu11-20261001/build-environment.json) |

Core 全量与 DSH 前批同时执行；单独复跑仅证明注册用例当次通过，不能改写为完整 Core 命令全绿。DSH 使用实际 Node 的绝对路径设置 `UNDERSTAND_BOOK_TEST_NODE`，命令采用 `--test-concurrency=1`。临时目录放在仓库 `tmp/mu11-*`。

## 真实隔离入口与恢复

- [HTTP 回执](linux-multi-reader-mu11-20261001/http-smoke.json)：匿名拒绝、两账号登录、Secure / HttpOnly / SameSite=Lax、CSRF / Origin 拒绝、跨账号现场拒绝、保存笔记、授权 PDF Range / 越权 PDF 拒绝及退出失效。
- [Chromium](linux-multi-reader-mu11-20261001/chromium-browser.json) 与 [WebKit](linux-multi-reader-mu11-20261001/webkit-browser.json)：真实 Reader、独立标签页、横竖尺寸变化保留草稿、切账号清除私人草稿、刷新恢复，各有 5 项基本流程，最终额外覆盖两次同名版本选择。WebKit 刷新时会把旧页面的取消 fetch 报为 `pageerror`；只有与同阶段 `Load request cancelled` 相匹配、且无 DOM `error` / `unhandledrejection` 的导航诊断单列保存，其他异常仍失败。原始失败与诊断日志保留。
- [启动记录](linux-multi-reader-mu11-20261001/runtime-startup.log)：实际 SQLite 3.53.2，WAL、FULL、foreign_keys=ON、busy_timeout=5000ms、schema=4。普通用户无沙箱配置，网络制作四项 capability 均关闭，reason=`not_configured`。
- [恢复回执](linux-multi-reader-mu11-20261001/recovery.json)：两账号、两发布、三授权、13 现场、一接单恢复前后一致；5 份私人文件逐字节一致。`export-user` 后由**上述实际旧 release 二进制**在独立 8789 端口读取到测试笔记。首次演练误用了旧程序不存在的 `/memory/list`，改为其真实 `/memory/recall` 后完成。
- [最终生命周期](linux-multi-reader-mu11-20261001/lifecycle-final.json)：活动实例有序停止约 0.141 秒，最终停止约 0.127 秒，均退出 0；修正后的备份不改父目录且快照 0700；最终恢复根可读原回答。禁用 B 拒绝旧 Cookie 和新登录；设置新默认发布包后，旧现场 / 原回答继续持有原 PublishedBookRef；SIGKILL 后 systemd 重启仍读取同一已保存 turn。

实机 Rust 为 dev 构建，五个产物复制到候选目录的 `target/release` 仅供已有 launcher 使用，目录名不代表优化构建。网络 Web 来自同一实时工作树，最终 Web 还包含同名发布版本标记，源码与产物均同步到隔离目录。测试的 cgroup 不含活动渲染子进程；取消和渲染进程清理由 MU7 / MU10 既有专项证据覆盖。

## 一次真实 Provider 回执

用户授权现有 DeepSeek 路由一次调用，最多 512 输出 token。隔离 Host 经一次性代理发往 `deepseek-v4-flash`，代理强制 `max_tokens=512`，并在转发前创建独占尝试标记，阻止自动重试或后续付费请求。

[Provider 回执](linux-multi-reader-mu11-20261001/provider-smoke.json)：**请求 1 次，HTTP 200，completion_tokens=23，prompt_tokens=5286，total_tokens=5309**；finish_reason=stop。最大输出预算只限制输出 token。代理不记录 Key / 请求正文。

[Host / SSE 回执](linux-multi-reader-mu11-20261001/paid-host-smoke.json)：实际经历 `model.started → model.finished → run.finalizing → answer.patch → run.completed`，首个 SSE 事件约 0.0002 秒，最终 `settled / saved`。恢复与强杀后读取相同 turn；本轮没有再次提交模型问题。

## 隔离阶段原始限制（后续部署结果见下）

1. MU0 指定 4 vCPU / 8 GiB RAM / 40 GiB **空闲**本地 SSD；当前机器为 4 CPU / 约 3.63 GiB RAM，最终空闲约 1.4 GiB。足量材料与完整模型 / 渲染混合压力未完成。
2. 正式 `--release` 候选构建及该产物完整验收待执行；本轮使用 dev 二进制，不能作为正式发布包通过记录。
3. Core 全量命令仍有一次注册测试超时，单独复跑通过；完整稳定全绿记录仍缺。旧 MU10 的其余教学夹具 / DSH 失败已获得本轮通过证据。
4. 当前没有域名和客户端可信入口；实体 iPhone 的证书信任、中文输入、选区、来源返回与后台恢复未验。Playwright WebKit 另列，不能替代实体设备。
5. 生产旧资料的完整 PublishedBookRef 映射、停写迁入和公网切入口未执行。现网旧服务 PID `4164381` 保持不变，旧 Nginx 配置未改；候选、一次性 Provider、旧版本演练和独立 Nginx 在交付前停止，验收数据 / 恢复点保留，见 [清理回执](linux-multi-reader-mu11-20261001/cleanup.json)。
6. 真实宿主断电与存储掉电持久性、同候选 Tauri 完整来源 / 富呈现、RE7 正式验收仍按原计划保留。

## 继续正式部署（2026-10-01）

用户明确决定 MU10 预构建相关失败可按通过处理，并要求继续部署；该类测试不再作为本次部署阻挡项，原始测试结果保留。按现有机器部署新版，容量 / 实体设备的未验范围继续单列；迁移前保留旧数据与入口恢复点。

- [x] 正式优化构建与 IP HTTPS 证书 / 自动续期。
- [x] 完整材料映射、预览、旧服务停写与独立根迁入。
- [x] 真实入口切换、原资料验证、回执和 checkpoint 更新。


## 生产切换结果

- 入口 **https://115.190.121.150**；原 HTTP 8080 返回 308 跳转。外网客户端启用正常证书校验，HTTPS 返回 200；[入口回执](linux-multi-reader-mu11-20261001/production-entry.json)。
- 正式目录 `/opt/understand-book/releases/mu11-20261001`；五个 Rust 入口完成 `--release` 优化构建，耗时 8m41s；同源码网络 Web。见 [构建日志](linux-multi-reader-mu11-20261001/production-build.log)、[构建收据](linux-multi-reader-mu11-20261001/production-build-receipt.txt)。
- `understand-book-multi.service` active / enabled，普通用户、loopback 8788；旧服务 inactive / disabled。Provider 配置沿用原路由，未复制隔离实验的一次调用上限或代理配置。见 [运行回执](linux-multi-reader-mu11-20261001/production-runtime.json)。
- Let's Encrypt IP 证书已签发，覆盖 115.190.121.150，证书当前截止 2026-10-08 02:37:02（香港时间）。`understand-book-cert-renew.timer` 每六小时检查，续期后 reload Nginx；模拟续期通过，见 [签发](linux-multi-reader-mu11-20261001/certbot-issue.log) / [续期演练](linux-multi-reader-mu11-20261001/certbot-renewal-test.log)。80 端口既有站点只增加 ACME challenge 路由。
- 只清理两个旧目录中可重建的 debug 编译缓存，保留现网 / 旧 release 二进制、源码和数据。部署后磁盘空闲约 5.55 GiB。
- 迁入 reader：**9 份材料、449 条 Memory 记录、17 段对话、25 个回合、6 个有效阅读位置、0 个旧 pending**。原 Memory 及原 private 文件逐字节核验，聊天原字段逐项核对；记录见 [部署回执](linux-multi-reader-mu11-20261001/production-deploy.json)。91 份待审核私人 JSON 均解析并原样保留。
- 四个旧来源清单在副本中以原 `truth_file=source.txt` 为准调整 canonical 路径，补现有发布合同要求的来源身份。正文和 base 未改。重复 missing-paper 测试目录的旧位置归档，正式论文位置继续生效；原 session、registry 和来源清单保存为该账号 `private/mu11-legacy-metadata.json`。
- 停写后原始私人数据 / 配置备份：`/opt/understand-book/backups/mu11-legacy-raw-20261001`；完整材料 / 服务快照及迁移 operation：`/opt/understand-book/backups/mu11-production-20261001`。旧 lx6 数据与原 books 根继续保留。

最终真实 Chromium 验证采用正常证书校验：账号 reader 登录、9 份材料、原书正文、刷新、PDF Range、退出撤销全部通过，脚本错误为空，模型提交数为 0；见 [浏览器回执](linux-multi-reader-mu11-20261001/production-browser.json)。

## 当前已知限制

MU10 预构建超时按用户决定不挡本次发布，并未伪造为测试全绿。冻结的 8 GiB / 40 GiB 空闲容量、完整模型 / 渲染混合压力与实体 iPhone、Tauri 完整来源 / 富呈现、RE7 正式验收尚未补齐。此次继续部署及下面的修补没有再提交付费模型问题；上一轮获授权的一次真实 Provider 回执继续保留。

## 现网制作与历史入口修补

初次上线遗漏了服务根的 `presentation-sandbox.json` 与普通宿主账户的 systemd 启动权限，故制作能力为 `not_configured`。现已安装同 release 优化 worker、独立运行时 `/opt/understand-book/tools/render-runtime` 和专用账号的 polkit `manage-units` 授权，现网四项能力均为 true / ready。主服务继续以 understand-book 运行，生成任务仍以 nobody 隔离执行。[部署回执](linux-multi-reader-mu11-20261001/production-followup-deploy.json)。

使用现有 MU7 测试宿主、现网优化 worker 和实际服务账号完成四组实机验收：文件与凭据隔离、断网、Matplotlib / Manim / Chromium 真实产出；内存 / 进程 / 输出与临时盘边界；取消 / 超时 / 重启清理；网络制作的预览、私人保存和跨用户隔离。**4 通过，57.27 秒**。[日志](linux-multi-reader-mu11-20261001/production-render-integration.log)。首次按单元名过滤的 polkit 规则无法授权 systemd 252 的 StartTransientUnit；其授权详情没有 unit 字段，四项先失败，原因与配置更正见 [原始日志](linux-multi-reader-mu11-20261001/production-render-permission-first.log) 及 [发布运行单](../Linux多人阅读-MU11发布运行单.md)。

旧历史未丢失：ai-agent-engineering 实际返回 6 段，打开第一段显示 3 个旧问题；ai-infra-book-complete 的旧源本来没有聊天。新现场没有默认选择聊天，原空白提示容易与无历史混淆。`RightRail.vue` 现明确“本书历史”，空白区展示本书历史数量与打开入口；无历史时解释按材料分组。既有 22 项 RightRail 测试与 vue-tsc / Vite 网络构建通过。没有重跑数据迁移，也没有新增聊天或付费模型请求。

更新后的真实 Chromium 验证通过：空白区历史入口 → 6 段历史列表 → 打开旧记录 / 3 个问题 → 刷新保留；四项制作 capability 为 ready，不可用提示消失，脚本错误 0。[浏览器回执](linux-multi-reader-mu11-20261001/production-followup-browser.json)、[网络构建](linux-multi-reader-mu11-20261001/production-followup-web-build.log)。
