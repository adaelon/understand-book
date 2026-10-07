# MU10 逐项验收矩阵

本表按冻结合同逐项记录。通过表示所列范围有实际证据；部分项的剩余条件继续阻止相应正式验收。发布门槛和日志汇总见 [MU10 记录](linux-multi-reader-mu10-20260930.md)。

当前双浏览器复验：[Chromium 4 通过](linux-multi-reader-mu10-20260930/chromium-final.log)、[WebKit 4 通过](linux-multi-reader-mu10-20260930/webkit-final.log)。八个进程终止窗口：[Windows](linux-multi-reader-mu10-20260930/kill-final.log)、[Linux](linux-multi-reader-mu10-20260930/linux-kill-final.log)。

当前 Windows 多人回归：[119 通过](linux-multi-reader-mu10-20260930/mu-final.log)。MU2–MU9 用例位于 `crates/server/src/tests/mu*_tests.rs`；阶段报告位于同目录的 `linux-multi-reader-muN-20260930.md`。MU7 专属实机证据见 [MU7 报告](linux-multi-reader-mu7-20260930.md)，MU8 页面证据见 [MU8 报告](linux-multi-reader-mu8-20260930.md)。

## I01–I15

| ID | 正例 | 反例 / 失败断言 | 状态 |
|---|---|---|---|
| I01 | 本人已授权对象可读 | 身份伪造 / B 对象 / 资源 / 旧别名返回 401/403/404；MU4 | 通过 |
| I02 | A/B 相同 ID 各取自己的数据 | 坏用户、坏路径、坏 Store 不回退 local；MU2/MU4 | 通过 |
| I03 | 双窗口两笔提交都保留 | 未冲刷 / 活动实例不驱逐；MU2 | 通过 |
| I04 | A1/A2/B1 各自阅读原书 | 切书不改变另一个现场；MU5 | 通过 |
| I05 | 旧回答 / 笔记归原材料与聊天 | 旧代次 reader.state 与效果返回失效；MU1c/MU5/MU6 | 通过 |
| I06 | 同键同内容返回原 turn | 不同规范化内容冲突、已关闭键不能重开；MU6/MU10 | 通过 |
| I07 | queued 前真实文件与接单均提交 | preparing kill、存储失败不误报接受；教学回执真实 kill 补齐，见 T75 | 通过 |
| I08 | 完整未领取输入可重建一次 | claimed / 确认依赖丢失不重放；MU6/MU10 | 通过 |
| I09 | History 决定持久交付 | 事件溢出 / 重启仅重建投影，terminal kill 不再调用模型；MU6/MU10 | 通过 |
| I10 | 新能力发布保留旧绑定 | 改版 book_id / 来源变更、缺包与未就绪拒绝误绑；MU3 | 通过 |
| I11 | 同一用户 Tutor revision 一致 | 旧会话 / 旧 revision / 关闭后提交拒绝；MU5 | 通过 |
| I12 | MU7 真实隔离渲染交付 | 测试密钥 / 其他用户根 / 网络不可达，缺隔离不执行；MU7 | 通过（阶段实机） |
| I13 | Windows 本地与 MCP 既有入口可用 | MCP 私人库 / 写权限拒绝；预构建失败阻止完整回归结论 | 部分 |
| I14 | 现场 / Book / 模型 / 观察 / 制作均有额度 | 超额拒绝、卸载释放、SSE 不反压；冻结容量硬件未满足 | 部分 |
| I15 | 运行处理完后可删原聊天 | preparing/queued/claimed/unsaved 均 CHAT_BUSY；MU6/MU10 | 通过 |

## T01–T79

