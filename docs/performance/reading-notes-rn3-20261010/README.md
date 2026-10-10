# RN3 统一编辑与记录入口验收

2026-10-10，Windows 本地 Chromium。RN3 已完成；RN4 接续检索、排序与完整平台连续性验收。

## 已交付

原文选区、已交付回答、演示现场和旧笔记使用 App 的同一个页面内草稿与 NoteEditorPanel。自己的文字与原文／助手摘录分区，摘录只读；旧混排正文完整进入编辑区，身份为“阅读笔记”；保存保留 Markdown 缩进和首尾换行。Notes、Markdown 批注和 PDF NoteCard 共用 NoteDetail 内容投影，编辑统一调用原子 replace。

原文创建传入 `note.association=selection`，保留摘录等于原 `raw_quote`。回答创建传入原 `session_id + turn_id`，不要求正文落点；正文放置继续沿独立 reanchor 入口。`answer-note-selection.ts` 使用现有 Markdown 源映射，将渲染规范化后的选区转换成已交付 Markdown 的连续原片段，复验转换后内容；覆盖下划线强调、有序列表、代码语言标记、表格、公式、链接与多段引用。来源按钮不进入摘录。

草稿保存材料作用域与固定关联，收起、外侧点击、翻页及同材料换聊天可续。新建另一条、切换材料／发布和主动退出登录先提供“保存并继续／放弃并继续／继续编辑”；失败保持文字和关联。保存期间暂时只读，避免返回时覆盖新输入；输入法组合输入不触发快捷保存。刷新／关闭接入 beforeunload。身份或材料失效立即清空私人草稿与待定切换；保存和注释列表的晚到结果按 readerKey／材料作用域过滤。

移动编辑区跟随 visualViewport 的高度与偏移调整，输入和保存可达；摘录区保留最小高度并可滚动。预览按需展开。

## 验证

| 验证 | 结果 |
| --- | --- |
| App.startup | 30 通过；四类草稿接点、回答直接保存、固定原聊天、旧笔记编辑、切换三选项、失败保留、身份清理与晚到保存失败 |
| RightRail / PDF placement | 42 + 1 通过；对话与来源回归、明确正文放置 |
| NetworkApp chat / recap / recovery | 6 + 1 + 6 通过；真实父子组件换聊天、退出前失败阻断与原关联保存、发布跳转及身份恢复 |
| answer-note-selection / NoteEditorPanel | 7 + 2 通过；源片段格式转换、跨片段拒绝、输入法生命周期、摘录单独保存与提交期间只读 |
| Chromium PDF／Markdown／回答／旧笔记 | 3 通过；实际 App、真实 PDF 文字层与 DOM 选区，服务响应使用固定样本；保存失败重试、PDF 批注编辑、切材料取消／保存，以及旧混排正文 |
| Chromium RN2 演示回归 | 1 通过；真实 Rust 路由与私人存储：V2 固定现场、失败重试、V3、删除原聊天、磁盘重开、SVG 资源、显式恢复与原现场追问 |
| Web build | vue-tsc 与 Vite 生产构建通过，保留既有 PDF 混合导入和大包提示 |

Vitest 共 95 项通过。旧来源测试补齐 RN2 已增加的可选 `note_mem_id` 参数断言；网络回顾测试的 App 替身补齐离开草稿接口。浏览器回答样本补齐实际回答必填的 profile_usage；这些调整不改变产品合同。

复现：

```text
pnpm --filter @understand-book/web exec vitest run src/App.startup.test.ts src/components/RightRail.test.ts src/components/RightRail.pdf-note-placement.test.ts src/NetworkApp.chat.test.ts src/NetworkApp.recap.test.ts src/NetworkApp.recovery.test.ts src/answer-note-selection.test.ts src/components/NoteEditorPanel.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep "RN3|resolved real PDF selection performs" --workers=1
pnpm --filter @understand-book/web build
```

演示回归按 [RN2 宿主命令](../reading-notes-rn2-20261010/README.md) 启动后运行 `reading-notes.spec.ts`，本次宿主已正常停止。[演示恢复画面](rn2-regression.png)；[缩小可用视口后的编辑区](rn3-keyboard-viewport.png)。

## 已知限制

- 移动验证使用 Chromium 390×844 和 390×370 可用视口，覆盖缩小视口下输入／保存；未将它记为实体手机键盘或 Windows 桌面包验收，这些继续由 RN4 执行。
- 草稿只驻留当前页面，刷新、进程退出和身份失效后不保证恢复；beforeunload 的提示由宿主决定。
- 回答摘录必须能映射为单个已交付 Markdown 片段中的连续原文；无法对应时显示重新选择提示，跨多个回答片段不拼接保存。
- 搜索、类型筛选、最近记录／原文顺序、完整返回动作及跨平台总验收仍属 RN4；RN2 原演示恢复能力与私人存储保留策略继续适用。
- RN1–RN3 仍为未提交工作区改动，未发布。
