# SESSION_CHECKPOINT_JSONL — 2026-10-02 23:43

## 新鲜度自检
- 写入时最新 commit：`5e10516 feat: integrate agent goals, presentation, build harness and reader updates`。
- 读入时对比 `git log -3`；工作树含多条其他工作线修改，以实际文件为准并保留这些改动。
- **JL0–JL9 已实现并验证，均未提交；当前入口：JL10 集成与发布验收。**

## 当前在做什么
JSONL 已接入桌面／服务启动、运行、领域关联、成果处置及备份恢复；本次阅读回顾的只读投影、接口、面板和原处跳转已完成，等待 JL10 真实整体验收。

2026-10-02 用户授权部署后，Linux 多读者服务已切换到 `ex13-jl-20261002`。两读者隔离、选定演示回顾、保存重开、原生备份还原及一次真实现场追问通过；追加 19,280 字节，旧前缀与其他聊天保持。公网登录、书库及阅读正常。完整记录和剩余范围见 [Linux 发布记录](docs/Linux上线-EX12-EX13-JL.md)；JL10 整体仍未完成。

## 下一步（可直接接手）
1. 按下方读序读取 ADR-0152／0153 和实施方案 §19，准备 Windows 桌面、Linux 多读者服务各自的独立验收数据根。
2. 在两种宿主执行“新聊天 → 含工具／演示回合 → 保留成果 → 打开回顾 → 重启回看 → 备份还原”；检查原来源、结果对象和演示版本。
3. 在 Linux 使用两个读者验证私人记录／回顾／引用隔离，并覆盖准入、停止、重连和历史切换。
4. 按 §19 固定问答与不同长度历史记录新增字节及涉及文件，验证普通回合不写回旧消息或无关会话。
5. 将实际命令、平台、结果和发布步骤写入实施方案 §19／代码链路，再刷新本页；执行既有部署放行规则。

## 未提交 / 未完成
- JL0–JL7 原实现仍在工作树：session_event／log／store、FrozenTurn 位置引用、运行增量、原阅读现场／来源／教学／成果、启动与备份恢复。
- JL8：新增 `session_recap.rs`；本地／多人 `GET /agent/history/recap?session_id=…&through_seq=…`，省略截点固定当前已提交末尾。
- 问题、绑定来源、成果、待继续项携带 turn／event_seq；当前对象可用性与历史状态分离。读取不补存不确定事件、不调用 provider／工具、不写学习事实。
- JL9：新增 `SessionRecap.vue`／`session-recap.ts`；RightRail 入口、手动刷新、全文、四区空态、不可用提示和原处跳转。
- 跨发布由 NetworkApp 打开明确原发布并选原聊天，App 等历史恢复后定位；切聊天丢弃旧回顾，打开面板不移动原文或演示。
- 演示补齐：`presentation_api` 同时认可真实交付事件；终态保存中断后仍能打开原版本及来源，未交付候选不可读。
- 连续处置补齐：笔记／标注成功保留后可撤销真实结果对象；两个截点分别恢复已保留／已撤销，重复撤销不重做；每次处置证据仍在日志。
- ADR-0153、CONTEXT、实施方案、架构、代码链路及本页已同步；本轮未创建 git commit。
- JL10 已完成 Linux 发布与首轮宿主验收；Windows 桌面整体验收、不同历史长度写入矩阵及完整连续使用场景仍待完成。下一步先读发布记录，复用已有结果；其他工作线沿各自检查点接手。

## 已完成验证
- Windows 独立 TEMP/TMP：后端相关回归按测试名称取最终结果，去重 97 项通过，无未解决失败。
- JL8 专项覆盖截点隔离、来源筛选、未确认写入、真实对象保留／撤销及连续处置、失败／忽略、目标、重开、原发布和私人授权；Memory／Learning／Reader 状态及 provider 次数保持不变。
- JL5 原来源压缩／重开和旧演示版本样本已增加回顾断言；JL9 演示终态中断与连续处置缺口均先复现失败，再修复通过。
- 后端日志：`tmp/jl89-regression.log`（95 通过）、`tmp/jl89-presentation-green.log`（27 通过）、`tmp/jl89-keep-undo-green.log`（17 通过）、`tmp/jl89-keep-undo-final.log`（1 通过）；分组重叠，不相加。
- 9 个 ignored 入口中，JL4 强杀子进程由父测试执行四个窗口；其余为浏览器宿主、演示环境或真实模型专项。
- 前端六文件去重 53 项通过：SessionRecap、RightRail、network-client、App.startup、NetworkApp.recap、effect-disposition；日志 `tmp/jl89-web-final.log`、`tmp/jl89-effect-web.log`。
- Web 类型检查／生产构建通过：`tmp/jl89-web-build.log`。保留已有 ts-rs、KaTeX 和大块资源提示。
- Playwright 390px／1440px 两项通过：打开只读、无水平溢出、四区可滚动、手动刷新、返回聊天后草稿和焦点保留。
- 浏览器日志 `tmp/jl89-browser.log`；截图 `tmp/jl89-recap-390.png`、`tmp/jl89-recap-1440.png`。使用既有移动工作区夹具；未替代 JL10 真实宿主／Linux 发布验收。

## 冷启动读序
1. [ADR-0152](docs/adr/0152-resident-linear-jsonl-session-log.md)、[ADR-0153](docs/adr/0153-session-reading-recap.md) 全文；[实施方案](docs/切片方案-会话JSONL与阅读回顾.md) §6–§8、§14–§20。
2. [CONTEXT](CONTEXT.md) 会话、RunScope／ReaderInputSnapshot、顺序日志、成果回执和阅读回顾定义；[架构](docs/架构.md) JL0–JL9；[代码链路](docs/代码链路.md) JL7–JL9。
3. `session_event.rs`／`session_event/projection.rs`、`session_log.rs`／`session_store.rs`、`session_recap.rs`：事实、截点、处置后继及四区投影。
4. `effect_disposition.rs:start/execute/complete`、`presentation_api.rs:delivered_references/delivered/source_binding`、lib／authorization／multi_user_host 的 recap 路由。
5. `SessionRecap.vue`、`session-recap.ts`、`RightRail.vue:openRecapTarget`、`NetworkApp.vue:openRecapPublication`、`App.vue:recapTarget watcher`；对应六文件测试及 `playwright/session-recap.spec.ts`。
6. JL10 恢复验收读 `run_admission.rs:frozen_input/recover`、`reader_maintenance.rs:backup_service/restore_service`，及 `tests/mu6_tests.rs:jl7_backup_restores_new_chats_checkpoint_and_original_domain_objects`。

## 本会话决策摘要
- 沿用 ADR-0153：固定已提交范围确定性派生，历史事实与当前可用性分离；不保存第二份回顾事实。
- 原 publication／对象／Presentation 版本决定跳转；真实交付事件可以独立于终态证明已交付，见实施方案 §18。
- 保留后撤销只接已有真实保留结果；业务与日志仍无跨存储事务，未确认结果保持待核对，见实施方案 §15／§17。
