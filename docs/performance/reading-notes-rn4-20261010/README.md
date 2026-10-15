# RN4 笔记回看与连续性验收

2026-10-10，Windows 上的 Chromium。功能实现与 Web 验收完成；Windows 桌面包及实体手机验收待完成。RN1–RN4 仍为未提交工作区，未发布。

## 已交付

当前材料的笔记与高亮共用搜索、类型筛选和排序。搜索覆盖用户文字、保留摘录、可读取的聊天／演示标题及来源标签；默认按真实 `generated_at` 首次保存时间倒序，兼容原生宿主毫秒、旧记录 Unix 秒字符串与 ISO 时间，缺失或无法解析时显示“保存时间未知”。原文顺序使用实际正文叶子顺序与段内范围；无正文位置的回答和演示继续可发现。旧笔记不从 blockquote 推断类型或作者。

主列表使用静态短文字预览，展开后共用 NoteDetail 完整阅读长文、公式和代码。来源显示章节标题或 PDF 页码。跨段高亮合并全部成员内容供检索，保持原分组删除入口。所有过滤只作用于当前已读取材料；结构化笔记按原发布过滤，未知旧记录保持未知。

回答笔记可返回原聊天的确切回合；原聊天删除时明确提示不可用，已存文字和摘录仍可读。Markdown／PDF 批注提供相同关联返回和 Notes 入口。正文来源往返、回顾打开笔记均定位目标记录，并清除会隐藏目标的搜索／类型筛选。编辑与重锚使用返回的新 mem_id 更新展开详情及演示绑定；删除成功移除记录并提示，失败保留原记录。切材料／发布／身份后清空私人视图，过滤晚到读取、保存、删除和回答切换结果。

## 编辑后的回顾身份

`MemoryDocument.note_replacements` 是原 v4 文档中的可选旧 ID → 当前 ID 只读导航关系。笔记 replace／reanchor 与此关系在同一原子写入中提交；连续替换直接指向当前记录，改回旧内容不形成环；删除清除相关关系。它不参与内容寻址，命令仍要求实际当前 mem_id。

`session_recap::resolve` 沿同一用户、书籍和原发布的既有检查，将历史笔记引用解析到当前记录；历史事实、截止序号与回合正文保持。RN4 服务端回归先复现了编辑后仍返回旧 ID 的失败，再验证新 ID 可打开以及删除后的不可用状态。

## 验证

RS3 桌面验收发现原生宿主写入毫秒字符串，旧前端一律乘以 1000，导致保存年份显示为五万多年。`reading-notes.test.ts` 已先复现，再验证毫秒、秒与 ISO 的统一排序及 2026 年显示（3 项通过）；不修改存储格式。

| 检查 | 实际范围 |
| --- | --- |
| memory | 143 项通过；含旧格式、原子失败、新增替换导航／重开／删除／改回原内容 |
| server session_recap | 5 项通过；历史截点、保留／撤销、编辑后新身份与删除。RN1 三项、RN2 两项本地／网络持久化也通过 |
| Web 定向测试 | 12 个文件合计 132 项通过（按失败修复定向复验汇总）；包含原文／PDF、检索、原回合、发布／身份晚到结果、放置、回顾及网络恢复 |
| Chromium 新 RN4 | 320／390／768／1440 px 四项通过：混合记录、摘录搜索、筛选、排序、长文／公式／代码、原回答、编辑新 ID、删除 |
| Chromium RN3／PDF 回归 | 3 项通过：原文、回答、旧笔记、真实 PDF 文字层、草稿失败重试、编辑及切材料 |
| RN2 真实演示宿主 | 1 项通过：V2 固定现场、V3、删除原聊天、存储重开、SVG、来源返回及原现场追问；宿主已正常停止 |
| Web 类型检查与生产构建 | 最终代码通过；保留原 PDF 混合导入及包体积提示 |

组件验收更新了两项旧预期：RN3 共用 Markdown 详情现在验证渲染文本与粗体；PDF 来源按钮按真实页码验证。旧 NP2a“回答强制先放正文”的源码字符串检查已移除，该行为由 App.startup 的 RN3 直接记录用例和浏览器测试覆盖。

图像证据：[320 px](rn4-320.png)、[390 px](rn4-390.png)、[768 px](rn4-768.png)、[桌面](rn4-1440.png)、[演示恢复](rn2-restored.png)。

复现入口：

```text
cargo test -p memory --lib
cargo test -p server session_recap_tests --lib
pnpm --filter @understand-book/web exec vitest run src/App.startup.test.ts src/components/RightRail.test.ts src/components/ReaderPane.test.ts src/components/PdfReaderPane.test.ts src/components/RightRail.pdf-note-placement.test.ts src/NetworkApp.chat.test.ts src/NetworkApp.recap.test.ts src/NetworkApp.recovery.test.ts src/reading-notes.test.ts src/note-placement.test.ts src/pdf-note-placement-flow.test.ts src/reader-annotations.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep "RN4|RN3|resolved real PDF selection performs" --workers=1
pnpm --filter @understand-book/web build
```

演示按 [RN2 宿主命令](../reading-notes-rn2-20261010/README.md) 启动后运行 `reading-notes.spec.ts`。

## 已知限制

- 扩展命令 `cargo test -p server rn --lib` 同时匹配了名称含 `returns`／`turns` 的其他测试；共 44 通过、2 忽略，MU10 百回合恢复失败（queued 数量 0／预期 100），独立执行同一测试也复现 0／100。失败位于 `crates/server/src/tests/mu10_tests.rs:918`，本次未修改该恢复链路，原因待定位。
- 当前未连接实体手机；RS3 已运行 Windows Debug 程序、读取笔记并生成分享预览，用户随后停止电脑操作。Windows 完整连续性与原生保存仍待补；四种模拟视口不代表实体平台验收通过。
- RN4 开始记录替换关系之前已经被编辑掉的历史 ID 无法反推；回顾如实显示原成果不可用。旧记录缺少发布、作者或时间时不补造信息。
- 原回答聊天删除后不能返回原回合；演示笔记继续沿 RN2 保留现场并支持独立读取。
- 草稿驻留页面、回答选区连续片段限制、旧演示恢复能力和私人资源保留策略沿 RN2／RN3 合同。
