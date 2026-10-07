# 多人服务删除单轮累计模型预算

上线时间：2026-10-01 18:05（UTC+8）。

## 行为

多人服务不再以单轮 64 次模型调用或累计 120000 Token 为停止条件。`model_calls_per_turn`、`tokens_per_turn` 配置及对应拦截已删除，累计用量也不再压低后续请求的输出额度。

所有模型目的继续共享公平并发、用户取消、授权检查和实际 usage 记录。单次请求显式输出上限及模型默认输出预留保持。实际执行结果直接保存到原回合，历史失败记录保持原义。

问题现场的一轮问答在五次模型调用后累计 122686 Token，第六次调用被旧服务拒绝；任务仍处于读取与整理证据阶段。持续执行规则见 [ADR-0150 §3](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md#3-多人服务模型调用与用量)。

## 验证

在 Linux 线上源码基线上执行 release 测试。修改前，三项回归分别复现第 65 次调用被拒绝、超过 120000 Token 后被拒绝、剩余额度把后续输出压成 1 Token；见 [修改前日志](remove-run-budget-20261001/red-tests.log)。

修改后，`cargo test --locked --release -p server --lib mu6 -j 1 -- --test-threads=2` 共 36 项通过，0 失败。覆盖调用次数、累计用量、三类模型请求的输出额度、未知 usage、重复累计 usage 帧、旧历史用量读取、取消、并发、接单保存、重启恢复及 SSE。集成用例在第一步超过旧 Token 上限后继续完成下一次调用，累计 240002 Token，回合完成且笔记已持久保存；见 [通过日志](remove-run-budget-20261001/green-tests.log)。

正式构建 `cargo build --locked --release -p server --bin server -j 1` 成功；见 [构建日志](remove-run-budget-20261001/build.log)。

## 生产发布

- Release：`/opt/understand-book/releases/mu12-20261001`。
- 源码范围：`service_limits.rs`、`run_admission.rs`、`tests/mu6_tests.rs`。部署前这三份本地原始文件与线上源码一致；仅发布本次修改。
- 切换前活动、排队、准备中或未保存运行：0。
- `understand-book-multi` 新进程为 784426，状态 active；运行中的可执行文件与本次构建产物为同一文件。
- 后端匿名 `/api/auth/me` 返回 401，公网 HTTPS 首页返回 200。
- 两个用户的聊天历史文件在切换前后逐字节一致。
- [机器可读发布回执](remove-run-budget-20261001/deployment.json)。

## 恢复点

`/opt/understand-book/backups/remove-run-budget-20261001` 保留原 Server 可执行文件及三份原始源码。需要回滚时，先等活动问答结束，停止 `understand-book-multi`，恢复对应文件后重新启动。用户数据目录保持原位。

## 验证范围

跨阈值模型调用使用 fake Provider 验证；上线检查覆盖实际程序、服务入口和聊天保留。本次没有重发原失败问题或提交新的真实模型问答。
