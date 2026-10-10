# 工具契约修复 — 2026-10-09

用户要求：修完本次工具审计确认的问题并上线。已于 2026-10-09 16:11:57（Asia/Hong_Kong）完成部署，16:12:23 公网验证通过。

- 工作目录包含其他任务的未提交修改；仅发布 `tmp/tool-contract-fix-20261009/manifest.json` 指定的 19 个文件。
- 运行中服务：`understand-book-multi`，新 PID 392043，旧 PID 382648；发布目录 `/opt/understand-book/releases/adm10-20261008`。
- 服务器验收目录 `/opt/understand-book/acceptance/tool-contract-20261009`，`backup/` 保留原 server 与 presentation_worker。build.sh 在完成后恢复默认二进制，候选程序存为 `*.candidate`。
- 最终构建成功，`build.exit=0`，用时 4 分 48 秒。发布前 run_admissions 没有 preparing/queued/claimed 或 unsaved 记录。
- Runtime 484、Book 10、Artifact 8、Server MCP 14、Presentation 17、Tutor 17 项通过；34 个 schema、35 个独立输入案例通过；实际模型接收最终定义且返回有效 write 参数。
- 验收完成：两程序已原子替换并重启，实际运行字节与候选一致，启动日志正常；公开与登录后 7 项接口通过，9 本书及四项制作能力保持。无待完成发布步骤。回退方式及忽略测试范围见验收文档。
- 详细依据：[验收与发布](docs/performance/tool-contracts-20261009/README.md)。原视频任务状态仍在根 SESSION_CHECKPOINT.md。
