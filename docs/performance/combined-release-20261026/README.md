# 未上线改动合并发布，2026-10-26

2026-10-26 09:27:37 HKT 已发布到 [Understand Book](https://understandbook.top)。正式版本 `combined-20261026`，服务 PID `162658`，schema 20，维护窗口 **14.48 秒**。原账号、授权资料、聊天与用户数据继续使用原数据根。

## 发布范围

- 知识画板 WB1–WB4：保存与重开、预构建内容导入、局部追问、原生图形与图文联动、分段正式交付，以及取消后保留已提交部分。
- Agent 公式约定：独立公式使用 `$$…$$`，行内变量使用 `$…$`；已有教学解释规则 `plain-explanation.v5` 保留。
- Harness 缓存机制：按实际能力选择 DeepSeek Messages、工具在历史尾部激活、状态增量、压力压缩及活动模型投影持久恢复。
- LangSmith：实际供应商与用途、标准用量、首次终态导出、重试与 spool 恢复。
- 已完成的自动构建／恢复、结构容量和数学结构提取修改一并纳入。首页最新工作区展示与已上线管理界面保留，源码随包补齐。

以旧线上版本 `teaching-explanation-20261025` 为基础，冻结并覆盖 270 个应用源码文件；完整清单见 [source-files.json](source-files.json)。未把工作区的书籍产物、研究记录或临时目录当作服务器数据发布。服务器切换时 Windows 安装包为 0.3.7；同日 11:06:09 HKT 已另行更新为 **0.3.8**，见 [Windows 发布记录及验证限制](../windows-release-038-20261026/README.md)。

## 本次修复：Messages 续问被计价误拦截

真实模型验收的第一轮正常输出公式，第二轮在请求发出前出现 `MODEL_RATE_UNAVAILABLE`，原始结果见 [修复前记录](model-smoke-before-fix.json)。当时有效费率存在；根因是 `ModelRateSnapshot::estimate` 只接受旧 Chat Completions 的 text／image_url 内容，无法识别新协议保留的 thinking、tool_use／tool_result 和工具变更记录。

`crates/server/src/model_rates.rs` 现在计入 Messages 的顶层 system、思考与工具历史，并将原生 image 按现有图片计价规则处理，不把 base64 字节当成正文。费用仍按供应商实际 usage 结算；费率配置未变。增加一项覆盖真实续接形状的回归测试，连同原计价测试共 4 项通过。

## 验证证据

| 范围 | 结果与证据 |
| --- | --- |
| 构建与结构流程 | 10 组、260 项通过；[日志](core-tests.log)；Core 类型检查通过 |
| 正式前端 | `VITE_MULTI_USER=1` 的类型检查与生产构建通过；[日志](web-build.log) |
| Linux 后端 | server、manage_reader、publish_book、reader_maintenance、presentation_worker、video_audio_review 编译通过；[回执](build-completion.json) |
| 候选 API | schema 20；创建／连接各 6 次重试不重复建工作区；目标恢复、历史演示、知识画板导入／修改／读取通过；[回执](preflight.json) |
| Linux 画板制作 | 正式 worker 经沙箱渲染真实 BoardCanvas，中文、卡片及原生图形正常，errors 与画板 issues 均为空；[回执](board-preview.json)／[截图](board-preview-0.png) |
| 实际模型与计费 | 修复后首轮完成，补足测试额度后重启恢复同一对话并完成续问，分别输出独立公式和行内变量，主调用费用已结算；本次通过验收的已知供应商费用 0.220727 元；[回执](model-smoke.json) |
| 线上服务 | 实际运行程序与目标版本一致；公网入口应用内容一致（CDN 加入统计脚本），引用资源逐字节相同；原读者／管理员授权资料清单与正文读取保持；[回执](public-verification.json) |
| 公网浏览器 | Edge：现有首页、已保存回答与互动演示画板、改名保存、刷新恢复、390 px 阅读切换与关闭回阅读通过；页面异常与画板接口失败均为 0；[回执](live-browser.json) |

公网画板使用专用账号中的历史回答与互动演示验收夹具，并绑定现有已发布资料。当前所选资料没有预构建教学图，预构建导入沿用 WB1 的专项测试证据。

上线前已完成的 WB1–WB4、Harness 和 LangSmith 专项覆盖，分别沿用其实施记录。本次没有重复执行所有历史测试。公网浏览器通过服务器 SSH 转发连接公开 HTTPS，正常校验证书；该结果不用于用户本地网络时延比较。专用线上验收账号已停用，线上浏览器验收模型调用为 0。

![桌面画板](live-board-desktop.png)

![手机全文](live-board-mobile.png)

## 运行与恢复

- 最终服务与任务状态：[service-final.json](service-final.json)。
- 当前目录：`/opt/understand-book/releases/combined-20261026`。
- 前版目录：`/opt/understand-book/releases/teaching-explanation-20261025`，原运行程序与静态资源保留。
- 数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`；schema 仍为 20。
- 配置与可变数据备份：`/opt/understand-book/backups/combined-20261026`，含 service unit、nginx、环境配置、sandbox 配置和 mutable.tar.gz。不可变出版物与共享缓存未重复归档。
- 切换前确认无运行中的问答、模型／搜索／服务调用及教学后台任务。新版本健康检查通过后恢复公网入口。
- 必须回退时先等当前任务结束、暂停入口，再将上述运行配置恢复为前版，重载服务与 nginx，并复查登录、授权资料、正文与展示能力。已产生的新用户数据应保留；不要直接用旧备份覆盖。
- 构建只使用临时 512 MiB swap，结束后已清除并恢复 swappiness。为腾出构建空间，仅清理已安装 LibreOffice 的冗余 RPM 展开包，安装目录与生产数据保留。

## 已知边界

隔离模型验收曾因 2 元测试额度不足中断：第一轮后后台教练占用预留额度，第二轮未发送。测试额度补至 6 元后，在服务重启恢复的同一对话中补跑续问通过；该次被中断的后台教练调用缺少最终用量，保留 pending，不当作零费用。修复前那次候选试跑的后台调用也随测试服务停止而中断，其记录保留在服务器验收目录。这里的已知金额不是完整供应商账单，正式用户余额未改动。

本次两轮模型结果验证协议、续问、公式和结算可用，不证明长期缓存命中率或平均费用改善。历史缺失的 LangSmith 用量不会凭新代码补全，历史最终 PATCH 的限制仍然存在。正式环境已有 spool 配置，本次保留。更完整的机制和限制见 [Harness 记录](../harness-cache-20261025/README.md)与 [LangSmith 记录](../langsmith-repair-20261025/README.md)。
