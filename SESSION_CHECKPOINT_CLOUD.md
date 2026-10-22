# 云端学习实施检查点

更新时间：2026-10-21。目录：`E:/allwork/download/agent/understand-book`，基线 HEAD `33c19c7fa8aec9a4f1031122bc177743b234b9fd`。C1–C11 通用实现及本轮联合验证已完成；正式商品、课程和生产发布仍按下述边界处理。

## 最新修正：Windows 本机资料（10:55 HKT 已发布）

用户澄清本地导入不占云端空间，只有明确多端同步才受云端套餐限制。0.3.1 与配套网页已发布：新增真实原生文件库、离线原文阅读、单独云端副本操作；旧 0.3.0 提示升级。原生 6 项、Web 11 项、实际 WebView2 TXT/MD/EPUB/PDF/断网重载、三种公网 UI 入口与完整下载一致性通过。安装包 4,899,391 字节。未安装到用户当前应用，未改服务端程序或数据库。

权威证据与恢复位置：[本机资料发布记录](docs/performance/windows-local-materials-20261021/README.md)。代码入口 `apps/desktop/src-tauri/src/local_materials.rs`、`packages/web/src/local-materials.ts`、`PersonalMaterials.vue`。本机资料临时调用云端 AI 而不建立持久云端副本尚未接通；当前修复完成本机保存和原文阅读，不应将这条 AI 链路视为已完成。

## 授权与范围

用户明确要求按产品方案直接完成所有切片，不逐步确认。正式价格、各档额度、空间与保留期限、处理费率和实际课程仍待定。隔离验收使用明确的测试配置和原创材料，不将它们发布为正式商品。工作树有大量既有未提交修改，本轮未提交、推送；用户随后授权上线，已完成生产服务和 Windows 下载发布；不要回退其他工作。原 `SESSION_CHECKPOINT.md` 保留 OM6 历史并已更新当前云端发布指针。

产品合同：[产品方案](docs/产品方案-云端学习与考试冲刺.md)、[实施计划](docs/实施计划-云端学习与考试冲刺.md)、[ADR-0173](docs/adr/0173-cloud-learning-subscriptions-and-exam-sprints.md)。交付入口：[联合验收](docs/performance/cloud-learning-20261020/README.md)、[运行说明](docs/运行说明-云端学习与考试冲刺.md)。

## 已实现

- C1–C2：三档 × 月付/1/3/7 天商品与不可变已售权益；人工核款原子写入收款、原额度期与授额；续期、下一期换档、补充额度及读者/运营页面。测试或未完整配置的商品不可售。
- C3：账号跨工作区空间，整文件上传预留、重试复用、取消、下载、删除、持久成果占用及维护清理。公共资料不重复占私人空间，原私人材料按已有归属承接。
- C4：真实视频准备与学习原声复核按实际时长计量，同模型共享账号额度及父任务上限；晚到回执归原期，未知处理结果保留占用并支持明确继续和人工核对。
- C5–C6：课程套餐授权贯穿目录、材料、模型、MCP 与接续；个人考试日期、考纲、可用时间、直接讲题/短诊断入口；日历随当前日期重算，旧版本不误投影，最新考纲优先。
- C7–C8：Windows 同源云 Reader、系统浏览器 PKCE、独立桌面会话。正式包移除本地 Server/Runtime 与媒体 sidecar，维护工具独立，公开默认插件目录收缩。
- C9：PDF 原页和来源映射，PPT/PPTX 真实转换；原件、页码、公式/表格与后续写回保留。转换能力不足时提前限制对应格式。
- C10：原创教材/字幕/实际视频夹具、公共课程发布模板、分档课程、双用户实例与明确采用新版；外链导览独立于已准备材料。
- C11：真实 HTTP Reader/Admin 浏览器验收、旧 schema 升级与相关回归、实际模型/来源/费用记录、最终 Reader/Admin 及 Windows 构建。

元数据 schema 19 沿原升级事务增加 cloud_plans、cloud_storage、service_usage、course_access、exam_sprints、desktop_auth。服务额度仍只有原账本余额，没有另建平行余额。

## 教学实测修复与最终结果

真实调用发现并修复了否定页面意图、首请求工作区资料发现、教材中支持与精度的区分、考纲优先，以及普通考试终答跳过既有教学审阅的问题。有效考试目标的 observing 答疑现复用 `review_delivery`、原 Run adapter/费用及 AgentWorkReviewed；草稿通过前不显示。预算终结、引用 repair 和自由生成 fallback 都不能绕过。已有 active Tutor 不重复审阅，普通非考试聊天保持原行为。

