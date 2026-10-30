# 画板全屏与原文阅读

实现日期：2026-10-28；2026-10-29 00:09:26（香港时间）已合并上线，参见[发布与公网验证](../board-source-window-20261028/README.md)。基于主工作树 `d791eea5` 及其既有未提交改动；本次未回退或整体提交其他工作。上线版本从实际线上基线合并。

## 行为与原因

原四角按钮只执行适配全部内容，画布仍受顶栏、右栏和底部输入占位限制。现在保留独立的“显示全部内容”，另设带文字的“全屏画板”。支持时进入浏览器原生全屏，同时收起画板周围面板；不支持或被拒绝时占满窗口并明确提示。全屏中可打开讲解、讨论、提问，退出恢复原先布局、草稿、选择和演示实例。默认布局缩窄右栏、压缩顶栏和输入区，并可手动收起右栏。

原规则允许读过原文但不交付引用；普通回答编译器只保留正文实际使用的来源。没有发现正确附上引用后持久化丢失。本次为画板增加统一要求：书内实质主张应附支持该主张的可点击原文引用，直接文本、讲解块、分段提交均适用。生成与既有事实审核共同区分“已读原文”和“正文附上引用”；操作、寒暄及单纯调整布局不强制引用。

“原文引用 · N 处”放到讲解标题下，展开即可查看来源。点击行内引用或来源条目，先在画板上预览原文、上下文和高亮片段；明确选择“在阅读器中打开”才导航。关闭预览和从阅读器返回均保留画板现场。未附引用的历史讲解明确显示状态。不可打开的旧版本仍可查看留存片段。

## 实现入口

- `BoardCanvas.vue / BoardWorkspace.vue`：适配与全屏分开；Fullscreen API、布局恢复、可收起侧栏和输入区。全屏目标为 `document.documentElement`，保证原文弹窗仍可见。
- `BoardReadingPane.vue / BoardSourcePreview.vue`：首屏引用入口、来源预览、加载和失败重试、可用来源的显式阅读器入口。
- `App.vue:openBoardSource/openBoardSourceInReader`：预览与导航分开。手机先显示阅读器并等待布局，再同步引用读位；成功才隐藏画板并保存返回点。
- `runtime/board_author.rs:SOURCE_CITATION_POLICY → server/board_api.rs:freeze_follow_up`：工具与直接画板回答共享引用约定。
- `agent_prompt.rs / orchestrator.rs / tutor_*review_rules.md`：来源策略 v14 的画板例外；事实审核接收宿主提供的画板范围，并检查实际交付中的引用。

## 验证证据

| 范围 | 结果 |
| --- | --- |
| `board-canvas.test.ts`、`board-workspace.test.ts` | 61 项通过：适配与全屏独立、原生退出、被拒绝时的提示、草稿与实例保留及既有交互 |
| `board-source-preview.test.ts`、`board-reading-sources.test.ts`、`App.board-source.test.ts` | 14 项通过：来源首屏入口、旧快照、加载重试、明确导航、返回画板、手机先显示再定位 |
| runtime 定向测试 | 9 项通过：策略传达、审核输入、引用编译/合并/裁剪、无来源普通操作继续可用 |
| server 定向测试 | 3 项通过：冻结上下文、修订后提交保留引用、取消重开后已提交引用可读；原始输出见 `citation-server-tests.log` |
| `board-workspace.spec.ts` 定向浏览器场景 | 3 项通过：桌面原生全屏与来源预览；390px 中文/英文布局及降级；既有四类内容、演示状态、原文返回与重开 |
| `pnpm --filter @understand-book/web typecheck` | 完成后的完整类型检查通过 |

视觉验收直接查看真实 Vue 组件在 1440×960 与 390×844 视口的截图。桌面全屏画布宽高等于浏览器视口；手机降级画布为 390×844。英文窄屏工具栏的溢出已修正，工具、退出和输入仍可操作。

截图：`board-sources-desktop.png`、`board-fullscreen.png`、`board-fullscreen-source-preview.png`、`board-fullscreen-mobile.png`、`board-source-mobile.png`、`board-fullscreen-mobile-english.png`。浏览器使用预览夹具，来源预览与画板均为生产组件；App 生产导航由集成测试覆盖。

复验入口：`pnpm --filter @understand-book/web exec playwright test board-workspace.spec.ts --grep "canvas fullscreen fills|mobile fullscreen fallback|WB1 saves" --workers=1`。初次并行构建遇到本机内存不足，后改为串行完成检查；没有更改全局运行配置。

## 已知边界

- 已有无引用回答不会因发布自动改变内容；后续回答使用新的画板引用规则。
- 历史无引用回答不会凭文字自动补造来源；需要重新读取原文后产生带引用的新解释。
- 未做真实模型生成回放；引用规则、审核输入、编译和持久化路径已验证，截图中的回答与原文为验收夹具。
