# SESSION_CHECKPOINT — 2026-10-12（TO／TB 集成中）

## 新鲜度自检
- 工作区：E:/allwork/download/agent/understand-book-integrate-to-tb；分支 codex/integrate-to-tb。
- 起点提交：fed0977（TO），正在合入 1b2a141（TB）。git log 与 MERGE_HEAD 为实际依据。

## 当前在做什么
将 TO0–TO14、选区保存修复和 TB1–TB5 集成。首页已单独保存为 6d93d12。共享文件冲突已处理，定向回归、浏览器路径及工具文档生成检查全部通过。

## 下一步（可直接接手）
1. 阅读 docs/performance/integration-to-tb-20261012/README.md，继续其中尚未完成的步骤。
2. 已完成 Server 117、Runtime 149、Memory 1、Core 90、Web 20 项；TB5 浏览器 1 条、首页 4 条及文档检查通过。
3. 本轮证据已保存在集成目录，历史实验原件已恢复；检查结果见集成记录。
4. 完成合并提交，将 main 快进到经过验证的集成结果；刷新本页。

## 未提交 / 未完成
- 当前合并尚未提交；不要切换或重置集成工作区。
- TB6 跨材料与真人验收后续进行，用户已选择先上线使用再反馈。
- EXISTING 笔记与 Goal reader_action 完成判断沿 TO5 已知问题。
- 尚未部署本轮成果，未更新安装态插件；原工作区本地材料和临时文件保留。

## 冷启动读序
1. docs/performance/integration-to-tb-20261012/README.md — 提交、合并依据、验证目的与进度。
2. docs/切片方案-Tutor使命与认知积木.md 的并行边界和 TB3–TB6；docs/切片方案-工具契约与Agent可用性优化.md 第 7 节 — 原合同和符号所有权。
3. docs/performance/tool-usability/selection-intent-fix/README.md 与 docs/performance/tutor-building-blocks-tb5/README.md — 保存权限和理解视图回归入口。

## 验证环境
- Rust +1.95.0；CARGO_TARGET_DIR=D:/codex-build/understand-book-integrate-to-tb-195。
- TEMP/TMP=D:/codex-build/integrate-to-tb-temp；dev/test debug=0、incremental=0；Runtime codegen-units=16。
- 集成工作区依赖目录复用主目录已安装的 pnpm 依赖。
