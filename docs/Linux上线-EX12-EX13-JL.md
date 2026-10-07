# Linux EX12 / EX13 / JL 发布

## 范围与进度

用户已确认上线 EX12、EX13、JL、配套网络前端与选定演示 revision 2。沿用现有多人服务、账号、书库及 Provider 配置。JL 从新聊天启用，旧 JSON 聊天文件原样保留。BSR 在途生成产物不纳入本次内容发布。

当前工作树还包含“排查演示页与来源阅读体验”聊天完成的 UI 修复，并已一并上线：50%–150% 演示缩放、紧凑工具栏、按需显示说明、来源短标签及编号、保留原文结构的格式化来源正文。部署后比对 16 个相关运行文件和公网实际返回的主脚本，确认均属于本次 release，见 [范围核对回执](performance/linux-ex13-jl-20261002/presentation-ui-inclusion.json)。

- [x] 只读确认现网：`chat-recovery-20261002`，服务 active，三个账号。
- [x] 冻结当前运行源码与资源，在新 release 构建五个 Linux 程序和网络 Web。
- [x] 隔离验证 EX12 / EX13 与 JL 的运行、回顾、重启和备份恢复；接入选定演示。
- [x] 停写备份、切换服务和网络前端，2026-10-02 23:38:52 HKT 启动新版。
- [x] 公网验证 HTTPS、登录、9 本书、阅读刷新、PDF 和渲染能力；核对正式数据中的选定聊天及版本。

## 已确认环境

- 服务：`understand-book-multi.service`，入口 `https://115.190.121.150`，后端 `127.0.0.1:8788`。
- 旧 release：`/opt/understand-book/releases/chat-recovery-20261002`。
- 数据根：`/opt/understand-book/data/multi-reader`，约 257 MB。
- 账号：`adaelon`、`puff`、`reader`。
- 发布前磁盘可用约 3.4 GB、内存总量约 3.7 GB；构建并发为 1。
- 本地源码以工作树快照为准，Git 基线 `5e10516`；EX12 / EX13 / JL 包含未提交文件。

## 验证目的

- Linux 编译及网络前端类型检查发现跨平台依赖、缺失资源和接口不匹配；失败则修复候选，现网保持运行。
- JL 验证发现日志重开失败、成果/来源断链、跨读者读取及备份漏项；失败则修候选后再切换。
- EX 验证发现部署仍用旧指导、演示无法打开、现场重开丢失及修订绑定错误；失败则修安装、数据接入或对应路径。
- 切换前检查活动及未保存运行；存在则等待或沿已有恢复流程处理，不丢弃运行。

## 发布结果

