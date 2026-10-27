# 预构建知识进入画板，2026-10-26

2026-10-26 12:57:20 HKT 已发布到 [Understand Book](https://understandbook.top)。正式版本 `board-prebuild-20261026`，服务 PID `174506`，schema 20，维护窗口 14.8 秒。Windows 0.3.8 下载文件继续可用；本次更新在云端 Reader 和服务端生效。

## 问题与行为

AI Infra 已经有 Pass1、Discourse 与 BookStructure，旧画板目录却只读取正式教学图，因此预构建标签错误地显示没有可加入内容。新目录直接读取 Reader 已接纳的现有成果，不要求再生成教学图，也不重新运行模型。

| 既有成果 | 界面分类 | AI Infra 实际条目 |
| --- | --- | ---: |
| Pass1 图节点 | 概念与论断 | 2,949 |
| Discourse 条目 | 篇章脉络 | 4,276 |
| BookStructure 章节、贯穿主题与阅读重点 | 全书结构 | 981 |
| 合计 | 全部预构建 | 8,206 |

目录按 60 项分页，筛选与搜索在全部候选中执行；不会只搜索当前加载页。导入保存类型、条目身份、来源版本、原始快照、可读正文与原文绑定；重开、关联画板查询、全文搜索、原文预览和局部追问继续沿用原画板链路。BookStructure 包括 14 个章节结构、11 条贯穿主题和 956 个阅读重点。篇章模式和常用作用标签转为中文说明。

`BoardTarget::Artifact / BoardLookupTarget::Artifact → board_api::catalog / resolve_import → board_prebuild::catalog / resolve → BoardWorkspace / BoardReadingPane / BoardSourcePreview` 构成新增路径；原 `Prebuild` 教学对象保持原身份，新增内容不伪装成教学对象。Rust 生成对应 TypeScript 合同，完整范围见 [source-files.json](source-files.json)。

## 验证

| 范围 | 证据 |
| --- | --- |
| 原画板回归 | 47 项通过，4 个需独立宿主的测试忽略；[日志](server-tests.log) |
| 新导入路径 | 无教学图时的三类导入、结构三种条目、保存重开、来源解析、局部追问冻结、过期来源拒绝、分页与跨页搜索通过；[日志](prebuild-tests.log) |
| 真实 AI Infra | 直接加载线上既有成果，三类数量与原文绑定通过；[回执](real-book.json) |
| 可读篇章标签 | 最后标签修订后的导入、原始快照保留与原有导入路径回归通过；[日志](labels-test.log) |
| 前端 | 5 文件合计 50 项通过；初跑中的 3 个失败修正后，相关文件 30 项复测通过，其余 4 文件 20 项沿用初跑通过结果；[初跑](web-tests-initial.log)、[复测](web-retest.log) |
| 正式构建 | 前端类型检查与生产打包通过；Linux 六个运行程序构建通过，最终源码逐文件与冻结包一致；[前端](web-build.log)、[Linux](build-completion.json) |
| 真实界面 | 实际组件连接 Rust 路由；分类、全文搜索、三类导入、刷新保留、原文预览、390 px 布局通过，控制台错误为 0；[回执](browser.json) |
| Linux 候选 | 最终 Linux 二进制，复制真实公开出版物到隔离账号，分页、搜索、导入、重开、关联、原文与中文标签通过；模型调用 0；[回执](preflight.json) |
| 正式公网 | 独立验收账号的新学习工作区读到三类真实条目，导入与重开、原文解析通过；HTML 与引用资源匹配，Windows 0.3.8 安装包完整字节保持；[回执](public-verification.json) |

界面截图来自本地隔离宿主与真实 AI Infra 成果；正式公网验证通过 HTTPS API 执行。最后的中文标签修订在回归测试与最终候选 HTTP 中核对。线上用户的原画板未写入测试内容，专用验收账号已恢复停用，未发起模型请求。

![预构建内容目录](catalog.png)

![手机目录](mobile-catalog.png)

## 运行与恢复

- 新版：`/opt/understand-book/releases/board-prebuild-20261026`；前版：`/opt/understand-book/releases/combined-20261026`。
- 数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 20；没有数据迁移。
- 配置与可变数据备份：`/opt/understand-book/backups/board-prebuild-20261026`；线上原账号和授权目录保持。
- 切换前等待问答和后台任务空闲，备份后切换，健康检查通过再开放入口。[发布回执](deployment.json)。
- 构建遇到内存回收等待，临时交换空间由 256 MiB 增至 512 MiB，完成后全部移除并恢复原 swappiness。[构建资源回执](build-completion.json)。
- 回退时先等待运行任务结束，暂停入口，将服务、环境、nginx 与沙箱 worker 配置恢复为前版，再核对健康并恢复入口。保留新版期间产生的用户数据；旧版不能读取新增 Artifact 类型的画板项目，不能把这类新增内容作为可用的旧版功能承诺。

## 已知边界

Agent 回答、互动演示与“我的理解”仍按当前学习工作区的私有范围读取；旧范围中的历史记录没有自动迁入。正式教学知识点仍需要原有教学图。本次直接接入的三类预构建不受这两项前置条件限制。

Windows 安装包此前已更新为 0.3.8。本次云端画板更新无需重新打包客户端；安装升级与原生 WebView2 验证的既有限制见 [Windows 发布记录](../windows-release-038-20261026/README.md)。
