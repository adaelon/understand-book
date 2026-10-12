# Tutor、Reader 与 MCP 整合记录

2026-10-12（Asia/Hong_Kong）。原 main 的未提交成果与 reading-share 的 MCP0–MCP9 分别保存，再通过 merge 整合到 main。

## 提交与范围

| 提交 | 内容 |
|---|---|
| `12b765b` | 整合前 main，已包含阅读笔记／分享以及 PDF、对话改进 |
| `45b08ba` | 原 main 的 Tutor TC0–TC6、运行时修复、构建与 Reader 改进、首页、测试及相关证据 |
| `8e7e5fe` | MCP0–MCP9：接入、读取、调用次数、购包、Reader 来源、后台及验收 |

整合分支为 `codex/integrate-mcp-tutor`，合并完成后 main 快进到同一提交。原 reading-share 工作树和分支保留在 MCP 独立成果提交；继续开发需从最新 main 出发。本地临时目录、备份、测试输出和历史实验保留。

## 合并处理

- 术语、架构和代码链路的冲突均为双方新增章节，保留全部内容。
- checkpoint 合并两条工作线的现状、入口及未完成事项；原完整版本保留于两个成果提交。
- `NetworkApp.vue:login` 保留 main 提前解除忙碌状态的行为，并继续执行 MCP 账号目标／来源打开。
- App、ReaderPane、API、客户端、服务端模块注册、多人宿主、材料库和 Nginx 配置自动合并。

## 验证

Web 回归检查登录后的 MCP 入口、来源定位、阅读恢复、Tutor 接续及双方 Reader 改动；Rust 回归检查授权、计次、持久恢复和教学合同。Core 回归检查原 main 的构建成果，类型检查与构建检查接口接线和产物生成。

| 检查 | 结果 | 证据 |
|---|---|---|
| Web 定向回归 | 28 文件、273 项通过 | [日志](integration-mcp-tutor-20261012/web-tests.log) |
| Admin 全部测试 | 6 文件、30 项通过 | [日志](integration-mcp-tutor-20261012/admin-tests.log) |
| Web／Admin 类型检查与生产构建 | 均通过 | [Web](integration-mcp-tutor-20261012/web-build.log)、[Admin](integration-mcp-tutor-20261012/admin-build.log) |
| Core 类型检查 | 通过 | [日志](integration-mcp-tutor-20261012/core-typecheck.log) |
| Core 定向 8 文件 | 累计 206 通过，1 项原有测试仍失败 | [首轮](integration-mcp-tutor-20261012/core-tests.log)、[导入复跑](integration-mcp-tutor-20261012/core-relocation-recheck.log)、[策略复跑](integration-mcp-tutor-20261012/core-policy-recheck.log) |
| Memory／Read-tools／Runtime 定向回归 | 分别 13／3／71 项通过 | [Rust 首轮](integration-mcp-tutor-20261012/rust-regression.log) |
| Server 定向回归 | 首轮 199 通过、9 失败、15 默认忽略；修正后 13 项全部通过，累计 208 项通过 | [首轮](integration-mcp-tutor-20261012/rust-regression.log)、[修正后](integration-mcp-tutor-20261012/rust-fixture-recheck.log) |

累计按唯一用例计数，复跑有重叠；默认忽略的实模／外部客户端等入口未执行。以上不是全仓全量测试。

### 测试夹具修正

1. MU4 原夹具向已采用 JSONL 的会话调用 `save_agent_history_path`，在业务断言前触发 `SESSION_EVENT_WRITE_REQUIRED`。改用既有 `SessionStore::create/accept/append` 写入真实事件，同名会话／演示夹具也采用事件写入。认证、隔离、撤销和持久断言保持。
2. JL4 测试假定日志最后一条必为 `turn.finished`，但现有 `session_runtime::link_teaching` 会在完成后补写关联。测试检查完成事件唯一，且末尾除 `teaching.linked` 外的最后事件仍为 `turn.finished`；恢复和私有 continuation 断言保持。
3. BookStructure 旧策略夹具把所有策略强制改成 v2，碰撞当前已使用 v2 的组织／发现策略。仅为当前 v4 策略制造历史 v2，验证当前策略集恢复正确且历史文件不变。
4. 两项 U1 导入迁移测试在并行编译／测试时超过 5 秒，单独运行原测试均通过（3.798／3.831 秒）；没有调整超时或断言。

## 已知问题与边界

- `build-orchestrator.test.ts` 的 `exposes BookStructure without Pass2 and invalidates it when a Pass2 audit arrives later` 仍失败。原断言缺少现有 `structure_chapter_selection`；试探补齐后又暴露其要求直接生成 `structure_unit`，而 `technical_learning` 当前先执行 `structure_outline`。该用例包含后续旧单元调度／候选／Pass2 失效流程，需独立按当前合同更新。最终保留 `45b08ba` 的该用例全文；MCP 合并对 `packages/core/src` 没有差异。两轮失败保留，未计为通过。
- TC7–TC8、MCP10 仍待实施；本轮没有执行真实读者、实模 Agent、浏览器视觉、目标 Linux／HTTPS、备份恢复或上线验收。
- Web／Admin 构建保留既有大 chunk 提示，Rust 保留既有 ts-rs 属性解析警告。
- 本轮完成本地 Git 整合，没有推送或部署。原成果已有部署状态仍以各自回执为准。

## 环境与重跑

Rust 使用 `+1.95.0`、`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`、dev/test debug=0、incremental=0、D 盘 TEMP/TMP、`-j 2`。pnpm 按锁文件离线安装。

```text
cargo +1.95.0 test -p server -p runtime -p memory -p read-tools --lib -j 2 -- mcp tutor continuation memory_review turn_failure compaction auth::tests:: auth_password control_store adm2_tests adm3_tests mu3_tests mu4_tests book_title --test-threads=2
cargo +1.95.0 test -p server --lib -j 2 -- mu4_tests:: session_runtime_tests::jl4_local_run_reopens_and_public_history_hides_continuation --test-threads=2
pnpm --filter @understand-book/admin test --maxWorkers=2
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/admin build
pnpm --filter @understand-book/core typecheck
```

Web／Core 定向文件清单见日志。本地 `.worktree-local/mcp-tutor-integration-20261012/` 保留文件选择清单、原始命令回执和合并进度。
