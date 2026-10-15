# RS5 书架与阅读接续卡

2026-10-10。功能／Web 完成，接续 RN1–RN4、RS1–RS4 未提交工作树；未提交、未发布。

## 已交付

主窗口刷新后，书架顶部显示该窗口已有现场的确切材料／发布、已保存位置的原文标签与短摘录，以及所选聊天在同发布下的上次问题。没有问题时只展示位置，没有保存现场时继续使用正常选书网格。封面复用 BookCover；接续区在窄屏重排。

`GET /workspaces/:id/resumption` 经过现有用户、workspace 和材料授权，读取原持久 checkpoint 与聊天事实，不装载 resident workspace、不挂接、不推进 generation／revision／checkpoint_seq。问题取自确切发布，旧记录缺少发布绑定时不推断；摘要为短预览，正文和聊天保持原存储。

点击“继续阅读”进入原恢复队列，读取最新 workspace 并 attach，恢复材料、发布、聊天、读位和布局。卡片展示后若材料／发布／聊天已变化，停止挂接、更新卡片并提示再次继续；请求失败保留重试入口。另一窗口的阅读现场不参与选择，重复窗口的挂接冲突仍沿原分叉合同。

网络阅读页“打开书”进入同一书架。当前 Reader 和未发送文字保持挂载；笔记草稿沿既有离开处理。选择另一材料沿原 book/open 命令，冷启动选书沿原 workspace 创建。书架可见时重新核对身份、书目和接续摘要；授权撤销、账号变化及晚到响应沿现有作用域清理。

## 验证

| 检查 | 实际结果 |
| --- | --- |
| 服务端 RS5 | 2 项通过；冷现场预览前后直接比较 generation／revision／checkpoint／checkpoint_seq，完全相同；覆盖另一窗口、私人归属、授权撤销、空聊天和聊天删除 |
| Workspace 回归 | 18 项通过；含服务重启后先预览再以原 stamp 挂接，恢复同发布／聊天／读位／布局；重复窗口分叉、接管、空闲恢复、CAS、旧运行作用域、演示与笔记访问；`workspace-regression.log` |
| Web 定向 | 5 文件 34 项通过；新增 5 项覆盖刷新只读、同书多发布、当前窗口指针、原阅读页与未发送文字保持、无记录／无问题、授权变化、身份清理、卡片出现后发布变化；`web-tests-final.log` |
| Chromium | 320／390／768／1440 px 共 4 项通过；真实组件与封面排版、刷新后卡片、无横向溢出、主要按钮可达、预览只发 GET、显式点击才 attach、冲突可重试；`browser-final.log` |
| Web 类型检查与生产构建 | `VITE_MULTI_USER=1` 构建通过；既有 PDF 混合导入和包体积提示保留；`web-build.log` |

实际截图已查看：[320 px](bookshelf-320.png)、[390 px](bookshelf-390.png)、[768 px](bookshelf-768.png)、[桌面](bookshelf-1440.png)。

聊天删除测试的初始样本仍为运行中，原有 CHAT_BUSY 正确拒绝删除；样本补齐取消终态后通过，业务删除规则保持。失败记录在 `server.log`，最终结果在 `server-final.log` 与 `workspace-regression.log`。

复现入口：

```text
cargo test -p server mu5_ --lib
pnpm --filter @understand-book/web exec vitest run src/NetworkApp.resumption.test.ts src/NetworkApp.recovery.test.ts src/NetworkApp.chat.test.ts src/NetworkApp.recap.test.ts src/network-client.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test --config playwright.resumption.config.ts --workers=1
# 设置 VITE_MULTI_USER=1 后：
pnpm --filter @understand-book/web build
```

## 已知限制与接续

- 卡片以当前窗口已有 workspace 指针为入口；没有该指针的新窗口只显示正常书架。
- 位置摘要展示已保存原文位置，完整 PDF 现场与布局仍沿原 checkpoint 恢复。问题最多预览 240 字，位置摘录最多 140 字；旧聊天缺少确切发布时不显示问题。
- 浏览器用固定业务响应验收实际 UI；成功恢复、服务重启、持久状态与授权由组件集成和真实服务端存储测试覆盖，未进行线上发布验收。
- RS3 原生保存、实体手机及 RN4 平台待办继续保留。本轮未恢复 Computer Use。
- 下一切片为 [RS6 已交付回答分享](../../切片方案-阅读分享与安静呈现.md#rs6-已交付回答分享)。
