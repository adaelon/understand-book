# Linux 多人阅读 MU2 用户私人持久化

日期：2026-09-30。基线 HEAD `5e10516`；实现基线是 MU1c 后实时工作树，源码副本在 `tmp/mu2-baseline`。既有工作保留。

## 实施步骤

- [x] 读取 checkpoint 全部冷启动合同，核对私人路径与后台入口。
- [x] 显式路径、服务元数据基础 schema、OS 写入者锁与用户注册表。
- [x] 用户范围后台访问、确认身份/到期、闲置回收与持久化边界。
- [x] A/B 隔离、同用户写入、故障与回归测试。
- [x] 更新架构、代码链路、方案状态与 checkpoint。

## 固定规则

待确认敏感操作有效期 10 分钟，重复提交同一候选不续期；Run 冻结随机操作 ID，使用时再核对身份和到期时间。临时候选和有效期不写入历史。原操作缺失、重启丢失或过期时确认返回 `SENSITIVE_CONFIRMATION_EXPIRED`，要求重新提交并确认。

用户最后一次注册表访问超过 30 分钟后可回收；注册表只回收没有外部句柄、没有 pending 回合或运行中的复核/回填、没有有效确认的实例。Run、后台任务、未保存结果和维护操作须持有该用户句柄。回收前冲刷已读并提交历史；失败保留实例。两个现场取得同一用户句柄，共用短写锁。常驻上限 10，分配前尝试闲置回收，无可回收空间返回 USER_RUNTIME_CAPACITY。

服务库基础表为 users、auth_sessions、book_publications、book_grants、reader_workspaces、run_admissions；只提供受信用户创建和路径读取，登录、发布、接单状态转换在后续切片接入。服务库不保存候选正文、聊天正文或学习投影。

服务与维护工具先获得 `service.lock` OS 独占锁；锁文件不删除，进程退出释放锁。Memory 与 Learning 连接持有锁租约，注册表退出不能使仍存活用户或数据库连接脱离写锁。私人根不读取环境或回退到本地用户。

服务库使用 schema 1 与独立 application ID；未知版本、其他数据库或未登记的已有表拒绝写入。Learning 保留原事件/投影表及 DELETE journal，为已知旧表登记版本和独立 application ID。实际链接 rusqlite 0.40.2 / libsqlite3-sys 0.38.2 / SQLite 3.53.2。服务库 WAL/FULL、外键及 5 秒 busy timeout 读取实值，启动记录不含私人路径。

Linux JSON 提交在内容 sync 后同步目录项，新建目录同步父目录。Memory/History 保留原备份恢复；Intent 移除“旧文件改成随机备份 → 新文件就位”的空窗，改用一次原子替换。原随机备份没有恢复入口，在中间退出会造成索引缺失，这是本片修复的真实持久化问题。

## 验证记录

首次 all-targets 编译发现 rusqlite 0.40 的无符号整数转换改为 opt-in；启用 `fallible_uint` 保留原有范围检查，不改变 Learning 序列类型。首次失败日志：[check.log](linux-multi-reader-mu2-20260930/check.log)，修正后的编译通过见 [check-2.log](linux-multi-reader-mu2-20260930/check-2.log)。

首批 MU2 测试 8 通过/1 失败，失败来自 Windows 规范化根含扩展前缀，而夹具断言比较了未规范化的根；按同一 canonical 根比较后复验通过。之后新增后台归属及私人演示/Intent 测试，12 项通过。原始记录：[mu2-first.log](linux-multi-reader-mu2-20260930/mu2-first.log)、[mu2-tests.log](linux-multi-reader-mu2-20260930/mu2-tests.log)。

`cargo test -p memory -p server -- --test-threads=1` 首次全量：Memory 124 通过；Server 334 通过、24 失败、33 原有忽略，383.20 秒。它使用了旧测试默认临时根，遗留共用 learning.db 的 user_version=1/application_id=0 是本轮早期测试产物，被最终版本门槛拒绝。保留拒绝与失败结果，未改共享临时库、未放宽 schema 门槛，见 [regression.log](linux-multi-reader-mu2-20260930/regression.log)。

后续命令仅给测试子进程设置 `TEMP/TMP=tmp/mu2-final-isolated`，业务服务未按用户修改进程环境：

- `cargo test -p memory -p server --lib mu2_ -- --test-threads=1 --nocapture`：Memory 新增 1 项、Server MU2 14 项均通过；覆盖最终锁租约、schema、容量、busy 与确认/回收实现。见 [mu2-final.log](linux-multi-reader-mu2-20260930/mu2-final.log)。
- 从首次日志提取 24 个失败用例，对最新 Server 测试可执行文件传入 `--exact --test-threads=1 <24 个用例名>`：**24 全部通过**，27.09 秒。见 [regression-retry.log](linux-multi-reader-mu2-20260930/regression-retry.log)。
- `cargo test -p server --lib host::tests:: -- --test-threads=1`：**24 全部通过**，1.33 秒，覆盖最终后台归属读取、复核期间前台新写入、启动和有序冲刷。见 [host-final.log](linux-multi-reader-mu2-20260930/host-final.log)。

- `cargo test -p server --bins --test presentation_preview -- --test-threads=1`：Book MCP 5 项、CLI 1 项、演示预览集成 1 项通过，10 项原有忽略。见 [binaries-final.log](linux-multi-reader-mu2-20260930/binaries-final.log)。

按不同用例汇总，Memory 125 项、Server 库 358 项、二进制/集成 7 项通过，共 **490 项**；43 项原有忽略。该数字合并首次全量和修正后的定向复验，**不是一次全量命令全绿**；doc-tests 未重跑。本轮没有调用付费 Provider 或运行原有忽略的真实浏览器验收；保留既有 ts-rs serde 属性解析警告。

## 已知限制

本片提供服务持久化内核，网络用户/现场挂接仍在 MU4/MU5；接单/排队及跨存储恢复在 MU6，维护/迁移工具使用同一 ServiceWriter。没有迁移真实用户、部署或开放多人入口。

Linux 的目录同步代码尚未经历实机 kill/掉电验收；Windows 仍沿原文件同步/恢复合同，sync_parent 不承诺 Windows 掉电目录耐久。测试证明独占锁跨进程有效并随进程退出释放、已提交数据库可重开及受控 IO 故障不丢内存待写项，不代表完成 MU9/MU10 全套崩溃窗口。服务根要求本机可靠文件系统；HTTPS/iPhone 和容量仍需后续实测。
