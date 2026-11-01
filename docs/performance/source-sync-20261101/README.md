# 正式源码回并 — 2026-11-01

将本地未提交功能与正式服务 `tutor-recovery-20261101` 的源码合并到 `main`，基线为 `bdfdb5293c78cf51e1bd4600bc5a4c4f6d7de9c2`。正式源码取自 `/opt/understand-book/releases/tutor-recovery-20261101`，核对 2,728 个源码及配置文件，补回 122 个文件的差异，其中 55 个为新增文件。

## 合并内容

- `memory::boards::mind_map → BoardWorkspace / MindMapLayer`：可编辑思维导图、原子编辑、撤销与草稿恢复，以及与自由画图并存的布局和预览。
- `memory::workspace_notes → server::workspace_notes_api → runtime::workspace_notes → NotesWorkspace`：稳定笔记身份、原始来源、跨材料复用及共享关系。
- `server::board_author_edits / board_author_regeneration → runtime::orchestrator`：候选随手工修改重定位、冲突保留与重新生成，模型失败时保留已有画板成果。
- 本地想法笔记理解路径、练习修复、手机便笺操作与公开错误提示一并保留。验收与发布经过见各功能原有记录。

已对照 Git 历史保留本地较新的构建执行器、桌面 0.3.8 及发布检查。重叠修改保留双方有效内容；原有整板提问和讨论接续回归继续保留。

## 集成修正

- 导图 `prepare` 的 `operations`、`read_item_ids` 与 `revise` 的 `operations` 补入既有逐操作参数合同，允许原生编辑经过实际工具校验。
- 空间笔记工具声明同时要求读取和写入权限，符合其可修改内容的既有工具权限合同。
- 画板工具描述保持英文；补齐“恢复100%缩放”的英文译文。
- 启动测试替身提供真实组件已有的 `flush` 接口；历史及即时错误断言使用统一公开提示。额度停止用例从持久化状态和原有落盘断言验证保存结果。
- 完整教学示例继续经过真实调度验证，允许精简模型合同省略重复的选择示例；纠错回归同时验证不同错误可以修正、重复错误以 `AGENT_NO_PROGRESS` 停止且不产生交付。

## 验证

- Memory：333 项通过，1 项既有忽略。
- Runtime：1,018 项通过，16 项既有忽略。
- Server：分批运行与定向复验合计 1,282 项不同测试通过，117 项既有忽略；按最终测试清单逐项核对，无遗漏、无剩余失败。
- Web：1,362 项不同测试通过。完整运行先有 1,352 项通过，修正受影响夹具与译文并保留整板提问回归后，对应两个文件 122 项全部通过；完整类型检查通过。

本机原始日志保留在 `tmp/source-reconcile-20261101/`。截图、构建日志与源码快照保留在本机，不纳入本次提交。

## 已知问题

服务端 `resident_commit_failure_keeps_pending_and_startup_recovers_interrupted` 首轮失败，随后使用同一测试程序单独复验和后续分批运行均通过，未修改该测试或实现。首次失败的具体原因尚未确定；以上通过数为最终逐项结果，不代表首次整轮零失败。
