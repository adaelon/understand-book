# EX13.4 确切版本修订上下文

2026-10-02。**实现及确定性验证完成**。合同见 [EX13 第 5 节](../../../切片方案-EX13-Goal工作计划与演示修订上下文.md#5-按确切版本组织修订)与 [ADR-0155 §4](../../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md#4-历史裁剪与版本修订)。新增真实模型调用 0。

## 追问与按需读取

`presentation_api::follow_up_context` 在原 admission 路径验证 receipt、确切版本与保存现场，投影标题、assumptions、完整 state，以及 `content_access` 中的 author 工具名、实际文件目录、entrypoint 和绑定确切 reference 的 read 参数。选择旧 revision 后，即使已有新 revision 或新现场，仍保留原回执；当前 Goal 的 requirements 与未完成工作项继续由既有采样投影提供。

正文不再自动进入追问。`presentation.author.read/search(file="readable_content")` 将版本已有的可读正文作为逻辑文本读取，支持现有字符范围、查询、结果预算和 continuation。没有新增正文副本或摘要。省略 file 的原 read 行为保持，源码可用实际 file 与 offset/length 定位。新增正文入口解决原 read 把整篇正文作为不可分页元数据时，超过 48 KiB 结果预算无法恢复的问题。

保存现场中的参数、控件、选项、步骤、显示结果和来源引用完整保留，并继续标为页面观察数据。新书源断言仍需取得来源证据。制作指导版本为 `ex13.v2`，editing 指导说明按版本定位、当前缺陷、Goal 剩余工作与两种写入意图。

## 写入合同

| 场景 | 结果 |
| --- | --- |
| 有 receipt，based_on 匹配其 reference | 从确切版本创建候选，按原规则继承来源和兼容的冻结参数；交付时在同一对象上新增 revision。 |
| 有 receipt，省略 based_on，new_object 缺省或 false | 返回可修正的 validation 错误，给出确切基底和显式新建方式；不保存候选。 |
| 有 receipt，based_on 指向别的版本 | 拒绝，即使那个版本真实存在。 |
| new_object=true 且提供 based_on | 拒绝冲突参数。 |
| 省略 based_on，new_object=true | 创建独立对象；不继承旧页现场或来源资格。 |
| 无 receipt、无 based_on | 普通新建继续可用，new_object 可省略。 |

显式新建的当前运行候选可以继续 patch。带已交付基底的 patch 仍须匹配 receipt，修改后仍需新预览。跨运行 read/search/patch 拒绝旧 candidate_id，旧预览不授予新运行交付资格。已交付 reference 保持可读。

## 请求记录

同一受控输入保存 [改动前请求计划](before/request.json)、[改动后请求计划](after/request.json)及各自的 [before/context](before/context.json)、[after/context](after/context.json)。适配器捕获真实 `AgentRequestPlan` 的 instructions、ordered_messages 和 tools，返回固定答案；未向提供方发送请求。fixture 包含旧 revision、较新的版本和现场、36,000 字节正文、完整参数现场及未完成 Goal 工作项。

| UTF-8 字节口径 | 改动前 | 改动后 |
| --- | ---: | ---: |
| 追问 agent_message 原字符串 | 37,142 | 1,706 |
| 请求计划紧凑 JSON | 75,065 | 39,657 |

追问减少 35,436 字节（95.41%），正文标记只出现在改动前请求中；instructions 相同。两次 fixture 的生成身份不同，比较用于体积与内容保留，不用于证明整份历史前缀字节相同。`usage=null`，此处字节不换算成 token、缓存命中或费用。

## 验证与重放

生产修改前，两个回归分别在“全文仍被发送”和“遗漏 based_on 仍保存候选”处失败。修改后：

| 命令 | 结果 | 验证对象 |
| --- | --- | --- |
| `cargo test -p runtime --lib` | 449 通过，0 失败，3 忽略 | new_object 解析/schema/历史、保存后投影保留意图、超 48 KiB 正文连续取回，以及 Goal/制作/交付合同。 |
| `cargo test -p server --lib presentation_store_tests::` | 19 通过，0 失败，4 忽略 | 首个真实请求计划、旧版本与完整现场、按需正文与源码、写入意图矩阵、同对象 revision、来源和参数继承、存储重开。 |
| `cargo test -p server --lib presentation_author_tests::editing::` | 4 通过，0 失败，1 忽略 | 局部 patch 原子性、保留原内容、修改后新预览，以及跨运行 read/search/patch/deliver 隔离。 |

日志：`tmp/ex13-4-red.log`、`tmp/ex13-4-runtime.log`、`tmp/ex13-4-server.log`、`tmp/ex13-4-editing.log`。Server 测试进程使用独立 TEMP/TMP：`tmp/ex13-4-tests`，避开 EX13.3 已知的公共临时库问题。新测试第一次参数继承断言使用了扁平 values，核对现有合同后改为真实页面的 `values.page`，产品参数继承规则未变。

重录：将 `EX13_FOLLOW_UP_RECORDING_DIR` 指向新输出目录，运行 `cargo test -p server --lib ex13_follow_up_projects_locators_and_reads_the_exact_old_version_on_demand`。改动前录制保持冻结。体积可用 PowerShell `ConvertFrom-Json` 读取：上下文取 UTF8.GetByteCount(context)，请求取 UTF8.GetByteCount(ConvertTo-Json -Depth 100 -Compress)。

## 已知限制

同对象和新对象的交付测试使用真实私有存储，并手动填入既有宿主预览回执；本片未改浏览器行为，未重跑浏览器或真实模型验收。既有 Runtime 测试继续约束三环境预览和后续图片观察。本次未提交、未部署。EX13.5 摘要优化、EX13.6 真实接续、EX12.4 最终原生验收和 EX12.5 连续使用仍待完成。
