# SESSION_CHECKPOINT — 2026-10-10 17:45（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：`7fd8a08 docs: carry forward reading notes and sharing plan`。
- 读入时对比 `git log -3`；若不同，以实际 Git 和工作树为准。
- RN1–RN4、RS1–RS8 本次整理为 `codex/reading-share` 集成提交，目标为本地 main；未发布。main 其他工作线的未提交改动继续保留。

## 当前在做什么
**RS8「理解空间的单项阅读」功能／Web 已完成；RN1–RN4、RS1–RS8 功能主线已实现，平台待验项与后续新功能独立接续。**

理解空间已采用短预览列表与单项详情；原话、系统解释、暂定理解、实际表现、帮助条件及历史版本分别显示。依据按需展开且可见才记录反馈展示；纠正沿原证据及幂等命令，历史沿原对象分页。见 [RS8 验收](docs/performance/reading-share-rs8-20261010/README.md)。

## 下一步（可直接接手）
1. RN／RS 功能已整理提交；接续时用 `git log -3` 确认 main 集成状态，保留 main 中产品首页与构建重试等独立在途改动。
2. 若接续平台验收，先读 [RS3 已知限制](docs/performance/reading-share-rs3-20261010/README.md) 与 [RN4 验收](docs/performance/reading-notes-rn4-20261010/README.md)，按隔离目录准备原生保存／取消／切现场的实测；Computer Use 仍处于用户停止状态，不自行恢复。
3. 若接续新功能，从实施方案 §7 选择并明确下一切片；停读意图、理解前后对照与问题回看尚未排期。

## 未提交 / 未完成
- RN1–RN4、RS1–RS8 的代码、测试、合同及验收记录已纳入本次集成提交；临时运行目录与 test-results 留在原工作区。
- 集成验证：Web 20 文件 182 项、memory 143 项、server 定向 52 项通过（5 项按原设置忽略），Web 类型及生产构建通过。
- RS8 最终 Web 2 文件 9 项通过；Chromium 4 项通过，覆盖 320／390／1280px 长文、纠正、未知状态、依据可见性、帮助条件和历史。最终类型／生产构建通过；日志与 PNG 见 RS8 验收。
- RS8 已复现并修复反馈回执延迟失败覆盖纠正重试的问题；两类错误分别保留。当前解释仍沿服务端最近 20 条；无对象暂定解释没有新增历史接口，纠正草稿仅驻留当前组件。
- RS7 图解分享功能／Web 已完成，见 [RS7 验收](docs/performance/reading-share-rs7-20261010/README.md)。当前图解同次固定图面和参数；笔记图解在独立 960×640 页恢复核对，不重置原演示。
- RS7 视频／音频、WebGL、嵌入页、Shadow DOM、未展开滚动区及无法完整恢复的旧现场明确拒绝；原生保存／实体手机仍待验。
- RS6 回答分享、RS5 只读接续及 RS4 固定选材／截点已完成；原来源、Reader、未发送文字及原导航保持，见 [RS6](docs/performance/reading-share-rs6-20261010/README.md)、[RS5](docs/performance/reading-share-rs5-20261010/README.md)、[RS4](docs/performance/reading-share-rs4-20261010/README.md)。
- RN1–RN4、RS1–RS3 功能已实现。RS3 用户确认的无配图纸页与精简弹窗保持；原生保存和实体手机验收延期仍有效。
- Windows Debug NSIS 候选已构建，未安装、未覆盖原安装、未发布。原生逐页保存／取消／切现场、安装后资源路径与 Release 行为、RN4 平台连续性均未实测完成。
- 用户已停止 Computer Use，后续不得自行恢复。隔离窗口 PID／目录与重启隔离变量见 [RS3 验收](docs/performance/reading-share-rs3-20261010/README.md)。
- MU10 百回合恢复 queued 0／预期 100 为既有独立失败，本轮未修改或复验；旧笔记历史 ID、未知旧记录与删除聊天限制沿 RN4。
- 分享草稿／PNG 驻留当前页面；回顾分享另在关闭回顾时结束。普通文字内嵌媒体、过宽对象与不可分过高内容沿 RS2 报错。
- Tutor T19 真实体验待验，见 [Tutor 方案](docs/切片方案-Tutor预构建不阻塞教学.md)。
- 其他工作线入口：[EX](checkpoint_ex.md)、[JSONL](SESSION_CHECKPOINT_JSONL.md)、[DSH](SESSION_CHECKPOINT_DSH.md)、[运营后台](SESSION_CHECKPOINT_ADMIN.md)、[ADM](checkpoint_ADM.md)、[INV](checkpoint_INV.md)、[工具契约](SESSION_CHECKPOINT_TOOL_CONTRACTS.md)、[LangSmith](SESSION_CHECKPOINT_LANGSMITH_FULL.md)。

## 冷启动读序
1. [实施切片](docs/切片方案-阅读分享与安静呈现.md) §2、§5、RS8、§7–§8；[ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md) §1／§5；`CONTEXT.md` 理解空间／LearningEvidence 相关术语。
2. [RS8 验收](docs/performance/reading-share-rs8-20261010/README.md)、`packages/web/src/components/UnderstandingSpace.vue` 及其组件／浏览器测试；`TutorPanel.vue` 与 App／RightRail 原入口。
3. `packages/web/src/api.ts` 理解投影／证据／纠正接口；`crates/server/src/teaching.rs` 的 understanding／evidence_view／纠正路由；`crates/memory/src/learning_evidence.rs` 的 correct_evidence／对象历史。服务端合同本轮保持。
4. [架构](docs/架构.md)「阅读笔记与分享」「普通使用驱动的理解接续」；[代码链路](docs/代码链路.md) RS8；[RS3 待验项](docs/performance/reading-share-rs3-20261010/README.md)。

## 本会话决策摘要
- RS8 沿 ADR-0160 §1／§5 完成局部呈现，不增加能力判定或新存储；实现与验证落于 RS8 验收和架构。
- RN／RS 功能主线完成；RS3／RN4 平台延期与用户停止 Computer Use 的约束继续保留。
