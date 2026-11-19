# 本地版功能交付

围绕受影响的用户路径完成实现、验证与记录。先确定用户可观察的结果，再选择能识别该路径失败的已有测试；失败时修正根因并复验。界面修改还需打开实际页面或应用操作并查看效果。

共享合同使用 `pnpm verify:extractor`；Core 使用 `pnpm --filter @understand-book/core test`；Reader 使用 `pnpm --filter @understand-book/web test`。Rust 修改运行对应 crate 的定向测试。桌面资源和安装路径沿[本地应用验证入口](apps/local-desktop/README.md#验证入口)。

插件源码合同使用 `pnpm test:maintenance-plugin`；Book MCP 使用 `pnpm build:book-mcp` 构建。完整测试入口为 `pnpm test`，产品构建入口见 [EDITION.md](EDITION.md)。

历史真实试验需要显式输入：`MCP_SOURCE_EVIDENCE` 指向 HTTP 来源证据，`BSR4_EVIDENCE_DIR` 指向主题回放目录，`SR6_RANKING_INPUT` 指向已保存向量。未提供时对应历史回放显示跳过，普通原创样本继续运行；模型试验需要自己的服务、材料和预算。

提交结果应区分执行通过、语义通过、失败与未执行，写明受影响入口、证据和剩余问题。原始材料、账号、凭据及本地运行证据留在数据目录；可公开的回归输入放在对应模块的 fixtures 中。
