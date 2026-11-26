# 本机 Agent 接入与跨入口学习切片方案

2026-11-19 · **本地版已接入 ACP、个人 CLI/MCP、教学、论文与 Skill。** 切片 1–3 定义首条跨入口学习路径，切片 4–7 定义教学、论文、社区 Skill 和 ChatGPT Web 接入。第 5 节记录各入口的实际验证范围。[ADR-0189](adr/0189-local-agent-learning-with-acp-and-mcp.md)

## 1. 用一段真实原文贯穿两个入口

1. 读者在原创演示书《让每一次理解都有出处》中圈住“候选位置”，选择「使用我的 Agent」，问“为什么找到位置还要读原文？带我看依据”。右栏显示 Agent 正在查哪份材料。点回答中的来源能回到实际读过的段落，再一键返回原选区。
2. 读者说“把我的这句话记在这段旁边：候选帮我找，原文帮我判断”。笔记落在选定的段落，可以编辑、撤销。读者随后问“如果两段看起来矛盾呢？”并把这个问题留到下次。
3. 重开应用，从另一个已连接的 Agent 说“继续上次那本书”。它能找到书名、那条笔记和未解决的问题。读者要求“把两段并排给我看”，Reader 显示两处原文。读者能从任何一个入口回到相同的学习记录。[原创演示原文](../examples/quickstart/book.md)

## 2. 现有基础和接入位置

| 已有入口 | 本次接入 |
| --- | --- |
| 正式本地桌面启动进程内 Reader，按用户数据目录加载书库、模型设置与私人记录。现有插件管理服务于构建工具。[本地宿主](../crates/local-desktop-host/src/lib.rs) · [构建插件发现](../crates/desktop-support/src/plugin_manager.rs) | 在本地桌面设置增加「我的 Agent」。宿主负责发现、启动与关闭 ACP 子进程。共享阅读界面从宿主取得可用入口。云端浏览器的托管 Agent 沿原有运行方式。 |
| 本机 `book_mcp` 可用显式目录绑定一本书，通过 stdio 提供检索、原文和论文工具。它以访客身份运行。[启动与身份](../crates/reader-service/src/bin/book_mcp.rs) · [书籍工具合同](../crates/book-tool-contracts/src/lib.rs) | 复用相同的检索和读取实现。读者在 ACP 回合中实际打开的原文，由本机 Reader 记录到该回合。个人连接的 CLI 和 MCP 也调用同一 Reader 命令，私人写入只有一份。 |
| 阅读器已有选区、证据账本、笔记修订和任务投影。当前本地任务列表按当前工作区和材料过滤。[选区](../packages/web/src/useReaderSelection.ts) · [原文证据与引用](../crates/runtime/src/orchestrator.rs) · [个人笔记](../crates/reader-service/src/personal_note_api.rs) · [任务查询](../crates/reader-service/src/learning_task.rs) | 读者身份、绑定材料、原操作结果和课程判断都从这些入口取得。外部“继续上次”需要能在已授权的书中找到原任务，再打开确切材料，不能只查询窗口中碰巧打开的那本书。 |
| Workbench 负责本地材料基础准备，论文元数据有既有投影，完整语义构建可交外部 Harness。[材料准备](../packages/core/src/build-workbench.ts) · [读书与构建资源](../apps/local-desktop/README.md) · [多 Harness 预构建](切片方案-多Harness预构建与DeepSeekHarness适配.md) | CLI、论文来源和教学 Skill 使用这些就绪状态。能读原文时即可问书，论文元数据、正式课程等能力按材料已有成果开放。 |

## 3. 两个入口怎样共用一次学习

