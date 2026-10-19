# 同源累积发布与保留现场的接入（VI9）

## 发布与接入

已接纳的成果按批次调用 `publishVideoEnrichmentBatch`。批次保存材料身份、已接纳覆盖及完整依赖清单，文件位于 `video-enrichments/<batch_id>/`；目录内的贡献不可原地替换。批次与 `video_enrichments.json` 使用既有阶段发布事务共同提交。每批最多 31 个依赖文件，加一份目录清单。

`PublishedLibrary::publish` 从最近登记的同书发布和本次输入累积批次。交错完成的生产者可以只携带自己的新批次；后发布的快照保留已登记的其他批次、依赖与译文。缺文件、材料不符、相同批次被改写或覆盖声称的译文缺段，均在登记前拒绝。规范 `transcript_translations.json` 从贡献组装，覆盖按已接纳区间合并，缺口继续可见。

发布清单保存 `previous_publication`，形成同源发布链；新发布沿既有登记事务承接前一发布的读者授权。自动接入选择获授权的最新后继，独立于管理员的默认版本选择。原视频或英文转写变更继续使用新 `book_id`，由普通材料入口主动打开。

`reader/adopt_same_source_publication` 在窗口的状态串行化入口判断草稿、准备/排队/运行和结果待保存。区域或范围独立存在也算草稿。接入替换资源与能力投影，保留 Reader、现场代次、聊天选择和消息；网络 checkpoint 与该次变更一起提交，提交失败恢复旧资源。

## 浏览器现场与恢复

视频窗口每三秒在提问间隙尝试接入。界面正在编辑区域/范围、持有草稿、发送中、回答中或历史恢复失败时不发起接入。服务器再次检查持久状态。接入请求期间短暂锁定新提问输入，结束后恢复。

同源接入不替换 App、播放器或媒体 URL。播放位置、播放/暂停和倍速保留；正文依旧使用英文 segment_id，译文补齐后恢复可见段的滚动偏移。页面显示当前译文段数和视觉索引覆盖秒数。

网络恢复通过相同现场代次辨认已提交的同源接入，补取漏收的发布投影。后到通知只触发重新查询，不能指定回退目标。回答及来源继续保存当时的确切发布。普通 `book/open` 仍使现场换代。

本地宿主持有一个共享 ReaderWorkspace，沿该范围阻止草稿期间接入；本地会话旁的 `*.video-publication.json` 保存恢复用发布引用。需要独立窗口接入时使用多人宿主的独立 ReaderWorkspace。

## 验证入口

```powershell
. .\.worktree-local\environment.ps1
pnpm --filter @understand-book/core test test/video-enrichment.test.ts test/automatic-build-close.test.ts test/video-transcript.test.ts
pnpm --filter @understand-book/core typecheck
cargo test -p server --lib video_
cargo test -p server --lib mu3
pnpm --filter @understand-book/web test src/video-adoption.test.ts src/video-question.test.ts src/components/VideoReaderPane.test.ts src/components/RightRail.test.ts src/App.startup.test.ts src/NetworkApp.chat.test.ts
pnpm --filter @understand-book/web typecheck
node --import tsx evals/video/vi9-browser.mjs
```

浏览器脚本复制 VI3 原片与英文成果到独立测试工作区，运行真实 HTTPS/Server/Web、两个窗口及可控制终态的模型记录器。两个成果批次分别包含四段中文夹具、两张原片真实帧与索引夹具；Rust 测试宿主调用正式发布器。凭据写入 `evals/video/generated/vi9/<时间>/`。

## 验收结果

2026-10-10：Core 11、Server 27、Web 89 个定向用例通过；Core/Web 类型检查及 Server 构建通过。原片双窗口验证草稿与在途固定、保存后接入、同一播放器、播放/暂停/倍速、英文锚点、聊天、刷新恢复、依赖缺失拒绝与后到通知不回退。

每个已接纳批次发布一次，本次两批耗时 5,411 / 2,857 ms。每个发布复制原媒体 225,917,024 字节，三个快照合计 677,751,072 字节；整讲生产需按有意义的批次聚合。凭据见[浏览器结果](../evals/video/results/vi9-20261010/verification.json)、[定向测试](../evals/video/results/vi9-20261010/tests.json)及[完整验收记录](验收-视频理解.md)。

## 已知限制

- 本片验证已接纳成果的发布和接入。中文、视觉索引的模型生产与内容质量分别在 VI10、VI11–VI12 完成；记录器回答不作语义推理。
- 发布仍物化完整媒体副本，成本按实际原片大小随发布次数累积。批次粒度与体积由本片验收记录给出。
- 独立窗口行为通过多人宿主验收；本地宿主保留既有单现场语义。Linux、安装态、真实读时模型与 DSH 沿原待办处理。
