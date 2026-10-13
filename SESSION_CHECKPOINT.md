# SESSION_CHECKPOINT — 2026-10-13 品牌版本发布中

## 当前状态

- 工作区：E:/allwork/download/agent/understand-book；分支 main；发布前基线 6b77e35。
- 用户已确认焙橙书本与歪头页页负形 logo，并明确授权它与近期未上线改动一起发布。无需再次请求上线批准。
- 主标识 SVG、透明 PNG、浏览器／触屏图标已接入；YY0–YY7、手机学习布局、得意黑及自动构建等待修复已整合。
- 128 项组件／分享测试、97 项自动构建测试、29 项手机／分享浏览器场景、多用户生产构建通过。生产浏览器 37 个不同场景均有通过证据；一次历史切换测试首轮失败及后续定向结果记录于发布目录。

## 发布入口

- [本次发布记录](docs/performance/logo-brand-release-20261013/README.md)；本地发布脚本 tmp/logo-brand-release-20261013/publish.py。
- 线上 https://understandbook.top/；服务器既有 release 为 /opt/understand-book/releases/to-tb-20261012。本次替换 dist-multi 静态文件、同步 Web 源码，不更换 Rust 服务或数据目录。
- 下一步：提交候选源码；比对线上前端基线 7b94889；备份入口及源码；上传新资源并原子替换 HTML；线上浏览器与资源验证；记录实际提交、恢复点并推送。

## 接手读序

1. 本次发布记录与 assets/brand/logo/v1/README.md。
2. [ADR-0165](docs/adr/0165-yeye-reading-companion-and-brand-integration.md)、[YY0–YY7 方案](docs/切片方案-页页品牌融入与动效.md)、YY7 验证记录。
3. 手机布局与得意黑的各自交付记录，以及代码链路中的本次集成说明。

## 保留事项

- 历史设计草稿、临时制作与其他本地书籍产物保留在原位置；发布包只含生产入口与所需资源。
- 原有待办：TB6、TC7／TC8、TC2 剩余质量与读者验收；EXISTING 笔记 Goal reader_action 问题见 TO5，既有 Core BookStructure 路线失败见集成记录。本次不是这些待办的完成声明。
- 实体手机键盘与原生 WebView 尚未验收；当前移动证据来自 Chromium 视口／键盘模拟。本地安装态插件与 Windows 安装包没有整体更新。
- 既有 PDF 混合导入、大 chunk 及 lottie-web eval 构建提示保留。
