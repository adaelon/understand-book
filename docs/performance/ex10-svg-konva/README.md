# EX10 SVG / Konva 实验

输入和判据见[切片合同](../../切片方案-EX10-Agent二维交互图形库对照.md)。固定 Konva 10.7.0 的完整源码与 MIT 许可证在 `vendor/`；不依赖运行时网络下载。`common.md`、`B-api.md`、`C-api.md` 和 `input.json` 为模型输入；`independent-answers.md` 仅供验收。第一批冻结记录为 `frozen.json`，装配修复后的第二批为 `batch2/frozen.json`。前三批均已按合同停止，第三批 B1/C1 已保存交付页，C 最终回执标为 incomplete；[最新第三批结果](../explorable-explanation-ex10-svg-konva-batch3-20260928.md)和[可操作入口](index.html)已交付。

## 重现入口

在仓库根目录用 PowerShell 执行：

```powershell
cargo test -p server --lib ex10_ -- --nocapture
cargo test -p server --lib ex10_preflight -- --ignored --nocapture
$env:EX10_CONTENT=(Resolve-Path docs/performance/ex10-svg-konva/preflight/content.json).Path
cargo test -p server --lib presentation_ex10_browser_host -- --ignored --nocapture
```

宿主等待时，在另一终端进入 `packages/web`：

```powershell
pnpm exec playwright test playwright/agent-presentation-ex10.spec.ts --workers=1 --reporter=line
```

宿主有五分钟生命周期，结束后可通过 `POST http://127.0.0.1:4175/stop` 提前停止。Windows 重新链接 Server 测试程序前须停止该宿主，否则运行中的 exe 会使链接失败。

正式 Agent 入口一次只运行一个样本，已有目录拒绝覆盖：

```powershell
$env:EX10_CONDITION='B'
$env:EX10_RUN_DIR='E:\allwork\download\agent\understand-book\docs\performance\ex10-svg-konva\runs\B1'
cargo test -p server --lib ex10_agent_comparison -- --ignored --nocapture
node docs/performance/ex10-svg-konva/audit.mjs docs/performance/ex10-svg-konva/runs/B1
```

必须先判定当前样本，再按合同顺序决定下一样本。已停止批次不得用同名目录或改提示补抽。模型配置从既有 `.env` 读取，凭证不进入证据。

## 证据身份

`preflight/` 为工程小场景，不计入 B/C 成功率。`runs/` 保存真实 Resident 请求、响应、用量、装配前/后 HTML、时间、候选及最终交付；Agent 原稿不手工修复。第一批的 adapter 装配误将库写入后续模型历史，C1 因上下文耗尽未交付，原始证据保留。当前实现已移至 test-only Author Write 内部：模型调用保持原始标记，候选保存完整源码；回归测试覆盖这一差别。`batch2/runs/B1/` 在第三次请求遭遇 Provider length 截断，尚未写出页面，按合同停止。

先行两类失败已保留：首次测试未完成最终回答历史提交；工程示例使用 ResizeObserver 在展开时触发循环通知错误。前者修正测试生命周期，后者仅修正示例的尺寸事件。三视口 Reader 最终证据在 `preflight/reader-check.json`。

## 浏览和验收

在仓库根目录运行 `python -m http.server 8770 --bind 127.0.0.1 --directory docs/performance`，打开 `http://127.0.0.1:8770/ex10-svg-konva/index.html`。两页用最小宿主接口加载未修改的原始内容；独立预览不提供 Reader 持久化。`viewer-check.mjs` 检查入口与原页拖动、播放、暂停，记录在 `viewer-check.json`。

正式原稿 Reader 检查为 `packages/web/playwright/agent-presentation-ex10-generated.spec.ts`；设置 `EX10_SAMPLE=B1` 或 `C1`，以及可选 `EX10_RUN_DIR`（绝对路径），以 `EX10_CONTENT` 启动对应宿主后使用 `playwright.ex10.config.ts`。选择器对应第一批原稿，新的独立样本须按其实际 HTML 定位。测试日志完成不表示所有判据通过，结果字段与 verdict 一并阅读。

B 三视口完整记录；C 的完整 Reader 记录仅桌面和窄屏，短屏按钮被宿主遮挡的失败日志保留。`check-b.mjs` 和 `check-c.mjs` 是针对原稿结构的独立数值/几何检查，参数为各自样本目录。成本审计为 `audit.mjs`；汇总在 `cost-summary.json`。

## 第三批重做（2026-09-28）

用户明确要求重新做批次，`batch3/` 独立冻结并保存 B1/C1。两组实际交付，C 不再出现库源码进入模型历史；C 最终回执仍有 `TURN_LIMIT_EXCEEDED`。质量错误触发停止，未运行后续样本。冻结配置、原始请求/响应、原稿、逐项判定和本批成本均在该目录。

`batch3/check-b.mjs` 与 `batch3/check-c.mjs` 针对第三批原稿结构，独立验证三视口 126 组数值/实际图元；C 还测量实际命中图并用真实鼠标/触屏拖动圆点。`viewer-check.mjs batch3` 验证新的可操作入口。生成页 Reader 测试可通过样本目录中的 `reader-selectors.json` 选择实际控件；`EX10_WIDTH=640` 仅补验短屏，不重新抽样。两组短屏均复现宿主表单遮挡；C 初次清理错误及短屏补验分别保留日志。
