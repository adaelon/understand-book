# MCP6 Agent 自主使用信息

日期：2026-10-11。状态：MCP6 已实现，未提交／发布。依据：[切片方案 §7／MCP6](切片方案-内测MCP与调用额度.md)、[ADR-0162 §6](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。

## 实施进度

- [x] 初始化 `instructions` 给出选材、定位、读证据、引用、计次与恢复路径。
- [x] 11 个工具说明用途、输入、分页／来源与计次；内容参数沿共享合同补充字段含义。
- [x] [MCP 接入](MCP接入.md)提供地址／凭证占位符、请求头、阅读路径和失败恢复。
- [x] `mcp_contract` 定向测试及读取／远程协议回归。
- [x] 更新切片表、代码链路和 checkpoint。

## 实现合同

`remote_mcp.rs:dispatch` 在初始化结果中返回 `INSTRUCTIONS`。Agent 先从免费目录取得确切发布和可用能力，再定位并读取证据；来源坐标为绝对 UTF-16 左闭右开范围。续页复制 `next_calls`，多个集合分支分别处理；`complete` 不覆盖候选自身的召回上限。无来源投影不能作为已核实原文引用。

`mcp_read.rs:tool_definitions` 复用 `book-tool-contracts` 的 description／use_when／do_not_use_when，将 `book.text` 统一为远程别名 `book_text`，追加远程参数、错误、来源、额度和续页说明。共享参数只增加 description，保留其必填项、类型、枚举和边界；MCP 专属引用与分页字段沿 MCP2 合同。

`mcp_usage` 的工具描述解释免费查询、账号共享余额、批次到期及停止规则。`usage_value` 的 `top_up_instruction` 提供每包价格／次数、期限、免费优先和联系管理员方式。所有余额、来源和错误字段继续使用 MCP5 实际响应；数据库仍为 schema 12。

## 验证

合同测试放在 `mcp_contract_tests.rs`，复用 `remote_mcp_tests.rs` 的真实 HTTP／SQLite／发布材料夹具，覆盖初始化和 11 个工具的发现、共享 schema 保持、从目录取得标识到检索／两页原文／来源 URL 的完整调用，以及已购批次时间、耗尽后发现稳定和各类失败恢复。限流错误映射以直接确定性用例验证。

`mcp_contract` 定向 **5 通过、0 失败、0 忽略**（4 项新增、1 项既有）；读取与远程协议回归 **10 通过、0 失败、0 忽略**。运行结果和环境记录见 [MCP6 验证](performance/mcp6-20261011/README.md)。

## 已知限制

- 尚未提交／发布；真实 Agent 客户端验收沿 MCP9，HTTPS 代理运行沿 MCP10。
- MCP6 验证时 `top_up_url` 为 null；现已由 [MCP7](内测MCP-MCP7实现.md) 接通购买页与 Reader 来源打开，并更新初始化说明和相关断言。
- 没有更改成功记次边界：消费提交后断连仍保留记次，重新成功请求再次消耗次数。
