# RS4 阅读回顾呈现与选项分享

2026-10-10。功能／Web 完成，接续 RN1–RN4、RS1–RS3 未提交工作树；未提交、未发布。

## 已交付

回顾展示真实聊天标题、从对话开始到原截点的范围、当前查看重点和四类事实。点击条目或“重点查看”切换查看焦点；“选入分享”独立勾选内容，未选内容不进入图片，空回顾不能生成分享。

`recapShareSource` 将所选条目的文字、状态、来源和不可用说明复制为页面内分享素材，固定原聊天、`through_seq` 和时间。分享编辑只改标题、感想与配色，使用阅读回顾卡版式；原问题和“已回答／待继续”等状态保持事实身份。长内容沿现有分页输出，预览与逐页保存使用同一 PNG Blob。

刷新回顾只更新四区，已经打开的分享草稿保留原内容、截点与编辑；关闭后重新生成或“重新选材”再采用当前回顾。来源和成果动作继续携带原回合、截点与原对象；跨发布读取沿既有 NetworkApp 导航。关闭回顾、聊天／身份／发布变化清理草稿和图片。

同一 workspace 的普通 revision 更新保持回顾和分享。作用域监听分别比较聊天 ID 与 readerKey，避免网络状态对象更新时因新数组引用而误清空草稿；`revision-before.log` 记录修复前失败，`recap-final.log` 覆盖修复与真实作用域切换。

修复下载后 Esc 同时关闭分享与回顾的问题：分享弹窗处理自己的 Escape，RightRail 在分享弹窗打开时不接管该按键；返回后保留回顾勾选并恢复生成按钮焦点。原生宿主 `host::now_ts` 写入毫秒，旧 `recapTime` 按秒解析会显示 58743 年；已先复现再统一毫秒、秒和 ISO 的显示，页面与图片共用修正。

## 验证

| 检查 | 实际结果 |
| --- | --- |
| Vitest 定向 | 6 文件 63 项分轮通过：`unit-final.log` 为 61 项；补时间与普通 revision 用例后 `recap-final.log` 为 2 文件 10 项，其中新增 2 项 |
| 数据与组件 | 四区／多条目、聚焦与选择分离、空回顾、不可用来源与成果、刷新失败、旧草稿与编辑冻结、重新选材、聊天／身份／发布清理、状态身份、内部导航字段不渲染 |
| 原导航 | RightRail 原文／成果／编辑后笔记定位回归；NetworkApp 原发布与当前发布往返保留确切聊天、目标与截点，见 `navigation.log` |
| Chromium 回顾 | 7 项分轮通过：原 JL9 两视口见 `browser.log`，RS4 四宽度 320／390／768／1440 px 与长内容分页见 `browser-rs4.log` |
| 实际导出 | 四宽度均生成 2 页并逐页下载，与对应预览逐字节相等；未发送聊天草稿保持；打开／刷新／导出的业务请求仅两次 GET recap，无写入或模型请求 |
| 完整性 | 长回顾各页正文合并与原文逐字符相等；续页保留事实状态、聊天范围和原截点，无未选内容、内部引用或掌握判断 |
| 原分享回归 | RS1 笔记分享两视口 2 项通过，含预览／下载／阅读连续性；`share-regression.log` |
| Web | 最终 vue-tsc 与 Vite 生产构建通过，保留既有 PDF 混合导入和大包提示；`web-build-final.log` |

首轮单测使用的 Teleport 替身在父组件更新时重新挂载子组件；涉及草稿生命周期的用例改用真实 Teleport，并验证同一个弹窗保留标题与内容。浏览器也验证了刷新时保留编辑。早期浏览器定位沿用了旧菜单入口及 select 标签选择器，已按实际“问答操作”与 combobox 修正。失败日志与后续通过日志均保留。

已查看实际成图：[第一页](recap-page-1.png)、[第二页](recap-page-2.png)。界面：[320 px](recap-320.png)、[桌面](recap-1440.png)、[320 px 分享](share-320.png)、[桌面分享](share-1440.png)。

复现入口：

```text
pnpm --filter @understand-book/web exec vitest run src/session-recap.test.ts src/components/SessionRecap.test.ts src/components/ShareImagePanel.test.ts src/reading-share.test.ts src/components/RightRail.test.ts src/NetworkApp.recap.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/session-recap.spec.ts --workers=1
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep RS1 --workers=1
pnpm --filter @understand-book/web build
```

## 已知限制与接续

- 浏览器业务响应使用固定回顾事实，真实执行界面、字体、分页、PNG 编解码与文件下载；跨发布往返使用组件集成测试，未进行线上服务验收。
- Windows 原生保存、安装后的 Release 行为和实体手机验收继续沿 [RS3 待办](../reading-share-rs3-20261010/README.md)；本轮未恢复 Computer Use。
- 分享草稿和图片保存在当前页面；刷新页面、关闭回顾或切换现场后结束。单条不可分内容超出版面时沿既有导出错误提示重新选材。
- 下一切片为 RS5 书架与阅读接续卡，合同见[实施方案](../../切片方案-阅读分享与安静呈现.md#rs5-书架与阅读接续卡)。
