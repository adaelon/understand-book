# Understand Book · Agent 使用与配置指南

> 本文件面向协助用户安装、配置和使用 **Understand Book** 的 AI Agent（如 Claude Code、Codex、OpenCode、Antigravity、Cursor、ChatGPT 等），也可供用户直接阅读或把本文链接交给自己的 Agent 执行。

---

## 1. Understand Book 是什么

**Understand Book** 是一款面向深度阅读与主动学习的开源应用（Rust + TypeScript + Vue 3 + Tauri 2 + MCP）。核心思路是**双时序架构**：

1. **构建时（离线准备）**：把教材、论文、Markdown、EPUB 或音乐/视频整理成可稳定定位的知识底座，包含：
   - 稳定原文与层级段落定位（`source.txt` + `base.json` 的 LID 节点）
   - 概念与断言关系图（Pass1 节点与有向边）
   - 篇章功能索引（`discourse_index.json`）、公式语义（`formula_semantics.json`）、全书结构主线（`book_structure.json`）与内嵌插图（`assets/images/`）
2. **阅读时（按需取证 + 主动学习）**：
   - 不把整本书硬塞进上下文，而是让阅读 Agent 通过检索/取文工具按需定位并回读真实原文，回答附带可跳回原段的来源引用。
   - 围绕阅读讲解提供三种结构化学习动作：**「我来解释一下」**（双局部图两阶段对照）、**「练到会」**（闭卷对话独立验收）、**「给我一道应用题」**（新情境迁移练习卡）。
   - 笔记、高亮、草稿、历史对话和学习证据独立持久化在本机私人存储中。

### 本地版安装包与目录约定（Windows）