正式 release 为 `/opt/understand-book/releases/ex13-jl-20261002`，后端、网络前端和隔离渲染 worker 均指向该目录。切换前活动／未保存运行数为 0；旧服务停止后完成原生数据备份再启动新版。公网入口仍为 [Linux Reader](https://115.190.121.150)。原账号、书库、Provider 配置沿用。

- 网络前端构建成功；定向前端测试 64 项通过。快照复用本机依赖时需在测试宿主允许真实依赖目录，产品配置未改。
- Linux JL 定向测试 34 通过、1 忽略；会话日志专项 8 通过；EX12 3 通过、5 忽略；EX13 Server 5 通过、2 忽略。EX12 首次缺四个实验输入文件，补齐后通过。
- Runtime presentation 定向测试 83 通过。缺失的存档响应与量化书籍测试数据补齐后通过。
- 隔离多人服务中 revision 2 可读，27 个来源均可定位；保存/恢复 B=8、切聊天、只读回顾、另一账号隔离通过。
- 原生备份还原到独立目录，7 个聊天/选择/演示文件字节保持；恢复根启动及相同功能复验通过。
- Chromium 对接隔离 Linux 服务：1440px 演示计算器及长页、390px 阅读回顾通过，无脚本错误和页面水平溢出。
- 用现有 Provider 在隔离账号对保存现场追问，终态 completed，正确回答 B=8。日志新增 19,280 字节，原有 39,631 字节前缀及另外 3 个聊天文件不变。
- 服务账号通过真实 systemd/bubblewrap worker 完成绘图、三环境预览、交付及私有归属测试。相同 worker 通道的 320×420、640×240、960×720 长页操作均到达真实底部，点击结果为 1，正文读取为 END，截图齐全、浏览器错误为 0。
- 正式公网以 `reader` 登录，9 本书可见；AI Infra 阅读与刷新、PDF Range、可信 HTTPS、1440px/390px 布局通过，无脚本错误。绘图／动画／预览／authoring 能力均为 ready。
- 正式 `adaelon` 的选定版本及 JSONL 与准备文件逐字节一致，AI Infra 当前聊天指向发布入口。浏览器交互及来源验证使用同一成果的隔离账号，正式账号没有重设密码。

## 选定演示的发布方式

在 `adaelon` 的新聊天 `chat-ex13-release-20261002` 发布“1.3 交互讲解 · 已验收版本”。原演示内容、ID、revision 2、27 个来源及 based_on 保持，归属与交付回合改为本次发布入口；记录原 owner 与原作者回合供追溯。新日志明确记为部署导入，模型调用和 token 为零。旧聊天不导入、不改写。准备材料位于 `tmp/ex-jl-release/presentation/`，发布回执位于服务器验收目录的 `presentation/publication.json`。

访问方式：登录 `adaelon` → 打开 `ai-infra-book-complete` → 本书历史 → “1.3 交互讲解 · 已验收版本” → 打开演示或“本次阅读回顾”。本次设置该聊天为该账号在本书的当前选择。

## 备份与恢复入口

- 发布前数据备份：`/opt/understand-book/backups/ex13-jl-20261002/service`，由旧 release 的 `reader_maintenance backup` 在服务停止期间生成。
- 配置备份：同目录 `configuration/`，含 service、nginx、sandbox 及服务环境配置。该目录受限，不复制凭据到仓库。
- 保留旧 release：`/opt/understand-book/releases/chat-recovery-20261002`。
- 如需回退程序，先停止写入，恢复备份中的 service/nginx/sandbox 配置，重新加载服务定义并启动旧版，再检查 nginx 后重载。保留新版期间的数据；旧版不会展示 JSONL 聊天。完整恢复数据是另一项明确操作，不能直接覆盖上线后产生的新内容。
- 验收根：`/opt/understand-book/acceptance/ex13-jl-20261002`；隔离候选服务已停止，本机验收转发已关闭。

## 已知限制

- JL 按既定合同只展示新 JSONL 聊天；原 `agent-history.json` 留在原处，新版聊天列表不自动收录旧聊天。
- EX12.5 的 Linux 发布、长页、保存重开及现场追问已覆盖；线上同对象再次局部修订、长期连续使用、实体手机及学习效果尚未验收。
- JL10 的 Linux 发布、隔离、备份恢复及一次真实增量追问已覆盖；Windows 桌面整体验收、不同长度历史的完整写入矩阵及完整连续使用场景仍待收口，JL10 整体不标完成。
- 服务器直接启动本地 BrowserPreview 的 EX12 测试遇到 Chromium SIGTRAP，未通过；该入口绕过多人服务使用的隔离 worker。本次通过实际线上 systemd/bubblewrap 通道验证三环境滚动与点击，未修改浏览器沙箱参数。失败日志保留，不能写成全部宿主测试通过。

## 证据

本地可复核结果见 [发布验收目录](performance/linux-ex13-jl-20261002/)，主要回执为 [部署](performance/linux-ex13-jl-20261002/deployment.json)、[正式服务与成果核对](performance/linux-ex13-jl-20261002/production-receipt.json)、[公网浏览器](performance/linux-ex13-jl-20261002/production-browser.json)、[备份恢复](performance/linux-ex13-jl-20261002/recovery.json)、[现场追问与追加写入](performance/linux-ex13-jl-20261002/provider-smoke.json) 和 [隔离滚动预览](performance/linux-ex13-jl-20261002/network-scroll.json)。日志分组有重叠，不合计为去重测试总数。
