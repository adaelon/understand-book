# 视频学习界面验证

2026-10-15 开始，2026-10-16 完成。使用 VI18 的 77 分钟真实视频、720 段双语字幕与生产前端。Reader 使用独立的 memory/private 目录。

## 结果

- VideoReaderPane、TopBar、ReaderWorkspace、RightRail 和 video-question 选择集共 95 项通过。
- `pnpm -C packages/web build` 通过。
- `node evals/video/video-workspace-browser.mjs http://127.0.0.1:8796 docs/performance/video-workspace-20261015` 通过。
- 四种视口没有横向溢出；桌面输入框保持可见；手机能切换视频与伴读并保留草稿。
- 双语搜索、字幕跳转、手动浏览后暂停跟随、恢复跟随、画面框选、时间范围、专注退出通过；全过程播放器为同一 DOM 实例，页面错误为零。

## 截图

- [1440×900](video-1440.png)
- [1280×720](video-1280.png)
- [1024×768](video-1024.png)
- [390×844 视频](video-390.png)
- [390×844 伴读](companion-390.png)
- [确定性验证结果](verification.json)

## 已知限制

没有发送模型请求，也没有重跑视频预构建。当前样例的标题来自原文件 `external.mp4`。现有 Debug 程序直接读取工作区的 `packages/web/dist`，重新打开即可使用此次构建；未重新制作安装包。
