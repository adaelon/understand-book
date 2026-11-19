# ADR-0189 开源本地版接入读者自己的 Agent

2026-11-19 · **已接受，本地版已接入。** [切片方案与实施记录](../切片方案-本机Agent接入与跨入口学习.md)

读者可以在 Reader 中划住一段话，请自己已登录的 Agent 查证，也可以在这个 Agent 里说“接着学昨天那一段”，回到同一份材料、笔记和学习任务。**本地宿主通过 ACP 驱动读者选择的 Agent，公共 Reader 统一提供学习现场、工具和记录。** 这将已有的 Agent 教学决策权扩展到外部执行的 Agent。读者仍可选择内置 Agent 和自配模型。[原生学习环境](0119-agent-native-learning-environment-and-teaching-agency.md) · [双发行边界](0187-open-source-desktop-and-cloud-editions.md)

## 对话从两端开始

从书里提问时，本地宿主作为 ACP 客户端启动 Agent，以提问当时的选区和材料建立回合，并在会话中提供相应的 MCP 工具。外部 Agent 持有自己的登录、模型、工具循环和协议会话。现有模型 API 由内置运行时调度，资料的完整语义预构建由 Build Engine 和构建插件负责。把 ACP 接在模型适配器上，会让两套工具循环争用同一个回合。[模型接入决策](0186-multi-protocol-reader-model-providers.md) · [ACP 会话协议](https://agentclientprotocol.com/protocol/v1/session-setup)

从 Agent 或终端开始时，读者可以只连接指定书的访客 Book MCP，也可以显式启用个人学习连接，选择准许使用的资料和私人操作。后者经本机 Reader 读取任务、操作书页和保存笔记。授权属于这条连接和所选范围。换 Agent 可以接续读者的学习任务，原 Agent 的私有协议会话由它自己管理。云端 MCP 按账号读取已授权出版物，独立于本机个人连接。[访客 Book MCP](0035-book-mcp访客会话-修订p7无状态为连接式-三类记忆-临时游标-book-guide投影.md) · [账号 MCP](0162-beta-remote-mcp-and-prepaid-call-credits.md)

## 学习事实归 Reader

公共 Reader 以已有的书籍或发布版本、聊天、课程尝试和笔记修订标识操作对象。它记录工具实际交付的原文，生成可回跳的来源，接纳写入并返回可核对的结果。跨 Agent 接续读取这些记录和未完成任务。把上次聊天全文搬给另一个 Agent，无法可靠恢复被引用的笔记版本，也无法判断学习者何时看过提示。[回答来源](0086-runtime-owned-user-visible-source-references.md) · [笔记修订](0177-personal-notes-evolution-and-teaching-recall.md) · [学习接续](0184-task-led-learning-entry-and-continuity.md)

Agent 可以直接讲解、给提示、出变式，也可以调用社区贡献的教学 Skill。学习者原话、实际收到的帮助和新题上的独立表现分别记入原任务。个人笔记和高亮由本机保存。选择使用远程模型的 Agent 时，本轮选区和它取用的原文会交给该模型服务。[闭卷与帮助归属](0183-closed-book-dialogue-to-independent-performance.md)
