# SESSION_CHECKPOINT — 2026-10-10（Asia/Hong_Kong）

## 新鲜度自检
- 写入时功能提交：`68ebf4f feat: improve PDF reading, presentation interaction and conversation memory`；同步的 main 为 `de6376d`。
- 本次合并将 PDF、演示即时交互、对话理解与多人后台记忆并入本地 main，同时保留 RN1–RN4、RS1–RS8。
- 读入时对比 `git log -3`；以实际 Git 和工作树为准。main 目录其他未提交工作独立保留。

## 当前在做什么
本地功能整合：PDF 可见页优先读取与选区翻译、演示即时交互、对话理解和多人后台记忆，与阅读笔记／分享主线共同接续。未部署。

## 下一步（可直接接手）
1. 用 `git log -3` 确认 main 集成状态，查看下列验收记录中的功能与平台边界。
2. 如整理服务端扩展测试，修复 `tests/mu4_tests.rs:Fixture::seed_private` 的旧历史写法及 `tests/mu10_tests.rs` 读取冻结输入的旧假设，见对话理解验收。
3. 如接续平台验收，先读 RS3 已知限制与 RN4 验收，准备原生保存／取消／切现场的隔离实测；Computer Use 保持用户停止状态。
4. 如接续新功能，从阅读分享方案 §7 确定下一切片；停读意图、理解前后对照与问题回看尚未排期。

## 未提交 / 未完成
- 本工作区原有 71 个变更文件已纳入 `68ebf4f`；本次合并处理演示导出协议与新版理解空间刷新衔接。
- 本次集成验证结果见 `docs/代码链路.md` 的“PDF、演示与对话理解集成”条目。
- 对话理解与多人后台记忆的定向验证、恢复／费用归属和既有扩大回归失败见 [验收记录](docs/验收-对话理解与多人后台记忆.md)。普通理解由对话模型根据证据选择保存；尚未进行在线模型质量验收。
- PDF 原件保持不变；读取性能、翻译、来源限制及既有 MU4 夹具问题见 [PDF 验收](docs/performance/pdf-reader-fixes-20261010.md)。
- 演示本地控件即时更新；保存／追问与记录图解导出沿服务端校验；原演示文案笔误及验证边界见 [即时交互验收](docs/performance/presentation-immediate-20261010.md)。
- RN1–RN4、RS1–RS8 功能／Web 已完成，原生保存、实体手机、安装后的 Release 与 RN4 平台连续性仍待实测；Windows Debug NSIS 候选未安装、未发布，见 [RS3](docs/performance/reading-share-rs3-20261010/README.md)、[RN4](docs/performance/reading-notes-rn4-20261010/README.md)。
- RS8 当前解释沿服务端最近 20 条；无对象暂定解释没有新增历史接口，纠正草稿驻留当前组件，见 [RS8](docs/performance/reading-share-rs8-20261010/README.md)。
- RS7 视频／音频、WebGL、嵌入页、Shadow DOM、未展开滚动区及无法完整恢复的旧现场不能导出；分享草稿／PNG 驻留当前页面，见 [RS7](docs/performance/reading-share-rs7-20261010/README.md)。
- MU10 百回合恢复 queued 0／预期 100 为原主线既有失败；旧笔记历史 ID、未知旧记录与删除聊天限制沿 RN4。
- 普通文字内嵌媒体、过宽对象与不可分过高内容沿 RS2 报错；回顾分享在关闭回顾时结束。
- Tutor T19 真实体验待验，见 [Tutor 方案](docs/切片方案-Tutor预构建不阻塞教学.md)。
- 其他入口：[EX](checkpoint_ex.md)、[JSONL](SESSION_CHECKPOINT_JSONL.md)、[DSH](SESSION_CHECKPOINT_DSH.md)、[运营后台](SESSION_CHECKPOINT_ADMIN.md)、[ADM](checkpoint_ADM.md)、[INV](checkpoint_INV.md)、[工具契约](SESSION_CHECKPOINT_TOOL_CONTRACTS.md)、[LangSmith](SESSION_CHECKPOINT_LANGSMITH_FULL.md)；视频以其方案与验收记录为准。

## 冷启动读序
1. [对话理解 ADR](docs/adr/0161-conversation-first-learning-and-multi-user-memory.md)、[实施方案](docs/切片方案-对话理解与多人后台记忆.md)、[验收](docs/验收-对话理解与多人后台记忆.md)。
2. [PDF 验收](docs/performance/pdf-reader-fixes-20261010.md)、[演示即时交互验收](docs/performance/presentation-immediate-20261010.md)。
3. [阅读分享方案](docs/切片方案-阅读分享与安静呈现.md) §2、§5、RS8、§7–§8；[ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md) §1／§5；[RS8 验收](docs/performance/reading-share-rs8-20261010/README.md)。
4. `CONTEXT.md` 会话学习意图、理解假设、TutorLoop、ProfileFact、ReviewJob、理解空间相关术语；[架构](docs/架构.md) 对话理解与多人后台画像、阅读笔记与分享；[代码链路](docs/代码链路.md) 本次集成条目。

## 本会话决策摘要
- 演示恢复完成发送 ready；记录图解在取图后校验同一现场文字与来源，保留 RS7 导出与参数一致性校验。
- 新版理解空间继续按单项阅读；对话完成时刷新列表，卸载时停止监听。
