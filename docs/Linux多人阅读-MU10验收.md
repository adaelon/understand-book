# MU10 隔离、故障与容量验收

MU10 在独立服务根执行验收并产出发布门槛记录。冻结条件见 [MU0](performance/linux-multi-reader-mu0-20260930.md)，逐项结果见 [MU10 记录](performance/linux-multi-reader-mu10-20260930.md)。真实入口切换使用 MU11 运行单。

## 故障入口

`crates/server/src/tests/mu10_tests.rs` 使用 A/B、X/Y、临时 Memory / History / SQLite 和固定模型替身。HTTP 请求经过真实 Host、Cookie、CSRF、Origin 与对象授权。只有构造输入和检查持久结果使用内部接口。

| 用例 | 注入 / 确切断言 |
|---|---|
| `mu10_real_host_kill_commit_windows` | 父进程在 preparing / pending / teaching_receipt / prepared / queued / claimed / 私人笔记提交后 / terminal 八处杀掉子 Host；同根重开。缺 Pending 的键关闭；完整未领取回合只执行一次；教学原回应 / attempt / 绑定复用并补齐；claimed 不重放；笔记保留；已保存 History 只修索引 |
| `mu10_host_sqlite_busy_is_bounded_and_does_not_accept` | 另一 SQLite 连接持有 IMMEDIATE 写事务；真实 HTTP 返回 503、零接单、零模型调用；释放锁后原键可成功接单 |
| `mu10_slow_sse_does_not_block_commit_and_reconnect_resets_overflow` | 客户端停止读 socket，发布 4000 个 8 KiB 事件；发布与回答保存继续完成；旧游标得到替换快照 |
| `mu10_recovery_100_unclaimed_turns_keeps_original_inputs` | 在允许 100 接单 / 现场的隔离配置下构造 100 个完整 queued；重开并对账，100 个原输入与 attempt=0 保留，零模型调用，对账 ≤10 秒 |
| `mu10_linux_disk_full_keeps_unsaved_then_retries_without_model` | 仅把测试 History 放入独立 4 MiB tmpfs，写满到真实 ENOSPC；原 Pending 文件字节不变、未保存状态可见、聊天仍忙；移除填充文件后重试原结果，无模型重跑 |
| `mu10_linux_permission_failure_keeps_unsaved_then_retries_without_model` | 非 root 服务进程把临时 History 目录改为 0500，确认真实 PermissionDenied；同样验证保留原回合与恢复写入 |

提交点等待屏障与耗时采集均只编译进 Rust 测试程序。`MU10_KILL_POINT / MU10_READY / MU10_MANIFEST` 由父测试传给子进程，生产 Server 不读取这些变量。子进程测试为 ignored helper，不能单独启动。测试名中的 `windows` 指崩溃窗口，支持 Windows 与 Linux。

## 复现

Windows 使用一次验收专属的 TEMP/TMP；Linux 对应设置 TMPDIR。保留工作树中的 MU0–MU9 和 RE 依赖，再执行：

```text
cargo test --offline --locked -p server --lib mu10_ -- --test-threads=1 --nocapture
cargo test --offline --locked -p memory -p runtime -p server -- --test-threads=2
pnpm test
pnpm build
```

Linux 额外运行两个 ignored 故障用例。ENOSPC 用例需要允许挂载 tmpfs 的隔离验收进程；权限用例必须以普通账户运行，TMPDIR 须归该账户。测试自行卸载 tmpfs；不向宿主系统磁盘写满数据。发布包导入也在该非 root 用例的准备阶段真实执行。

## 冻结负载

`mu10_capacity_five_minutes --ignored --test-threads=1 --nocapture` 使用 10 用户、20 现场、两本材料、2 模型槽、每用户 1 活动回合和 2 未结接单。Provider 每次阻塞 10 秒，固定回答且 usage 未知。每用户持续 HTTP 读取，周期性保存私人笔记并在原回合结束后提交下一回合；负载运行 300 秒，再等未结回合排空。

`MU10_CAPACITY_REPORT=<absolute-json-path>` 保存原始样本数量、p95、每用户完成数、存活 Book 计费、模型并发峰值及结束后的资源占用。测试断言阅读 ≤250ms、私人写入 / 接单 ≤500ms、用户短锁 ≤50ms、控制库提交 ≤100ms、2 模型槽、每用户实际进展、资源池回空和 Book ≤2GiB。

锁时间采集覆盖 Host 现场操作、接单与终态保存持有用户锁的区间；控制库时间覆盖现场检查点事务、preparing 事务及 settled 更新。它们用于这一实际负载，不代表所有后台路径的全面 profiler。进程 RSS / 线程 / 文件描述符在 Linux 由 `/proc/<pid>` 外部采样，记录机器内存、磁盘及运行时依赖。

可用 `python3 scripts/linux/measure-mu10.py <cargo输出的测试二进制> <新的证据目录>` 启动负载并保存上述文件。采样器也断言 RSS ≤4GiB；指定机器条件和完整发布门槛仍由验收记录判定。

## 发布门槛

I01–I15 和 T01–T79 的每项结果记录实际执行环境、引用日志及剩余缺口。已有阶段证据和当前候选复验分别标明；需要实体设备、指定硬件、真实 Provider 或旧发布程序的项目不由模型替身代替。容量不足先定位实际争用，不更改冻结阈值。只有完整候选满足门槛，才能进入 MU11 对外开放。
