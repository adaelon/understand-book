# RN2 演示笔记保存与返回验收

2026-10-10，Windows 本地 Chromium、真实 Rust Reader 路由及私人文件存储。RN2 已完成；平台连续性总验收继续在 RN4。

## 已交付

演示工具栏“记一下”先取得点击时的 snapshot，保存成功后将不可变 `PresentationFollowUp`、标题及步骤／结果摘要送入 App 的同一 `noteEditor`。`NoteEditorPanel` 支持明确保存、收起续写、失败保留和按需 Markdown 预览；同材料换聊天保持原关联。已存演示笔记通过 `replace` 只编辑用户文字。

Notes 展开时才挂载 `NoteDetail` 和读取现场摘要。“打开演示”复用同版本宿主；“回到记录时”显式读取并恢复确切 snapshot。打开详情不重载演示；多个笔记指向同版本时继续复用宿主。笔记模式下的交互不会覆盖已记录现场，追问明确标示“绑定记录时的现场”。

## 实际接口与保留关系

- `POST /memory/presentation.read { mem_id, restore?: boolean }`：从当前用户的已存笔记解析原 owner、版本及现场，复验当前材料／发布。`restore=false` 返回同版本初始内容，`true` 返回笔记原现场。客户端任意 session／turn／版本字段不构成授权。
- `POST /memory/presentation.observe { mem_id, text, source_ref_ids }`：同一公开内容编译器校验笔记演示的可见内容与来源。
- 网络工作区使用同名 `memory/presentation.read`／`memory/presentation.observe` 只读命令，沿原授权材料加载和当前用户作用域。
- 来源 resolve／open 接收可选 `note_mem_id`，从已存笔记读取原版本的来源绑定，沿原正文定位与来源返回流程。
- 当前对话的发送请求接收 `note_mem_id`；服务端解析其确切回执后写入当前明确选中的聊天。若同时提交 `presentation_follow_up`，须与笔记回执一致。新回合可按已提交回执使用 `presentation.author` 读取原版本。
- `Record.note.association` 本身就是持久保留关系；编辑继承，删除仅移除此记录的读取依据。原私人版本文件已内嵌内容、SVG 和动画资源；聊天删除不删除这些文件，无额外副本或第二套引用账本。
- 保留读取复用 `presentation_api::render`，删除聊天后使用已通过交付／现场保存准入的公开文字作为原有内容编译器的文字依据。

## 验证结果

| 验证 | 结果与覆盖 |
| --- | --- |
| `cargo test -p server presentation_store_tests --lib` | 23 通过，5 个浏览器／人工宿主按原设置忽略；包括 RN1、原现场保存／恢复、EX13 及 RN2 本地场景 |
| `cargo test -p server rn2_ --lib` | 2 通过，1 个宿主忽略；V2 固定、同版本多个现场、V3、真实聊天删除、磁盘重开、两个笔记共享对象、来源读取、原现场追问及新聊天按需读源码 |
| 同组 RN2 网络用例 | 删除聊天后关闭并重新创建 UserRegistry／Authorization、重挂工作区读取；他人、其他材料、撤销授权均不能读取该笔记演示 |
| Web 定向 Vitest | App startup 23、NoteEditorPanel 1、presentation-document 4、presentation-host 3，共 31 通过；原入口回归、收起／切聊天／保存失败、原关联编辑、追问元数据及输入法快捷键 |
| Chromium `reading-notes.spec.ts` | 1 通过，真实宿主；下述完整交互链 |
| `pnpm --filter @understand-book/web build` | 类型检查与生产构建通过；保留现有 PDF 混合导入及大包提示 |

浏览器记录 V2 的 `count=1`、步骤 `explain`，继续调到 `count=3` 后保存仍关联原 snapshot；故障注入使一次笔记保存失败，重试保留文字。打开详情和同版本定位不重载 iframe；生成 V3、删除原聊天、重新打开服务状态并刷新页面后，显式恢复得到 `count=1`、步骤 `explain`、结果 `1/3`，版本内 SVG 解码成功。来源可回到正文；再调到 `count=3` 追问时，模型收到的仍是原 `count=1` 及 `explain`。

[最终浏览器画面](rn2-restored.png)：画面已继续调到 3，输入区仍明确绑定笔记记录的现场；具体收到的原参数由测试读取模型请求断言。

复现宿主：PowerShell 设置 `$env:RN2_BROWSER='1'`、`$env:PRESENTATION_TEST_PORT='4177'`，运行 `cargo test -p server rn2_browser_host --lib -- --ignored --nocapture`；另一个终端运行 `pnpm --filter @understand-book/web exec playwright test playwright/reading-notes.spec.ts`。宿主使用临时私人存储，300 秒退出，也支持 `POST http://127.0.0.1:4177/stop`。

## 已知限制

- 原文、回答与批注入口统一，材料切换／新建另一条的保存、放弃、继续编辑选择，以及完整退出提醒继续由 RN3 接入；当前已有草稿时再次记录会保留旧草稿并提示先处理。
- 首版草稿只驻留当前页面。RN2 接入主阅读窗口；附属演示窗口仍沿原查看／追问能力。
- 本次浏览器实际验证版本内 SVG；动画资源沿同一版本投影保留，未新增动画解码验收。Windows 桌面包、实体手机及全面窄屏验收在 RN4。
- 旧演示的自定义恢复能力取决于原 restorer 合同。材料不可用或授权撤销时沿原不可用状态。
- 私人演示存储延续既有保留行为，最后一条笔记删除后不主动回收磁盘文件。删除后的 `mem_id` 立即失去读取依据，其他笔记继续有效。
