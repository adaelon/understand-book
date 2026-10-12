# 内测 MCP 与调用额度切片方案

日期：2026-10-11。状态：**MCP0–MCP9 已完成；MCP10 待实施，远程服务尚未上线。**

决策入口：[ADR-0162](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。产品说明：[内测 MCP 产品约定](内测MCP-产品约定.md)。实际接口见 [MCP1 实现](内测MCP-MCP1实现.md)、[MCP2 实现](内测MCP-MCP2实现.md)、[MCP3 实现](内测MCP-MCP3实现.md)、[MCP4 实现](内测MCP-MCP4实现.md)、[MCP5 实现](内测MCP-MCP5实现.md)、[MCP6 实现](内测MCP-MCP6实现.md)、[MCP7 实现](内测MCP-MCP7实现.md)与 [MCP8 实现](内测MCP-MCP8实现.md)，MCP9 验收见 [实现记录](内测MCP-MCP9实现.md)，MCP10 的新增文件、类型和接口为拟实施落点；现状依据已在 `codex/reading-share` 工作树复核。接手入口：[内测 MCP 实施入口](内测MCP-实施入口.md)。

## 0 对齐确认单

**FrozenIntent**：仅向现有内测账号提供 Agent 可自主使用的远程资料检索与读取；每账号每月免费 1,000 次，额外 10 元购买 1,000 次、有效期 12 个月。外部 Agent 负责推理；成功标准是新接入 Agent 自行找到获准材料、读取可引用原文，并正确理解次数与停止状态。

**确认依据**：使用资格、纯检索读取、次数计量、免费额度规则和预付调用包均已确认；本次交付为 ADR 与可执行切片方案。

**RiskReceipt**：MCP 次数与托管模型费用分开；账号资格、材料授权和购买次数分别表达。原文读取有单页上限，来源和付费次数的跨月保留已纳入合同。

**ChangeType**：`[模型扩展]`，增加 MCP 接入与调用次数权益。

| 术语 | 状态 | 定义入口 |
| --- | --- | --- |
| 内测账号、内测码、ReaderAdmin、ManualReceipt | EXISTING | 现有账号准入、管理员与人工收款事实 |
| PublishedBookRef | EXISTING | `book_id + publication_id` 指定已发布材料版本 |
| AccountAllowance、AllowancePeriod | EXISTING | 托管模型的人民币成本额度 |
| 内测 Book MCP、MCP 免费使用额度 | NEW，已定义 | 账号授权的远程读取与月度免费次数 |
| MCP 付费调用包、MCP 调用记次 | NEW，已定义 | 有效期内跨月保留的已购次数与单次消费事实 |
| MCP 接入凭证 | NEW，已定义 | 为外部 Agent 开通、可撤销的 MCP 专用授权 |

领域对齐完成，术语已写入 [CONTEXT](../CONTEXT.md)。首版凭证采用可配置请求头的接入方式，是实施选择；客户端兼容范围见第 10 节。

## 1 已核对的现状与落点

| 现有源码 | 事实与实施影响 |
| --- | --- |
| [mcp.rs](../crates/server/src/mcp.rs)：`tools_list_result`、`dispatch_mcp_tool`；[book_mcp.rs](../crates/server/src/bin/book_mcp.rs) | 本机 stdio 绑定一本书；工具集包含 `book_query`、`book_synthesize`、`book_guide` 等模型命令。远程入口需单独选择纯读取工具。 |
| [book-tool-contracts](../crates/book-tool-contracts/src/lib.rs)；[read-tools](../crates/read-tools/src/lib.rs)：`Book::text/text_page` | 复用原工具说明和输入校验；本机 `text` 保留原行为，MCP2 的 `text_page` 直接从源范围分页读取。 |
| [published_library.rs](../crates/server/src/published_library.rs)：`list`、`load`、`authorize` | 已有按账号过滤的目录、不可变出版版本与缓存；`load` 在命中缓存时仍检查授权，可直接作为远程材料入口。 |
| [auth.rs](../crates/server/src/auth.rs)：`Principal`、`AuthService`；[authorization.rs](../crates/server/src/authorization.rs)：`context` | 现有 Principal 绑定网页登录会话；`context` 会取得私人 UserHandle。MCP 验证账号后直接读取 PublishedLibrary，不以伪造网页 Principal 或创建 ReaderWorkspace 接入。 |
| [multi_user_host.rs](../crates/server/src/multi_user_host.rs)：`Site::validate`、`dispatch` | 普通写请求要求站点 Origin、Cookie 与 CSRF。远程 Agent 需要专用 Bearer 分支，网页写请求继续沿现有校验路径。 |
| [control_store.rs](../crates/server/src/control_store.rs)：`ServiceWriter`、`ControlStore::open` | 已有进程写者和 SQLite 顺序升级。MCP 凭证、次数与消费记录进入同一 control.sqlite。 |
| [admin_store.rs](../crates/server/src/admin_store.rs)、[admin_schema.sql](../crates/server/src/admin_schema.sql) | 已有管理员操作回执、业务参数和人工收款表，复用其操作号和事务。 |
| [allowance_admin.rs](../crates/server/src/allowance_admin.rs)：`apply`、`list` | MCP4 已按 `entitlement_kind` 区分模型额度与 MCP 调用包；MCP 的 `period_id` 为空，使用 `pack_id` 指向购买批次。 |
| [NetworkApp.vue](../packages/web/src/NetworkApp.vue)、[AccountSettings.vue](../packages/web/src/components/AccountSettings.vue)、[network-client.ts](../packages/web/src/network-client.ts) | 已有网络账号入口；在账号级提供 MCP 接入、次数和购买说明，无需先打开一本书。 |
| [后台入口](../apps/admin/src/app.tsx)、[allowance.tsx](../apps/admin/src/pages/allowance.tsx)、[operations.ts](../apps/admin/src/lib/operations.ts) | 后台已支持人工操作与重复提交回执；增加 MCP 次数管理及收款用途展示。 |
| [Nginx 模板](../scripts/linux/nginx-multi-reader.conf.example) | `/api/` 当前清除 Authorization。为 `/mcp` 增加精确代理入口并传递所需请求头。 |

## 2 完整路径与模块归属

```text
账号页面：已登录内测用户 → 创建 MCP 接入凭证 → 配置 Agent 客户端
内容路径：Agent → HTTPS /mcp → 账号与凭证 → 已发布材料授权
          → 纯读取工具 → 内容结果准备 → 原子记次 → 来源 + 结果 + 额度
购包路径：管理员确认收款 → 同事务记录收款与调用包 → 用户/Agent 查询余额
```

Server 模块：已实现 `mcp_access.rs` 的凭证与账号级入口、`mcp_read.rs` 的获准版本纯读取与分页、`mcp_usage.rs` 的月度免费次数、赠送和成功记次；MCP4 已通过 `mcp_pack.rs` 加入购包、更正与付费消费来源，MCP5 已通过 `remote_mcp.rs` 完成协议接入。模块复用现有认证约定、PublishedLibrary、read-tools 和 ControlStore，职责不引入独立服务。

所有 MCP 调用使用共享的已发布材料缓存。读取期间不持有管理员账本事务；消费记录在短事务中提交。网页账号管理仍走 Cookie/CSRF，远程内容调用只走 MCP 凭证。

## 3 身份与远程接入

- 账号资格取现有已启用内测账号。内测码仍只用于注册，历史已获准账号可开通 MCP。
- 已登录用户创建、列出和撤销自己的 MCP 凭证；凭证只授权远程目录、内容读取与本人次数查询。
- 凭证绑定账号及现有 `auth_epoch`。账号停用、改密或管理员撤销全部授权后，旧 MCP 凭证失效；重新启用不恢复旧凭证。单独退出网页不等同于撤销 MCP 凭证。
- 每次远程请求从凭证解析调用账号，材料参数只能指定 PublishedBookRef。材料撤权后后续调用失败，不能因缓存命中继续读取。
- 首版使用 HTTPS `/mcp` 与 `Authorization: Bearer ...`。凭证的保存和校验沿用现有认证约定，账号页面只在创建时显示完整值；接入说明使用占位符。

首版协议合同采用 MCP `2025-11-25` 的 Streamable HTTP 请求/响应方式。POST 返回 JSON，通知按协议返回空响应；不提供服务端主动推送的 GET 返回 405。初始化声明实际支持的版本与 tools 能力，后续按该版本校验，不能原样回显任意版本。

`/mcp` 的 POST 不依赖网页 Cookie 或 CSRF。Host 沿用部署配置；无 Origin 的已认证 Agent 请求可接入，有 Origin 时验证站点来源。Nginx 仅在精确 MCP 入口转发 Authorization，并保留 MCP 协议头；普通 `/api/` 路径按原合同处理。

MCP1 已实现账号 API：`GET /api/account/mcp`、`POST /api/account/mcp/credentials`、`POST /api/account/mcp/credentials/{id}/revoke`。它们只操作当前登录者；管理别人的次数使用MCP4 管理员 API。实际响应见 [MCP1 实现](内测MCP-MCP1实现.md)。

依据：[MCP 传输规范](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)、[Claude Code 远程 MCP 接入](https://code.claude.com/docs/en/mcp)。首版凭证方案不声称已实现 MCP OAuth 授权发现。

## 4 工具与来源合同

| 远程工具 | 必要输入与输出 | 计次 |
| --- | --- | --- |
| `book_list` | 按账号过滤的可读材料、PublishedBookRef、名称、可用读取能力；支持目录分页 | 0 |
| `mcp_usage` | 免费余量、有效已购余量、各包到期信息、免费恢复时间、购买说明入口 | 0 |
| `book_search_text`、`book_concept` | PublishedBookRef 与既有检索参数；返回候选和可回读的定位 | 成功 1 |
| `book_text` | PublishedBookRef、LID 范围及页位置；返回原文、准确范围与下一页入口 | 每页成功 1 |
| `book_context`、`book_structure`、`book_guide_path` | PublishedBookRef 与既有范围参数；返回关系、篇章结构和已有阅读路径 | 成功 1 |
| `book_paper_metadata`、`book_paper_lexicon`、`book_paper_reading_guide` | PublishedBookRef 与既有论文读取参数；仅消费可用的已构建材料 | 成功 1 |

工具说明、纯读取参数含义与校验复用 `book-tool-contracts`。远程 schema 增加明确的 `published_book_ref`；原文分页参数单独定义，不能把远程附加字段直接送入拒绝未知字段的本机输入校验器。

MCP2 已实现本节读取工具和目录；实际使用 `next_calls` 返回完整继续调用参数，结构集合以 JSON Pointer 分片，详见 [MCP2 接口合同](内测MCP-MCP2实现.md)。额度已由 MCP5 在读取结果之外接入。

`book_list` 提供简明材料信息与可用能力，不直接返回整份 LID manifest；全书结构与关系结果以范围和分页按需提供。列表结果须说明是否完整以及如何继续，不能截断后伪装完整。

```text
RemoteBookRef = { book_id, publication_id }
TextPage = { published_book_ref, text, source_range, next_page?, source_url }
ContentResult = { data, sources, usage }
Usage = { free_remaining, paid_remaining, free_resets_at, paid_next_expires_at?,
          charged_units, charged_from?, top_up_url }
```

原文单页最多 8,000 字符，按项目既有 UTF-16 原文坐标表达范围。读取在源范围上分页，不先构造全部文本再截短。多页必须无遗漏、无重叠，保留原材料版本和页内定位；继续读取前照常检查材料授权。

`source_url` 使用站点 Origin 与明确的材料版本、原文范围。Reader 增加对应打开入口；未登录先登录，返回后仍定位同一材料与范围，材料未授权时明确提示不可访问。打开链接是用户选择的阅读操作，远程检索本身不修改网页现场。

## 5 次数与扣除合同

| 项目 | 固定规则 |
| --- | --- |
| 免费次数 | 每账号每自然月 1,000 次；首次开通立即获得完整当月额度 |
| 月边界 | 香港时间每月 1 日 00:00；未用免费次数不累计；不另设每日次数额度 |
| 购包价格 | 每包 10 元，即 1,000 分，授予 1,000 次；可重复购买 |
| 购包有效期 | 从确认收款并开通时起 12 个日历月；有效期内跨月保留 |
| 扣除顺序 | 免费次数优先；其次有效且最早到期的付费包，同到期日按开通顺序 |
| 额度归属 | 同一账号全部客户端共用；撤销或重建凭证不重发免费次数 |
| 补额 | 管理员可赠送当月次数，单独记录原因；不伪造收款 |

服务端时钟决定免费月份和包有效性，数据库时间沿用 Unix 秒，输出时间携带时区。12 个月按日历月计算；目标月份无对应日时取月末。有效区间为 `[opened_at, expires_at)`。

成功的内容工具返回正常空集合也计 1 次。接入、协议初始化、工具发现、目录和额度查询计 0 次。参数错误、权限拒绝、不可用材料/能力、额度不足、服务器执行或记账失败均计 0 次。每个成功分页和重新发起的成功调用分别记次。

一次内容调用按以下顺序完成：

```text
验证 MCP 身份、工具参数与材料授权
额度已耗尽 → 返回 quota_exhausted，跳过内容计算
执行纯读取并准备可编码的有界结果
读取或结果准备失败 → 返回失败，记次为 0
短事务：以事务时间确定免费月份与有效包
        原子选择一个可用次数 → 扣 1 → 写消费记录 → 提交
无可用次数或事务失败 → 不交付内容，返回对应失败
提交成功 → 返回内容、来源和扣除后的额度
```

前置余量查询用于跳过无额度的读取；并发扣次以最终事务为准。两个客户端竞争最后一次时，只允许一个事务扣除并交付内容，另一个得到额度耗尽；无需持久化模型费用预占状态。

成功以内容准备与记次事务提交为准。提交前崩溃无已记消费、无已交付内容；提交后连接断开，已记次数保留。客户端再次调用按新请求执行，JSON-RPC 请求 id 不作为跨客户端消费去重键。

月份首次使用时建立唯一的账号月份记录，重启和多客户端接入不重复发放；免费恢复不依赖后台定时发券。过期付费包保留历史，但从可用余额排除。

## 6 收款与次数持久化

以下为最小业务记录；具体表名在对应切片实现时采用现有命名风格。

```text
McpCredential = { credential_id, user_id, auth_epoch, label, created_at, revoked_at? }
McpFreeMonth = { user_id, month_hk, granted_units, used_units }
McpCallPack = { pack_id, user_id, receipt_id, price_fen, total_units, used_units,
                opened_at, expires_at }
McpUsage = { call_id, user_id, tool, published_book_ref, units: 1,
             debit_source: free_month | call_pack, debit_ref, completed_at }
```

凭据校验数据按既有认证约定保存，上述业务记录和账号响应不携带原始凭证。消费记录只存账号、工具、材料版本、次数、扣除来源与时间；检索正文和返回原文不进入次数账本。

复用 `manual_receipts`、`admin_operations` 和既有管理员权限。购包提交明确包数及收款依据；以每包 1,000 分/1,000 次计算，并保存当时规则。操作号、收款与调用包在同一数据库事务中成功或失败。重复提交沿用原操作号、直接比较业务字段并返回原回执；不同业务参数报冲突。已有渠道交易编号去重继续生效。

收款投影明确区分模型额度与 MCP 调用包。MCP 收款没有 `allowance_period`；模型额度的余额与成本汇总不因此增加。修改 `allowance_admin.rs::list` 等依赖路径，使同一账号两类收款均可读取、更正和解释，不能为 MCP 收款伪造模型额度期。

管理员误录更正沿用追加事实方式，收款金额更正与次数调整分别记录并关联原操作；只能扣回尚未消费的次数，已消费历史不删除。金额登记更正不声称已完成外部退款。赠送当月次数使用独立管理动作及原因。

已实现管理入口位于 `/api/admin/users/{user_id}/mcp/...`，支持查询、登记购包、赠送当月次数和更正；复用现有管理员操作回执。以现有 ControlStore 顺序 schema 升级新增凭证和次数表，不迁写旧模型消费记录。

例：本月免费 1,000 次耗尽后，购包 1,000 次并消费 500 次。下月返回 `free_remaining=1000, paid_remaining=500`；下一次成功调用后为 `999, 500`。在一个包到期后，其余包余额继续可用。

## 7 Agent 与界面合同

接入页面提供远程 URL、凭证创建/撤销、当前余量、购买说明，以及可交给 Agent 的简短说明。接入说明描述配置方式和权限；初始化说明与工具描述解释“先选材料、再定位、再读证据、最后引用”的路径，Agent 无需读取项目源码。

能力发现与额度查询在内容额度耗尽后仍可使用。内容工具不因余量为零从工具列表随机消失；耗尽由调用结果明确表达。

| 状态 | Agent 可执行的下一步 |
| --- | --- |
| 正常结果 | 使用结果并按来源引用，必要时继续分页 |
| 正常空检索 | 调整关键词或材料范围；本次计 1 次 |
| `quota_exhausted` | 停止重复内容请求，告知免费恢复时间和购买入口 |
| `unauthenticated` | 提示用户检查或重新开通接入凭证 |
| `material_unavailable` | 查看获准目录或由用户联系管理员 |
| `invalid_arguments` | 依据工具 schema 修正参数；本次不扣次 |
| 临时限流或服务不可用 | 根据明确返回的重试信息处理；未成功调用不扣次 |

业务失败作为 MCP 工具错误返回稳定错误码、可读说明及 `retryable`/明确下一步；身份和协议错误使用相应 HTTP 状态。所有次数来自服务端查询，Agent 不自行推算余额。

购买说明明确免费优先、后续消耗已购次数、12 个月期限和全部耗尽后停止。用户通过现有人工收款流程购买；Agent 只提供说明入口。读者页面区分免费余量与已购余量，并显示各包到期时间；管理员页面区分收款金额、MCP 次数和模型费用。

## 8 切片执行顺序

| 切片 | 依赖 | 状态 | 独立交付 |
| --- | --- | --- | --- |
| MCP0 | — | 已完成 | ADR、术语、产品约定与本方案 |
| MCP1 | MCP0 | 已实现 | 账号绑定的接入凭证及撤销；[接口与验收](内测MCP-MCP1实现.md) |
| MCP2 | MCP0 | 已实现 | 获准材料纯读取、原文与结构分页；[接口与验收](内测MCP-MCP2实现.md) |
| MCP3 | MCP0 | 已实现 | 月度免费次数、当月赠送与原子消费；[接口与验收](内测MCP-MCP3实现.md) |
| MCP4 | MCP3 | 已实现 | 付费包、人工收款及两类收款投影；[接口与验收](内测MCP-MCP4实现.md) |
| MCP5 | MCP1、MCP2、MCP4 | 已实现 | 完整远程 MCP 协议与扣次路径；[接口与验收](内测MCP-MCP5实现.md) |
| MCP6 | MCP5 | 已实现 | Agent 使用说明、工具自述和恢复信息；[说明与验收](内测MCP-MCP6实现.md) |
| MCP7 | MCP6 | 已实现 | 读者接入页面、额度展示与来源打开；[接口与验收](内测MCP-MCP7实现.md) |
| MCP8 | MCP4 | 已实现 | 管理员购包、赠送与更正界面；[实现与验收](内测MCP-MCP8实现.md) |
| MCP9 | MCP7、MCP8 | 已完成 | 双账号、真实 Agent、引用／账本与回归；[验收](内测MCP-MCP9实现.md) |
| MCP10 | MCP9 | 待实施 | Linux 发布运行单、恢复与线上验证 |

各切片完成时更新本表并在代码链路记录实际入口、文件、测试与证据；未运行的验收保持待验。MCP1–MCP9 已有实际实现和验收，MCP10 仍为计划。

### MCP1 — 内测账号接入凭证

**实现**：2026-10-11 已接通本人账号 API、独立 `McpIdentity` 和 schema 10；定向结果与旧 MU4 测试准备失败见 [MCP1 实现](内测MCP-MCP1实现.md)。
**输入与产出**：基于现有账号与 auth_epoch，提供本人凭证创建、列表、撤销及远程身份验证。
**落点**：`crates/server/src/mcp_access.rs`、`mcp_access.sql`、`mcp_access_tests.rs`；已修改 `control_store.rs`、账号路由和能力表。
**边界**：本片不执行内容工具或扣次；不把 MCP 凭证转换为可操作网页账号的 Cookie。
**验收**：真实 SQLite 中两个内测账号只能管理自己的凭证；未授权接入、已撤销凭证、账号停用及旧 epoch 均不能认证，重启后结果一致。实际用例入口：`cargo test -p server --lib mcp_access`。

### MCP2 — 材料读取与来源分页

**实现**：2026-10-11 已完成目录、九个纯读取工具、来源与续页；入口和验证见 [MCP2 实现](内测MCP-MCP2实现.md)。
**输入与产出**：按账号和 PublishedBookRef 复用已发布材料，形成第 4 节的纯读取调用与分页结果。
**落点**：`crates/server/src/mcp_read.rs`、`mcp_read_tests.rs`；`published_library.rs:list_manifests_page`、`read-tools/src/lib.rs:Book::text_page` 及 `mcp_text_page_tests.rs`；复用 `book-tool-contracts`。
**边界**：本片不接模型、私人 Reader 现场或计费流程。
**验收**：两账号隔离、同书不同 publication 的实际元数据与来源隔离、撤权后缓存续页拒绝、8,000 字符分页拼接与 UTF-16 坐标、嵌套结构及论文集合无遗漏拼接均通过。实际入口：`cargo test -p server --lib mcp_read`、`cargo test -p read-tools --lib mcp_text_page`，分别 4 项与 3 项通过。

### MCP3 — 月度免费次数

**实现**：2026-10-11 已完成 schema 11、账号月份余额、赠送回执和短事务成功消费；实际接口与验证见 [MCP3 实现](内测MCP-MCP3实现.md)。
**输入与产出**：以账号月份持久化免费次数、赠送次数和成功消费，提供短事务记次入口。
**落点**：`crates/server/src/mcp_usage.rs`、`mcp_usage.sql`、`mcp_usage_tests.rs`；接入 `ControlStore::open`，复用管理员操作回执与 MCP2 内容工具集合。
**边界**：本片只实现免费来源，不接收款或对外 HTTP；消费入口为后续付费来源保留明确的扣除结果类型。
**验收**：`cargo test -p server --lib mcp_usage` 的 8 项测试通过；真实 SQLite 与可控业务时间覆盖首次整月 1,000 次、香港月边界、旧次数不结转、最后一次并发竞争、重启／换凭证、赠送幂等与事务回滚。远程读取失败至记次的连接沿 MCP5 验证。

### MCP4 — 调用包与人工收款

**实现**：2026-10-11 已完成 schema 12、调用包批次、扣除顺序、管理员购包／赠送／更正和两类收款投影；8 项新增与 38 项相关回归通过，见 [MCP4 实现](内测MCP-MCP4实现.md)。

**输入与产出**：增加 10 元/1,000 次调用包、12 个月到期、免费优先和到期优先扣除；提供管理员购包与更正 API。
**落点**：`mcp_usage.rs`、`mcp_pack.rs`、`mcp_packs.sql`、`admin_store.rs`、`admin_api.rs`、`allowance_admin.rs`；管理合同见 MCP4 实现。
**边界**：本片不接外部支付渠道；收款、购包和模型额度按各自单位记录。
**验收**：同一操作号重试不重复发包；异常回滚不留下半笔收款；正常空结果计一次；跨月已购余量保留；12 个月到期及多包先到期先扣准确；同账号模型/MCP 两类收款列表与更正均可用。实际运行库测试 `mcp_pack` 及受影响回归；本机构建命令与日志见 [MCP4 验证](performance/mcp4-20261011/README.md)。

### MCP5 — 远程 MCP 完整路径

**输入与产出**：连接身份、材料读取和次数账本，提供 `/mcp` 的初始化、工具发现与调用。
**实现**：2026-10-11 已接通无会话 Streamable HTTP、身份／读取／扣次；新增 6 项及回归 46 项通过。接口、代理待验和购买入口衔接见 [MCP5 实现](内测MCP-MCP5实现.md)。
**边界**：本片实现声明的协议版本；工具集合按第 4 节固定，未接入 Agent 展示页面。
**验收**：以协议客户端完成初始化/发现/调用；无 Origin 的有效 Bearer 请求成功，错误 Origin 和无凭证请求拒绝；通知/GET/协议版本响应正确；耗尽不交付内容，记次失败不返回成功；代理保留凭证和协议头。实际 HTTP 客户端与 SQLite 验证已通过；命令及 Nginx 运行待验见 [验证记录](performance/mcp5-20261011/README.md)。

### MCP6 — Agent 自主使用信息

**实现**：2026-10-11 已补齐初始化 `instructions`、11 个工具描述与输入说明、购买恢复信息及 [MCP 接入](MCP接入.md)。定向 5 项（新增 4 项）、相关回归 10 项通过，0 失败／0 忽略；见 [MCP6 实现](内测MCP-MCP6实现.md)。
**输入与产出**：提供初始化说明、用途清楚的工具描述、结构化来源/额度和第 7 节错误恢复信息。
**落点**：`remote_mcp.rs:dispatch/usage_value`、`mcp_read.rs:tool_definitions`、`mcp_contract_tests.rs`；`docs/MCP接入.md` 作为站点接入说明的内容来源。
**边界**：本片不通过另一个托管模型教用户的 Agent 使用工具。
**验收**：协议结果中包含准确 schema、来源、扣次结果与时间；免费和已购都耗尽时目录/额度仍可查，工具集合稳定；接入说明使用占位凭证。`mcp_contract` 已通过，Windows 构建／运行命令见验证记录；真实 Agent 验收汇入 MCP9。

### MCP7 — 读者接入与来源打开

**实现**：2026-10-11 已完成账号接入窗口、本人次数、购买链接与确切来源恢复；Web 125 项、服务端 16 项和类型检查通过，见 [MCP7](内测MCP-MCP7实现.md)。
**输入与产出**：在账号页创建/撤销凭证、查看次数与购包说明；支持来源链接打开获准材料的指定原文。
**落点**：`packages/web/src/NetworkApp.vue`、`api.ts`、`network-client.ts`；新增 `components/AccountMcp.vue`、`mcp-source.ts` 及测试；`App.vue`／`ReaderPane.vue` 接入临时来源标记。
**边界**：本片不制作人工收款后台；首次接入不要求用户已打开某本材料。
**验收**：首次创建凭证、列表不回显完整值、撤销、账号切换、无材料账号、免费/付费余额与多包到期展示；未登录来源链接登录后恢复正确位置，未授权材料不可打开。运行新增组件测试、相关网络入口回归及 Web 类型检查。

### MCP8 — 运营后台次数管理

**实现**：2026-10-11 已接通账号 MCP 面板、混合收款更正与原操作恢复；[接口与验收](内测MCP-MCP8实现.md)。
**输入与产出**：管理员查询 MCP 使用、确认收款发包、赠送当月次数、追加更正，查看可解释回执。
**落点**：`apps/admin/src/pages/mcp.tsx` 及测试、`pages/users.tsx`、`pages/allowance.tsx`、`components/operation.tsx`、`lib/api.ts`、`lib/types.ts`、`lib/operations.ts`；沿 `app.tsx` 既有管理员入口。
**边界**：本片沿用现有管理员身份和人工收款方式，不加入自动扣款。
**验收**：金额与包数展示一致；超时后保留操作号并可取回原结果；两种收款用途可区分；普通读者操作被服务端拒绝；更正不重复增发或改写已消费事实。运行新增后台测试、原收款界面回归及 Admin 类型检查。

### MCP9 — 集成与新 Agent 验收

**实现**：2026-10-11 已完成；Codex CLI 0.162.0-alpha.2／2025-11-25 自主阅读及耗尽停止通过，引用和 3 条消费事实确定性核对。Server 87 项既有回归、2 项新增集成／并发用例、Web 126 项、Admin 30 项及两端构建通过；旧 MU4 10 项夹具失败单列。[实现与限制](内测MCP-MCP9实现.md)、[证据](performance/beta-mcp/README.md)。
**输入与产出**：在真实多用户宿主、数据库和已发布测试材料上完成完整路径，记录客户端与协议版本。
**落点**：`mcp_acceptance_tests.rs`、`mcp-source.integration.test.ts`、`scripts/mcp9-codex-acceptance.mjs`、`scripts/verify-mcp9-agent.mjs` 与 `docs/performance/beta-mcp/`。
**验收**：两个账号分别接入；新 Agent 只拿接入说明与阅读问题，完成选书、定位、读取及可解析引用；空结果换词、原文翻页、免费转付费、全部耗尽、购包后继续、材料撤权均有可观察结果。额度使用接近耗尽的测试夹具，不消耗正式付费次数。
**回归**：受影响的认证/材料/收款测试、本机 Book MCP、Web/Admin 测试与构建；不以模型自评代替引用解析和账本断言。小规模并发读取测实际耗时与返回量，用于确定首批部署容量。

### MCP10 — 发布与运行单

**输入与产出**：形成 Linux 数据备份、schema 升级、服务/代理发布及恢复操作步骤；发布版本与实现状态分开记录。
**落点**：`scripts/linux/nginx-multi-reader.conf.example`、既有构建/发布脚本，拟新增 `docs/内测MCP-发布运行单.md`。
**验收**：备份恢复在临时目录完成；升级后旧账号、材料和模型费用仍可读取；经正式 HTTPS 代理验证 MCP，使用明确标记的赠送次数验证扣次。真实收款只登记已确认的付款。
**恢复**：发布前验证旧服务对新 schema 的行为；不能读取时采用维护窗口恢复成套备份。开始真实 MCP 购包后，不得通过覆盖旧备份丢弃已发生的收款与消费，按运行单保留新增事实并执行修复。

## 9 完成判据

MCP1–MCP9 完成后，内测账号可通过受支持客户端独立接入，用自己的 Agent 阅读获准版本；用量与收款能解释，来源能定位，失败行为与停止条件确定。MCP10 完成后才把状态改为已上线。

每次运行验收前说明它检测的具体失败，以及失败后要修改的对应路径。单片通过后继续下一片；最终回归只覆盖本次触达路径及明确共享的既有功能。

## 10 已知限制与后续触发条件

- MCP1 凭证、MCP2 读取、MCP3 免费次数、MCP4 付费包与 MCP5 远程协议已实现；MCP6–MCP9 已完成 Agent 自述、接入／管理界面和集成验收；发布验收沿 MCP10 实施。
- 首版面向可配置请求头的远程 MCP 客户端；Codex CLI 0.162.0-alpha.2 完整阅读已验收，Claude Code 2.1.196 仅初始化／发现通过（模型凭证 401）。仅支持 OAuth 的客户端需要后续独立接入工作。
- 原文按服务端成功提交记次计量；响应传输中断后重新调用可能再次消费。首版不承诺网络交付恰好一次。
- 1,000 次月度免费额度与 10 元/1,000 次是内测初始政策。真实使用、付费意愿与托管成本形成证据后调整后续政策。
- 人工收款覆盖当前内测规模；核对与开通成为实际运营瓶颈时，再接自动支付。模型代答、私人资料构建等能力另行定价与授权后扩展。
