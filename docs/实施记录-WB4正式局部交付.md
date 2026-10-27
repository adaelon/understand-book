# WB4：正式局部交付与实时补充

日期：2026-10-25。状态：已实现并验收。存储、运行时、服务端与前端回归通过；真实浏览器完成分段生成、运行恢复、取消保留、连续交付与桌面／窄屏验收。

本记录承接 [WB3 图文联动与板书讲解](实施记录-WB3图文联动与板书讲解.md)，范围依据[完整设计 §9](设计方案-知识画板与持续讲解.md#9-实时板书与正式分段交付)与 §13。一个教学上完整的局部检查完成后即可正式加入画板，原运行继续制作剩余内容；已交付内容不因后续取消或失败而撤回。

## 1. 交付合同

`board.author` 新增 `commit`，与既有终态 `deliver` 区分。局部仍先准备完整正文、来源、图元、关系和演示引用，再使用正式 BoardCanvas 预览。实际截图必须进入后续模型采样，正文必须符合当前教学审核，才可正式提交。`commit` 返回真实回执并继续运行，不触发整轮完成。

`BoardDelivery.commit_id` 使用已持久候选的身份。同一回合允许多个不同局部；同一局部重试返回原回执，不重复插入。每个局部有独立回答条目和显式解释块身份。原有整轮交付没有 `commit_id`，保持 WB1–WB3 的保存与重试行为。

解释块身份沿用全板唯一约束：同一未提交局部修订时保持身份，后续局部使用新的身份。候选中的演示条目身份由局部回答与确切演示版本确定，因此调整引用顺序或重做同一局部预览不会丢失已经写好的图文映射。

每个新局部使用当前画板版本准备和预览，正式写入执行严格版本比较。用户移动内容后，旧候选不能覆盖新布局，制作方需重读并重新观察。条目、图关系与回执在同一画板快照中发布。

当前回合已提交过局部后，最终回答引用这些交付入口，不再自动复制一张总答卡。仍有新增解释时明确提交新的完整局部；旧 `deliver` 不会暗示未保存的新候选已经归板。

## 2. 持久化、事件与恢复

正式写入顺序为：画板快照 → 会话局部记录 → 确切演示交付与教学支持事实 → `board.committed` 运行事件。`CommittedPart` 保存真实回执、候选修订、编译后的正文、来源、教学选择和提交时间。

会话 `TurnDomain.board_commits` 保存正式局部；公开历史视图和 `RunSnapshot.board_commits` 投影其回执。取消、失败、草稿撤回和事件缓冲淘汰均不清空正式记录。板上写入已经成功但会话确认未完成时，沿同一候选和回执补齐登记，不重新制作内容。

前端从 SSE、HTTP 恢复及历史中合并交付身份。刷新先读取最新正式画板，再合并本地未保存位置与标题；保存冲突不阻止读到新局部。刷新期间出现更新的提交时继续追上最新修订，保留镜头、选择和已打开的演示实例。

画板与历史回答提供逐部分入口。部分仍在制作时显示后续仍在讲解；主动停止显示正常的“已停止”，并保留未完成说明。阅读区按真实提交回执与当前条目的关联显示“分段讲解 · 已保存在画板中”，旧整次交付仍使用完整回答标签。

## 3. 教学与演示边界

局部审核明确区分当前完整局部与整个原请求。审核仍读取真实原请求、用户表达、来源以及此前已经交付的内容，不把作者的局部计划当作缩窄原请求的依据。局部资格不等于整体完成，最终收口仍需处理完整请求。

正式局部记录公开交付的 `MessageDelivered`，不合成用户原话、掌握证据或未经实际展示的 `HelpDisplayed`。最终交付不会覆盖已经交付的教学选择。运行中的支持局部不提前打开尚未到达的用户作答边界。

可操作演示通过 `prepare.presentations` 引用本轮实际制作、观察并保存的精确版本，再纳入完整候选。局部正式提交后补齐既有演示交付资格，使页面在整轮结束前即可读取与操作。后继版本新增独立条目，当前旧版本的 iframe、输入和现场保持不变，由用户选择打开新版本。

工具发现将原生板书 `board_authoring` 与 HTML 演示 `presentation_authoring` 分开。原生板书可用不能代替 HTML 创作环境可用；HTML 环境缺失时仍返回确切的环境原因，原有执行权限与环境规则保持不变。

## 4. 验收结果

| 层次 | 当前证据 |
| --- | --- |
| 存储 | 54 项通过，包括 7 项 WB4 新行为；[运行日志](../tmp/board-wb4/memory-tests.log) |
| 运行时 | 整合编译通过，三组共 57 项通过、1 项需要外部记录的历史探针跳过；[板书与预览日志](../tmp/board-wb4/runtime-tests.log)、[教学范围日志](../tmp/board-wb4/runtime-scope-tests.log)、[工具发现日志](../tmp/board-wb4/runtime-discovery-tests.log) |
| 服务端 | 87 项相关测试通过；[基础回归日志](../tmp/board-wb4/server-tests.log)、[映射重排复测](../tmp/board-wb4/server-fixed-tests.log)、[教学与取消重开复测](../tmp/board-wb4/server-teaching-final-tests.log) |
| 前端 | 7 个相关文件共 267 项测试通过，类型检查与最终生产构建通过；[构建日志](../tmp/board-wb4/web-build-final.log) |
| 工具发现 | 原生板书与 HTML 演示能力已分开；环境不可用、原生可发现、环境可用时激活 HTML 的回归已通过 |
| 真实页面 | 单浏览器上下文、全新数据完整流程通过，耗时 1.8 分钟；桌面 1500×1050、窄屏 390×844；[执行日志](../tmp/board-wb4/browser-validation.log)、[完整结果](../tmp/board-wb4/browser/result.json) |

正式服务端 `cargo check -p server --lib` 已通过，确认测试专用预览接线不进入正式构建；[检查日志](../tmp/board-wb4/production-check.log)。

存储检查覆盖多局部有序保存、同局部重试、删除后不复活、用户移图后的版本冲突、写入故障原子性、终态不重复复制以及原有交付读取。类型由 Rust 导出，不手工维护第二份合同。

真实页面使用平均速度知识点，执行实际来源读取、演示制作、三种环境的浏览器预览、原生画板预览与后续模型采样，再正式提交。共保存 53 次受控模型采样及其工具消息，未替换页面 API、运行事件或画板存储。

| 实际场景 | 已观察结果 |
| --- | --- |
| 首部分提交后继续生成 | 正文可读，演示滑块可操作；运行仍为 running |
| 刷新断开 SSE | 运行状态与已提交回执从真实快照恢复，画板内容一致 |
| 第二候选预览后停止 | 仅首部分保留，第二候选未进入画板；历史正常显示未完成 |
| 两部分连续提交与候选重试 | 保存两条不同回执；重复提交返回原回执，没有重复条目 |
| 中途拖动原卡片 | 后续提交保留用户新位置 |
| 新演示版本加入 | v2 独立加入；v1 iframe 未重建、加载次数不变，输入值 1.5 保持 |
| 最终收束与手机重开 | 没有第三张整答卡；从纯画板地址重开恢复第二部分阅读位置 |

7 张最新桌面／手机截图已直接检查。页面脚本错误 0，非预期 HTTP 错误 0。关键画面：[运行中已保存首部分](../tmp/board-wb4/browser/desktop-delivered-still-generating.png)、[停止后保留首部分](../tmp/board-wb4/browser/desktop-cancelled-keeps-first-part.png)、[两部分与旧实验现场](../tmp/board-wb4/browser/desktop-two-parts-preserve-live-v1.png)、[桌面最终内容](../tmp/board-wb4/browser/desktop-final-two-parts.png)、[手机最终内容](../tmp/board-wb4/browser/mobile-final-two-parts.png)。测试宿主与 Vite 已停止。

## 5. 关键代码入口

- `crates/runtime/src/board_author.rs`、`orchestrator.rs`：局部提交调用、后续采样观察条件、继续运行与最终收口。
- `crates/server/src/board_author.rs:repair_commits`：候选提交、正式回执及写入成功后的登记恢复；`board_delivery.rs` 保持最终与中断路径不复制整答。
- `crates/memory/src/boards.rs:append_authored_part`：按候选身份去重、严格版本比较、图文与回执的原子保存。
- `crates/server/src/session_runtime.rs:record_board_commit`、`deliver_presentations`：持久会话投影与精确演示版本交付资格。
- `crates/server/src/teaching/board_delivery.rs:record_board_local_delivery`、`crates/runtime/src/tutor.rs`：完整局部的审核范围与真实公开帮助事实。
- `packages/web/src/agent-run-state.ts`、`useAgentRun.ts`、`board-workspace.ts`、`components/BoardWorkspace.vue`：事件与历史恢复、画板合并刷新、分部分入口与未完成状态。
- `crates/server/src/tests/board_wb4_browser_host.rs`、`packages/web/playwright/board-wb4.spec.ts`：实际网络执行的分段制作、取消与重开验收。

## 6. 验证边界

- 页面验收使用受控模型，真实工具、HTTP、SSE、浏览器预览和持久化均实际执行。该证据验证交付链路，不衡量外部模型自由生成内容的质量；受控适配器未实现后台记忆审阅，其日志中的 `REVIEW_EXECUTOR_FAILED` 属于本次夹具边界。教学局部审核与支持事实另有运行时、服务端回归覆盖。
- Windows 网络测试宿主仅通过 `cfg(test)` 的实例接线使用与桌面相同的真实 `BrowserPreview`，实际 probe 与 PNG 检查成功后才开放 HTML 预览，plot／animation 保持不可用。记录见 [浏览器 probe](../tmp/board-wb4/presentation-browser-probe.json)。生产网络的 Linux 隔离环境未在此次 Windows 验收中测试，其门禁未改动。
- 样例资料未提供可选 `source_manifest`，5 次对应 404 在结果中单独记录；正文来源与本轮必要请求均可用。需要外部历史记录的运行时探针跳过，不计入已通过的 57 项。
