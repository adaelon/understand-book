# 工作区资料展示 · 2026-10-21

> 2026-10-21 已上线，Windows 0.3.4 已发布并完成本机升级。最终线上验证见 [发布记录](../workspace-library-release-20261021/README.md)。以下保留本地实现阶段记录。

工作区采用左侧封面导航、右侧目标与资料卡布局。资料卡复用书籍封面、PDF 首页和已发布的视频画面；无封面时显示标题封面。添加资料、学习记录和归档折叠展示，共享课程的资料与路线采用可读名称和能力标签。

## 数据链路

- `book_title::read` 从已发布视频的 `video_source_manifest.json → source.original_path` 提取文件名；正式标题和导入标题优先。
- `book_cover::published` 复用发布清单中的 `video-enrichments/*/frames/*.png` 或 `video-enrichments/*/frame.png`，通过原资料授权下的 `video/cover` 图片入口读取，目录读取不解码视频。
- `PublishedLibrary::list` 返回 `media_kind`，`NetworkApp::materialBooks` 按完整发布引用接入视频标题与类型；已发布视频在全部资料中只出现一次。
- 合并后的目录对象不沿用原对象身份，版本序号按完整发布引用查找。阅读接续回归固定了同一本书两个版本分别显示 1、2 的场景。
- `MaterialCover` 在工作区、共享资料和阅读接续中统一显示封面；`sharedCapability` 显示转写、双语字幕、画面索引等能力。

## 验证

前端相关用例覆盖工作区创建、资料增删、目标修改、归档恢复、版本更新、草稿隔离、阅读接续、账号恢复、MCP 入口及视频展示；记录在 `web-tests.log`。生产构建包含 `vue-tsc --noEmit`，记录在 `build.log`。

服务端执行 `cargo test -p server --lib video_catalog_displays_original_title_and_published_frame`，通过真实发布与授权目录检查视频标题、类型和封面 URL，并从 HTTP 路由取得原 PNG 字节、确认无资料权限的账号不可读取，记录在 `server-cover-tests.log`。`cargo test -p server --lib book_title::tests` 验证 Windows/Linux 文件名以及正式标题优先级，记录在 `server-title-tests.log`。

`browser.mjs` 在本地 Vite 上使用确定性 API 数据，并渲染真实 `NetworkApp`。1440、1024、390 像素宽度下检查页面与卡片边界，操作资料添加展开、工作区切换、空工作区、书架、新建表单及共享路线。结果见 `browser.json` 与 PNG 截图。

预览素材：课程画面取自本机 `CS336_2026_Lecture02_PyTorch_BV1msTD6CE6j.mp4` 的第 60 秒；Rust 使用已有书库封面，AI Infra 展示无原封面时的标题封面。

## 限制

浏览器截图是本地接口夹具预览，不代表线上发布完成。视频没有已发布的画面时使用课程标题封面；本次没有创建抽帧任务或修改已发布资料。
