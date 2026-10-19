# SESSION_CHECKPOINT_WORKSPACE_QUOTA — 2026-10-18

## 新鲜度与状态
- 主目录 E:/allwork/download/agent/understand-book，基线 HEAD 402d9be；保留本线及其他工作线未提交修改。本线未提交、未部署，根 SESSION_CHECKPOINT.md 未覆盖。
- 最新授权E1 A–D联合验收、发现问题修复、上传入口改进已完成。D0/W1–W6/Q1–Q5/S1–S2/B1–B2/V1/E1通过；E2真实运行、成本对照与发布准备尚未开始。
- 默认Pass2 disabled、build_tutor false保留。Control schema18、learning schema3不变。没有委派新子代理，原视频worktree和整讲成果未改动。
- 本轮4190/4191临时HTTP宿主和4174页面服务已关闭；没有留下后台验收工作。
- 文档核对：59处本地链接/章节锚点、7份证据JSON有效；相关差异空白检查通过，交接记录45行。

## E1 实现与修复
- crates/server/src/tests/e1_workspace_integration_tests.rs经mu6_tests注册，串联临时真实DB、本机受控Provider HTTP、真实工具循环、后台整理、共享实例/视频/私人理解、个人准备账本、旧记录升级。
- PersonalPreparations.vue提供点击/键盘/拖放上传区、格式提示、文件名/大小、实际字节进度和保存/任务状态；390px适配，任务归属沿原B2。
- 修复纯画面/原声私人理解被文字行号约束阻止：cross_book::learning从本轮已绑定来源生成TeachingTarget.media_bindings，保存EvidenceTarget确切媒体身份，不补造文字位置。
- memory新增base-schema依赖及TeachingMediaBinding，旧目标省略新字段可继续读取。该字段由运行时派生、不进入模型可写Tutor schema；teaching::bind_target拒绝直接提供。新Web TS绑定已导出。
- W6旧行fixture改为直接写入历史数据，保留旧帮助引用形态；现有新假设入口仍要求真实用户表达。
- packages/web/playwright/workspace-e1.spec.ts按实际资料身份与导入成果验证两账号、上传/额度/接续/历史/视频/共享更新；不能固定用排序位置代替视频身份。
- personal-http.acceptance.ts新增UNDERSTAND_BOOK_ACCEPTANCE_OUTPUT，E1证据独立输出，不覆盖B2历史。

## A–D与验证
- A通过：同资料两个工作区独立；实际读取、冻结目标/改向、理解修订、后台原归属及重启原聊天接续。5次受控Provider HTTP请求。
- B通过：两账号独立共享实例；真实FFmpeg解码2帧、纯画面私人理解；一人加入私人资料并更新，另一人旧版不变，旧来源仍读原发布。公共成本12 micro-CNY只有1份，无新增模型调用。
- C通过：真实回答+后台整理2次请求；外部失败收费/重试/取消/未知/迟到回执经实际账本恢复。费用23,270、用户23,000、平台270 micro-CNY；补50%后余额120.7%，历史与重复回执不重算。
- D通过：v15控制边界升至18，原账保留；原JSONL和理解证据不改写，只带入选定记录；重复带入不生事实，重开可读。
- 28项去重定向测试通过：Server6、Memory21（含类型导出6）、Runtime1。Web类型检查及多人生产构建通过。
- 真实编译Engine+原生DSH+Rust HTTP：49次受控模型请求、637 micro-CNY测试费率、待核算0；预算拒绝前0请求，暂停和宿主重建复用Pass1，改目标/引用已有成果不新增调用。
- 真实8秒MP4+本地Whisper生成2段原文，个人视频0付费CLI/0模型额度扣减并进入原工作区。
- 浏览器完整用例11.6秒通过：81,016字节3块上传，键盘/拖放，真实40%截图；1.25%确认、暂停接续、继续阅读个人成果、历史保留、视频播放/1–4秒范围、共享更新/刷新；0页面错误，390px无溢出。
- 已查看桌面/手机上传、实际进度和完整手机完成状态截图。最后一张扩展视口高度以容纳完整区域，844px高视口的无溢出仍单独验证。

## 文档与证据
- docs/联合验收-工作区与统一额度-20261018.md：改动、A–D、后台/浏览器、计算、复跑与限制。
- docs/performance/workspace-e1-20261018/：README、verification、scenario-a/b/c/d、http-result、browser JSON及4张截图；没有宿主登录连接。
- 切片方案§18、设计头部、CONTEXT、架构、代码链路、B2媒体说明已同步；历史各片记录保留。
- 通过日志：target/e1-server-final.log的5项+e1-server-b-final.log的B；e1-memory.log的19项+e1-memory-fixed.log的2项；e1-runtime.log。
- HTTP：target/e1-http-acceptance.log、target/workspace-e1-20261018/http-result.json；浏览器target/e1-browser-fourth.log；类型/构建target/e1-web-typecheck-final.log、e1-web-build.log。
- 早期失败日志保留本地。Server B最初费用比较含查询as_of、浏览器最初假设固定排序与导入前版本，已修正测试；勿把这些脚本问题列为产品缺陷。

## 复跑与接续
- Server：cargo test -p server --lib -- e1_workspace_integration_tests::e1_ tests::cross_book_tests:: tests::video_workspace_tests::video_workspace_mixed
- Memory：cargo test -p memory --lib -- teaching:: learning_evidence::tests:: learning_records::tests::；Runtime：cargo test -p runtime --lib -- tutor::t15_tests::
- 忽略的e1_browser_host提供4190/4191，先完成Rust构建再启宿主，避免Windows二进制文件锁。HTTP/浏览器完整命令见E1记录。
- Engine沿target/b2-v1-20261018/bin/understand-book-build-x86_64-pc-windows-msvc.exe，发布器target/debug/publish_book.exe；Whisper目录沿personal-http.acceptance.ts。Engine本轮未改，复用上轮已编译版本。
- B2/V1历史52项及真实原声结果留在docs/performance/b2-v1-20261018，不并入E1的28项；完整视频语义构建证据留在video-operator-20261018。
- 下一步E2需明确真实模型测试预算，验证目标遵循、教学质量、公共/私人构建实费对照与迁入恢复说明。本轮没有真实付费调用或部署。
- 个人视频当前为本地转写/原文学习，完整语义预构建继续平台承担；原始PDF需工作台对齐，外部CLI未知金额仍待核算。生产需DSH根会话、有效费率、共享成果目录与媒体工具。
