# PN1–PN6、服务端 MiniLM 与首页入口合并发布

2026-10-30 11:50:34 HKT 发布到 https://understandbook.top，维护 10.57 秒。当前 release 为 `personal-notes-20261030`，服务 PID 506143，控制库 schema 22；MemoryDocument 使用 schema 5。公网资源与普通读者、管理员的登录、资料正文、历史及额度验证通过。临时数据副本、浏览器验收服务和构建交换文件已清理。

## 本次上线行为

- PN1–PN6：全局及就地记录、按账号保存设备草稿、稳定笔记身份与追加版本、版本比较和恢复、私人混合召回、教学采用引用、无材料讨论、个人联系、画板引用与综合稿。详细合同、界面和真实教学样例见[实施报告](../README.md)。
- MiniLM 成为在线服务随部署提供的能力。用户只需登录网页，不安装模型、不设置环境变量、不提供 embedding 密钥。服务器统一加载模型，各账号的笔记与派生索引继续隔离。
- 首页导航与底部注册区域的两处 Windows 下载链接均已移除，中英文同时生效。登录和注册入口保持可用。

## 模型供应和运行

模型为 `Xenova/paraphrase-multilingual-MiniLM-L12-v2`，固定 revision `2c4055b12046f11709e9df2c122e59ffbdc2f900`；Transformers.js 3.8.1，CPU q8、384 维。词法检索、向量检索与 RRF 融合继续使用原实现。

1. `scripts/linux/build-multi-reader.sh` 在依赖安装后运行 `scripts/prepare-local-embedding.mjs`。模型与推理运行时安装在 release 外的 `/opt/understand-book/models`，后续发布复用已经安装的版本。
2. 安装器固定下载模型 revision，执行真实中文 embedding，确认 384 维有效向量后才发布配置文件。运行时安装跳过不使用的 CUDA 下载；已存在的配置也要经过真实推理检查。
3. `scripts/linux/start-reader.sh` 默认使用 `/opt/understand-book/models/local-embedding.json`。配置缺失时阻止正式多人服务启动，并给运营方明确的准备命令；浏览器请求不承担下载和安装。
4. `multi_user_host → semantic_retrieval::preload → workspace-semantic-search warmup` 在服务启动时预热共享模型进程，避免让首次普通提问承担模型初始化。查询仍返回实际 ready/partial/unavailable；暂时不可用时保留词法检索与精确引用，不冒充语义命中。
5. 配置使用 `local_embedding_config.v1`，`runtime_dir`、`model_dir` 相对配置文件解析。`UNDERSTAND_BOOK_EMBEDDING_CONFIG` 可供运营方指定部署目录，属于服务配置，用户没有对应设置步骤。

本次服务器不能直接连接 Hugging Face，采用此前已做真实推理验证的同 revision 模型包供应服务器，再由正式安装器完成验证与配置。模型包保留在 `/opt/understand-book/acceptance/personal-notes-20261030/minilm-model.tar.gz`；安装、重复执行复用和实际服务用户的推理检查均通过。后续同机发布直接复用已安装文件；新服务器需要能连接模型源，或由运营方提供同版本模型包。

运营方在 release 目录复核时使用配置好的 Node：

```sh
"$UNDERSTAND_BOOK_NODE" --import tsx scripts/prepare-local-embedding.mjs \
  --config /opt/understand-book/models/local-embedding.json --check
```

## 线上源码合并

上一版为 `main-agent-planned-20261029`。它包含主工作树未收齐的空间笔记、可编辑思维导图和来源阅读窗口，本次以实际线上源码为基线合并 PN，保留这些已有功能。最终 128 个覆盖文件与编译目录逐字节一致，290 个前端构建文件与安装结果一致；原管理员页面随 release 保留。

旧空间笔记的画板类型继续使用 `Note / kind: note`。新个人想法使用 `PersonalNote / kind: personal_note`，避免把已有空间笔记解释成另一套数据。Agent 的 `board.author` catalog、source_read、prepare imports 也已加入实际个人笔记合同。资料导入仍要求真实活动材料，个人讨论不伪造材料。

实际发布源码在 `/opt/understand-book/releases/personal-notes-20261030`；覆盖包和文件清单在对应 acceptance 目录。主工作树根提交是 `bbf18a8c12bdf7b99ed402cc95195356f376d4e8`，含原有未提交工作；发布身份以实际 release 加覆盖包为准。后续发布继续从当前 release 合并，不用未收齐线上功能的根工作树直接覆盖现网。

## 验证证据