第五轮接入审阅后仍虚构评分后果。既有 workspace_goal 指引与 tutor_review_rules R3 因此补入通用考试依据规则（审阅 v17）：学习建议、考纲与评分要求分别判断，评分后果须有明确依据，不加单题特例。

第六轮 `costs/grading-grounded/` 用相同原问题、材料和考纲完成真实 revise→修改→accept。最终正文只在审阅结束后显示；核心解释、范围和两个来源通过。主循环 5 次、审阅 2 次，共 207,916 tokens、供应商请求 112.035 秒，7 条费用全部结算并精确对齐同账号/Run/workspace/额度期。全部六轮合计 53 次、1,587,128 tokens、373.461 秒，保留失败历史，不将测试记账金额当供应商成本。

第五轮另有后台 memory_review 的最终结算未观测：供应商 usage 已保存，旧脚本在账本 sent 时停止。该历史测量缺口保留；脚本现有界等待已发调用结算后再停止，不生成额外模型请求。最终轮没有后台调用，所有已发调用均结算。

## 验证与产物

- 服务 69 项、Runtime 69 项去重，共 138 项通过；4 个手动常驻宿主 ignored。逐项结果和原失败日志在 `docs/performance/cloud-learning-20261020/validation/results.json`。这不是全仓库测试全通过的声明。
- 最后统一编译 `cargo test -p server -p runtime --lib --no-run` 成功，5m30s，日志 `target/cloud-exam-grading-compile.log`。最终 `exam_` 6 项通过，日志 `target/cloud-exam-grading-runtime.log` 已汇入证据。此前教学交付 33、fallback 9、审阅持久化 2 项均通过，无需无故重跑。
- Server 测试程序 `target/debug/deps/server-d3e6da6e7b4a779d.exe`；Runtime `target/debug/deps/runtime-99d760291ad586c2.exe`。Cargo 已释放。
- Reader 最终 `VITE_MULTI_USER=1` 构建与 Admin 构建通过。真实浏览器覆盖商品、核款、日包、空间、账号隔离及桌面授权，页面错误为零；证据在 `docs/performance/cloud-learning-20261020/browser/`。
- 文档准备 5 项、原构建管线 21 项通过；真实 PPT/PPTX、隐藏页、公式与表格来源验证见 `docs/performance/cloud-learning-c9/README.md`。
- Windows `dist/UnderstandBookSetup.exe`：0.3.0 x64，3,154,625 字节，优化构建 8m26s，origin 为 `https://understandbook.top`。实际安装清单仅主 exe；桌面 3 项、维护 18 项、授权 4 项及实际 WebView2 三进程会话验证通过。详见 `apps/desktop/README.md`。

## 发布与环境边界

正式商品值、服务费率、实际授权课程尚未确定；真实大学课程覆盖、学习效果与完整成本样本不由小夹具代表。2026-10-21 09:22:14 HKT 已发布 `cloud-20261021`，schema 19，PID 489592；Windows 0.3.0 正式下载已开放，配套 API/Reader 已部署。旧数据副本逐表比较、完整备份、公网 API/静态及七条浏览器路径通过。没有操作用户现有安装；常驻 DSH 准备宿主尚未接入，正式上传／商品仍未开放。完整恢复位置与限制见 `docs/performance/cloud-learning-20261020/release-20261021/README.md`。

C: 临时盘曾满，后续编译和测试使用项目 E: TEMP/TMP，单 Cargo job、DEV/TEST_DEBUG=0、INCREMENTAL=0。早期缓存问题已解决，不需要再清理缓存或重复全量检查。

自动审批策略阻止删除 `tmp/c9-orchestrator-verification`，该目录已保留，没有绕过。最终回复须简短说明这项清理限制。

## 本次发布收口

生产原数据根保持；升级前备份 `/opt/understand-book/backups/cloud-20261021`，旧配置在 `/opt/understand-book/acceptance/cloud-20261021/previous-configuration`。旧版不兼容 schema 19，后续恢复不得覆盖上线后数据。服务器真实 PPT/PPTX 转换使用 `/opt/libreoffice26.8/program/soffice`；7.1 漏隐藏页。部署验证模型调用 0，正式价格／课程仍待定。脚本和隔离源码在 `tmp/cloud-release-20261021/`。该目录下临时依赖清理被自动审批拒绝，已保留，前端另建隔离入口完成。
