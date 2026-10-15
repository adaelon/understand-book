# 页页静态生产素材 v1

首批生产资源为日常抱书、奶油书封阅读、专用头像。清单与尺寸见 [manifest.json](manifest.json)。

| 资源 | 制作来源 | 生产输出 | 显示尺寸 |
| --- | --- | --- | --- |
| 日常 | [v7 日常稿](../v7/yeye-idle.png) | [600px WebP](../../../../packages/web/src/assets/yeye/yeye-idle.webp) | 120–300 CSS px，300px 下支持 2× |
| 阅读 | [v7 奶油书封稿](../v7/yeye-reading-cream.png) | [600px WebP](../../../../packages/web/src/assets/yeye/yeye-reading-cream.webp) | 120–300 CSS px，300px 下支持 2× |
| 头像 | [头像原稿](../avatar-v1/yeye-avatar.png) | [96px WebP](../../../../packages/web/src/assets/yeye/yeye-avatar.webp) | 24／32 CSS px，32px 下支持 3× |

全身原稿及原始生成提示保存在 [v7 设计记录](../v7/design-spec.json)。头像使用内置 image_gen，以 v7 日常形象为参考生成独立头部：保留歪头轮廓、双耳、奶油脸颊、向右看的眼神和闭口微笑，按小尺寸强化可读性。完整最终提示、输入与生成路径保存在 [头像制作记录](../avatar-v1/design-spec.json)。

[export.mjs](export.mjs) 用 sharp 的 Lanczos3 缩小原稿，并以无损 WebP 编码，保留透明度及原方形画布。三个文件共 327,136 字节；运行时只需要生产输出。原图不被改写。

复现导出：在有 `sharp` 的 Node 环境，从仓库根目录运行 `node assets/brand/yeye/production-v1/export.mjs`。本机 bundled runtime 的包目录可通过 `NODE_PATH` 提供，无需给 Web 增加依赖：

```powershell
$env:NODE_PATH = 'C:\Users\Lenovo\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules'
node assets/brand/yeye/production-v1/export.mjs
```

组件用法、生产路径验证及实际尺寸截图见 [YY2 交付记录](../../../../docs/performance/yeye-brand-yy2-20261013/README.md)。

## YY6 生产动画

`packages/web/src/assets/yeye/idle.json` 与 `reading.json` 分别复制自 `animation-v1/idle/lottie.json`、`animation-v1/reading/lottie.json`，保留原始图层与 1254×1254 坐标，大小为 69,728／53,491 字节。运行时 `loadYeyeAnimation` 将唯一的 `approved-art` 引用替换为本页所列同画布 WebP 的构建 URL；SVG 仍按原坐标绘制图片和局部矢量层。

运行库为固定的 `lottie-web@5.13.0`，对应 MIT 许可从 `animation-v1/licenses/lottie-web-MIT.txt` 复制至 `packages/web/public/licenses/lottie-web-MIT.txt`。产品按需加载当前 JSON 与运行库，success 保留在制作目录。播放策略、网络记录和截图见 [YY6 交付记录](../../../../docs/performance/yeye-brand-yy6-20261013/README.md)。
