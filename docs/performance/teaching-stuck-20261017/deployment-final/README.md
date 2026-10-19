# 连续卡住修复发布 — 2026-10-17

最终修复已于 **2026-10-17 09:16:43 HKT** 上线，现网版本为 `/opt/understand-book/releases/teaching-scaffold-20261017`。主进程 PID **110819**，服务 active；实际可执行程序、WorkingDirectory、演示 worker 配置和 Nginx 根均已确认指向该版本，见 [after-state.json](after-state.json)。

## 发布范围

候选从此前已上线的 `teaching-initiative-20261017` 复制源码和运行资源，仅覆盖本轮冻结的五个生产文件：runtime 的 tutor.rs、orchestrator.rs，memory 的 teaching_design.rs，以及 server 的 teaching.rs、teaching/design.rs。见 [release-source.json](release-source.json) 与 [source-comparison.json](source-comparison.json)。Web 沿用已发布的构建，schema14 和数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012` 不变。

Linux 独立构建 server、manage_reader、publish_book、reader_maintenance、presentation_worker 成功，耗时 **10 分 26 秒**，exit 0。见 [build.log](build.log) 与 [build.exit](build.exit)。源码归档保存在 `tmp/teaching-stuck-final-release-20261017/source.tar.gz` 及服务端 acceptance 同名归档。

## 空间处理

准备时可用空间为 833,671,168 bytes。仅清理两个已确认非运行旧版本的编译缓存：`current-20261016/target/release/{deps,build,.fingerprint,incremental}` 和 `working-tree-c12cbb9-huawei-viewport-20260919-1140/target/debug`，约 1.21 GB。执行前再次确认旧路径没有运行进程；旧正式程序、源码、Web、用户数据和验收证据保留。见 [cache-cleanup.json](cache-cleanup.json)。

复制候选时排除 target，避免重复约 595 MB 的缓存，复制完成后可用空间 1,878,151,168 bytes。构建、完整备份及发布完成后可用空间 **921,817,088 bytes**。

## 验证与切换

发布前代码结果：runtime 完整回归 622 pass / 8 ignored；随后仅调整目标描述，合同测试 1 pass；server 教学设计 12 pass / 1 ignored；相关 tutor 组 115 pass / 19 ignored。改后 live-02 已正常结束，6 轮、36 请求，均 HTTP 200。独立 6.1 sol 语义审阅结论与限制单独记录，见 [validation.json](validation.json)，没有把语义审阅写成全面效果证明。

第一次发布尝试在进入维护前发现一个 claimed、unsaved=0 的活动运行，因此停止于预检；没有修改 Nginx、服务或数据。见 [deploy-preflight-active.log](deploy-preflight-active.log)。等待它自然完成后，2026-10-17 09:15:37 HKT 只读复查活动集合为空，见 [activity-wait.jsonl](activity-wait.jsonl)。没有取消或改写用户运行。

随后按既有流程进入维护、再次确认没有活动或未保存运行、停止服务并完整备份，再一起切换主服务、演示 worker 和 Nginx。成功脚本始于 **09:15:50 HKT**，完成于 **09:16:43 HKT**；停服至本机验收为 **14.46 秒**。完整回执见 [deployment.json](deployment.json)，日志见 [deploy-success.log](deploy-success.log)。

本机和正式域名均验证 reader 11 项、admin 12 项授权版本清单前后相同，默认书目录和正文可读，四项演示能力 ready。受限书仍对 reader 返回 404。源站首页与阅读器入口精确字节匹配；公网入口引用的项目 JS/CSS 清单与资源字节匹配，两入口共 9 项引用资源通过。Cloudflare 注入的额外 analytics beacon 不参与项目资源对比。此次无 Web 修改，仍为 `reader-BbGgozCW.js` / `reader-BRVIARO0.css`。发布检查外部模型调用 **0 次**。

## 备份与回退

完整数据备份为 `/opt/understand-book/backups/teaching-scaffold-20261017`，大小 **351,421,712 bytes**。原配置保存在 `/opt/understand-book/acceptance/teaching-scaffold-20261017/previous-configuration-final`。回退程序 `teaching-initiative-20261017` 完整保留；需要回退时沿用维护流程恢复 unit、sandbox 与 Nginx，保留 schema14 的最新数据。完整备份仅用于确有数据损坏时恢复。

发布控制脚本和冻结归档位于 `tmp/teaching-stuck-final-release-20261017/`；本目录保存 control.py、deploy.py、static-check.py 和全部关键回执，远端证据位于 `/opt/understand-book/acceptance/teaching-scaffold-20261017/`。

## 已知限制

独立语义审阅确认这组六轮对话能够在用户持续卡住时缩小推理跨度、提供必要帮助并完成局部目标收束，未发现实质原文错误。首轮 accepted 反事实仍出现得偏早，后续及时修补。本次没有交互页面和独立迁移证据；模拟对话不等同于真人学习成效或统计收益。详见 [live-02/conversation.md](../live-02/conversation.md) 与 [content-review.md](../live-02/content-review.md)。部署验收确认版本、业务数据、能力与资源生效，没有额外声称线上真人教学质量已经验证。
