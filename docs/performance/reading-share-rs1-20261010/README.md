# RS1 笔记分享闭环验收

2026-10-10，Windows 上的 Chromium。RS1 功能及 Web 验收完成，接续 RN1–RN4 未提交工作树；未提交、未发布。Windows 安装包与实体手机保存体验在 RS3 验收。

## 已交付

Notes、Markdown 批注和 PDF NoteCard 的共用 NoteDetail 提供“生成分享图”，均调用 App 注入的同一选材动作。用户文字与保留摘录分别勾选，已保存正文只读。旧正文标为“阅读笔记”，新用户文字为“我的笔记”，保留片段为“原文摘录”或“助手解释”；演示笔记输出文字与已记录演示名称。

页面内分享草稿支持标题、感想、摘录卡／理解卡和暖纸珊瑚／奶油蓝。摘录卡将保留片段放在用户文字之前，理解卡先呈现笔记。来源沿所选记录的已有标签、章节／PDF 页码和当前对应材料的真实名称取得；元数据标题优先，其次已有材料名称。无已记录出处的旧笔记不补造来源；名称读取失败保留已有位置与“材料名称暂不可用”。标题编辑不改写来源。

`reading-share.ts` 使用原生 Canvas 和项目已分发的 Noto Serif SC／Noto Sans SC Variable，按实际字体测量文字后绘制 1080×1440 PNG。正文 44px，来源与身份标签 28px。预览直接显示这次生成的 PNG Blob，下载复用相同字节，不新增导出依赖。图片只接收内容和可读标签，不带记录／用户／聊天标识、文件路径或协议字段。

浏览器提供下载与“打开图片”入口，手机可在打开后长按保存。桌面复用原 dialog 插件选择目标文件，通过 `save_reading_share_image` 写入已生成的图片字节；取消或对话框等待期间切换现场不执行写入。

修改选材、标题、感想或样式会立即撤销旧预览，须明确生成新预览。失败保留分享编辑，可重试。关闭返回入口焦点，保持原读位；分享与笔记编辑草稿独立。切换身份、材料／发布或聊天即清理分享草稿；晚到来源读取和图片生成结果不重新显示。

## 验证

| 检查 | 结果与实际范围 |
| --- | --- |
| 定向 Vitest | 8 个文件 109 项通过（按受影响文件复验汇总）；选材身份、顺序、完整文字、格式／超长拒绝、失败保稿、旧生成结果失效、身份／发布／聊天清理、桌面取消及晚到选择，以及原笔记／来源回归 |
| Chromium RS1 | 390px、1440px 两项通过；有来源笔记、无来源旧笔记、演示文字笔记，实际下载并解码 1080×1440 PNG、非空像素、绘制全文与所选内容、字体加载及预览／下载逐字节一致；1440px 另验生成失败重试 |
| Chromium RN3／PDF 回归 | 3 项通过；真实 PDF 文字层、Markdown／回答入口、草稿失败／编辑与切材料 |
| 桌面 Rust | 图片写入测试 1 项通过；实际写入字节一致，目标不可写返回失败。编译同时验证新命令注册及权限配置 |
| Web 生产构建 | vue-tsc 与 Vite 通过；保留原 PDF 混合导入和大包提示 |

样例为浏览器实际下载文件：[摘录卡·奶油蓝](excerpt-blue.png)、[演示笔记·暖纸](presentation-paper.png)、[旧笔记·暖纸](legacy-paper.png)。[桌面预览界面](desktop-preview.png)。已查看中文字体和排版结果；自动断言验证所选文字、来源与下载字节。

复现命令：

```text
pnpm --filter @understand-book/web exec vitest run src/reading-share.test.ts src/reading-share-save.test.ts src/components/ShareImagePanel.test.ts src/components/NoteDetail.share.test.ts src/App.startup.test.ts src/components/RightRail.test.ts src/components/ReaderPane.test.ts src/components/RightRail.pdf-note-placement.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep "RS1|RN3|resolved real PDF selection performs" --workers=1
pnpm --filter @understand-book/web build
```

桌面测试首次因缺少打包 sidecar 中止；仅测试进程覆盖打包资源配置后通过，未修改发行配置：

```powershell
$env:TAURI_CONFIG='{"bundle":{"externalBin":[],"resources":[]}}'
cargo test -p understand-book-desktop reading_share --bin UnderstandBook
```

## 已知限制

- RS1 支持单张短篇纯文字与换行，按实际字号容量判断；Markdown 强调、引用、列表、链接、公式、代码、表格及图片资源给出具体不支持原因。超出一张版面时要求减少所选部分或缩短标题／感想，正文不截断、不缩小字号；完整格式和长文分图由 RS2 接入。
- 来源采用已存标签和当前可读位置；不增加另一套跨材料来源解析。原文选区／高亮的直接分享在 RS2，演示实际图面在 RS7。
- 浏览器业务响应使用固定笔记样本，实际运行 App、字体、Canvas、PNG 下载与解码；RS1 未重跑真实 Rust 演示宿主。RN2 既有现场恢复结果见其验收。
- Windows 安装包的原生保存对话框及实体手机未进行人工验收；390px 模拟视口不代替实体设备。桌面测试资源覆盖不代表安装包通过。
- 分享草稿和生成图片只驻留当前页面；关闭、刷新或切换现场结束该草稿，已保存文件不随原笔记变更。RN4 实体平台待验、既有 MU10 恢复失败沿原 checkpoint 保留。
