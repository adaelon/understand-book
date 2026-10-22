# Search / Fetch、独立分账与请求缓存优化发布

2026-10-21 **14:01:20 HKT**，[网站](https://understandbook.top/)与[运营后台](https://understandbook.top/admin/)已切换为 `search-cache-20261021`，服务 PID `520445`，schema 19 → 20。公网真实工具调用、账本、费用页面和既有入口验收全部通过。Nginx 两次入口重载之间的维护窗口为 **27.63 秒**；部署脚本的 69.17 秒还包含恢复公网后的验收时间。

## 发布范围

基于当前已部署的 `cloud-20261021` 生成独立候选，覆盖 34 个范围内源文件，保留此前已经发布的 9 个 Windows 0.3.1 配套前端文件。只纳入 Search/Fetch、共享额度和独立费用记录、对应 Reader/Admin 展示、历史原生工具名称稳定性、回合动态状态位置和 Tutor 稳定引用示例。

数学结构、阅读恢复、其他未发布实现及无关测试改动未纳入。完整范围见 [source.json](source.json) 和 [overlay.patch](overlay.patch)。Windows 0.3.1 安装包仍为原有 4,899,391 字节，公网下载与上一版本逐字节相同。工作区没有回退，也未提交或推送 Git。

## 生产配置

- `/opt/understand-book/multi-reader.env` 中显式设置 `DEEPSEEK_SEARCH_API_KEY`，与已运行的官方 DeepSeek `OPENCODE_API_KEY` 相同；验证只保存相等结论，不保存密钥。
- `DEEPSEEK_SEARCH_BASE_URL=https://api.deepseek.com`，`DEEPSEEK_SEARCH_QPS=100`，`DEEPSEEK_FETCH_QPS=25`。模型路由和模型密钥保持原配置。
- 服务数据根的 `web-rates.json` 分别配置 `web_search` 与 `web_fetch`：每次成功计费请求 5,000 微元，即 ¥0.005。二者复用现有额度预留、结算和核算机制，费用各自记录；搜索结果进入模型上下文产生的 token 费用继续计入模型类。
- 搜索缓存命中不新建物理请求账项；真实重试各自留回执；不确定费用保留待核算状态。上述机制的定向覆盖和原题实验沿用[接入实验](../README.md)与[缓存对照](../cache-optimization.md)。

当前费率本地复核有效期截至 **2026-11-20 13:36:30 HKT**。费率文件见 [web-rates.json](web-rates.json)，来源为 [DeepSeek 官方搜索文档](https://api-docs.deepseek.com/zh-cn/guides/search/)。

## 上线验收与费用

在正式网站使用专用账号 `search-release-20261021`，授予最多 ¥4 的验收额度。一次真实问答先发现工具，再以 `refresh=true` 实际调用 Search 和 Fetch；查询官方接口文档，读取官方正文，模型给出区分与来源链接。执行轨迹为 `tool.search → tool.search → web.search → web.fetch`。

| 费用类别 | 实际调用 | 已确认费用 |
|---|---:|---:|
| 模型：4 次回答调用 + 1 次账号级记忆审阅 | 5 | ¥0.108462 |
| Search | 1 | ¥0.005000 |
| Fetch | 1 | ¥0.005000 |
| 合计 | 7 | **¥0.118462** |

Search 返回 HTTP 200，传输耗时 352 ms；Fetch 返回 HTTP 200，104 ms。两者排队 0 ms，均为第 1 次物理尝试。费用与回执见 [verification.json](verification.json)。这笔上线验收费用与此前原题、缓存对照费用分别记录。

前台问答先结清 ¥0.111662，随后账号级 `memory_review` 又产生 ¥0.006800，最终完整计入模型成本；该账号级整理保留自己的任务归属，不重复摊入原问答。最终账号扣减与已确认费用均为 ¥0.118462，预留、待核算和未知费用均为 0。验收账号已停用，全部 7 条账项和回答记录保留；验收场景已解除临时占用。

公网浏览器实际验证通过：后台按三类显示金额与调用数，读者端显示模型 5、搜索 1、网页读取 1 次，并显示对应额度消耗。页面脚本错误为 0。[浏览器回执](ui-smoke.json)、[后台截图](admin-costs.png)、[读者截图](reader-costs.png)。

## 构建、迁移与既有功能

- Linux 六个正式程序完成 release 构建，最终构建耗时 5 分 41 秒；[日志](build.log)。范围拆分首次遗漏 DNS 超时解析函数的 `pub(crate)` 单行依赖，补齐后通过，未扩展业务范围。
- 隔离源码 Reader 类型检查、生产构建及 2 项费用测试通过；Admin 类型检查、生产构建及 4 项费用测试通过。
- 从原备份恢复隔离副本，先用当前 schema-19 服务完成其已有资料登记，再升级候选。首次隔离启动的登录检查曾返回一次 503；重启后登录和数据库写入检查通过，随后完整迁移流程通过。逐表比较原业务行，仅排除登录会话和既有读取路径更新的 `cloud_storage_adoptions.maintained_at` 时间戳。[迁移回执](api-preflight.json)。
- 正式切换在无活动或未保存问答、无在途费用时进行。最新生产数据升级前后核对一致，包括 4 个原有账号、14 份发布材料、51 条材料授权、129 个阅读现场和原有费用记录。以上数量在创建验收账号之前记录。
- 原有读者、管理员的授权列表、原文与目录、学习工作区入口、演示能力和未开放套餐状态通过；源站 HTML 与候选一致，公网入口及资源一致；[部署回执](deployment.json)、[静态资源回执](static-check.json)。

验收脚本最初使用了默认视频材料而未提供视频问题上下文，并在修订过程中占满专用账号的临时阅读现场；这些请求均在模型调用前被拒绝，费用为 0。随后明确选用已有 `understandbook` 文本材料并复用现场完成真实验收。没有为适配脚本修改产品约束。

## 备份与恢复

- 新版本：`/opt/understand-book/releases/search-cache-20261021`；旧版本：`/opt/understand-book/releases/cloud-20261021`。
- 数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`。
- 升级前可变数据归档：`/opt/understand-book/backups/search-cache-20261021/mutable.tar.gz`，629 个文件、13,078,132 字节。包括数据库、用户记录及可变配置；已验证数据库完整性和业务行，并下载本机私有备份目录。
- 不可变发布材料快照：`/opt/understand-book/backups/search-cache-20261021-publications/published`，8,080 个文件、4,075,743,972 字节，采用硬链接保留。
- 原配置：`/opt/understand-book/acceptance/search-cache-20261021/previous-configuration`；恢复说明见 [snapshot.json](snapshot.json)。原配置也已保存在本机私有备份，不进入公开证据。

若需回退旧程序，应将 schema-19 归档恢复到新的数据根，恢复不可变材料，调整 publication 目录并使用旧配置。旧程序不能直接打开已升级的 schema 20；公网已经恢复写入，回退时必须保留上线后的数据。独立验收副本已清理，保留日志和回执；未清理其他任务的版本或用户数据。

## 已知限制

- 费率采用现有有效期机制，需在上述截止时间前复核并续期；过期后系统会拒绝新的付费联网请求。
- 本次烟测用于验证真实工具和费用链路，不构成新的缓存 A/B 实验。原题加连续追问的缓存输入占比 57.49% → 70.67%、费用 ¥0.566267 → ¥0.360068，以及没有实质质量退步的判断，仍以[原对照报告](../cache-optimization.md)的样本范围为准。
- 本次模型正确走完工具并引用官方文档，但在要求“三句话”之外附加了执行说明和证据边界说明；未将回答篇幅控制判为通过。
- 发布材料硬链接快照用于同机发布回退，不是独立磁盘灾备；本机已下载的完整可变数据备份不包含那 4.08 GB 不可变材料。