| 验证 | 结果与证据 |
| --- | --- |
| Linux 发布编译 | 六个正式二进制构建成功；真实服务用户模型检查通过：[build-completion.json](build-completion.json) |
| 部署脚本与工具合同 | 启动 7 项、构建 4 项通过；实际 board.author 注册表中的个人笔记查询、回读和导入验证通过 |
| 保留已有功能的合并 | 前端类型检查、54 项相关 Web 测试及生产构建通过；合并后的 memory/boards 82 项测试通过 |
| 模型实际供应 | 首次安装和重复复用均返回 ready、384 维：[embedding-install.json](embedding-install.json) |
| 实际数据副本升级 | 控制库 21→22；旧资料、聊天、图板、空间笔记可读，思维导图创建/修改/冲突/撤销/重做通过：[integration-qa.json](integration-qa.json) |
| 个人笔记与真实召回 | 幂等保存、追加版本、冲突、恢复、旧版读取、账号隔离、当前画板引用、无材料聊天、删除后退出召回均通过；真实 MiniLM 将“为什么看过了还是讲不出来？”对应的想法排在第 1 位 |
| 桌面/手机实际交互 | 1440/390px 上保存、修改、历史对照和服务器真实语义搜索通过；没有脚本错误：[candidate-personal-browser.json](candidate-personal-browser.json) |
| 上线数据保留 | 58 份聊天文件、100 份图板文件、1 个空间笔记本与 18 条额度记录保持：[deployment.json](deployment.json) |
| 公网实际资源 | 首页、阅读入口、管理员页面、9 个引用资源和画板预览资源与 release 一致；读者与管理员的原能力可用：[public-verification.json](public-verification.json) |
| 首页视觉与链接 | 中英、1440/390px 下载入口为 0，登录/注册存在且无横向溢出。候选四场记录见 [candidate-home.json](candidate-home.json)，公网截图见下方 |
| 临时资源清理 | 临时验收数据和服务清除，2 GiB 构建交换文件移除，swappiness 恢复 0，正式服务 active：[cleanup.json](cleanup.json) |

公网浏览器批量检查前三场通过；连续打开新浏览器上下文时，英文手机场景发生连接关闭。该场景随后单独在真实公网重跑通过，见 [published-home-en-390.json](published-home-en-390.json)。这次重跑没有修改产品或替换公网内容。

| 实际界面 | 截图 |
| --- | --- |
| 公网中文首页 | [桌面](published-home-zh-CN-1440.png) · [手机](published-home-zh-CN-390.png) |
| 公网英文首页 | [桌面](published-home-en-1440.png) · [手机](published-home-en-390.png) |
| 公网注册区域 | [中文桌面](published-begin-zh-CN-1440.png) · [英文手机](published-begin-en-390.png) |
| 合并版个人表达 | [桌面](candidate-personal-1440.png) · [手机](candidate-personal-390.png) |
| 合并版历史对照 | [桌面](candidate-history-1440.png) · [手机](candidate-history-390.png) |
| 合并版语义召回 | [桌面](candidate-recall-1440.png) · [手机](candidate-recall-390.png) |

本次发布检查没有调用收费对话模型；真实 MiniLM 为服务器 CPU 推理。此前 PN 的真实 DeepSeek 教学对照与无材料讨论证据保留在实施报告，未用发布验收替身替代这些结果。浏览器验收曾在保存响应返回、列表和版本记录尚在加载时过早截图；最终截图等待保存完成提示及列表就绪后重拍。

## 恢复点与已知限制

一致备份位于 `/opt/understand-book/backups/personal-notes-20261030`，包含维护停写后的可变数据和旧服务、Nginx、环境及隔离配置；旧 release 保留。维护期间、重新开放访问之前的失败可以恢复备份和旧程序。现在已经开放访问，不能直接把新数据回退到发布前快照，也不能让旧 schema 21 程序打开 schema 22 数据；后续问题优先修复当前版本，需恢复时必须先保留上线后的新增数据并制定对应恢复方式。

- 原有图板中有 8 个来源缺少出版版本，旧程序也无法打开；本次保留原状态，未虚构补齐来源。
- 设备草稿仍只保存在当前设备；保存后的笔记由服务端持有。
- 语义召回不保证找到全部相关想法。历史版本通过确切引用读取，当前向量索引面向当前版和明确关联的后续表达。
- 当前性能数字沿用实施报告中的实测环境；服务器单个验收问题证明真实模型与返回链路可用，不作为多用户并发或真人学习收益的结论。
