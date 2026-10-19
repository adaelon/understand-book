# 视频 worktree 与目标工作区集成验收

2026-10-18。当前源码以主目录 `402d9be` 为基线，合入视频 `35e199f`、`95dbcf5` 及其未提交改进，并保留工作区、额度和共享发布修改。未部署。

## 实际构建

新编译 Build Engine 使用已有课程的 8 秒片段，在本轮独立目录完成：故意缺失 ffprobe 导致导入中断，恢复原片探测；通过内嵌 Python 和本地 small.en 模型完成真实语音转写；移除运行时依赖后再次执行已完成任务，原构建回执保持不变，没有重复识别。新编译发布器随后导入成果，保留视频原片、转写、时间映射及原始识别结果。

- [构建结果](compiled-video-result.json)
- [发布结果](compiled-publication.json)

完整课程的已有翻译、视觉、图谱及讲解成果直接复用。本轮没有重新运行完整课程语义生成，也没有使用外部付费模型。

## 混合共享工作区

使用真实 Rust 服务、临时账号、10 秒 H.264/AAC 视频及实际浏览器，覆盖：确认个人目标、两人分别使用共享发布、书籍切视频保留草稿、实际播放、时间范围、画面框选及图片附件、刷新恢复、空闲保持旧版本、显式更新后另一实例仍保持旧版、原视频来源可读、模型调用数不增加。

入口：`packages/web/playwright/video-learning-workspace.spec.ts`，宿主为 `server::tests::video_workspace_tests::video_workspace_browser_host`。

- [结果](mixed-video-verification.json)
- [视频提问草稿](mixed-video-draft.png)
- [明确选择更新后](mixed-video-updated.png)

## 真实完整课程

使用当前新编译服务和生产前端，读取 VI18 原有的 77 分钟课程及 720 段双语字幕，私人目录独立。浏览器覆盖搜索、字幕跳转、手动浏览与跟随、画面框选、时间范围、手机草稿和专注模式。

1440、1280、1024、390 像素四种宽度无横向溢出；切换布局保持同一播放器；页面错误及模型请求均为零。7 张实际截图均已查看。

- [结果](verification.json)
- [1440 桌面](video-1440.png) · [1280 桌面](video-1280.png) · [1024 窗口](video-1024.png)
- [390 手机视频](video-390.png) · [390 手机伴读](companion-390.png)

入口：`node evals/video/video-workspace-browser.mjs READER_URL OUTPUT_DIR`。本轮验证服务关闭后，报告中的本机 URL 不再提供服务。

## 验证范围

浏览器验证播放、草稿、范围与版本行为，不调用问答模型。视频问答的实际请求图片、来源观察和失败收尾由 Runtime 回归覆盖；已有整讲语义质量证据沿 [VI18 状态](../../VI18-实施状态.md)。构建费用及 B1 自动视频调度边界见 [集成记录](../../视频工作区接入-20261018.md)。
