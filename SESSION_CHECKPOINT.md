# SESSION_CHECKPOINT — 2026-10-11 15:24（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：8e3a76c chore: establish local development baseline for parallel features；已核对 git log -3。
- 工作区 E:/allwork/download/agent/understand-book-video-understanding；分支 codex/video-understanding。读入时对比 git log -3，不一致以 Git 为准。

## 当前在做什么
**VI18 在 Windows 实际验证范围内完成。用户明确要求的整讲视觉、图谱和讲解已全部接纳、发布并验收，最终播放通过。**
- 543/383/383 个视觉/图谱/讲解任务，720 段英文及中文；四项能力均覆盖 0–4641376.145 ms。原英文、base 和时间映射保持，原正式材料及 VI16 pending 草稿、VI17 修订副本保留。
- 最终 Book 19ac880d-a788-43f4-9725-6c56f4980189；publication 01a129cf-c1b7-72c2-bac7-40f9740fda6f。
- 最终快照：D:/codex-build/understand-book-vi18/production/service/library/published/19ac880d-a788-43f4-9725-6c56f4980189/01a129cf-c1b7-72c2-bac7-40f9740fda6f；3769317314 字节（3.51 GiB）。
- 1262 个视觉单元（新增1240）、863 个独立图谱节点、1046 条图谱贡献关系、917 条讲解及508条关系；两处原图/语义样本与最终发布对照通过。
- 主控 production/complete-state.json 为 complete，visual/graph/discourse-result complete，verification.json 和最终 reader.json 均 passed。无需继续生产或重跑已接纳成果。
- Windows NSIS：D:/codex-build/understand-book-vi18/package-source/dist/UnderstandBookSetup.exe，134934553 字节；全部发布门通过，现有安装未升级。
- 独立分发目录 reader-installed：编译 Engine 完成全部383项讲解；七项实际导入/Whisper/恢复/发布/原声检查通过。最终release Reader 1080p、三处跳转、1.5倍速、刷新、Range/HEAD通过，就绪4381ms，无脚本错误；Windows Edge双窗口保持通过。
- Core59个不同用例、Server视频22 passed / 1 ignored、Core类型检查/Web build通过。候选反馈2项红→绿，连续会话3项；46个保存反馈的任务全部接纳。
- 约5.99小时墙钟、已保存运行累计模型17.30小时；输入250547970 tokens（缓存225384064），输出3505399；金额未知，中断且无最终用量事件的运行不计入token总数。

## 下一步（可直接接手）
1. 查看 docs/验收-视频理解.md 的VI18及 evals/video/results/vi18-20261011/verification.json、reader.json；最终产物已完成，不需继续模型生产。
2. 如需重放最终成果，用 evals/video/vi18-reader.mjs 指向上面的快照、package-source/packages/web/dist 和独立reader-installed目录；正式维护命令见VI18安装说明。
3. 后续扩展验证按方案§9：Provider live、DSH、Linux、完整逐字真值、原生安装升级/Tauri WebView；原VI16草稿须沿维护入口选择采纳。

## 未提交 / 未完成
- VI0b/VI0c及VI1–VI18工作树改动全部未提交；保留其他任务既有修改，没有commit或升级现有安装。
- 本次整讲生产、报告和checkpoint已完成。读时真Provider、DSH、Linux、逐字真值、原生升级/WebView属于既有待验范围。
- 整讲视觉/图谱使用源码入口；编译入口整讲视觉仍无完整通过凭据。早期视觉长进程缺最终回执原因未定；编译讲解和独立短片安装验证通过。
- 原8793/5175服务未重启；原E:/allwork/Understand Book安装及相关其他进程未动。
- 存储：缓存删除被自动审批blocked by policy，未执行；后以Windows透明压缩及已结束记录迁移完成。C/D/E最终余量约1.02/0.96/1.44GB，全部文件内容保留；见storage.json、D根compression*.log。
- part-1/.build实际位于本工作区.worktree-local/vi18-records/part-1-build；part-4/.build位于C:/codex-build/understand-book-vi18/part-4-build；D原路径为目录连接，最终verifier已通过这些路径。迁移或清理时须保留记录与连接目标。
- 模型SQLite仍在工作区.build/codex-state；未改全局Codex配置。构建加载.worktree-local/environment.ps1后用D:/codex-build/understand-book-vi12、debug=0、incremental=0、D盘TEMP/TMP，保持串行构建。

## 冷启动读序
1. docs/VI18-实施状态.md、docs/验收-视频理解.md VI18；results/vi18-20261011/verification.json、reader.json、storage.json — 最终状态、凭据与物理位置。
2. docs/视频安装与整讲验收-VI18.md、docs/切片方案-视频理解与渐进伴读.md VI18/§7–9 — 正式命令、验证范围与待验入口。
3. evals/video/vi18-run.ts、vi18-verify.ts、vi18-reader.mjs — 完整恢复的一次发布、覆盖/用量与实际Reader验收。
4. skills/build/video-executor-runtime.ts、video-semantic-series.ts；Core video-lecture.ts、video-graph.ts、video-discourse.ts、video-visual-scan.ts — 编译重入、输入上限、候选反馈与流式抽帧。
5. docs/代码链路.md、docs/架构.md VI18；VI17说明及ADR-0146 — 入口、分发、来源改版及前版边界。

## 本会话决策摘要
- 分发采用编译Engine/Rust入口与随包媒体工具，Python模型由用户配置；见VI18安装说明。
- 四分区保持单写者，最多三个连续独立语义任务；前一贡献接纳后冻结下一输入。候选错误有限反馈，I/O错误保持失败；见代码链路VI18。
- 已全部接纳的图谱/讲解一次汇总发布，避免重复中间快照；原运行日志保留，报告引用路径和用量；见架构VI18与storage.json。
