# SESSION_CHECKPOINT — 2026-10-10 10:13（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：`4e0b68e feat: integrate book structure, tutoring and multi-user reader`。
- 读入时对比 `git log -3`；若不同，以实际 Git 和工作树为准。
- RS0 已加入笔记升级前置设计，未提交；RN1–RN4、RS1–RS8 待实施。工作区有其他工作线的未提交修改，以当前文件为准。

## 当前在做什么
**阅读分享与安静呈现：设计修订已落档，下一刀 RN1「笔记模型与存储合同」。**

笔记保存“自己的想法＋可返回的原文、回答或演示现场”。RN1–RN2 交付演示记录／返回闭环，RN3–RN4 完成统一编辑、搜索和最近记录；随后 RS1–RS3 交付图片分享。演示版本、现场和必要资源随笔记保留，删除原聊天后仍可返回。

## 下一步（可直接接手）
1. 按冷启动读序读取已接受的笔记合同与当前读写入口，查看拟修改文件的工作区差异。
2. 在 `crates/memory/src/lib.rs` 的 Record／SaveInput／replace／reanchor 路径固定笔记专属字段，映射用户文字、保留片段、原材料／发布和主要关联；同文不同现场分别保存。
3. 在 `document.rs` 沿现有机制实现 v3→v4，准备真实旧存储结构样本，覆盖旧正文、标识、选区、放置及其他私人记忆保持；保留现有旧格式升级入口。
4. 在 server memory 路由、`workspace_client.rs` 与 Web `api.ts` 接通原文／回答／演示关联笔记的创建、读取和原子编辑，透出原 `generated_at`；正文放置沿原命令另行执行。
5. 定向验证关联准入、身份区别、编辑继承、失败原子性和旧存储升级，写回实际合同、切片状态与代码链路，再刷新 checkpoint 指向 RN2。

## 未提交 / 未完成
- RS0 修订：[ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md)、[切片方案](docs/切片方案-阅读分享与安静呈现.md)、ADR-0083 扩展说明、CONTEXT、架构、代码链路和本页已修改；功能代码尚未改动。
- 本次 96 处相对链接及锚点、切片状态与依赖、术语、RN1 接手入口及文档差异空白检查通过；功能验收分别归 RN／RS 各片。
- RN1 当前 schema 为 v3；新笔记仍强制选区或正文放置，演示读取仍要求原聊天存在，不能把现有回执当作独立留存已实现。
- RN2 需从已存笔记解析版本、现场与资源，覆盖删除原聊天／服务重启后返回，以及当前对话追问绑定；复用原演示存储和宿主。
- RN3 共用一个笔记编辑草稿：收起保留，同材料换聊天不串关联，材料切换和新建另一条前处理未保存内容；页面刷新后的持久恢复不在首版。
- RN4 覆盖当前材料内搜索、类型筛选、最近记录／原文顺序、详情及平台连续性；RS1 依赖 RN4。
- `NoteEditorPanel.vue`、`NoteDetail.vue`、`ShareImagePanel.vue`、`reading-share.ts` 为拟新增接点。主笔记列表在 RightRail，NoteCard 主要用于 PDF 弹层。
- 原 Tutor 主线 T19 真实体验验收仍待完成，入口为 [Tutor 方案](docs/切片方案-Tutor预构建不阻塞教学.md) 的 T19；基础成品准入发布状态见 [上线记录](docs/performance/tutor-artifacts-deployment-20261009/README.md)。
- 其他任务入口：[EX](checkpoint_ex.md)、[JSONL](SESSION_CHECKPOINT_JSONL.md)、[DSH](SESSION_CHECKPOINT_DSH.md)、[运营后台](SESSION_CHECKPOINT_ADMIN.md)、[ADM](checkpoint_ADM.md)、[INV](checkpoint_INV.md)、[工具契约](SESSION_CHECKPOINT_TOOL_CONTRACTS.md)、[LangSmith](SESSION_CHECKPOINT_LANGSMITH_FULL.md)。
- 视频工作继续以[视频方案](docs/切片方案-视频理解与渐进伴读.md)和[验收记录](docs/验收-视频理解.md)为准。

## 冷启动读序
1. [ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md)：全文；[ADR-0083](docs/adr/0083-unquoted-note-explicit-body-placement.md)：设计扩展及 §1、§4–§6，区分现行准入与 RN 扩展。
2. [实施切片](docs/切片方案-阅读分享与安静呈现.md)：§1–§1.3、§5、RN1–RN4、§6、§8；首刀以 RN1 为准。
3. [CONTEXT](CONTEXT.md)：阅读笔记、笔记关联、笔记编辑草稿、笔记保留的演示现场、Memory replace、无引用来源 Note、Note 正文放置、授权访问上下文。
4. [memory](crates/memory/src/lib.rs)：Record、SaveInput、保存／替换／重锚及记录身份；[document.rs](crates/memory/src/document.rs)：版本门禁和升级；[server](crates/server/src/lib.rs)：memory 保存／替换／重锚路由；[workspace_client.rs](crates/server/src/workspace_client.rs)：对应用户命令。
5. [api.ts](packages/web/src/api.ts)：MemoryRecord、SelectionContext 与 memory 接口；[App.vue](packages/web/src/App.vue)：noteEditor、saveNote、saveAgentSelection；[RightRail.vue](packages/web/src/components/RightRail.vue)：Notes 主列表与回答摘录。
6. [PresentationFollowUp](packages/web/src/generated/PresentationFollowUp.ts)、[presentation_store.rs](crates/server/src/presentation_store.rs)、[presentation_api.rs](crates/server/src/presentation_api.rs)：不可变现场、版本读取和原聊天依赖；[架构](docs/架构.md)：待实施笔记数据流；[代码链路](docs/代码链路.md)：最新 RS0 修订条目。

## 本会话决策摘要
- 内容、关联和正文显示位置分别表达，新笔记保留明确内容身份，旧正文按已有事实读取，见 ADR-0160 §7。
- 已存笔记保留原演示版本、现场与资源，删除聊天不失去返回能力，见 ADR-0160 §8。
- 轻量编辑、收起续写、按需预览；先完成 RN1–RN4，再进入 RS1，见 ADR-0160 §9–§10。
- 分享继续使用所选内容和原来源，同一排版驱动预览／PNG；静态图面导出归 RS7，见 ADR-0160 §2–§6。