**身份与执行。** 书内发问时，读者选择内置 Agent 或已配置的本机 Agent。后者由桌面进行 ACP 握手，在 `session/new` 带入本次可用的 MCP 服务。Agent 的更新用于显示回答、真实工具活动和等待输入。来自外部客户端的个人学习连接由读者在本机设置启用，绑定所选书或工作区，并明确能否操作页面、写入私人记录。连接可以断开或撤销。单书访客仍可独立使用原 Book MCP。首批验收 OpenCode 原生 ACP 和 Codex 的 ACP 适配器，自定义启动项支持可执行文件和分开的参数。[OpenCode ACP](https://opencode.ai/docs/cli/acp/) · [Codex ACP](https://github.com/zed-industries/codex-acp) · [Agentero 会话接入参照](https://github.com/poco-ai/Agentero/blob/main/docs/backend/context.md)

**回合与引用。** 提交时固定材料与修订、选区或原生位置、聊天、问题及已有的课程尝试。首问提供必要摘文和位置，其余按需读取。在已选范围内跨书查证时，沿用确切来源身份。已验证的选区和工具实际交付的原文范围连同书籍归属记入原回合，复用 `TurnEvidenceLedger` 与 `source.present` 的验证、引用和渲染规则。排名候选用于定位，精确命中可以证明某词出现于此，解释段落含义则需读到相应原文。PDF 图或公式只有页内位置时保留该页与选区，正文映射成立后才给出正文跳转。切书后到达的回答回到原聊天，引用仍打开原材料。[来源规则](adr/0086-runtime-owned-user-visible-source-references.md) · [选区规则](adr/0135-selection-answer-continuity-and-reader-preference-priority.md) · [跨书来源](adr/0169-cross-book-reading-and-private-understanding.md)

同一 ACP 会话依次处理问题。会话所带的 MCP 将工具调用转给持有原回合的 Reader，Reader 在接收调用时绑定当前运行，完成时仍向该运行交付。取消后工具调用尚未收尾，或新问题改用了另一份授权范围时，宿主另开 ACP 会话。直接启动原有访客 `book_mcp` 可以取回正文，支持可点击引用还需要这条回合内的交付记录。

**本机操作与回执。** CLI 和 MCP 按连接范围调用管理该数据目录的 Reader。宿主公布本次本机连接地址，已有实例就复用。没有运行实例时，安装版可按已选配置启动宿主，无界面脚本也能启动同一公共 Reader 服务。桌面与脚本同时使用时由同一实例提交私人写入。读者要求保存的笔记返回稳定笔记 ID、修订和放置位置，主动提议的批注沿用保留或撤销。并排显示要等页面回报实际打开的两处位置，无窗口时返回可打开的位置和显示入口。笔记写入成功与窗口是否打开分别报告。[Reader 操作与可撤销提议](adr/0030-e-agent阅读器形态-外层入口-reader双向共享-可撤销提议-session层提议-idle会话边界-精炼上下文.md) · [写入与来源归属](../crates/reader-service/src/memory_commands.rs)

**换 Agent 与接续。** 一次运行固定已选 Agent。读者可以补充问题、取消或切换。支持运行中补充的适配器可以直接送达，其余在原聊天保留下条输入，并显示何时发送。取消或适配器退出后，Reader 终结原运行，保留已交付内容与已成功的操作。换 Agent、旧协议会话无法恢复或材料范围改变时，从 Reader 的原任务、上次问题、确切笔记或材料引用和待完成步骤重建上下文。旧进程的迟到结果不得推进新任务。费用按 Agent 能报告的实际信息显示，供应商未报告的用量记为未知。[会话日志](../crates/reader-service/src/session_store.rs) · [任务接续](adr/0184-task-led-learning-entry-and-continuity.md)

Reader 发出的原问题已在本机聊天里。从外部聊天说“把这个疑问留到下次”时，个人 MCP 将选定的问题、来源和下一步显式保存到对应学习任务，返回可重开的任务引用。普通 MCP 只能看到实际发送给工具的内容，外部聊天中其余消息由原客户端保管。外部助手写入的摘要保留助手署名。

**教学与学习者原话。** 当前任务决定题面、帮助和资料的可见范围。教师可按教学规则查证，学习者收到的提示或讲解才记入对应尝试。闭卷时，展示给学习者的外部工具和接续摘要沿用同一遮盖规则。通用 Agent 自带历史和文件工具，在无法保证本次独立作答条件的客户端中，可以求助或回到 Reader 作答。Agent 代写、转述和总结保留助手来源，独立表现由原练习接纳学习者实际提交的内容后认定。[闭卷尝试](adr/0183-closed-book-dialogue-to-independent-performance.md) · [课程帮助归属](../crates/reader-service/src/course_context.rs)

## 4. 按用户路径交付

### 切片 1 · 从阅读选区请自己的 Agent 查证

从设置连接已安装的 OpenCode，实际走通握手、发问、工具读取、流式显示、引用跳转和取消。问答栏沿用现有来源样式，输入框附近显示当前 Agent，缺少命令或登录时指向对应设置并保留问题。先通过本机 Reader 给本轮提供确定性检索、读取和来源呈现，查一段原文即可回答时只发生所选 Agent 所需的调用。

**验收**　在正式本地壳用第 1 节的选区发问，检查真实 Agent 读过的段落和点击后的位置。用受控适配器分别送入只有排名候选却引用段意的回答、切书后的迟到回答和取消后的完成事件，验证来源、聊天及运行终态。另保留真实适配器的取消结果和实际窗口截图。

### 切片 2 · 从外部 Agent 找回问题，操作同一本书

依赖切片 1。个人 MCP 和面向用户的 CLI 接入同一组操作，支持列出已选材料、读取现场、查找或继续任务、读原文、跳转与并排、写笔记和高亮。请求以书和来源引用定位对象，歧义时返回可选择的书名与位置。CLI 对脚本提供 JSON 结果和失败退出状态。写入回执带原对象及操作标识，重试复用已提交结果。Reader 显示真实写入，个人笔记沿原修订规则编辑，重复句中的高亮保留确切选区。

**验收**　从外部 Agent 找回第 1 节的问题，用 CLI 保存指定笔记，再让 Reader 打开并编辑同一笔记。反向从外部留下新疑问，换回 Reader 从原任务继续。断开一次保存响应后重试，磁盘中只有一条新笔记。关闭窗口后经个人连接添加一处高亮，重开仍在原句。两本书出现同一句话时，测试比较、返回、撤销及撤销连接后的访问结果。用访客 Book MCP 读取相同原文，验证它看不到私人笔记和任务。

### 切片 3 · 安装、切换和移除 Agent 后继续学

依赖切片 1，与切片 2 联合验收首条完整路径。「我的 Agent」提供发现、添加、自定义命令、登录指引、连接测试、安装、更新、断开和卸载。使用 ACP Registry 或上游发布信息定位适配器，实测通过的组合标明平台与能力。设置用“连接已有安装”和“由本应用安装”区分管理范围，卸载只移除本应用管理的组件。增加 Codex ACP，按真实协商结果提供补充、取消和会话恢复。[ACP Registry](https://agentclientprotocol.com/get-started/registry)

**验收**　用原生 OpenCode 和 Codex 适配器分别独立完成划词查证，从 OpenCode 留下问题，在 Codex 通过切片 2 接回同一任务。覆盖已安装、未登录、安装失败后重试、应用管理组件卸载和用户已有安装断开。核对当前 Agent 名称、回合归属、笔记、旧聊天和另一工具中的已有安装。在独立公共源码导出中验证 ACP、CLI 和个人 MCP 的构建与接入说明。发布支持按 Windows 与 macOS 各自的真实安装包记录。[公共源码导出](双发行源码导出.md) · [本地发行验证](切片方案-开源本地应用与云端服务双发行.md)

### 切片 4 · 用自己的 Agent 讲解、练习并回课

依赖切片 1–3。复用当前课程或练习的准备、交付与作答入口，支持“直接讲”“给一点提示”“换一道题让我试”。换 Agent 或重开应用后，仍接续原问题与原尝试的帮助记录。普通解释按普通问答保存，具体题目与学习者的实际表现才进入对应教学判断。

**验收**　同一目标先求助，再用一道尚未被代答的变式自行作答。核对题面、学习者原话、已显示帮助、反馈和回课步骤。让外部 Agent 请求闭卷前的解答或代报“我已独立完成”，验证个人工具的可见范围和课程记录。程序用真实私人存储断言状态，真实教学样本依据原文和学生回答单独评价。[课程验证入口](实施记录-书籍与成套课程入口.md#功能验证入口) · [闭卷记录](实施记录-闭卷对话练到会.md)

### 切片 5 · 从终端或 Agentero 导入论文并查元数据

依赖切片 2。CLI、个人 MCP 与 Reader 调用同一材料准备入口。导入本机 PDF 后返回所属材料、可读状态和后续准备动作，可按材料取元数据、查正文并写高亮。Agentero 作为可选论文来源，通过它的 `paper_list`、`paper_get` 选取论文和元数据，有可取得的原文时交给同一导入入口。DOI 或 arXiv 标识可沿 Agentero 的标识导入能力解析。只有元数据时保留出处并提示补原文。完整语义预构建沿现有插件和已确认的构建计划。[Agentero 论文接口](https://github.com/poco-ai/Agentero/blob/main/docs/backend/mcp.md) · [论文元数据工具](book工具.md)

面向脚本的目标用法如下。导入返回材料引用，原文读取返回带材料、修订与实际范围的定位结果。后续命令复用这个位置，引用历史回答时还携带所属聊天与回合。

```text
understand-book paper import "<PDF 路径>" --json
understand-book paper metadata <材料引用> --json
understand-book note add --at <来源引用> --text "这个结果只覆盖第一组" --json
understand-book highlight add --at <选区引用> --json
```

**验收**　同一份可用论文分别走本机文件和真实 Agentero Vault。核对题名、作者、元数据出处、原文跳转和准备状态。用一条只有元数据的记录验证补原文路径。对导入后尚未生成的字段显示实际缺口，再在已有预构建的论文上核对确切字段与证据。验收材料可从[学习材料包](../materials/README.md)选取。

### 切片 6 · 导入社区 Skill，实际带读一篇论文

依赖切片 2、4、5。接受本地 Skill 目录和 GitHub 来源，读取 `SKILL.md` 的名称、说明和随附资源，安装到本机可写的学习工作区或所选独立书库的私人配置位置。适配器有原生发现能力时通过其支持的位置加载，否则由用户显式选用并把所需教学指令交给本轮 Agent。设置展示实际生效方式。脚本资源仅在用户选择的 Agent 具备相应执行能力时使用。[Agent Skills 格式](https://agentskills.io/specification) · [Agentero 导入参照](https://github.com/poco-ai/Agentero/blob/main/docs/backend/skill-import.md)

**验收**　用一个“带读实证论文”Skill 完成“找研究问题 → 读方法和结果 → 并排证据 → 请学习者解释差异”。在另一个未启用该 Skill 的工作区正常问答，在原范围停用或移除后开新回合。分别核对加载方式、实际取证、读者作答和原笔记，使用工具成功与教法是否达到目标分开记录。

### 切片 7 · 从 ChatGPT Web 读指定的本机材料

依赖切片 2。为允许自定义 MCP 的 ChatGPT 工作区提供本机接入指引。通过 Secure MCP Tunnel 连接指定书的 stdio Book MCP，读者需要个人操作时显式选择切片 2 的个人连接及范围。云端账号 HTTPS MCP 继续服务该账号获准的出版物，其凭证和额度归云端。已有源码只读 MCP 隧道示例可以参考，读书连接指向实际的学习工具。[OpenAI 接入与验证](https://developers.openai.com/plugins/deploy/connect-chatgpt) · [项目隧道参照](代码只读MCP.md) · [云端账号 MCP](MCP接入.md)

**验收**　在真实 ChatGPT Web 会话发现工具，检索指定书、回读命中原文并核对位置。用个人连接保存一条已授权笔记，再到本地 Reader 打开同一条。测试断开隧道、撤销个人连接和恢复后的可见范围，分别记录客户端、认证方式和走通的读写操作。

## 5. 验证基线与实施记录

2026-11-19 已用当前工作树的 `target/debug/book_mcp.exe` 对原创演示书走过本机 stdio 的初始化、工具发现、检索和原文回读。发现 17 个工具，`book_search_text` 检索“知识图谱”命中 `1.3.4`，按返回的 LID 调用 `book_text` 读回对应段落。此次未调用模型。它是切片 1 的读书基线。

### Windows 本机实施记录

| 用户路径 | 实现及证据 |
| --- | --- |
| 在书中选择 Agent | 已接入 OpenCode、Codex ACP 和自定义命令。Windows 上 OpenCode 1.19.6、Codex ACP 2.7.0 均走通受管安装、握手、选择、更新和卸载。首次 OpenCode 冷启动触及原先的 30 秒上限，现允许等待 60 秒。日志在 `docs/evidence/local-agent-20261119/`。 |
| 取证、引用与授权 | 两个真实适配器均用本机受控模型回读 `1.3.4`、生成引用、打开来源并取消后续回合。OpenCode 切换到 Codex 后，新 Agent 收到了同一聊天的原问题和已交付回答。为使 Codex 2.7.0 首轮发现工具，宿主在创建带 MCP 的会话时采用其支持的就绪等待参数。浏览器从“候选位置”划词发问，点击回答定位“真正支撑回答的是取回的原文”，再回到原选区。另用请求授权的 ACP 进程验证仅此次允许与取消。[协议回归](../crates/reader-service/src/local_acp/tests.rs) · [真实适配器验收脚本](../scripts/verify-local-agent-acp.py) |
| 从终端或外部 Agent 继续学习 | 已从 CLI 和个人 stdio MCP 在同一数据目录检索、回读并保存。重开后，两条笔记（其中一条是待续问题）、一处精确高亮和原任务都能找到；重试三种写入均返回原对象。撤销凭证对已打开的 MCP 连接生效，只读连接和访客 Book MCP 均拒绝写入。可用[个人连接验收脚本](../scripts/verify-personal-connection.py)复跑；命令见[个人学习 CLI 与 MCP](个人学习CLI与MCP.md)。 |
| 提示、换题、作答和重开 | 在原创演示书上，OpenCode 与真实模型给出提示后，Reader 把帮助记在第一次尝试。新变式要求判断两条相关边能否推出因果，教师追问后接受了澄清，原库保留第二次尝试和答题原话。22 次转发中 21 次报告的用量合计 203,164 tokens。首次变式过于接近原题，后续追问也出现过一次对反事实表述的误读。[题面、反馈与已知限制](实施记录-闭卷对话练到会.md) |
| 论文和 Skill | 已用 Agentero 0.16.2 的独立 Vault 导入一篇 90 页论文，Reader 落盘的 PDF 与原件一致。经工作台 PDF/OCR 处理得到 1,951 个可定位位置，从书库打开后读回了标题和作者。仅含元数据的条目可按原 ID 补 PDF。启用带读 Skill 后，真实 OpenCode 向受控模型传入其指令，回读实验方法与结果，交付两条可点击引用并请学习者解释差异。设置中覆盖安装、启用、停用与移除。验收目录为 `docs/evidence/local-agent-extensions-20261119/`。[导入与使用](本机论文与教学Skill.md) |
| ChatGPT Web | 已提供单书只读和授权个人连接两种 stdio 配置，以及 Secure MCP Tunnel 接入步骤。真实 ChatGPT 工作区的工具发现、读取和写入待目标账号联调。[接入步骤](个人学习CLI与MCP.md#通过-chatgpt-web-连接本机) |

独立公共源码导出在 Windows x64 的空构建目录中，以 `cargo build --locked --offline --release` 生成了个人 CLI 和访客 Book MCP。用这对程序复验了私人读写、幂等重试、重开、授权撤销和访客隔离，配上发行用 Node 与 Workbench 后，也完成了 PDF 导入、元数据查询与正文准备。构建与验收结果保存在 `docs/evidence/local-agent-20261119/public-cli-build.json`。

Windows x64 安装包 `dist/UnderstandBookLocalSetup.exe` 已生成。将最终包解到独立目录，用包内 WebView2 应用验证了两种启动方式。自行启动时可从演示书添加和测试 Agent，重载后仍选中它，关闭窗口后监听端口关闭。已有命令行宿主时，窗口接回原聊天，包内 CLI 发出跳转，页面精确标记“候选位置”并回报已显示；关窗后原宿主继续运行。包内的个人 CLI、访客 Book MCP 和 GUI 的回合 stdio 入口均通过复测。窗口证据在 `docs/evidence/local-agent-learning-20261119/ui/`，包内程序的读写与重开结果在 `docs/evidence/local-agent-20261119/windows-packaged-cli.json`。本轮以解包运行验收，系统安装、升级和卸载流程沿用本地发行专项验证。

### 当前边界

- 每轮建立独立 ACP 会话。普通问答送入当前聊天最近六轮问答，并按轮次带入较早的读者原话。旧引文有固定预算；若有省略，Reader 给出起始轮次，Agent 可用本轮只读工具 `reader_history` 分页取回用户原文。后续纠正按时间顺序生效，笔记保存与页面操作以实际回执为准。第八轮同 Agent 和换 Agent、另一聊天的隔离及长句续读已有真实 Reader 配受控子进程的回归。[长对话实验与复跑入口](切片方案-长对话状态保持与规则遵循.md)
- 书内 ACP 从当前打开的材料取证。外部个人连接可以在读者授权的多本书间检索、定位与并排。部分客户端会将上游网络故障编码成普通回答并返回正常结束，Reader 只能按客户端实际报告的协议状态记录。
- Windows 和 macOS 的发行验收分别留证。当前机器可以验证 Windows；macOS 安装包及真实 ChatGPT Web 会话尚待相应环境。
- 从 Vault 选择已有论文和补 PDF 已用真实 Agentero 验收。用全新 DOI 调用 Agentero 的标识导入时，上游书目查询在当前网络失败，Reader 返回了原因。在线解析新 DOI/arXiv 标识仍取决于 Agentero 可达的书目服务。