- **最新 Windows 安装包（v0.3.18）**：[UnderstandBookLocalSetup.exe](https://github.com/adaelon/understand-book/releases/download/v0.3.18/UnderstandBookLocalSetup.exe)（[Release 页](https://github.com/adaelon/understand-book/releases/tag/v0.3.18)，约 238.2 MB，未签名）。
  - 安装向导提供**安装目录选择页**，可直接安装到 D/E 盘，不必装在 C 盘。
  - 随包自带本地 Node、PDF 读取/OCR 与离线语音 Python 运行环境（不含 461 MB Whisper 权重，模型改为应用内按需下载）。
- **独立应用身份**：`app.understandbook.local`（与云客户端并存，升级/卸载均保留用户资料）。
- **默认数据目录（Windows）**：`%LOCALAPPDATA%\app.understandbook.local`
  - `settings.json`：权威配置文件，保存 `library_root`（书库根目录）、`model_settings`（模型服务与绑定）、`voice_model_dir`（可选离线语音模型目录）。
  - `data-location.json`：若迁移/导入过旧数据，此文件记录实际生效的数据目录路径（读取配置前应先检查它）。
  - `.env`：可选环境变量覆盖（如自定义语音路径、`FFMPEG` 等）。
  - `library/`：默认自有书库父目录；`memory/` 与 `private/`：私人阅读记录、笔记与产物。
  - 可通过环境变量 `UNDERSTAND_BOOK_LOCAL_DATA_DIR`（绝对路径）指定独立数据目录。

---

## 2. 如何把 GitHub 上发布的学习材料（书）放进书库里

Understand Book 在 GitHub Releases 提供已预构建好的现成材料包，解压后即可直接阅读与提问，无需重新预构建。

### 当前官方材料包

| 材料 | 下载直链 | 解压后的书目录 |
| --- | --- | --- |
| 复旦统计（校准版，18 节 + 153 图） | [fudan-statistics.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-26/fudan-statistics.zip) | `fudan-statistics/prebuilt` |
| 最优化方法 I（讲义 + 习题 + 历年试题） | [optimization-review.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/optimization-review.zip) | `optimization-review/prebuilt` |
| 深入理解 AI Infra（第 1–12 章 + 482 图） | [ai-infra-book.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/ai-infra-book.zip) | `ai-infra-book/prebuilt` |
| Agent Memory 综述论文（含原 PDF） | [agent-memory-paper.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/agent-memory-paper.zip) | `agent-memory-paper/prebuilt` |
| 贝多芬第九交响曲（四乐章音频 + 乐谱） | [beethoven-symphony-9.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/beethoven-symphony-9.zip) | `beethoven-symphony-9/prebuilt/m1`–`m4` |

完整列表与来源说明见 [materials/README.md](../materials/README.md)。

### 放进书库的精确规则（避免放错层级）

本地书库通过扫描 `library_root` 下的一级子目录来发现书籍；**可被识别的书籍目录根下必须直接包含 `base.json` 和 `source.txt`**。

因此不要把整个 ZIP 解压后的外层文件夹（里面套着 `original/` 和 `prebuilt/`）直接当成一本书，而是把其中的 **`prebuilt/` 目录复制到 `library_root/<书名目录>`** 下：

1. **找到当前生效的 `library_root`**：
   - 先看 `%LOCALAPPDATA%\app.understandbook.local\data-location.json`（若存在则取其中的路径作为 `<data_dir>`，否则 `<data_dir> = %LOCALAPPDATA%\app.understandbook.local`）。
   - 读取 `<data_dir>\settings.json` 中的 `library_root` 字段（若尚未首次启动，默认是 `<data_dir>\library\.understand-book`）。
2. **下载、解压并将 `prebuilt/` 放入 `library_root`**（以《复旦统计（校准版）》为例的 PowerShell 脚本）：

```powershell
# 1. 解析当前本地版的数据目录与书库根目录
$homeDir = if ($env:UNDERSTAND_BOOK_LOCAL_DATA_DIR) { $env:UNDERSTAND_BOOK_LOCAL_DATA_DIR } else { Join-Path $env:LOCALAPPDATA "app.understandbook.local" }
$locFile = Join-Path $homeDir "data-location.json"
$dataDir = if (Test-Path $locFile) { Get-Content $locFile -Raw | ConvertFrom-Json } else { $homeDir }
$settingsFile = Join-Path $dataDir "settings.json"
$libraryRoot = if (Test-Path $settingsFile) { (Get-Content $settingsFile -Raw | ConvertFrom-Json).library_root } else { Join-Path $dataDir "library\.understand-book" }

# 2. 下载并解压 ZIP
$tmp = Join-Path $env:TEMP "ub-book-import"
New-Item -ItemType Directory -Force -Path $tmp, $libraryRoot | Out-Null
$zip = Join-Path $tmp "fudan-statistics.zip"
curl.exe -fSL -o $zip "https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-26/fudan-statistics.zip"
Expand-Archive -Path $zip -DestinationPath $tmp -Force

# 3. 将 prebuilt 目录复制到 library_root 下（保留清晰目录名）
$dest = Join-Path $libraryRoot "fudan-stat-calibrated"
if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
Copy-Item -Path (Join-Path $tmp "fudan-statistics\prebuilt") -Destination $dest -Recurse

# 4. 校验关键文件存在
Test-Path (Join-Path $dest "base.json"), (Join-Path $dest "source.txt")
```

3. **在应用中打开**：回到 Understand Book 首页点击刷新（或重启应用），书库中即会出现对应书卡；也可以在 **学习资料** 中直接选择解压出来的 `prebuilt` 目录，或使用 CLI `understand-book serve "<解压路径>\fudan-statistics\prebuilt"` 启动。
4. **导入用户自己的资料**：在顶部 **学习资料** 导入 PDF，或使用个人连接 CLI `understand-book paper import "<PDF 路径>"`（桌面安装版已内置 Node 与中英文 OCR，无需系统预装 Node/Python）。

---

## 3. 如何配置模型 API

阅读问答、「我来解释一下」、「练到会」、应用题和后台任务共用同一套模型配置（详细协议说明见 [模型配置使用说明](模型配置使用说明.md)）。

### 方式一：在界面中配置（推荐）

打开首页右上角 **桌面设置**（或阅读器顶部 **更多阅读工具 → 打开桌面设置**），默认停留在 **AI 模型**：

1. **选择服务商或添加服务**：内置 OpenAI、DeepSeek、Anthropic、Google Gemini、Ollama 模板；使用中转站、自建网关或 Magpie 时选择 **自定义服务**。
2. **填写地址与密钥**：填入 Base URL 与 API Key（本机 Ollama 默认免密）。
3. **获取或填写模型 ID**：点 **从服务获取型号** 选择模型，或手填模型 ID。
4. **核对关键能力（重要）**：
   - 必须为支持工具调用的模型开启 **原生工具调用**（`capabilities.native_tools: true`），否则阅读 Agent 无法调用取文和检索工具。
   - **推理模型务必留足输出上限**：例如 `output_token_limit` 建议设 `16384`（实测 `4096` 在教学/三图场景容易截断）。
5. **绑定用途与验证**：设为默认模型（可按需为主 Agent、教学反馈、后台任务分别指定不同模型），点 **保存并应用**，再点 **测试文本生成** 验证连通。

### 支持的 API 协议

| `api_protocol` | 典型端点 | 说明 |
| --- | --- | --- |
| `openai_chat_completions` | `https://api.openai.com/v1` | 最通用的 OpenAI Chat 兼容协议（DeepSeek、Magpie、常见中转站） |
| `openai_responses` | `https://api.openai.com/v1` | OpenAI Responses 协议 |
| `anthropic_messages` | `https://api.anthropic.com` | Anthropic Messages 协议 |
| `gemini_generate_content` / `gemini_interactions` | `https://generativelanguage.googleapis.com` | Google Gemini 原生协议 |
| `ollama_chat` | `http://127.0.0.1:11434` | 本机 Ollama `/api/chat`，免密 |
| `deepseek_messages` / `cohere_v2_chat` / `bedrock_converse` | 对应官方端点 | 各自官方协议 |

### 方式二：文件配置（供 Agent 协助配置或自动化测试）

- 桌面版权威存储在 `<data_dir>\settings.json` 的 `model_settings` 字段。**Agent 修改 `settings.json` 时必须先读取原文件并保留 `schema`、`library_root`、`voice_model_dir` 等其他字段，且严禁将真实 API Key 写入仓库、日志或聊天公开记录。**
- 独立启动或自动化脚本也可通过环境变量 `UNDERSTAND_BOOK_MODEL_SETTINGS=<json 文件路径>` 与 `UNDERSTAND_BOOK_MODEL_CONFIG_ID=<模型配置 ID>` 指定。

---

## 4. 如何搭配 GitHub 上的 Magpie（`yetone/magpie`）

[Magpie](https://github.com/yetone/magpie) 是开源的本机统一 AI 网关与多 Agent 模型切换器。它会在本机启动轻量代理（默认地址 **`http://127.0.0.1:3425`**），可以把你已有的订阅或多家 API（如 Codex / ChatGPT Plus、Claude、Antigravity `agy-cli`、DeepSeek、Kimi、GLM 等）统一转换为标准 OpenAI / Anthropic / Gemini 接口，并支持路由组和配额故障自动切换。

### 将 Magpie 接入 Understand Book 的步骤

1. **安装并启动 Magpie**：按 [`yetone/magpie`](https://github.com/yetone/magpie) 说明启动菜单栏应用或终端网关（`magpie tui`），确认本机网关正在 `http://127.0.0.1:3425` 运行，并在 Magpie 中启用想要路由的模型或路由组。
2. **在 Understand Book 中添加自定义服务**：
   - 打开 **桌面设置 → AI 模型 → 添加服务**，选择 **自定义服务**。
   - **服务地址（Base URL）**：`http://127.0.0.1:3425/v1`
   - **API Key**：`magpie`（如在 Magpie 中修改过网关密钥，请填对应密钥）
   - **协议**：`OpenAI Chat Completions`（`openai_chat_completions`）
3. **选择模型并开启工具调用**：
   - 点击 **从服务获取型号**（自动拉取 `GET http://127.0.0.1:3425/v1/models` 返回的模型清单，例如 `codex/...`、`agy-cli/...` 或自定义路由组名），或直接手填模型 ID。
   - 确认开启 **原生工具调用**（`native_tools: true`），推理模型将输出上限设为 `16384`。
   - 点击 **保存并应用**，再用 **测试文本生成** 确认 Magpie 转发成功。

### 对应的 `settings.json` 中 `model_settings` 片段参考

```json
{
  "revision": 1,
  "services": [
    {
      "id": "magpie-local",
      "name": "Magpie 本机网关",
      "base_url": "http://127.0.0.1:3425/v1",
      "auth": "bearer",
      "api_key": "magpie",
      "headers": {},
      "deployment": { "kind": "direct" }
    }
  ],
  "models": [
    {
      "id": "magpie-default",
      "name": "Magpie 默认模型",
      "service_id": "magpie-local",
      "model_id": "<替换为 Magpie 返回的实际模型 ID>",
      "api_protocol": "openai_chat_completions",
      "mode": "native",
      "output_token_limit": 16384,
      "capabilities": { "native_tools": true, "streaming": true }
    }
  ],
  "bindings": {
    "default_model": "magpie-default",
    "agent": null,
    "teaching": null,
    "background": null
  }
}
```

> **搭配提示**：如果在 Understand Book 中同时使用 **「我的 Agent」**（ACP 接入 Codex / OpenCode / Claude Code），Magpie 同样可以统一管理这些外部 Agent 底层所使用的模型与配额回退，形成“Understand Book 负责书库、取证和教学编排 + Magpie 负责本机多模型统一路由”的组合。

---

## 5. 如何配置离线语音模型

自 `v0.3.18` 起，Windows 本地版**随包内置独立 Python 语音运行环境**（`voice-runtime/python`），而 **461 MB 的中英文 Whisper 模型（`Systran/faster-whisper-small`）改为按需下载**，以大幅减小安装包体积并允许存放到其他盘符。

### 方式一：界面按需下载（推荐）

1. 打开 **桌面设置 → 书库与工具 → 离线语音识别模型**（或在问答框点击麦克风图标，看到未下载提示后点 **去下载语音模型**）。
2. **选择存放目录**：默认在 `<data_dir>\voice-model`；若 C 盘空间紧张，点 **更改目录** 选择 D/E 盘目录（如 `D:\Models\understand-book-voice`）。
3. **选择下载源**：国内网络推荐切换为 **HF-Mirror（国内镜像）**（`https://hf-mirror.com`），海外或代理环境使用 **HuggingFace（官方源）**。
4. 点 **下载模型（461 MB）**，界面实时显示下载进度，完成后自动校验 `model.bin` 大小（`483,546,902` 字节）与 SHA256（`3e305921506d8872816023e4c273e75d2419fb89b24da97b4fe7bce14170d671`）。
5. 校验通过后**无需重启立即生效**，后续录音转写完全离线运行，不需要联网或配置云端语音 API；不需要时也可在同页点 **删除模型** 释放空间。

### 方式二：复用本机已有模型

如果本机已经有 `faster-whisper-small` 模型目录（需包含 `model.bin`、`config.json`、`tokenizer.json`），直接在 **离线语音识别模型** 中点 **使用已有模型** 选中该目录即可立即启用，无需重复下载。

### 方式三：HTTP API / 环境变量（供 Agent 自动化配置）

当本地服务运行在 `http://127.0.0.1:<port>` 时，提供以下本地接口：

- `GET /api/reader/voice-status`：返回 `{ available, max_seconds, runtime_available, can_download }`。
- `GET /api/reader/voice-model`：返回运行环境、当前模型目录、来源（`env | configured | default | bundled | none`）及实时下载进度。
- `POST /api/reader/voice-model/download`：参数 `{ "target_dir": "D:\\voice-model", "source": "hf-mirror" }`（或自定义 `"base_url"`）。
- `POST /api/reader/voice-model/select`：参数 `{ "dir": "D:\\existing-whisper-small" }`。
- `POST /api/reader/voice-model/cancel`：取消下载并清理临时目录。
- `POST /api/reader/voice-model/remove`：参数 `{ "delete_files": true }`（仅删除已知 Whisper 模型文件，保留目录内其他用户文件）。

**优先级顺序**：`<data_dir>\.env` 中显式配置的 `UNDERSTAND_BOOK_VOICE_MODEL_DIR`（与 `UNDERSTAND_BOOK_VOICE_PYTHON`）> `settings.json` 中的 `voice_model_dir` > 默认 `<data_dir>\voice-model` > 随包 `voice-runtime\model`。macOS 目前通过 `<data_dir>\.env` 配置这两个环境变量。

---

## 6. 如何使用这个软件（界面与外部 Agent 协作）

### 界面主路径

顶部四个固定入口覆盖日常学习流程：
- **学习**：本机书库首页。点 **查看这本书** 可选 **阅读原文**，有课程的书还可选 **跟着课程学**。
- **我的理解**：查看某本书已沉淀的概念理解、纠偏记录与进展。
- **学习资料**：打开已有预构建目录或导入本机 PDF。
- **学习画板**：按资料创建或重新打开自由画板整理结构。

阅读界面中：
- 划选任意段落可直接围绕选区提问、高亮或记笔记；
- 点击回答里的引用编号可直接跳回对应原文段落；
- 可点击问答框的 **语音输入** 录音转文字并核对后插入输入框；
- 可按需开启 **演示页**，让回答附带可交互的可视化讲解页；
- 可从回答的任意原句另开 **追问对话** 分支，不污染主线。

### 让外部 Agent 与 Understand Book 协作的三种方式

1. **应用内「我的 Agent」（ACP）**：在 **桌面设置 → 书库与工具 → 我的 Agent** 连接 OpenCode、Codex 或自定义 ACP 命令，测试连接后在问答框旁切换该 Agent 读当前书（与内置模型共享取证、「我来解释一下」和闭卷编排）。
2. **访客只读 MCP（让外部 Agent 查指定一本书）**：
   - stdio 命令：`<安装目录>\bin\book_mcp.exe "<书籍 prebuilt 绝对路径>"`
   - 只读该书检索、原文、结构与元数据，不接触用户私人笔记。
3. **个人学习 CLI & MCP（跨工具接续任务与笔记）**：
   - 在 **桌面设置 → 书库与工具 → 个人学习连接** 勾选授权书籍和权限（读 / 写笔记高亮 / 导航 / 导入管理），复制凭证。
   - 配对终端（避免把凭证写进命令行参数）：
     ```powershell
     Get-Clipboard -Raw | understand-book connect --token-stdin
     understand-book material list --json
     understand-book book search <book_id> "关键词"
     understand-book book text <book_id> <lid>
     ```
   - 接入支持 stdio MCP 的客户端（含 Claude Code / Codex / ChatGPT Tunnel）：命令设为 `understand-book mcp`。详见 [个人学习 CLI 与 MCP](个人学习CLI与MCP.md)。

---

## 7. 「我来解释一下」「练到会」「给我一道应用题」分别是什么、会发生什么与底层原理

在普通文本书阅读中，每当 AI 完成一条有原文依据的讲解后，问答区下方会出现这三个独立的学习动作（课内讲解同样对应这三种机制）：

| 动作 | 定位 | 资料与历史是否可见 | 点击后会发生什么 | 记录与边界 |
| --- | --- | --- | --- | --- |
| **我来解释一下** | 讲后开放式复述与推理对照（ADR-0192） | **可见**（开放式） | 绑定当前讲解，先让你用自己的话讲；提交后生成**双局部有向图 + 焦点对照**，指出一致处与最关键分歧 | 属于私人讲后反馈，不是考试评分；看过三图后若再开「练到会」，闭卷会感知该帮助并要求新变式验证 |
| **练到会** | 显式闭卷对话直到独立掌握（[ADR-0183](adr/0183-closed-book-dialogue-to-independent-performance.md)） | **隐藏**（收起原书、旧讲解、笔记与三图） | 切入闭卷界面，AI 围绕刚讲的窄目标出一道自洽题并持续追问/补讲，直到你在无本次提示的新变式上独立通过 | 严格区分“受助通过”与“独立完成”；可随时暂停/退出，恢复回到原现场 |
| **给我一道应用题** | 情境迁移单选练习卡（[ADR-0178](adr/0178-in-context-retrieval-practice-and-learning-map.md)） | **可见**（不收起原书） | 现场构造一道全新情境的 2–6 选 1 应用题卡，支持逐级提示、提交核对与回看原文出处 | 检验能否把书中的关系迁移到新细节中做判断，不进入多轮闭卷锁定 |

### 深入原理一：「我来解释一下」为什么不会“替学生脑补”？（两阶段隔离与双局部图）

普通聊天里，AI 已经看过标准答案，听学生说一句含糊的结论时很容易自动脑补学生没讲出的前提，然后夸一句“你理解得很对”。Understand Book 在后端用**公共 Reader 统一编排的两阶段机制**从架构上阻断这种假通过（内置模型和本机 ACP 均强制走同一编排）：

1. **非解释分流闸门**：点击「我来解释一下」后，如果用户发的其实是纯求助或新提问（如“这个公式里的 $n$ 是什么，能再讲一遍吗？”），系统直接分流回普通问答，不会硬画空图，也不误记理解证据。
2. **阶段 1：盲抽学生表达（完全隔离）**：
   - Reader 启动一次**无对话历史、不传原讲解正文、不传参考答案、不挂载任何查书工具**的干净调用，只传入“原问题 + 极简主题词表 + 学生本轮原话”。
   - 模型只能把学生**实际说出来的**命题和推理方向画成右侧的**「本次表达局部图」**（`student_graph`）。如果学生只抛了结论没说理由，图上就只出现零度孤立结论节点，绝不替学生倒推前提。
3. **阶段 2：书侧局部取证与关系对照**：
   - Reader 的 `read-tools` 围绕原回答锚点 LID 执行**确定性局部取证投影**：按同一书源版本接合 Pass1 有向边（保留精确方向与主张）、Discourse 篇章功能（解释/支持/反驳/问答）与 BookStructure 章节主线；若某本书缺少后两者，自动软降级到原回答已验证的出处 LID 与局部原文。
   - 模型结合书侧证据和阶段 1 已冻结的学生图，生成左侧 2–4 条核心关系的**「书中推理局部图」**（`book_graph`）、焦点对齐组（`rows`）、默认折叠的旁路思路（`side_notes`，不把延伸内容算成漏答）以及**最多 1 处最关键的追问**。
4. **确定性核验与整卡原子呈现**：
   - 渲染前，Reader 校验节点标签紧凑度、焦点组不重复归属，以及书侧每条引文是否真属于本轮已读 UTF-16 原文区间；若格式或引文不通过，私下附诊断重写最多 1 次。
   - 界面等待时显示真实两步进度（“正在整理你的表达 → 正在核对书中原文与对照”），全部通过后**整卡一次性展开**。宽容器左右并排两张局部图，窄栏上下叠放；点击任意节点、连线或顶部 `① ② ③` 焦点标签，双侧对应子图同步高亮，底部面板并列展示成立条件、判断状态和双方原句（书侧可直跳原文 LID）。
   - 7 类对照状态：`一致`、`有分歧`、`本题核心未提`、`另一种成立解释`、`据本段待核对`、`书中依据不足`、`原因已澄清`。
5. **两条诚实的纠正路径与帮助回执**：
   - **「你把我听反了」**（`dispute_interpretation`）：只围绕**首次原话**重核解读，不把这次纠正算作“看图后学会的受助表达”。
   - **「改一下我的想法」**（`revise_understanding`）或在主输入框回复追问：记录为看过三图后的新一轮表达，并绑定已实际渲染的显示回执（`/tutor/display`）。
   - 若后续对同一目标再点 **「练到会」**，闭卷引擎会自动感知该三图已经揭示过书中推理，要求换用新变式检验真正独立掌握。

### 深入原理二：「练到会」点击后会发生什么？

1. **进入隔离闭卷现场**：隐藏原文、旧讲解、笔记、实验与历史对照，界面只保留当前闭卷目标、本轮对话和作答框。
2. **自洽首题与状态机约束**：服务端私有 `closed_book` 状态对象锁定当前目标（`target`）、判据（`criteria`）、尝试次数与已给帮助；模型提出的每道题必须把必需条件写全，不得要求用户翻看其他面板。
3. **帮助归属与独立变式验收**：只要本次尝试里学习者要过提示（如点「给一点提示」）或 AI 做过补讲，服务端就会把帮助标记记在当前尝试上——这一轮即便答对也不能直接标为“独立掌握”，系统会换一道同目标、未代答的新变式，直到学习者在无提示条件下独立完成。
4. **防外部 Agent 代答**：当外部 Agent 通过个人 MCP 接入时，活动闭卷期间无法偷看被隐藏的历史解答，也不能替用户代报“我已独立完成”。

### 深入原理三：「给我一道应用题」点击后会发生什么？

1. **围绕原出处出新情境题**：服务端依据当前回答锚定的原文出处，要求模型先在内部自检并构造一道完整的新案例（`situation` + `question` + 2–6 个有区分度的 `options`），要求正确答案必须依赖该新案例的某个具体细节，不能仅靠照抄上一段的口诀或关键词猜出。
2. **题面与答案/提示隔离**：正确答案（`correct_option`）、解析（`solution`）与提示（`hint`）保留在服务端私有教学结构中，前端卡片只呈现情境、问题和选项。
3. **分层提示与核对**：卡片支持按需索取分级提示（从仅提醒注意题中已知条件，逐步到指向自己的笔记、原书段落与分步示范，最多 5 级）；提交选项后展示解析并可跳回原书对应段落。

---

## 8. Agent 协助用户时的行动清单与安全边界

| 用户需求 | Agent 应执行的操作 | 注意事项 |
| --- | --- | --- |
| 安装 Windows 本地版 | 引导下载 `v0.3.18` 的 `UnderstandBookLocalSetup.exe`，提醒在目录页可选 D/E 盘 | 切勿删除用户既有 `%LOCALAPPDATA%\app.understandbook.local` 数据 |
| 把 GitHub 上的书放进书库 | 读取 `settings.json` 找 `library_root` → 下载对应 Release ZIP → 将 `prebuilt/` 复制到 `library_root/<书名>` → 校验 `base.json` 和 `source.txt` | 不要把外层包目录（含 `original/`）误当作书根；不要把 `.build/` 打进分享包 |
| 配置模型 API / 搭配 Magpie | 确认服务商或 Magpie（`http://127.0.0.1:3425/v1`）可用 → 在界面或安全合并写入 `settings.json` 的 `model_settings` → 开启 `native_tools: true`、推理模型输出上限留足 `16384` | 保留 `settings.json` 其他字段；绝不泄露或提交用户 API Key |
| 启用离线语音 | 优先推荐界面「离线语音识别模型」选 D/E 盘目录 + `HF-Mirror` 下载；或调 `/api/reader/voice-model/download` / `/select` | `v0.3.18` 已内置 Python 运行环境，无需用户自己装 Python 或跑 `pip` |
| 外部 Agent 读用户的书 | 只查指定书用 `book_mcp "<书籍目录>"`；跨会话记笔记/留任务用 `understand-book mcp` + 个人连接配对 | 严格遵守授权范围；切勿在闭卷「练到会」期间替用户作答或伪造独立完成 |
