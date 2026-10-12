# SESSION_CHECKPOINT — 2026-10-12（Asia/Hong_Kong）

## 新鲜度自检
- 整合分支：`codex/integrate-mcp-tutor`；原 main 为 `12b765b`。
- main 原未提交成果已保存于 `45b08ba`；MCP0–MCP9 已保存于 `8e7e5fe`（`codex/reading-share`）。
- 整合已完成，main 与整合分支指向同一合并提交；精确提交号以 `git log -1` 为准。验证及保留问题见 [整合记录](docs/performance/integration-mcp-tutor-20261012.md)。

## 当前在做什么
Tutor TC0–TC6、运行时修复、构建及阅读器改进与 MCP0–MCP9 已整合到 main；下一步按 Tutor TC7 或 MCP10 接手。

## 下一步（可直接接手）
1. 先读 [整合记录](docs/performance/integration-mcp-tutor-20261012.md)：Web 273、Admin 30、Rust 295 项通过；Core 累计 206 通过，1 项原有 BookStructure 测试仍失败，更新该用例须遵循当前大纲／发现路线。
2. Tutor 继续 [切片方案](docs/切片方案-Tutor下一步协商与低门槛阅读入口.md) TC7、第 9／11 节：补齐工程矩阵、固定材料轨迹及真实读者验收。
3. TC7 通过后进入 TC8，验证旧二进制读取、同版本撤入口／指导的降级及发布准备。
4. MCP 继续 [实施入口](docs/内测MCP-实施入口.md) MCP10：编写发布运行单，验证 schema 12 备份／升级／恢复及目标 Linux／HTTPS 代理。
5. MCP10 按 4 个活跃读取任务起步复测容量，核对授权、引用、扣次及旧账号／材料／模型费用后更新发布状态。

## 已保存成果与未完成
- TC0–TC6：类型化阅读入口、建议交付与单次接纳、普通／演示宿主、恢复、真实用户表达与理解／画像证据分流。见 [TC6 实施记录](docs/performance/tutor-continuation-tc6.md)。
- main 其他成果：Tutor 回合上下文／压缩／输出预算及无进展修复、构建缓存与写入恢复、前端体验和产品首页；部分原成果已有部署记录。本轮合并不改变线上部署。
- MCP0–MCP9：凭证、获准读取、月度免费次数及付费调用包、远程协议、来源定位、运营管理和集成验收；见 [MCP9](docs/内测MCP-MCP9实现.md) 与 [验收记录](docs/performance/beta-mcp/README.md)。
- TC7–TC8、MCP10 未实施；真实读者验收、通用教学质量、Linux／HTTPS、备份恢复仍待各自后续切片。
- TC6 实模仅一条脚本化纠正轨迹，真实读者为 0；TC2 公开内容质量问题仍保留。旧二进制对新增教学事件／Move／准备记录的兼容性待 TC8。
- MCP 原验收记录中的 MU4 旧夹具已改为事件写入，相关 13 项回归通过；原始失败记录保留。一个原有 Core BookStructure 用例仍待独立更新。
- 本地临时目录、测试输出和历史实验仍保留；两个原 worktree 均保留。Computer Use 继续保持先前停止状态。

## 冷启动读序
1. 本 checkpoint、Git 状态及整合验证记录；根据准备接手的工作线选择下面入口。
2. Tutor：[切片方案](docs/切片方案-Tutor下一步协商与低门槛阅读入口.md) TC7、第 7／9／11 节，[ADR-0163](docs/adr/0163-tutor-continuation-and-contextual-reading-entry.md) §4–§7，[TC6 实施记录](docs/performance/tutor-continuation-tc6.md)及其真实轨迹。
3. Tutor 实现：`crates/server/src/interaction_input.rs`、`teaching.rs`、`tests/tutor_continuation_tests.rs`；`crates/memory/src/teaching.rs`、`learning_evidence.rs`；`packages/web/src/components/TutorContinuation.vue`、`App.vue` 与 `local-reading-action.ts`。
4. MCP：[实施入口](docs/内测MCP-实施入口.md)、[产品约定](docs/内测MCP-产品约定.md)、[切片方案](docs/切片方案-内测MCP与调用额度.md) MCP10、[MCP 接入](docs/MCP接入.md)、[MCP9 验收记录](docs/performance/beta-mcp/README.md)。
5. MCP 实现：`crates/server/src/control_store.rs`、`mcp_access.sql`、`mcp_usage.sql`、`mcp_packs.sql`、`remote_mcp.rs`、`mcp_acceptance_tests.rs` 与 `scripts/linux/nginx-multi-reader.conf.example`。

## 环境
- Rust `+1.95.0`，沿用 `CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`；dev/test debug=0、incremental=0，TEMP/TMP 使用 D 盘，`-j 2`。
- 实模与外部 Agent 验收入口默认忽略；本轮整合回归使用确定性测试，不将忽略项计为通过。
- 原 main／MCP 的完整 checkpoint 分别保存在 `45b08ba:SESSION_CHECKPOINT.md` 和 `8e7e5fe:SESSION_CHECKPOINT.md`。
