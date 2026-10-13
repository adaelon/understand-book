# Understand Book 主标识 v1

用户于 2026-10-13 确认[原图](understand-book-logo-approved.png)：焙橙书本内减出歪头页页，脸颊留白连续接入下方书缝。

[矢量母版](understand-book-logo.svg) 沿确认稿轮廓整理为连续 Bézier 路径，移除纸面纹理与渐变，使用品牌色 `#B65A17`；猫脸与外部均透明。保留宽脸、高低耳与不对称书页。[1024px 透明图](understand-book-logo-transparent.png) 由母版导出。

`export.mjs` 从矢量母版生成 Web 使用的同形 SVG、32px 浏览器图标、180px Apple touch icon 与 192/512px 应用图标。方形图标使用 `#F7F3EB` 纸色背景，母版留白保持一致。在可用 sharp 的 Node 环境运行 `node assets/brand/logo/v1/export.mjs` 即可重建。

产品身份位使用此标识。首页情境、书架接续、助手身份与状态反馈继续使用页页角色资源。规范见 [ADR-0165 §7](../../../../docs/adr/0165-yeye-reading-companion-and-brand-integration.md#7-主标识)，上线记录见[品牌版本发布](../../../../docs/performance/logo-brand-release-20261013/README.md)。
