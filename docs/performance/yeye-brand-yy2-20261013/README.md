# YY2 生产素材与静态组件（2026-10-13）

YY2 已在本地完成。首批日常、阅读与专用头像资源可由 `YeyeMascot` 展示，固定占位及可访问性已验证。下一片为 YY3 书架接续。设计范围见 [实施切片](../../切片方案-页页品牌融入与动效.md#yy2-生产素材与静态组件)。

## 实现与用法

- [yeye-assets.ts](../../../packages/web/src/yeye-assets.ts) 通过 Vite 静态 import 映射 `idle`、`reading`、`avatar`。阅读明确采用 `yeye-reading-cream`。
- [YeyeMascot.vue](../../../packages/web/src/components/YeyeMascot.vue) 提供 `variant`、`size`、`decorative`、`label`。全身默认 160px，头像默认 32px；`size` 指定正方形 CSS 像素。宿主按可用空间决定尺寸与显示条件。
- 组件从首帧保留宽高，图片铺满占位并保持比例；图片到达不改变布局。默认装饰语义、空替代文字、不进入键盘焦点。需要独立表达身份时传 `decorative=false`，根节点提供一个可读图像名称，默认“页页，阅读伙伴”。
- 全身为 600×600 无损 WebP，头像为 96×96 无损 WebP，共 327,136 字节。原始美术、头像最终生成提示与可复现导出脚本保存在 [制作记录](../../../assets/brand/yeye/production-v1/README.md)。头像由内置 image_gen 基于已确认日常稿制作，生产导出保持透明度。

```vue
<YeyeMascot :size="128" />
<YeyeMascot variant="reading" :size="160" />
<YeyeMascot variant="avatar" :size="24" />
<YeyeMascot variant="avatar" :decorative="false" label="页页，阅读伙伴" />
```

[静态预览](../../../packages/web/yeye-preview.html) 使用真实组件，开发服务器地址为 [本地预览](http://127.0.0.1:4174/yeye-preview.html)。预览页未加入产品构建入口，书架、助手和首页将在 YY3–YY5 接入。

## 验证

| 验证 | 检出的具体失败 | 结果 |
| --- | --- | --- |
| 3 项 [组件测试](../../../packages/web/src/components/YeyeMascot.test.ts) | 装饰重复朗读、命名丢失、头像误用全身资源、尺寸切换／错误事件丢占位 | 3 项通过 |
| [Web 生产构建](build.log) | Vue／TypeScript 合同错误、资源导入或构建入口失败 | 类型检查和构建通过 |
| [生产验证脚本](verify.mjs) | 非根路径下图片 404、尺寸错误、延迟图片推移内容、横向溢出、装饰暴露到可访问性树 | 2 个视口、26 个实例通过 |

生产验证使用 Web 当前 Vite 配置单独构建预览，设置 `/yy2/` base，在真实 Chromium 加载构建产物。三个资源均从 `/yy2/assets/` 返回 200；分别读取实际宽高和固有像素尺寸。1440px／1×、390px／2× 下，先拦住图片请求，记录角色与相邻文字边界，再放行并解码；前后位置完全一致。页面无脚本错误，只有显式命名的一个图像进入可访问性树。原始结果见 [verification.json](verification.json)、[生产验证输出](production-verification.log)。

已查看实际截图：24／32px 头像在暖纸、白色、浅杏底保留双耳、眼神与脸颊轮廓；120／128／160／220／300px 全身图边缘正常，阅读图使用奶油书封。

截图：[桌面完整预览](static-1440.png)、[手机完整预览](static-390.png)、[桌面头像原尺寸](avatars-1440.png)、[手机头像 2×](avatars-390.png)。

复现命令从仓库根目录执行：

```powershell
pnpm --filter @understand-book/web test -- src/components/YeyeMascot.test.ts
pnpm --filter @understand-book/web build
node docs/performance/yeye-brand-yy2-20261013/verify.mjs
# 手动预览
pnpm --filter @understand-book/web dev --host 127.0.0.1 --port 4174
```

## 已知限制

- 本地实现尚未提交、推送或发布；页面实际融入依 YY3–YY5，动效依 YY6–YY7。普通产品构建尚无组件宿主，因此本轮另用生产预览验证资源输出。
- 浏览器证据来自 Chromium 视口模拟；实体手机显示仍待页面接入时验收。全身输出以最高 300px／2× 为本批基准，更大尺寸可从保留的原稿另行导出。
- Web 构建保留既有大 chunk 提示。
