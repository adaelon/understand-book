# 切片 A 真实章节首轮协议

来源为当前 `docs/project-book/chapters/06-全书结构与语义候选召回.md` 完整文件。运行 `node --import tsx docs/performance/chapter-reading-a-20261017/prepare-fixture.ts` 调用正式 Markdown 块映射与分段器，生成原样 source.txt 与 base.json。独立导入后实际章 LID 是 `1`，显示章号仍是第 6 章；来源记录在 source-record.json。共 102 个规范节点、11,854 UTF-16 单位。没有 BookStructure 或教学地图，这是受支持的原文局部导读路径。

三个 fresh session 的首条自然输入分别位于 first-contact、existing-basis、specific-confusion/input-01.json；input-02.json 仅结束实验，不模拟读者回应。三个输入分别关注摘要与候选保留、目录/概念图与全书结构用途、未被章节接受的候选与主题证据。首条消息明确导读并如实说明起点，不指定教师要选择哪个问题或怎样教学。

每次运行检测的是：实际首轮是否由真实起点选择一个有用途的问题，进入 TeachingDesign，再被实际交付兑现。如果只列术语、没有适合该起点的支撑、设计未消费或交付不符，则保留失败的请求/响应/持久化材料，修改对应机制并只重跑受影响起点。内容结论由来源、实际 design 与可见首答作具体判断，不从 HTTP 200 推断。

在仓库根目录、项目既有 Rust 编译环境已初始化时运行，每个场景顺序一次（不要和其他 cargo 编译并行）：

```powershell
$env:TEACHING_LIVE_BOOK_DIR = (Resolve-Path 'docs/performance/chapter-reading-a-20261017/fixture').Path
$env:TEACHING_LIVE_SOURCE_BASIS = 'docs/project-book/chapters/06-全书结构与语义候选召回.md; complete file; canonical chapter LID 1'
$env:TEACHING_LIVE_OUTPUT = (Resolve-Path 'docs/performance/chapter-reading-a-20261017/first-contact').Path
cargo test -p server --lib teaching_design_live_subagent_conversation -- --ignored --nocapture
```

后两次只将 OUTPUT 改为 existing-basis、specific-confusion。不得重用已生成 request-0001.json 的输出目录；失败重跑用带 retry 后缀的新目录，保留原记录。开始前清除上次实验留下的 TEACHING_LIVE_STATE、TEACHING_LIVE_START_TURN 环境变量，确保不是恢复旧会话。

教师为独立 gpt-6.1-sol agent，仅处理普通 `request-NNNN.json` 中的真实 instructions/input/tools/tool_choice、图片等，按该次 schema 输出 `response-NNNN.json`，形状为 `{ "text": null | string, "tool_calls": [{"id": string, "name": string, "arguments": object | string}] }`。每次先写 .pending，再 rename 为 .json；不得改 request、input、源码或服务器状态。普通 turn 必须有非空正文或工具调用。教师不读其他起点、质量结论、预期选题或 reviewer 输出。

内容接纳由另一个独立 gpt-6.1-sol agent 处理 `kind=structured_completion` 的 request；它只读真实 CompletionRequest 中 system/user/schema，不读教师的额外轨迹或实验结论，按请求 schema 直接写 response-NNNN.json。教师不得代写这个响应。文件通道只是实际模型完成的传输，prepare_agent_chat、execute_prepared、工具执行、独立接纳、持久化与最终交付仍由服务器真实路径承担。

服务器保存 configuration.json（完整原文、来源路径、state_directory）、各次实际请求/响应、turn-01.json（含冻结 Tutor、持久化 turn、deliveries、elapsed_ms）、answer-01.md、可选页面源码和 complete.json。内容检查另保存实际 design（从真实 tutor.step 调用及后续 current_design 提取）、选题为何适合起点、支撑来源与帮助/邀请条件、最终可见首答以及是否兑现设计。模型调用数为文件请求数；provider token 用量在此适配器不可取得，必须明确缺失，服务器估算与实际用量不得混称。

本轮只验证三个真实首轮；没有模拟多轮理解进展、真人效果、部署或切片 B/C 验收。
