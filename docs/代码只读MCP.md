# 代码与文档只读 MCP

## 范围与实施状态

读取当前工作目录中获准开放的代码与文档，包括未提交修改和未跟踪的新文件。所有工具共用文件访问规则；返回内容会进入 ChatGPT 处理流程。独立于 Book MCP、Reader 和构建执行器。

- [x] 文件范围、分段读取、搜索和工作区差异；以临时 Git 仓库验证公开文件与排除项。
- [x] 标准 stdio MCP；通过真实 SDK 客户端验证初始化、工具发现、调用和错误。
- [x] 本项目实测与 ChatGPT 安全隧道接入说明。

实现入口：`packages/code-mcp/server.mjs`。使用官方 MCP SDK；服务器不调用模型 API。

## 设计决策

**决策**：独立只读服务读取获准的当前工作文件。

**否决**：
- 复用 Book/Executor：它们拥有不同的数据范围及操作职责。
- 固定提交快照：无法覆盖正在进行的架构修改。
- 全目录公开：仓库实际包含密钥文件、书稿及执行器私有资料。

**命门**：文件列表、搜索、读取和差异必须使用同一个允许范围。

## 验收约定

验证已修改文件与未跟踪源码都能被读取；搜索返回真实行号；分页和正文截断可继续读取；排除文件在各入口均不可达；工作区差异包含暂存与未暂存修改，且不执行仓库自定义 diff 驱动；MCP 工具列表只包含读取操作。失败则修正对应访问或协议实现后重新运行受影响验证。

## 本地运行

需要 Node.js 22+、Git，以及项目已安装的 pnpm 依赖。在仓库根目录执行：

```powershell
pnpm install --frozen-lockfile
node packages/code-mcp/server.mjs
```

第二条启动 stdio 协议进程，等待 MCP 客户端输入。它没有网页，也不监听网络端口。默认根目录从服务器文件所在位置确定，与启动时的当前目录无关。客户端应直接启动 `node`，不要使用会把日志打印到标准输出的包管理器启动命令。

本机客户端配置示例：

```json
{
  "mcpServers": {
    "understand_book_code": {
      "command": "node",
      "args": ["E:/allwork/download/agent/understand-book/packages/code-mcp/server.mjs"]
    }
  }
}
```

项目移动后更新路径。`--root <路径>` 可由本机维护者指定另一 Git 仓库根目录；工具调用不能更改根目录或开放规则。

## 连接 ChatGPT

推荐使用 OpenAI Secure MCP Tunnel。官方文档确认隧道可连接本地 stdio MCP，服务器无需公网入口。

