# RS7 图解静态分享与专注呈现

2026-10-10。功能／Web 完成；接续 RN1–RN4、RS1–RS6 未提交工作树，未提交、未发布。

## 已交付

演示工具栏“导出当前图解”沿原 iframe 通信读取当前版本与现场。等待字体和图片后，桥接器在同一浏览器任务中读取控件、自定义状态、步骤、可见结果和图面；DOM／SVG 使用实际计算样式，Canvas 2D 保留当前像素。控件转换为当前可读值，隐藏控件不进入分享文字；自定义状态仅用于现场核对，不整份打印内部对象。原 iframe、缩放、参数与未发送问题保持。

演示笔记的“分享记录时的图解”使用原 `memory/presentation.read { mem_id, restore: true }`，在临时 `sandbox="allow-scripts"` 页面恢复原版本，沿原 note observe 完成内容验收后取图。控件、自定义参数、步骤和可见结果须与记录现场相符；对象字段按值比较，不因 Rust JSON 对象的字段重排误报失败。临时页面完成或失败即移除，不写演示现场，不改变正在看的演示。

分享草稿固定图解、现场说明、助手说明、适用条件和原来源；标题与感想独立编辑。默认 1440×1080 横版，可切换 1080×1440 竖版，图面不倾斜，主图留足宽度。长说明按原规则续页，每页保留版本、身份、来源和页序；过大图面要求调整方向或演示布局，不裁切或无限缩小。失败可以返回图解重新取图。预览与逐页下载使用同一 PNG Blob。

来源沿原 turn／note_mem_id 读取真实材料名称，失败保留原标签及不可用状态。App 使用原材料／聊天清理入口，并使迟到取图失效；新分享不会被旧取图覆盖。关闭后恢复入口焦点。分享本身不保存笔记、现场或学习事实，也不运行模型。

## 验证

| 检查 | 结果 |
| --- | --- |
| Web 定向 | 6 文件 82 项通过：原文／笔记／回答分享回归，图解方向、必需现场说明、过大图面、同窗口频道、迟到结果、换身份／聊天及新旧分享竞争；见 `web-tests.log` |
| 最终面板 | 6 项通过，包含返回图解、预览作废和逐页保存原 Blob 的相关路径；见 `panel-final.log` |
| Chromium | 13 项桥接／RS7 用例通过；随后最终相关 12 项通过，包括 RS2 富文本回归、6 项桥接及 5 项 RS7；见 `browser-final.log` |
| 实际图面 | 同页真实 SVG、Canvas 2D、MathML、带内嵌字体的 KaTeX、DOM、版本内 SVG 图片与伪元素；参数从 1 改为 3 时，两种绘图的色块面积变为三倍，说明同步 |
| 静态文件 | 横竖版全部页面逐张下载并与预览字节比较，尺寸分别为 1440×1080／1080×1440；来源及助手身份保留，无内部回合／现场标识 |
| 记录现场 | note read／observe／source 路径固定旧版本与 count=1，当前演示保持 count=3；异步恢复、部分恢复拒绝、恢复结果不一致拒绝及对象字段重排均覆盖 |
| 失败与连续性 | 图片失败、媒体不支持、来源不可用、重试、原 iframe 加载次数、未发送问题与 390px 窄屏；隐藏控件隔离见 `hidden-controls.log` |
| 类型／构建 | vue-tsc 与 Vite 生产构建通过，保留既有 PDF 混合导入／大包提示；见 `web-build-final.log` |

实际图片：[横版主图](landscape.png)、[横版现场](landscape-2.png)、[横版说明](landscape-3.png)、[竖版主图](portrait.png)。界面：[桌面](panel-desktop.png)、[390px](panel-mobile.png)。已查看成图和窄屏界面。

首轮发现导出消息尚未加入宿主原消息准入，导致静态结果被忽略；已补充与原窗口／频道绑定的类型分支。旧桥接测试还缺少现行协议要求的 channel 和来源标签函数，已修正测试装配，生产隔离保持。初轮日志见 `browser.log`、`browser-second.log`。

复现：

```text
pnpm --filter @understand-book/web exec vitest run src/presentation-document.test.ts src/presentation-host.test.ts src/reading-share.test.ts src/components/ShareImagePanel.test.ts src/App.startup.test.ts src/components/NoteDetail.share.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/reading-share-diagram.spec.ts playwright/presentation-bridge.spec.ts --workers=1
pnpm --filter @understand-book/web build
```

## 已知限制与接续

- 视频／音频、WebGL、嵌入页、Shadow DOM、含未展开滚动区的图解明确拒绝静态输出。连续动画仍在原演示查看；本次输出是一个静态现场。
- 版本内图片与字体须可完整嵌入；过大的画布要求调整布局。原文字分享中的内嵌媒体限制沿 RS2 保留。
- 旧演示没有完整 restorer、恢复值或结果不同，会显示具体原因，不以初始画面替代。记录现场在独立 960×640 视口恢复，图解可按该视口重排。
- 浏览器用固定业务响应验证真实组件与渲染；删除聊天后的真实存储保留关系沿已完成 RN2。本轮无服务端改动，未重复该存储验收。
- 原生保存、实体手机与安装后的资源路径／Release 行为继续沿 RS3 待验；本轮未恢复 Computer Use。
- 下一切片：[RS8 理解空间的单项阅读](../../切片方案-阅读分享与安静呈现.md#rs8-理解空间的单项阅读)。
