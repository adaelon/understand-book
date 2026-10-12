# TO 与 TB 集成 — 2026-10-12

## 范围与步骤

目标：将 main 的 TO3–TO14、选区保存修复与 reading-share 的 TB1–TB5 合入 main，保留原有工具契约、教学指导及学习记录行为。用户已确认按此顺序实施。TB6 的跨材料与真人验收后续进行。

- [x] 分别保存首页、TO、TB 提交；仅收录对应源码、测试和复现依赖。
- [x] 在独立集成工作区合并共享文件，按符号职责解决冲突。
- [x] 完成受影响的 Runtime、Server、Core、Web 验证，生成工具参考文档。
- [x] 保存集成提交，合回 main，刷新 checkpoint。

起点：main `93b476d`，reading-share `9f1ea18`。TO3–TO14、选区保存修复、首页和 TB1–TB5 均为起点上的未提交修改；TB 已同步 TO3。

## 合并依据

- Tutor 工具定义、参数示例与状态投影沿 TO；COMMON、SELECT、UNDERSTANDING 和呈现阶段指导沿 TB。
- orchestrator 同时保留工具发现／交付／保存意图修复与 TB 来源后缀修复。
- 教学事实、帮助条件、理解回读沿 TB；共同结果预算和输出契约沿 TO。
- Server 注册和双方测试取并集；既有 TO3 重复内容只保留一份。
- checkpoint 改为集成状态；原实验记录及其当时结论保持历史身份。

## 验证目的与失败处置

- Runtime 工具契约、发现／交付、Tutor 和来源用例：检测合并丢分支、指导与可用操作不符、来源后缀处理回归；修对应合并接点。
- Server 教学示例、回读／帮助／接续、选区保存用例：检测重复模块、事实或帮助丢失、保存权限回归；修注册或原业务接点。
- Core Executor 相关用例：检测提交遗漏新增生命周期依赖及接口漂移；补齐源文件或修契约接线。
- Web 组件、类型、构建及 TB5 浏览器路径：检测理解条件遗漏、教法保存／纠正重开失败和前后端类型不符；修对应组件或接口。
- 工具参考生成与漂移检查：检测最终工具定义未进入文档或生成依赖遗漏；从合并源码重新导出。

集成使用独立 Rust target。已有历史模型实验不重新采样；本次回归在隔离本地存储运行。

## 已知问题

- TB6 真人及跨材料验收尚未完成；没有学习收益结论。
- EXISTING 笔记回执可能仍无法满足 Goal 的 reader_action 完成判断，见 TO5 已知限制。
- TO14 原始样本保留 13 通过、2 笔记失败、1 执行器未决；选区保存修复另有红绿证据，不改写原实验。

## 结果

首页 `6d93d12`；TO `fed0977`；TB `1b2a141`。集成提交 `4aebee8`，main 已快进到该提交；随后以 `55e767f` 更新本记录与 checkpoint。集成收口时尚未推送或部署；同日 22:36:57（Asia/Hong_Kong）已发布该源码并通过正式 HTTPS、数据保留和账本验收，见 [TO／TB 发布记录](../to-tb-release-20261012/README.md)。

Git 报告四个冲突文件：checkpoint、Runtime 契约测试、代码链路和 TO 方案。契约测试保留 TO 后续新增的 47 个示例及 TO13 导出；代码链路合并双方记录；TO 方案保留完整完成状态。共享业务代码自动合并后逐接点核对，保留 TO 保存意图修复与 TB 教学／来源修复。


## 集成中修复

新 Windows worktree 将 agents/*.md 检出为 CRLF，原发布合同对提示资产的校验返回 release_prompt_hash_mismatch，DSH 回归出现五项失败并阻断后续流程。原始日志保存在 core-tests.log；确认共同原因后中止该批次。给现有提示资产增加 text eol=lf 属性，保留合同与提示正文，重新检出为 LF 后重跑。该规则覆盖正常 Windows checkout；没有新增校验机制。

## 复现环境与命令

集成工作区为 E:/allwork/download/agent/understand-book-integrate-to-tb。Rust 使用 RUSTUP_TOOLCHAIN=1.95.0、CARGO_TARGET_DIR=D:/codex-build/understand-book-integrate-to-tb-195、CARGO_PROFILE_DEV_DEBUG=0、CARGO_PROFILE_TEST_DEBUG=0、CARGO_INCREMENTAL=0、CARGO_BUILD_JOBS=2；TEMP/TMP=D:/codex-build/integrate-to-tb-temp。

```powershell
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib -- tutor_ continuation_ selection_intent:: --test-threads=1
pnpm --filter @understand-book/core test test/automatic-build-executor-session.test.ts test/build-executor-mcp.test.ts test/build-executor-tool-adapter.test.ts test/dsh-build-session.test.ts test/executor-transport.test.ts test/build-executor-version-boundary.test.ts
pnpm --filter @understand-book/core typecheck
pnpm --filter @understand-book/web test src/components/TutorPanel.test.ts src/components/UnderstandingSpace.test.ts
pnpm --filter @understand-book/web build
pnpm docs:tools:test
```

Server 的 TB2 回放会写自身历史证据目录；本轮在独立工作区运行后将新结果另存于集成记录，再恢复历史原件。浏览器沿 TB5 README 的冻结夹具与真实接口运行，截图和回执同样单独保存。


## 定向验证结果

| 范围 | 结果 | 证据 |
|---|---|---|
| Server 教学、理解、接续、选区保存 | 117 通过，19 按原配置忽略 | server-tests.log |
| Runtime 契约、工具暴露、结果交付、Tutor、来源 | 147 首轮通过；2 项补齐本机夹具后通过，共 149 | runtime-tests.log、runtime-fixture-recheck.log |
| Memory 学习证据重放与纠正 | 1 通过 | memory-tests.log |
| Core Executor、MCP、DSH、传输与版本边界 | 90 通过；类型检查通过 | core-tests-green.log、core-typecheck.log |
| Web Tutor 与理解视图 | 20 通过；类型检查与生产构建通过 | web-tests.log、web-build.log |
| TB5 真实接口浏览器 | 1 条完整路径与 host 通过；已核对窄屏、桌面与纠正重开截图 | tb5-browser.log、tb5-host.log、tb5-browser/ |
| 首页浏览器 | 4 通过 | home-browser-final.log |
| 工具参考生成器 | 7 通过 | tool-reference-tests.log |
| 工具参考生成与源码漂移 | 通过；65 个工具条目、17 个 Tutor 场景 | tool-reference-generate.log、tool-reference-check.log |

Runtime 两个旧用例依赖被 Git 忽略的 `.understand-book/quantification-essence`，首次失败是缺少 base.json。仅复制主工作区现有 base.json/source.txt 到集成工作区的忽略目录，然后只重跑这两个用例；没有修改原材料或测试判据。未把本机书籍加入提交。

浏览器最初复用依赖目录时，Vite 拒绝工作区外的字体文件；首页用例虽通过，但不能据此确认字体加载。随后只在本地测试服务器允许主工作区 node_modules 路径，重新完成首页与 TB5 验证；没有修改产品 Vite 配置。最终截图无横向溢出、文字和帮助条件可读，纠正内容重开后保留。

复跑 Runtime 时沿上述 Cargo 环境使用 `cargo --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib -- tool_registry:: tool_exposure:: tool_result:: tutor source_presentation book_handoff book_continuation book_outputs structured_result write_examples --test-threads=1`；Memory 为 `cargo test -p memory --lib learning_evidence -- --test-threads=1`。工具文档用 `pnpm docs:tools` 与 `pnpm docs:tools:check` 生成和检查。
