# Tutor 基础成品准入上线

2026-10-09 20:32:02 HKT 已部署至 [understandbook.top](https://understandbook.top/)。服务 active，公网登录、全部 10 个发布的就绪接口和 AI Infra Tutor 面板验证通过。

## 发布范围与结果

- 在现有 `/opt/understand-book/releases/inv9-20261009` 上仅更新 `crates/server/src/tutor_api.rs` 并构建、替换 Server；线上原文件与本地修复直接比较，差异仅包含本次准入调整。前端、其他程序和服务配置沿用原版。
- Linux `cargo build --locked --release -p server --bin server -j 1` 通过，耗时 3 分 26 秒。切换前在途或未保存运行数 0；启动后直接比较运行进程的可执行文件与目标程序，确认加载新版。
- 生产根继续使用 `/opt/understand-book/data/multi-reader-inv9-20261009`，schema 9。4 个账号、10 个发布、39 条授权、9 个默认选择保留；书籍重建 0、重新导入 0。
- 更新前 10 个发布均提示缺少回执；更新后 AI Infra 默认新版、量化精要、Agent Memory 综述三项 ready。其余 7 个发布实际缺少 discourse 或 BookStructure，接口反馈具体基础缺项。
- 公共 DNS、正常 TLS 验证下使用原 reader 账号登录，逐项读取发布就绪接口，再打开 AI Infra 默认新版的学习会话面板，实际显示“可以开始或继续学习”。页面错误 0，验收模型请求 0。

证据：[部署回执](deployment.json)、[更新前](before.json)、[更新后](after.json)、[实际界面](ai-infra-tutor-ready.png)、[构建日志](build.log)。本地回归见 [49 项验证记录](../tutor-artifact-admission-20261009.md)。

## 回退

旧程序、原 `tutor_api.rs`、原构建回执和服务配置保存在 `/opt/understand-book/backups/tutor-artifacts-20261009/`。确认没有在途或未保存任务后，停服务，恢复旧程序与该源码、构建回执，再启动服务即可；数据根和书籍版本不切换。健康检查使用正式 `Host: understandbook.top`；未登录身份接口应返回 401。

首次切换检查使用了回环 Host，收到 403 并自动恢复旧程序；日志确认候选后端正常启动。修正检查请求后重新切换及公网验收通过。

## 已知限制

AI Infra 的旧发布仍缺基础成品，使用已有默认新版即可。其他缺项来源此次未生成内容。服务器剩余约 1.5 GB 空间；T19 真实教学效果与成本验收仍待实施。