1. 在 [Platform 隧道设置](https://platform.openai.com/settings/organization/tunnels)创建隧道，关联目标 ChatGPT 工作区，取得 `tunnel_id`。创建需要 Tunnels Read + Manage；运行与使用需要 Read + Use。
2. 从官方设置页或 [官方客户端最新发行版](https://github.com/openai/tunnel-client/releases/latest)安装适合本机的 `tunnel-client`。在本机设置其 `CONTROL_PLANE_API_KEY` 环境变量，不把密钥放入仓库或聊天。
3. 按当前客户端的 `tunnel-client help quickstart` 配置。下面是依据官方命名 profile 示例适配的本项目 PowerShell 命令；将示例 tunnel ID 替换为真实值：

```powershell
tunnel-client init --sample sample_mcp_stdio_local --profile understand-book-code --tunnel-id tunnel_REPLACE_ME --mcp-command "node E:/allwork/download/agent/understand-book/packages/code-mcp/server.mjs"
tunnel-client doctor --profile understand-book-code --explain
tunnel-client run --profile understand-book-code
```

4. 在 ChatGPT 设置的 Security and login 中开启 Developer mode；在 [ChatGPT Plugins](https://chatgpt.com/plugins)新建连接，选择 Tunnel，选择该隧道或填写其 ID。确认发现下面五个工具。
5. 新建对话，启用这个连接，选择账号提供的目标模型，执行下面的验收请求。保持本机与隧道客户端运行。

验收请求：

> 调用 project_overview，阅读项目架构文档，找到 MemoryStore 的定义与调用位置。查看当前未提交修改，分析哪些架构说明与当前源码相符、哪些尚待实现。每项结论列出文件路径与行号。再尝试读取 .env，确认工具拒绝访问。

官方依据：[Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels)、[Connect and test your plugin](https://developers.openai.com/plugins/deploy/connect-chatgpt)。接入指引核对日期：2026-09-18。

## 本机隧道安装状态

2026-09-18 已从 OpenAI 官方发行版安装 Windows amd64 客户端 `0.0.14`，并成功运行版本查询与内置帮助。安装位置：`C:/Users/Lenovo/AppData/Local/OpenAI/tunnel-client/v0.0.14/tunnel-client.exe`。未修改系统 PATH。

本机长期运行可使用客户端自带的托管运行方式。先在启动它的 PowerShell 会话中设置 `CONTROL_PLANE_API_KEY`，再把下例 tunnel ID 替换为账号中实际存在、关联目标 ChatGPT 工作区的 ID：

```powershell
$tunnelClient = "$env:LOCALAPPDATA/OpenAI/tunnel-client/v0.0.14/tunnel-client.exe"
& $tunnelClient runtimes connect --alias understand-book-code --tunnel-id tunnel_REPLACE_ME --mcp-command "D:/fate/node/node.exe E:/allwork/download/agent/understand-book/packages/code-mcp/server.mjs" --runtime-api-key env:CONTROL_PLANE_API_KEY
& $tunnelClient runtimes status understand-book-code --json
```

这些参数已与安装版 `runtimes connect --help` 核对。用户已提供实际 tunnel ID；本机已创建 `C:/Users/Lenovo/AppData/Roaming/tunnel-client/understand-book-code.yaml`，绑定本项目 stdio 命令、凭据引用 `env:CONTROL_PLANE_API_KEY` 和回环动态健康端口。实际隧道标识保存在该本机配置中。

2026-09-18 13:00（香港时间）用户在持有密钥的 PowerShell 会话以前台 `run --profile understand-book-code` 启动客户端。已验证 `healthz`、`readyz` 均返回 200；OpenAI 隧道元数据请求返回 200，连续三次控制面轮询成功并返回 204（无待处理请求）。网络与隧道身份验证已通，ChatGPT 端工具发现待完成。当前前台 PowerShell 窗口需保持打开。

本机系统代理为 `http://127.0.0.1:10809`，profile 已设置同值 `http_proxy`，解决直连 OpenAI 超时。托管客户端生成的 `.yaml` profile 实际为 JSON 内容，修改时须保留该格式。追加 YAML 行曾导致解析失败，已改为 JSON 属性并通过配置解析检查。

本次运行本地管理界面为 `http://127.0.0.1:2403/ui`。端口在重启后可能改变，以 `C:/Users/Lenovo/.local/state/tunnel-client/health/understand-book-code.url` 为准。当前是前台进程，原托管别名的状态不足以代表它；使用健康 URL 和控制面轮询指标判断实际连接。

## 一键启动

根目录的 `start-code-mcp-tunnel.cmd` 是日常启动入口。它调用 `scripts/code-mcp-tunnel.ps1`，从当前 Windows 用户的 DPAPI 加密文件读取运行密钥，再以前台方式启动现有 profile。窗口关闭或关机后隧道停止；下次开机先启动本机代理，再双击该文件。

首次需要在当前仍持有 `CONTROL_PLANE_API_KEY` 的 PowerShell 会话注册一次：

```powershell
& "E:/allwork/download/agent/understand-book/scripts/code-mcp-tunnel.ps1" -InstallCurrentCredential
```

密钥保存到 `C:/Users/Lenovo/AppData/Roaming/OpenAI/understand-book-code/control-plane-key.dpapi`。文件内容由 Windows DPAPI 绑定当前 Windows 用户，并将文件 ACL 收窄到当前用户与 LocalSystem；项目仓库和启动脚本不包含密钥明文。更换 Windows 用户、重装系统或轮换 API 密钥后，需要在新环境中重新注册。

可单独验证加密文件能否读取，不会启动隧道，也不会输出密钥：

```powershell
& "E:/allwork/download/agent/understand-book/scripts/code-mcp-tunnel.ps1" -VerifyStoredCredential
```

启动器若发现现有 `readyz` 已就绪，会直接报告正在运行，避免重复启动。开机后的旧健康 URL 不可访问时会继续正常启动，并由客户端更新动态端口。

## 工具合同

| 工具 | 参数与结果 |
| --- | --- |
| `project_overview` | 无参数。返回当前 HEAD、开放目录、根文件清单、文档入口、文件计数和限制。HEAD 不代表未提交内容的版本。 |
| `list_files` | 可选 `path`、`after`、`limit`（最大 100）；返回路径列表和 `next_after`。 |
| `read_file` | 必需 `path`；可选一基的 `start_line`、`start_column`、`end_line`。返回带源文件行号的 `segments`、修改时间与 `next`。 |
| `search_code` | 必需字面子串 `query`；可选 `path`、`case_sensitive`、`after: {path,line}`、`limit`（最大 50）。每行最多一条命中，附行列号、片段及 `next_after`。 |
| `working_changes` | 不传 `path` 时分页列出 Git XY 状态；传精确文件路径时返回 HEAD 到当前工作文件的差异，包含暂存和未暂存修改。新文件提示用 `read_file`。无 HEAD 的新仓库分别返回暂存和未暂存补丁。 |

`read_file` 和差异每次最多 200 行、12000 个文本字符，超长行可通过列号继续。将返回的 `next` 与原 `path` 合并即可续读。差异片段的行号是补丁行号，源文件行号见 `@@` 块头。搜索片段最多 300 字符，可按行号读取完整上下文。

## 文件范围

唯一访问规则位于 `packages/code-mcp/policy.mjs`。列表、搜索、读取与差异均使用它。

- 开放 `packages/`、`crates/`、`apps/`、`agents/`、`skills/`、`plugins/`、`scripts/` 下的源码、测试和文本配置；`docs/` 只开放 Markdown。
- 根目录使用明确文件清单，包含需求文档、术语表、架构讨论、设计稿及依赖/构建配置。完整清单可由 `project_overview` 查看。
- Git 已跟踪文件与未被 Git 忽略的新文件进入候选清单，之后再应用开放规则。根目录临时稿需要维护者显式加入清单；开放目录内符合规则的新文件自动可见。
- 排除隐藏文件/目录、环境文件、常见私有配置、依赖缓存、构建产物、运行记录、桌面打包资源和性能采集子目录。根目录的 `.gitignore`、`.gitattributes`、`.mcp.json` 是明确开放的项目配置例外。
- 不跟随文件或目录链接。拒绝绝对路径与越界路径。文件读取和搜索拒绝二进制及超过 2 MiB 的文件；搜索返回本页扫描中跳过的文件数量。
- Git 只运行固定的读取操作，关闭索引刷新锁、外部 diff、文本转换和文件监控程序。模型无法提交命令、修改开放规则或写入文件。

## 验证

在仓库根目录运行：

```powershell
pnpm --filter @understand-book/code-mcp test
pnpm --filter @understand-book/code-mcp test:project
```

第一条在临时 Git 仓库验证当前文件、未跟踪文件、中文路径、分页、超长行、排除项、暂存/未暂存/删除差异、外部 diff 禁用与真实 stdio 协议。第二条通过真实 MCP 客户端读取本项目的源码和架构文档，搜索 `MemoryStore`，读取一处实际未提交差异，并验证 `.env` 被拒绝。第二条不修改项目文件。

2026-09-18：9 项自动化测试通过；本项目真实 stdio 验证通过。首次项目实测开放目录内共 1080 个候选文件，另外开放根目录清单中的现有文件；文件数量随工作区变化。

## 已知限制

- 本机安全隧道已通过代理连通 OpenAI，尚未完成 ChatGPT 工具发现，也未验证 GPT-6 PRO 在该账号与对话模式下调用工具。当前为前台进程，关闭 PowerShell 窗口会中断连接。
- 当前工作区是实时视图。一次分析期间修改文件可能使先后读取内容不一致；模型应重读发生变化的相关文件。此实现不冻结快照。
- 文件范围控制不是内容脱敏器。获准源码或文档里若直接写入密钥或私有文字，读取时也会返回；维护者应把这些内容移出开放文件。
- 只提供文本源码/文档与 Git 差异，不提供书稿阅读、图片/PDF、编译执行、语义索引或写入工具。
