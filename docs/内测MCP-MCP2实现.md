# MCP2 材料读取与来源分页

日期：2026-10-11。状态：MCP2 已完成，尚未提交或发布。

依据：[切片方案 §4／MCP2](切片方案-内测MCP与调用额度.md)、[ADR-0162 §1／§2](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。沿用已确认的账号、PublishedBookRef、纯读取和 UTF-16 来源合同。

## 调用入口

```text
McpAccess.authenticate(本次请求的 Bearer) → McpIdentity
McpRead::new(&共享 PublishedLibrary Mutex, 配置中的站点 Origin)
  .call(&identity, tool_name, arguments) → Result<Value, ToolError>
```

`McpRead` 不保存身份或材料授权。调用方每次先验证 MCP 凭证；内容读取经 `PublishedLibrary::load` 检查启用账号及确切发布授权，包含缓存命中和续页。取得已发布 Book 后释放库锁，再计算读取结果。过程不创建 UserRuntime、ReaderWorkspace 或模型调用。

`tool_definitions()` 返回目录和九个内容工具的 schema。纯读取参数沿用 `book-tool-contracts`；先取出远程专属字段，再调用原有严格输入校验器。材料引用只接受 `book_id` 和 `publication_id`，不接受指定用户或目录。

| 工具 | 原有参数 | 远程附加参数 |
| --- | --- | --- |
| `book_list` | — | `after?: PublishedBookRef`、`limit?: 1..50`（默认 20） |
| `book_text` | `lid`、`end_lid?` | `published_book_ref`、`start_utf16?` |
| `book_search_text` | `query`、`match_mode?`、`scope?`、`order?`、`cursor?`、`page_size?` | `published_book_ref`、`page?` |
| `book_context` | `lid`、`granularity?`、`k?` | 同上 |
| `book_concept` | `query`、`anchor_lid?`、`limit?` | 同上 |
| `book_structure`、`book_guide_path` | `at?` | 同上 |
| `book_paper_metadata`、`book_paper_lexicon` | 无 | 同上 |
| `book_paper_reading_guide` | `mode?`、`stage?` | 同上 |

## 目录与材料能力

`PublishedLibrary::list_manifests_page` 按 `(book_id, publication_id)` 排序，SQL 每次最多取 `limit + 1` 条已授权记录。目录返回 `items`、`complete` 和 `next_calls`，每项包含明确发布引用、`name`、默认发布标志及 `available_tools`。名称取已构建论文标题，无标题时使用 book_id。

目录读取发布清单及必要标题，不加载 Book 或占用正文缓存。可用工具依据已发布附件确定；结构、导读、论文元数据或词表缺失时不列为可用能力。直接调用缺失能力返回 `MCP_CAPABILITY_UNAVAILABLE`。

## 原文分页

`Book::text_page(lid, end_lid, start_utf16)` 直接遍历选中源范围的 UTF-16 单元，最多解码 8,000 个 Unicode 标量值；不构造整个范围的字符串。emoji 等非 BMP 字符保持完整，一个此类字符占两个 UTF-16 单元。

```text
data = {
  text,
  source_range: { start, end },       // 本页绝对 UTF-16 坐标，[start, end)
  requested_range: { start, end },    // 原请求 LID 范围
  next_start_utf16: number | null
}
```

第一页省略 `start_utf16`；后续使用返回的 `next_calls[].arguments`，保留材料版本和原 LID 范围。页尾就是下一页起点，间隔、换行和非 BMP 字符均原样保留。反向 LID 范围、越界位置及代理对中间位置返回校验错误；末页不再返回续页调用。

## 结构与检索续页

非原文工具保留原工具投影在 `data` 中，各层数组默认返回前 20 项。每个被省略的集合都生成明确的续页调用：

```json
{
  "name": "book_structure",
  "arguments": {
    "published_book_ref": { "book_id": "sample", "publication_id": "publication" },
    "page": { "path": "/throughlines", "offset": 20, "limit": 20 }
  }
}
```

`page.path` 是原投影的 JSON Pointer；续页的 `data` 是该集合的分片，`data_path` 标识原集合，`total_items` 表示其总项数。嵌套集合使用原位置，例如 `/throughlines/0/lids`，其继续位置不随外层页改变。可直接照抄 `next_calls`；无需客户端生成游标或猜测缺失项。

`book_search_text` 保留原检索游标与总命中数，并把下一批命中的调用也放入 `next_calls`。顶层 `complete` 表示本次投影没有待返回的集合分片／检索批次；概念候选自身的 `matched_count`、`truncated` 和 `limit` 继续表达候选召回范围，不能把候选预览当作完整原文。

## 来源

所有内容结果携带 `published_book_ref`、`sources`、`complete` 和 `next_calls`。原文来源给出本页准确范围；其他结果从返回的 LID 指针及嵌套页所属字段的证据位置派生来源。

```text
Source = { published_book_ref, start_lid, end_lid, source_range, source_url }
source_url = 配置中的 Origin + /?
  book_id=...&publication_id=...&start_lid=...&end_lid=...
  &start_utf16=...&end_utf16=...
```

URL 查询参数由 URL 编码器生成；坐标均为规范原文的绝对 UTF-16 坐标。检索数据仍包含每次命中的精确位置，结构来源指向对应 LID 的原文范围。

## 错误

| 错误码 | 含义 |
| --- | --- |
| `MCP_READ_INPUT_INVALID`、原 `BOOK_TOOL_INPUT_INVALID` | 远程参数或原工具参数错误 |
| `MCP_MATERIAL_UNAVAILABLE` | 无权访问或确切发布不可用，重新查询目录 |
| `MCP_CAPABILITY_UNAVAILABLE` | 指定发布没有该已构建能力 |
| `MCP_TOOL_UNAVAILABLE` | 工具不属于本片纯读取集合 |

来源范围、LID、搜索游标及存储错误继续使用原模块的结构化 ToolError。工具错误尚未转换为远程 HTTP／JSON-RPC 响应。

## 验证

| 运行 | 结果与日志 |
| --- | --- |
| `cargo test -p read-tools --lib mcp_text_page` | 3 通过，0 忽略；[日志](performance/mcp2-20261011/text-page.log) |
| `cargo test -p server --lib mcp_read` | 4 通过，0 忽略；[日志](performance/mcp2-20261011/mcp-read.log) |
| `cargo test -p read-tools --lib` | 171 通过，1 项因缺真实书籍样本失败；[日志](performance/mcp2-20261011/read-tools-regression.log) |
| `cargo test -p server --lib -- mcp::tests tests::mu3_tests search_text_rest_mcp_and_resident_contracts_have_parity search_text_real_book_mcp_pages_match_the_core_result` | 27 通过，2 失败（旧 schema 断言、缺真实样本）；[日志](performance/mcp2-20261011/server-regression.log) |
| `cargo test -p read-tools --lib search_text_real_book_replays_all_formula_occurrences` | 补齐样本后 1 通过；[日志](performance/mcp2-20261011/real-book-read-tools.log) |
| `cargo test -p server --lib search_text_real_book_mcp_pages_match_the_core_result` | 补齐样本后 1 通过；[日志](performance/mcp2-20261011/real-book-mcp.log) |

新增用例覆盖真实 SQLite 的账号隔离、目录完整续页、同书不同发布的实际元数据与来源隔离、缓存撤权、账号停用、私人目录／工作区零创建，以及跨 LID、非 BMP 字符、页长与 UTF-16 坐标。结构、导读、论文元数据、词表、论文阅读指引及概念投影经全部续页拼回后与原完整投影逐值相同；检索在两种页长下遍历全部续页分支，无遗漏或重复。

真实样本从主工作树 `E:/allwork/download/agent/understand-book/.understand-book/quantification-essence` 复制 `base.json` 和 `source.txt` 到本工作树同一忽略目录；两项复验检查 32 次真实公式命中与五页读取。全库测试触发的 TypeScript 导出仅改变换行，已恢复这些本轮测试生成的文件。

## 已知限制

- 原 `mcp::tests::mcp_tool_characterization` 在 `mcp.rs:1109` 仍断言 `granularity.enum`。现行 `book-tool-contracts` 将可空字段表示为 `anyOf[0].enum`，其契约用例已明确这一形状；本轮未修改这两个文件，也未改写旧断言。受影响的发布材料测试及其余本机 MCP 用例通过。
- 远程 `/mcp` 协议与记次在 MCP5 接入；本片提供库入口，未开放 HTTP 内容端点、提交或发布。
- 来源 URL 的 Reader 打开、登录后恢复和无权限提示沿 MCP7 实施，当前只交付版本与坐标合同。
- 结构／关系／论文结果复用现有确定性投影后分页，续页会重新计算投影；原文分页直接作用于源范围。
- 本机 stdio 工具合同保持原样。Windows 上无 `--lib` 的定向命令在启动无关二进制测试时遇到 OS error 87；本片使用明确的库测试入口。
