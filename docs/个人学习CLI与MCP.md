# 在终端、外部 Agent 和 ChatGPT 中读书

本地桌面安装资源的 `bin` 目录包含两个命令。Windows 使用安装目录下的 `bin\understand-book.exe` 和 `bin\book_mcp.exe`。macOS 使用应用包内的 `Contents/Resources/bin/`。下文用可执行名举例，也可以使用完整路径。

源码环境可运行 `cargo build --locked -p reader-service --bin understand-book --bin book_mcp`。从源码独立运行 `serve` 并导入 PDF 时，先在同一源码目录执行 `pnpm install --frozen-lockfile` 安装格式工具的依赖。桌面安装版随包提供 Node 和格式工具。

## 选择读书入口

| 使用目的 | 客户端启动的 stdio 命令 | 可见内容 |
| --- | --- | --- |
| 让外部 Agent 查一本已准备的书 | `book_mcp "<书籍工作区绝对路径>"` | 该书的检索、原文、元数据和访客向导 |
| 接续自己的任务、查笔记、留下问题 | `understand-book mcp` | 个人连接中授权的材料和操作，以该次 `tools/list` 返回为准 |

## 启用个人连接

在本地 Reader 设置打开个人学习连接，选择材料及读写范围，复制凭证。在终端的当前进程设置 `UNDERSTAND_BOOK_PERSONAL_TOKEN`，或通过标准输入交给一次配对命令。以下 PowerShell 示例从剪贴板读取刚复制的凭证，环境变量和命令行参数中都无需写入凭证原文。

```powershell
Get-Clipboard -Raw | understand-book connect --token-stdin
understand-book tools --json
understand-book material list --json
```

配对命令先向 Reader 验证凭证，再将它保存到本机个人配置的 `personal-cli.json`。使用同一数据目录的 CLI 和 stdio 客户端都可继承这次配对。单独启动的客户端也可以在自己的进程环境中设置 `UNDERSTAND_BOOK_PERSONAL_TOKEN`。在 Reader 中重新启用连接会换发凭证，撤销后已打开的 MCP 连接也会失去访问权限。`understand-book disconnect` 清除当前配置中 CLI 保存的凭证。

安装版工具会寻找持有这份数据目录的活动 Reader，需要时启动本地应用。同一配置也能从源码或终端运行 `understand-book serve "<已准备书籍目录>"`。首次使用新的数据目录时，在宿主页面选择材料并启用个人连接。`UNDERSTAND_BOOK_LOCAL_DATA_DIR` 选择配置根目录，工具和宿主都读取其中的 `data-location.json` 来接续已迁移数据。单独运行的本地服务可用 `UNDERSTAND_BOOK_READER_URL` 显式指定其 HTTP 回环地址。

## 从实际来源保存

启用个人连接时选择原创演示书，再运行下面的 PowerShell 命令。检索结果给出实际命中的段落和选区，原文读取结果保留当前修订。

```powershell
$bookId = 'quickstart-demo'
$found = understand-book book search $bookId "知识图谱" | ConvertFrom-Json
$hit = $found.occurrences[0].ranges[0].source_ref
understand-book book text $bookId $hit.lid | Set-Content -Encoding utf8 passage.json
$hit | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 hit.json
understand-book note add --at "@passage.json" --text "这里的图谱提供检索线索。" --operation-id my-note-01
understand-book highlight add --at "@hit.json" --operation-id my-mark-01
understand-book highlight list $bookId
understand-book task list $bookId
```

`--at` 接受原文读取的完整结果，或者其中的 `source_ref`。精确高亮一处命中时，可使用检索结果 `occurrences[].ranges[].source_ref`。新建笔记和高亮时可给定 `--operation-id`，脚本重试同一次保存时复用原值。每次不同的保存使用不同标识。成功后以 Reader 返回的对象和位置为准。

```powershell
$paper = understand-book paper import "D:\papers\paper.pdf" | ConvertFrom-Json
understand-book paper metadata $paper.book_id
@{ source_ref = $hit; text = '如果两个段落结论不同，该怎么验证？'; operation_id = 'question-01' } |
    ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 next-question.json
understand-book call task_save "@next-question.json"
```

`next-question.json` 包含原文读取返回的 `source_ref`、要留下的 `text` 和本次 `operation_id`。通用命令 `call <工具名> <参数 JSON>` 支持工具列表中的其他操作。`@文件` 和 `-` 分别从文件和标准输入读取 JSON。

查询授权书籍的元数据需要读取权限，导入 PDF 时还需启用写入和管理权限。用刚返回的 `book_id` 可以继续查询这次导入的状态。`workbench` 表示材料仍在准备，进度可在 Reader 的材料工作台查看。材料可读后，在个人连接中勾选新书并重新配对，就可以检索正文并保存高亮。[本机论文与教学 Skill](本机论文与教学Skill.md)

## 通过 ChatGPT Web 连接本机

1. 在 OpenAI Platform 创建隧道，把它关联到目标 ChatGPT 工作区，取得 tunnel ID 和隧道运行密钥。在已配对的本机账号安装官方 `tunnel-client`，让它能从 PATH 启动上表所选的命令。在当前终端设置 `CONTROL_PLANE_API_KEY`，个人学习凭证沿用本机配对。[Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels)
2. 使用下方命令创建个人学习配置、检查运行条件并保持客户端运行。访客用途可改用独立的配置名和隧道，将 stdio 命令设成 `book_mcp "书籍工作区绝对路径"`。在 ChatGPT 创建自定义 MCP 连接时选择 **Tunnel**，指定这条隧道并检查发现的工具。本机选择的资料会通过工具调用进入 ChatGPT 会话。[OpenAI 连接步骤](https://developers.openai.com/plugins/deploy/connect-chatgpt)

   ```powershell
   $tunnelId = Read-Host 'Platform tunnel ID'
   tunnel-client init --sample sample_mcp_stdio_local --profile understand-book-personal --tunnel-id $tunnelId --mcp-command "understand-book mcp"
   tunnel-client doctor --profile understand-book-personal --explain
   tunnel-client run --profile understand-book-personal
   ```

3. 在新会话中说“请在这本书里找‘知识图谱’，读回命中处的整段，指出依据”。个人连接可以接着说“把这个未解决的问题留给下次”，然后在 Reader 中查到同一任务。撤销个人凭证后再请求私人任务，应返回授权错误。停止隧道后，Web 调用需要等隧道客户端恢复。

访客读取指定书与个人笔记写入使用不同的连接配置。ChatGPT Web 的工作区授权、隧道连通性与真实客户端工具调用需要在目标账号逐项验证。
