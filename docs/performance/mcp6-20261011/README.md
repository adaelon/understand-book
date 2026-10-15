# MCP6 验证记录

日期：2026-10-11。实现见 [MCP6](../../内测MCP-MCP6实现.md)，用户说明见 [MCP 接入](../../MCP接入.md)。

## 运行目的

- `mcp_contract` 检测初始化／工具用途缺失、描述追加损坏共享 schema、按返回定位无法读回原文、来源坐标或余额不符，以及耗尽后发现变化或恢复信息缺失。失败修正对应描述、响应或新测试中的错误合同假设。
- 远程协议与读取回归检测发现修改影响调用分发、原有授权／分页／扣次行为。失败修正读取或协议接点，不调整已成立的断言。
- 使用真实 TCP HTTP、临时 SQLite 与发布材料；限流映射使用确定性错误输入。没有调用线上服务、模型或实际收款。

## 结果

| 运行 | 结果／证据 |
| --- | --- |
| 测试构建 | 通过，3m 19s；[build.log](build.log)；保留既有 ts-rs 属性警告 |
| `mcp_contract` | 5 通过、0 失败、0 忽略；含 4 项新增和 1 项既有概念合同用例；[日志](mcp-contract.log) |
| 原远程协议 6 项 + 读取 4 项 | 10 通过、0 失败、0 忽略；[日志](regression.log) |

新文档的本地链接均可解析。

首轮新测试有两处错误假设：检索命中实际提供 `start_lid/end_lid`，不是 `lid`；共享空输入 schema 省略 `properties`，远程追加字段后去掉字段会留下空对象。核对 `TextOccurrence` 和 `input_schema` 后分别修正读取参数与比较中的空对象处理，保留所有类型／枚举／边界比较；同时在工具说明与接入文档写明实际检索到原文的参数映射。[首轮日志](mcp-contract-initial.log)。

## 重跑

```powershell
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
cargo rustc -j 1 -p server --lib --profile test -- -C link-arg=/DEBUG:NONE
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_contract
& 'target/debug/deps/server-6b365f9831000c84.exe' remote_mcp::tests::remote_mcp mcp_read::
```

测试程序路径取本次构建产物，后续以实际 Cargo 输出为准。沿 MCP5 关闭测试调试信息与增量编译，TEMP 使用 D 盘；项目构建配置未改。修正测试后的首次重编译与本机其他编译同时运行，触发 LLVM 内存不足；停止已报错的本任务 rustc，改为 `-j 1` 重试。[环境失败日志](build-memory-failure.log)。

## 已知限制

- E 盘开始时仅约 110 MB 可用。核对路径后，将 `target/debug/deps/UnderstandBook.pdb`、`server-e6140f0c20c22aad.pdb` 两个旧调试符号文件移到 `D:/codex-build-cache/understand-book-reading-share-mcp6/preserved/` 保留，为链接腾出空间。
- 真实 Agent 客户端沿 MCP9 验收，HTTPS 代理沿 MCP10；MCP7 的来源打开与购买页面尚未实现。
- 未重跑无关旧本机 MCP schema 断言及 MU4 JSONL 夹具失败；本片不修改它们。
