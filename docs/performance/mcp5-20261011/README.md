# MCP5 验证记录

日期：2026-10-11。实现与接口见 [MCP5](../../内测MCP-MCP5实现.md)。

## 运行目的与结果

- `remote_mcp` 检测协议／身份边界、读取与扣次顺序、额度耗尽仍交付、记账失败仍交付、重复消费等具体失败；失败应修复 `remote_mcp` 或其 Host／读取衔接。最终 **6 通过，0 失败，0 忽略**。
- 回归检测新读取准备步骤改变原分页／授权、专用路由影响网页 Cookie／CSRF、消费／收款与旧 schema 行为受损；失败应回到相关接点修复。**46 通过，0 失败，0 忽略**。
- 新增用例使用真实 HTTP、发布材料和 SQLite；没有调用付费模型或线上收款。

| 证据 | 内容 |
| --- | --- |
| [build.log](build.log) | 最终测试构建成功；保留既有 ts-rs 属性警告 |
| [remote-mcp.log](remote-mcp.log) | 新增六项最终运行，包含 GET 不支持版本头的拒绝 |
| [regression.log](regression.log) | MCP1–MCP4、控制库、ADM2／ADM3，共 46 项 |

## 重跑

沿 MCP4 的进程级设置关闭调试信息，未修改项目构建配置。使用 D 盘 TEMP 避免 C 盘临时空间不足：

```powershell
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
cargo rustc -p server --lib --profile test -- -C link-arg=/DEBUG:NONE
& 'target/debug/deps/server-6b365f9831000c84.exe' remote_mcp
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_access:: mcp_read:: mcp_usage:: tests::mcp_pack_api_tests tests::adm2_tests tests::adm3_tests control_store::
```

测试程序路径是这次 Cargo 的产物；后续以实际输出为准。首轮编译发现测试夹具写错 Cookie 常量，修正为现有 `auth::COOKIE` 并使用 `auth::csrf`；最终编译和用例均通过。

E 盘构建空间不足以容纳后续链接前，核对绝对路径后将以下可重建旧缓存从 `target/debug/incremental/` 移到 `D:/codex-build-cache/understand-book-reading-share-mcp5/preserved/` 保留：`UnderstandBook-27a768nir75ya`、`server-2k2hfec8im34c`。未删除工作资料。

## 验证范围

只运行上述定向测试。原本机 MCP 过时 schema 断言、旧 MU4 JSONL 样本失败、本机安装包及真实 Agent 体验未复验。Nginx 模板已核对精确 `/mcp` 的凭证／协议头配置；本机没有可用 Nginx 或 WSL，未执行代理运行测试，正式 HTTPS 代理验收沿 MCP10。
