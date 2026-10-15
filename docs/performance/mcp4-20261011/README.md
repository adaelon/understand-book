# MCP4 验证记录

日期：2026-10-11。实现合同见 [MCP4](../../内测MCP-MCP4实现.md)。

## 本次构建环境

默认调试构建先后遇到 C 盘临时目录、E 盘产物空间不足，均未进入测试。清理缓存操作被执行策略拒绝，未执行删除；随后将工作树 `target/debug/incremental/server-10eut6w4327og` 完整移动到 `D:/codex-build-cache/understand-book-reading-share-mcp4/preserved/` 保留，E 盘恢复约 3.8 GB 空间。D 盘新构建目录运行辅助程序时报 OS error 5，因此最终沿原工作树 `target/` 编译。

后续验证使用以下进程级环境变量；未修改 Cargo 项目配置：

```powershell
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
```

`CARGO_PROFILE_TEST_DEBUG=0` 关闭调试信息，保留测试断言。D 盘保留目录和构建尝试只含可重建编译产物；原工作资料未移动。

MSVC 默认生成 PDB 时遇到 LNK1140；`/PDB:NONE` 未消除该失败，后续采用链接器帮助明确支持的 `/DEBUG:NONE` 关闭调试输出。最终构建命令及定向运行结果见下节。

## 结果

构建通过：[build.log](build.log)。

```powershell
cargo rustc -p server --lib --profile test -- -C link-arg=/DEBUG:NONE
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_pack
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_usage::tests::mcp_usage control_store:: mcp_access:: mcp_read:: tests::adm2_tests tests::adm3_tests
```

测试程序路径为这次 Cargo 产物；后续重编译时以实际生成路径为准。

| 运行 | 结果 | 证据 |
| --- | --- | --- |
| MCP4 定向 | 8 通过／0 失败／0 忽略 | [mcp-pack.log](mcp-pack.log) |
| 受影响回归 | 38 通过／0 失败／0 忽略 | [regression.log](regression.log) |

定向覆盖和边界见 [实现合同](../../内测MCP-MCP4实现.md#验证)。回归包含旧 schema 升级及与新库一致性、原免费额度／赠送／并发、凭证／材料读取、管理员权限／回执和模型收款／更正。没有运行全库测试、界面验收或远程 MCP 协议验收。