| ID | 场景 | 判定 | 实际证据 / 未完成项 |
|---|---|---|---|
| T01 | 无 Cookie 访问数据 API | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T02 | 伪造 body user_id / 身份头 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T03 | A 查询 B 的 workspace / chat / turn | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T04 | A 订阅或取消 B 的 Run | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T05 | 未授权用户请求 PDF / 图片 / 映射 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T06 | 对资源使用 HEAD / Range / ETag 条件请求 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T07 | 使用旧路由别名或 Host 旁路 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T08 | A 读取 B 的演示、候选、私人目标成果 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T09 | POST 缺 CSRF 或 Origin 不可信 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T10 | 登录失败洪泛 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T11 | 退出 / 改密 / 禁用账号后旧 Cookie | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T12 | 请求内部目录、..、编码穿越、symlink | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T13 | 从多人公网入口触发本地身份回退 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T14 | 普通用户访问 Provider / prebuild / 全站观测 | 通过 | MU4：真实 Host / 路由清单 / 账号、对象、资源、会话、CSRF 反例；当前 MU 回归 |
| T15 | A1 读 X，B1 读 Y，同时切书 | 通过 | MU5：双用户多现场、挂接 / 分叉 / 接管、检查点恢复；MU8 浏览器 |
| T16 | A1 与 A2 读不同书 | 通过 | MU5：双用户多现场、挂接 / 分叉 / 接管、检查点恢复；MU8 浏览器 |
| T17 | 刷新、复制标签页、旧连接仍存活 | 通过 | MU5：双用户多现场、挂接 / 分叉 / 接管、检查点恢复；MU8 浏览器 |
| T18 | 手机旋转 / 键盘 / 选区变化 | 部分 | MU8 浏览器：视口 / composition 保持原现场，零问题 POST；实体 iPhone 待 MU11 |
| T19 | 同用户两窗口同时写不同笔记 | 通过 | MU2/MU4：同名私人对象、同用户双句柄写入、容量 / 驱逐 / 损坏库反例 |
| T20 | 画像后台复核与用户显式修改并发 | 通过 | Server 画像治理 / 复核回写与 revision 回归；见 rust-full.log |
| T21 | 两用户使用相同遗留对象 ID | 通过 | MU2/MU4：同名私人对象、同用户双句柄写入、容量 / 驱逐 / 损坏库反例 |
| T22 | 私人缓存命中 / 用户 runtime 驱逐重载 | 通过 | MU2/MU4：同名私人对象、同用户双句柄写入、容量 / 驱逐 / 损坏库反例 |
| T23 | A1 关 Tutor，A2 旧运行尝试推进 | 通过 | MU5：tutor_shared_revision_and_old_teaching_control_are_enforced |
| T24 | 同用户同时改 Tutor 的同 revision | 通过 | MU5：tutor_shared_revision_and_old_teaching_control_are_enforced |
| T25 | 显式打开演示附属视图并追问 | 通过 | MU8 浏览器、network-client 与演示 / PDF 清理；RE 账号场景见既有 RE 报告 |
| T26 | A 退出后 B 在同一浏览器登录；纳入 RE 时含排版切换及展开批注 | 部分 | 当前 Chromium / WebKit 各 4 通过，草稿 / PDF / 演示隔离通过；RE 排版和批注沿既有阶段证据，实体环境待 MU11 / RE7 |
| T27 | 同键相同请求并发提交，包括 preparing 阶段 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T28 | 同键但问题 / 选区 / 材料 / 任务动作改变 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T29 | 202 响应丢失，重连查回 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T30 | 队列满、单轮请求超限、账号被禁用 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T31 | A 长模型任务，B 读写 / 问答 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T32 | 同聊天跨窗口同时提交不同请求 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T33 | 不同用户请求集中，A 大量开窗口 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T34 | 翻译、画像复核、压缩等旁路集中触发 | 通过 | MU6：同键并发、规范化冲突、响应丢失、容量 / 份额 / 旁路资源；当前 MU 回归 |
| T35 | 第一次 SQLite 提交后 kill，私人 turn 尚无 | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T36 | 私人准备完成后、queued 前 kill | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T37 | 普通问答 queued 后、领取前 kill | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T38 | claimed 后、是否发模型请求不明时 kill | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T39 | 工具已写笔记，终局前 kill | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T40 | 最终 History 已写、settled 前 kill | 通过 | MU10：真实 Host 子进程在指定提交边界被 kill，重开原服务根；linux-kill-final.log / kill-final.log |
| T41 | 历史终态提交 disk full / 权限故障 | 通过 | MU10：真实 tmpfs ENOSPC、普通用户 PermissionDenied；Pending 字节保留、CHAT_BUSY、原结果 retry-save |
| T42 | SQLite busy / JSON 替换失败 / WAL 恢复 | 通过 | MU10 Host SQLite 写锁；MU6 JSON 替换失败；MU9 WAL 备份 / 恢复 |
| T43 | 慢 SSE、事件缓冲溢出、断线 | 通过 | MU10 不读 socket + 32MiB 事件洪流 + 旧游标；MU6/MU8 断线与跨重启快照 |
| T44 | 重启后使用旧 SSE 游标 | 通过 | MU10 不读 socket + 32MiB 事件洪流 + 旧游标；MU6/MU8 断线与跨重启快照 |
| T45 | 取消 A 的 queued / claimed Run，并与领取交错 | 通过 | MU6 指定取消 / 领取交错；MU5 固定原 Run / 失效 Reader / 演示现场；MU8 浏览器 |
| T46 | 提问后 A1 切书 / 换聊天再收到旧回答 | 通过 | MU6 指定取消 / 领取交错；MU5 固定原 Run / 失效 Reader / 演示现场；MU8 浏览器 |
| T47 | 提问后手动改变当前演示版本 / 滑块 | 通过 | MU6 指定取消 / 领取交错；MU5 固定原 Run / 失效 Reader / 演示现场；MU8 浏览器 |
| T48 | 发布同内容新能力包，旧 Run 继续 | 通过 | MU3 不可变发布、原材料引用、readiness、只读 PDF 与存活 Book 预算；当前 MU 回归 |
| T49 | 内容更新新 book_id，旧引用点击 | 通过 | MU3 不可变发布、原材料引用、readiness、只读 PDF 与存活 Book 预算；当前 MU 回归 |
| T50 | 准备中的包 / readiness 不完整 | 通过 | MU3 不可变发布、原材料引用、readiness、只读 PDF 与存活 Book 预算；当前 MU 回归 |
| T51 | 只读挂载 published 后走全阅读链路 | 通过 | MU3 不可变发布、原材料引用、readiness、只读 PDF 与存活 Book 预算；当前 MU 回归 |
| T52 | 包缓存驱逐 / 旧包清理提议 | 通过 | MU3 不可变发布、原材料引用、readiness、只读 PDF 与存活 Book 预算；当前 MU 回归 |
| T53 | 沙箱代码读取测试密钥与其他用户哨兵 | 阶段实机通过 | MU7 Linux 四组真实执行器证据；文件 / 网络 / CPU / 内存 / 进程 / 磁盘 / 输出拒绝 |
| T54 | 沙箱访问外网 / 本机服务 / 元数据地址 | 阶段实机通过 | MU7 Linux 四组真实执行器证据；文件 / 网络 / CPU / 内存 / 进程 / 磁盘 / 输出拒绝 |
| T55 | 沙箱 fork / 内存耗尽 / 超时 / 大输出 | 阶段实机通过 | MU7 Linux 四组真实执行器证据；文件 / 网络 / CPU / 内存 / 进程 / 磁盘 / 输出拒绝 |
| T56 | 取消与主服务重启时遗留子进程 | 部分 | MU7 取消 / 超时 / 遗留单元清理已有实机证据；生产主进程重启与进程树回收留 MU11 |
| T57 | 沙箱输出 symlink / 路径逃逸 / 恶意 HTML | 阶段实机通过 | MU7 Linux 四组真实执行器证据；文件 / 网络 / CPU / 内存 / 进程 / 磁盘 / 输出拒绝 |
| T58 | 未安装 / 配置失败的沙箱，直接调用制作工具 | 通过 | MU7 missing_executor / invalid_sandbox_configuration：确定性关闭，禁止普通 Command 回退 |
| T59 | 生成 frame 伪造来源或宿主消息 | 通过 | MU7 iframe WindowProxy / 来源 / 通道 / schema；当前 Web 测试与 MU8 附属演示 |
| T60 | 迁移一次、重跑、在中间失败 | 通过 | MU9 真实私人文件、SQLite Backup API、写入者锁、独立根恢复 / 中断续接；当前 MU 回归 |
| T61 | 两个遵守锁协议的服务 / 维护工具打开同一写根 | 通过 | MU9 真实私人文件、SQLite Backup API、写入者锁、独立根恢复 / 中断续接；当前 MU 回归 |
| T62 | 备份并恢复多用户数据与包绑定 | 通过 | MU9 真实私人文件、SQLite Backup API、写入者锁、独立根恢复 / 中断续接；当前 MU 回归 |
| T63 | 旧二进制回滚及新旧入口切换 | 部分 | MU9 单用户独立导出根由当前读取器读取；指定旧 release 与入口停写切换留 MU11 |
| T64 | 遗留 pending 与新 queued 混合恢复 | 通过 | mu9_t64_restored_legacy_pending_and_network_queued_recover_separately |
| T65 | Provider 超时且 usage 不明 | 通过 | MU6 unknown usage / 预算 / 许可实际退出；MU4 观察撤权与 MU6 disabled_or_revoked_queued_requests |
| T66 | 禁用用户 / 撤销材料权限时运行仍活动 | 通过 | MU6 unknown usage / 预算 / 许可实际退出；MU4 观察撤权与 MU6 disabled_or_revoked_queued_requests |
| T67 | Windows / Tauri 正常启动与来源 / 演示 | 部分 | Windows Tauri 17 单测、当前程序启动 / 正文 / 私人笔记 / 刷新；来源和演示沿 Server / Web 回归 |
| T68 | Book MCP / Codex / DeepSeek 预构建回归 | 未通过 | Book MCP / CLI 通过；Core 与 DSH 预构建回归失败，详见本轮原始日志 |
| T69 | 日志、观测、错误和计量检查 | 通过 | MU4 私人存储错误脱敏 / Cookie 持久化；Server observability 白名单 / 计量回归 |
| T70 | 真实 Nginx HTTPS + iPhone Safari | 待 MU11 | 真实 Nginx HTTPS 与实体 iPhone 未运行，归 MU11 |
| T71 | 并发读 / 模型 / 渲染混合压力 | 部分 | 五分钟 10 用户 / 20 现场诊断；硬件条件不足，完整渲染混合压力未达冻结正式环境 |
| T72 | 有序停止与强制终止后启动 | 部分 | MU6 有序停止与 MU10 真实 kill / restart；生产停机、强制终止及恢复点演练留 MU11 |
| T73 | X 书 Run 排队 / 运行时切到 Y，再读取 reader.state / 构造定位上下文 | 通过 | MU1c/MU5/MU6：旧现场实时读失效、原材料私人写入、敏感确认重启失效 |
| T74 | 待确认画像操作的确认消息 queued 后重启 | 通过 | MU1c/MU5/MU6：旧现场实时读失效、原材料私人写入、敏感确认重启失效 |
| T75 | 教学准备回应已写、绑定或完成标记未写时 kill / restart | 通过 | MU10 Windows / Linux 在 teaching_receipt 真实杀掉子 Host；重启复用完整原回应 / attempt，补齐原绑定，单回合执行且保存。见 kill-final.log / linux-kill-final.log |
| T76 | A1 提问，A2 删除其 preparing / queued / claimed / unsaved 聊天 | 通过 | MU6 CHAT_BUSY、queued/claimed/unsaved 占位、删除 History 后故障与全现场清理 |
| T77 | 正常删除被多现场选中的聊天，在历史删除后、现场清理前重启 | 通过 | MU6 CHAT_BUSY、queued/claimed/unsaved 占位、删除 History 后故障与全现场清理 |
| T78 | 多现场持有不同 Book，LRU 驱逐后继续开页；闲置现场卸载再恢复 | 通过 | MU3/MU5 存活 Book 与现场卸载 / 重载；MU2 有效确认保活 / 到期失效 |
| T79 | 用户 runtime 闲置但仍有待确认操作；到期后回收再确认 | 通过 | MU3/MU5 存活 Book 与现场卸载 / 重载；MU2 有效确认保活 / 到期失效 |

## 2026-10-01 MU11 补充证据

以上表格保留 MU10 当次结果，新增证据见 [MU11](linux-multi-reader-mu11-20261001.md)。T62：真实 Linux 两账号快照恢复、私人文件相同通过；T63：现网旧 release 在独立端口读导出笔记通过，生产入口切换仍缺；T68：DSH 当前 35 项全覆盖通过，Core 全量 1061 通过 / 1 超时、该文件单独 25 通过；T70：真实 Nginx 测试证书 TLS 与 Playwright 双浏览器通过，可信入口 / 实体 iPhone 仍缺；T72：隔离 systemd 有序退出 0、强杀重启保留 saved turn 通过。真实 Provider 已执行一次，输出 23 / 上限 512 token；T71 正式硬件与渲染混合压力限制不变。

2026-10-01 后续发布：用户明确决定预构建相关失败可按通过处理、不挡部署。MU11 已完成优化构建、可信 IP HTTPS、生产迁入与切换；9 份材料 / 449 条记忆 / 17 段对话 / 25 回合迁入与真实浏览器通过。测试原始结果不变，正式容量与实体设备的未验范围单列。见 [现网说明](../Linux多人阅读-现网运行说明.md)。
