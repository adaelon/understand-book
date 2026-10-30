# 画板来源临时阅读窗口

实现日期：2026-10-28；2026-10-29 00:09:26（香港时间）已上线。参照用户提供的读通鉴临时阅读区截图：大幅窗口、独立目录、可连续上下阅读、关闭返回原现场。保留工作树已有修改，未提交或回退其他工作。

## 线上发布 — 2026-10-29

2026-10-29 00:09:26（香港时间）已发布到 https://understandbook.top/ 。发布目录为 `board-source-window-20261028`，源码提交 `ce641fdd`，分支 `codex/board-source-release-20261028`。从实际线上 `dcee5074` 基线合并本次改动，保留思维导图、共享笔记、页面恢复和后台监控，主工作树的其他未提交内容未整体部署。

合并后 115 项前端测试、5 个桌面/手机浏览器场景、4 项原文窗口后端测试、引用规则与冻结上下文各 1 项测试通过；完整类型检查和多人版网页构建通过。截图直接查看，全屏铺满视口、正文连续滚动、目录切换、当前位置打开和草稿保留符合预期。日志及合并后截图在 [deployment](deployment/) 内。

Linux 正式构建耗时 15 分 13 秒，300 个前端文件与本地验证构建逐字节相同，管理界面沿用现网构建。最终 Linux 程序在生产数据隔离副本验证 3 个账号和 9 张旧画板，实际完成可用来源的连续分页、引用高亮、章节跳转与当前位置打开，并验证读取不改变主阅读器。既有导图编辑、撤销/重做、笔记和监控通过。参见 [副本验证](deployment/integration-qa.json)。

正式切换维护 10.31 秒，schema 仍为 21，无数据迁移。54 份聊天文件、100 份画板文件逐字节保持，17 条额度记录保持；切换前后没有进行中的问答或后台复核。服务 PID 422201，无异常重启。恢复点为 `/opt/understand-book/backups/board-source-window-20261028`，旧版本 `editable-mind-map-20261028` 保留。参见 [发布回执](deployment/deployment.json)。

00:15:55 完成公网复核：首页、阅读器、管理页面、引用的脚本/样式及画板预览资源与发布文件相同，普通读者和管理员登录、正文、历史、额度、演示能力正常。首次 TLS 连接超时，后续复核通过，原始输出保留。00:16:37 从 Windows 经公网独立比对首页、阅读器和 7 个脚本/样式资源，通过。参见 [公网账号与资源验证](deployment/public-verification.json)、[独立公网比对](deployment/public-local-verification.json)。

历史引用中有 8 个缺少出版版本，旧线上程序在隔离副本同样返回 `OBJECT_NOT_FOUND`；已有卡片和摘录保持。参见 [旧版本对照](deployment/legacy-source-verification.json)。

## 行为与实现

画板行内引用和来源条目打开独立临时阅读区，初次定位并高亮原文。正文滚到边缘时读取前后文，上方插入后保留可见段落；通过目录切换章节，也可回到原引用处。桌面显示左目录和正文，窄屏将目录收为抽屉。窗口关闭后画板的选择、视图和追问草稿继续保留。

`BoardSourcePreview.vue → BoardSourceReader.vue → api.boardSourceRead → boards/source.read`。窗口有自己的段落与读位，不调用主阅读器的 goto 或 viewport 保存。正文按需读取，已渲染内容限制为 96 段；目录只加载一次。Markdown、表格、公式和图片沿既有来源渲染能力显示，多个精确引用范围分别高亮。

后端 `board_api::source_window` 复用画板保留的 `SourceBinding`，核对书籍、出版版本及已有引用有效性，每窗最多返回 20 个叶节点；按规范原文顺序分页。`previous_lid/next_lid` 表示当前窗口首末节点，before/after 请求排除锚点。目录中的结构节点映射到首个正文叶节点。云端跨资料沿 `PublishedLibrary` 加载引用绑定的确切出版物。

`network-client.ts` 将 source.read 归为只读动作，不加入写队列，不安装返回中的旧工作区快照。明确点击“在阅读器中打开”才调用 source.open，并携带窗口当前读到的段落；重试保留同一目标。跨资料延续独立页面打开，原画板和主书保留。失效原文继续展示引用时的摘录；视频、音乐使用原有预览。

## 验证

| 范围 | 结果 |
| --- | --- |
| `board-source-reader.test.ts` | 7 项通过：精确引用与 Markdown、前插锚点、前后增量、目录与回引用、错误重试、切换来源、出版版本图片资源、96 段上限 |
| `App.board-source.test.ts`、`board-source-preview.test.ts` | 13 项通过：明确导航、当前位置与失败重试、关闭及恢复焦点、保留画板、失效快照与媒体分支 |
| `network-client.test.ts` | 29 项通过：含来源分页不阻塞写入、不回装旧 revision |
| `cargo test -p server --lib board_source_window --jobs 1 -- --test-threads=1` | 4 项通过：全文前后分页无重复遗漏、结构节点与 UTF-16 多段高亮、当前工作区只读、原出版版本及跨资料导航；原始结果见 `server-tests.log` |
| 既有本地 `wb1_four_sources_retain_full_content_and_original_links_after_chat_deletion` | 1 项通过：四类来源保留、来源解析与显式原文打开；见 `local-source-regression.log` |
| `board-workspace.spec.ts` 五个定向浏览器场景 | 通过：桌面临时阅读、手机临时阅读及英文布局、原生全屏与来源、手机全屏降级、既有画板状态与演示实例保留 |
| Web 完整类型检查 | 通过 |

浏览器验收使用三章共 123 段的代表性正文，验证实际滚轮触发前后读取、上方插入不跳位、目录切到第三章、回到原引用、显式打开第三章当前段落，以及关闭后草稿保留。桌面为 1440×960，手机为 390×844；直接查看截图确认正文排版、引用高亮、目录抽屉和中英文按钮均可用。桌面窗口宽度为可用视口、上限 1800px，手机铺满页面。

验收中修正了两处实际问题：章节加载使焦点按钮禁用后，Esc 原本无法抵达弹窗局部监听；现由挂载期间的文档键盘监听接管，并在卸载时清理。手机目录遮罩原本继承全局按钮的胶囊圆角，现明确使用矩形遮罩。

截图：`board-source-reader-desktop.png`、`board-source-reader-mobile.png`、`board-source-reader-mobile-outline.png`、`board-source-reader-mobile-english.png`。复验入口：`pnpm --filter @understand-book/web exec playwright test board-workspace.spec.ts --grep "temporary source reader|canvas fullscreen fills|mobile fullscreen fallback|WB1 saves" --workers=1`。本机按顺序执行 Rust、Web 与浏览器检查，避免编译和浏览器竞争内存。

## 已知边界

- 本次发布没有模型请求，未做真实模型生成回放；引用规则、编译、审核输入和持久化路径已验证。
- 历史缺少出版版本的 8 个来源仍不可打开，已确认与旧线上行为一致；详见上方旧版本对照。
- 临时阅读只使用已经保存的真实来源绑定，不自动给历史无引用回答补造出处。
- 浏览器场景使用三章长正文夹具和生产 Vue 组件；后端真实书籍及跨出版物路径由 Rust 集成测试验证。
