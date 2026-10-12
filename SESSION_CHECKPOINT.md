# SESSION_CHECKPOINT — 2026-10-12（TO／TB 已合并）

## 新鲜度自检
- 主工作区：E:/allwork/download/agent/understand-book；分支 main。
- 写入时最新提交：4aebee8 merge: integrate TO tool contracts with TB teaching and understanding；本页及集成记录随后以文档提交收口。
- 读入时对比 git log -3。TO 来源 fed0977；TB 来源 1b2a141；首页独立提交 6d93d12。

## 当前在做什么
TO0–TO14、选区保存修复和 TB1–TB5 已通过集成验证并合入 main；首页单独保存。代码合并完成，尚未推送或部署本轮成果。

## 下一步（可直接接手）
1. 阅读 docs/performance/integration-to-tb-20261012/README.md，查看提交、回归证据及两项测试环境修复。
2. 如继续发布，从 main 的已合并代码按既有发布流程构建，明确发布版本并保存部署／回滚记录。
3. 如处理 EXISTING 笔记问题，先读 docs/performance/tool-usability/to5/README.md 已知限制，再定位 Goal reader_action 与 reader.note 回执；选区保存意图已经修复，不要重复修。
4. TB6 留作后续跨材料与真人使用验收；从 TB5 实际理解视图和 TB4 已交付教具接续。

## 未提交 / 未完成
- TO／TB 与首页代码均已提交；原主工作区的本地书籍、临时文件和其他未跟踪输出保留。
- TB6 尚未完成；用户已选择先上线使用再反馈，没有学习收益结论。
- EXISTING 笔记可能仍无法满足 Goal 的 reader_action 完成判断，见 TO5。
- TO14 原始样本保留 13 通过、2 笔记失败、1 执行器未决；保存意图修复有独立红绿证据。
- 本轮未推送远程、未部署生产服务、未更新安装态插件；其他既有待办沿原方案。

## 冷启动读序
1. docs/performance/integration-to-tb-20261012/README.md — 合并记录、实际验证结果、测试环境与历史证据保存。
2. docs/切片方案-Tutor使命与认知积木.md 的并行边界／TB3–TB6、docs/切片方案-工具契约与Agent可用性优化.md 第 7 节 — 已集成的行为与原所有权。
3. docs/performance/tool-usability/selection-intent-fix/README.md、docs/performance/tutor-building-blocks-tb5/README.md — 保存权限和理解视图真实路径。

## 验证结果与环境
- Server 117、Runtime 149、Memory 1、Core 90、Web 20 项通过；Core 类型、Web 类型／生产构建通过。
- TB5 真实接口浏览器 1 条及 host 通过；首页浏览器 4 条通过；窄屏／桌面／纠正重开截图已核对。
- 工具参考生成器 7 项通过；生成与漂移检查通过，65 个工具条目、17 个 Tutor 场景；没有新增真实模型实验。
- 新 Windows checkout 的提示资产 CRLF 导致原发布合同不兼容，已用 agents/*.md text eol=lf 修复并重跑 Core。
- 两个 Runtime 旧用例需本机 quantification-essence 的 base.json/source.txt；补齐忽略目录后只重跑这两项，均通过。
- 集成工作区：E:/allwork/download/agent/understand-book-integrate-to-tb；分支 codex/integrate-to-tb；保留供回溯。
- Rust +1.95.0；CARGO_TARGET_DIR=D:/codex-build/understand-book-integrate-to-tb-195；TEMP/TMP=D:/codex-build/integrate-to-tb-temp；dev/test debug=0、incremental=0、jobs=2；Runtime codegen-units=16。
- TB2／TB5 新回归结果已另存集成目录，历史实验原件保持；本轮测试服务器均已停止。
