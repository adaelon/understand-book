# Markdown 自动链接导致构建中断

2026-10-26，《庄子》标准深读在 Pass1 完成后准备 profile_sidecar 时失败。
请求 ID：`abreq1_c341bb8daa59404e1393b0511d8ede620ba2e3c913faa8e18713d771f47169a7`。

根因：`mdast-util-gfm-autolink-literal` 的 AST 后处理把含转义符或 HTML 实体的链接文本替换为没有 `position` 的节点。`md-adapter.ts:collectDisplaySegments` 依赖源码位置，因此抛出 `Markdown parser node lacks a valid source position: text`。

修复：在源码定位解析中禁用 GFM 的 AST 后处理，保留 tokenizer 的原节点及原生自动链接解析。显示文本继续正常解码，源码跨度保持原文坐标。

验证：新增 5 个回归样例，修复前均复现相同错误。修复后运行 `md-adapter`、`profile-sidecar-context`、`source-alignment-evidence` 三组测试，26 项全部通过。通过现有 `build-sidecar.mjs` 打包并替换本机引擎后，同一 invocation 成功越过故障点，Pass1 确认 146/146，进入 profile_sidecar 并持续提交任务。

本机旧引擎备份：`E:\allwork\Understand Book\understand-book-build.before-zhuangzi-parser-20261026-2303.exe`。
构建继续使用原计划与 invocation，关闭 Pass2、并发上限 8，不设 token 上限。
