# 在本机 Reader 导入论文和教学 Skill

在本机 Reader 打开一本书，从问答区的 Agent 选择框打开“管理我的 Agent”。“工作区教学 Skill”使用当前学习工作区。单独打开书时，Skill 随该书保存。

## 从 Agentero 选取论文

打开准备使用的本机 Vault，启动 Agentero 提供的 MCP 服务。下面的命令固定了 Vault 和监听地址，桌面应用中已经运行同一服务时可以直接查询。

```sh
agentero --vault "<Vault 绝对路径>" mcp serve --listen 127.0.0.1:8765
```

在“从 Agentero 或本机导入论文”中查询并选择一篇。Reader 保存 Agentero 返回的题名、作者、标识、Vault 和条目路径，并把找到的 PDF 交给材料工作台。导入结果给出材料 ID 和当前准备状态。点击“打开已导入的材料”可以进入工作台查看进度，准备好后可在 Reader 中逐段阅读和划线。也可以输入本机 PDF 的绝对路径直接导入。服务使用其他端口时，启动 Reader 前将 `UNDERSTAND_BOOK_AGENTERO_MCP` 设为完整的本机 MCP 地址。

只有目录元数据的论文会显示“已保存元数据，待补原文”。可立即在同一表单补充 PDF，也可稍后在“已有材料 ID”填写同一引用，或从 CLI 补充。原论文的目录元数据随材料保留。

```sh
understand-book paper import "<PDF 绝对路径>" --json
understand-book paper metadata <材料ID> --json
```

首次使用 CLI 时，在本机 Reader 中启用个人学习连接、勾选当前材料，使用 `understand-book connect --token-stdin` 配对。导入和安装需要启用写入与管理。为其他客户端读取新书，请在连接设置中勾选它。CLI 和个人 MCP 另提供以下调用，其中 `paper` 是目录查询返回的路径。

| 工具 | 参数示例 |
| --- | --- |
| `agentero_papers` | `{"query":"memory"}` |
| `agentero_import` | `{"paper":"papers/example"}` 或 `{"identifier":"arXiv:1706.03762"}` |
| `paper_import` | `{"book_id":"<保存的材料ID>","path":"C:/papers/example.pdf"}` |
| `paper_metadata` | `{"book_id":"<材料ID>"}` |

命令行可用 `understand-book call <工具名> @args.json` 传入上表的参数。元数据中的 `source` 来自 Agentero，`projection` 来自 Reader 当前可用的论文证据。

## 用教学 Skill 带读

在“工作区教学 Skill”输入含有 `SKILL.md` 的目录绝对路径，或公共 GitHub 仓库中该目录的页面链接，点击“导入 Skill”。Reader 将目录和随附资源复制到当前范围，显示名称和说明。点击“启用”，再向选定的 Agent 提问。启用的指令和资源目录会随新回合传入。支持工作区 Skill 发现的 Agent 也可以从其工作目录下的 `.agents/skills` 读取。

可以先导入本仓库的实证论文带读示例，打开一篇有方法和结果的论文，选中描述结论的一段，问“研究者比较了什么？请带我核对差异”。示例要求 Agent 回读两处证据并等待学习者解释。停用后再开始新回合，该 Skill 的指令会从传入上下文移除。移除操作删除当前范围安装的副本。[实证论文带读示例](examples/read-empirical-paper/SKILL.md)

CLI 和个人 MCP 提供 `skill_list`、`skill_import`、`skill_enable`、`skill_remove`。安装参数为 `source`，仓库根链接还可以提供相对目录 `subdir`。启停参数为 `name` 和 `enabled`，移除只需要 `name`。
