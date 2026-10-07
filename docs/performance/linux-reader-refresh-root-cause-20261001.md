# Linux 刷新到正文可读：根因与修复（MU13）

## 问题与测量口径

用户反馈刷新后“正在加载工作区”约 8 秒。MU12 已处理布局、初始化串行等待与静态资源缓存，但该次单点总耗时没有分离材料下载与浏览器连接排队。

本轮在实际 HTTPS 站点、现有材料、真实账号会话中，用 Chromium Resource Timing / 请求时间线定位。分别记录导航开始、加载提示出现、工作区挂载、视口内正文出现后的两个动画帧；PDF 以文本层出现后的两个动画帧为判据。页面只挂载空框不算正文可读。采样不记录密码、Cookie、私人正文或聊天。

受测材料为 `mastering-rust`、`ai-agent-engineering`、Transformer 论文。刷新复用同一个诊断现场；正常对照先等待初次加载结束。另列首次加载尚未收尾就刷新的观测，避免混入稳定刷新对照。

## 已确认的文字阅读根因

### 材料接口未压缩，清单传输阻挡首屏

`App.init` 必须先拿到完整 manifest 才能寻址首屏正文。MU12 的 gzip 只覆盖 `/assets/`，没有覆盖 `/api/books/`。Rust 书的清单传输 668,926 字节；响应首字节出现在刷新后 570.3 ms，末字节在 3,817.3 ms，单是响应体传输就用 3,247 ms。脚本已命中浏览器缓存，正文渲染没有秒级主线程长任务。

保持 HTTP/1.1、同一材料及同一现场，只增加材料 JSON gzip 后：清单传输变为 86,856 字节，请求从 3,435 ms 降为 181 ms；刷新至正文从 3,940 ms 降为 408 ms。其中加载提示到正文为 3,558 → 315 ms。这证明主等待在材料传输，而非此前已经修过的逐章标题请求或历史计算。

### HTTP/1.1 连接数限制让并行请求仍然排队

正文按现有段接口读取，共 20 个请求。代码同时发起请求，但 HTTP/1.1 同源连接限制使其分四批真正发送。

保持 gzip 与相同材料，统一增加 200 ms 网络延迟、5 MiB/s 吞吐上限进行对照：HTTP/1.1 的正文发出时间分布在 2,512 / 2,716 / 2,921 / 3,125 ms 四批；HTTP/2 在 1,294–1,298 ms 内同时发送。刷新至正文从 3,402 → 1,580 ms；加载提示到正文从 2,269 → 526 ms。

| 对照 | 刷新至正文 | 加载提示至正文 | 大清单传输量 |
| --- | ---: | ---: | ---: |
| 原配置，Rust 稳定刷新 | 3.940 s | 3.558 s | 668,926 B |
| 仅材料 gzip，Rust 稳定刷新 | 0.408 s | 0.315 s | 86,856 B |
| gzip + HTTP/1.1，加 200 ms 延迟 | 3.402 s | 2.269 s | 86,854 B |
| gzip + HTTP/2，加 200 ms 延迟 | 1.580 s | 0.526 s | 86,046 B |

原始时间线：[原配置](linux-reader-refresh-root-cause-20261001/settled-baseline.json)、[只加压缩](linux-reader-refresh-root-cause-20261001/gzip-fixed.json)、[HTTP/1.1 延迟对照](linux-reader-refresh-root-cause-20261001/gzip-latency.json)、[HTTP/2 延迟对照](linux-reader-refresh-root-cause-20261001/h2-latency.json)。

AI 书在初次加载未收尾就刷新的追加观测中，HTTP/2 下总计 3.479 s，其中 HTML 导航占 2.925 s，加载提示至正文只占 0.471 s。原配置也观测到过 Rust 总计 8.092 s，但其中加载提示出现前占 4.443 s。因此总刷新时间与用户看到加载提示的持续时间必须分别报告。

## PDF 路径

在 gzip + HTTP/2 下，Transformer 论文工作区于 0.882 s 挂载，实际 PDF 正文却在 13.390 s 出现。原件 2,646,721 字节，读取接口忽略 Range，返回完整 200；大文件使用 tiny_http 默认 chunked 传输，缺少 Content-Length。PDF.js 因而无法按需读取页面字节。

原始时间线：[PDF 修复前](linux-reader-refresh-root-cause-20261001/pdf-final.json)。

修复涉及服务端、代理及阅读器：`multi_user_host::respond` 对已经完整缓存在内存中的 PDF 保留已知长度；Nginx 仅在原始 PDF 路由启用 `proxy_force_ranges`；`PdfReaderPane.loadPdfDocument` 关闭 PDF.js 的整文件流式读取及自动预取。每次分段请求仍经过现有账号及材料授权，API 继续 `no-store`。代理功能依据 [Nginx 官方文档](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_force_ranges)。

仅开分段、保留默认流式加载的中间对照仍需 13.860 s；关闭流式读取与自动预取后为 8.477 s。后者首屏前已完成的分段响应合计 811,713 字节，包含多次默认 64 KiB 字体和对象读取。扩大为 256 KiB 后多取了数据，反而为 9.923 s，已撤回，保留 64 KiB。中断的初始请求未完整计入 Resource Timing 的字节数，因此不把此合计写成实际线路总流量。

进一步时间线显示，默认 URL 加载器先请求完整文件、探测到分段能力后再取消；该请求取消后，首个必要分段仍等了 2.1–2.7 s 才收到首字节。`pdf-range-source.ts:pdfRangeSource` 改为首个请求就读取 0–65535 字节，从 Content-Range 得到总长，再接入 PDF.js 自带的 PDFDataRangeTransport；其后只读 PDF.js 需要的区间。旧本地接口忽略 Range 返回完整 200 时，直接将已取到的完整字节交给 PDF.js。切换材料/卸载取消请求；授权失败终止后续读取并显示错误。

限定首个请求后，PDF 为 6.004 s。剩余时间线发现论文地图 `/reader/paper_minimap.state` 启动时读取三次，每份约 133,768 字节，仍未压缩。最终将 JSON gzip 放在整个 `/api/` location，地图每份降为约 24.5 KB；API 授权、no-store 与 SSE 的流式传输规则保持不变。PDF 最终为 4.079 s，工作区挂载为 0.284 s。完整 PDF 请求已消失，首屏前完成的 PDF 区间合计 811,713 字节。

最终配置的 Rust 文字书刷新为 0.315 s，加载提示到正文为 0.238 s。证据：[文字最终](linux-reader-refresh-root-cause-20261001/text-complete.json)、[PDF 最终](linux-reader-refresh-root-cause-20261001/pdf-complete.json)、[PDF 首屏](linux-reader-refresh-root-cause-20261001/pdf-complete.png)。

## 交付与验证

- 现网沿用 `/opt/understand-book/releases/mu12-20261001`，追加 MU13 补丁；Nginx 已热更新，Server release 已重新构建并启动，网页新版本已发布，旧静态资源保留。
- Nginx 改前恢复点 `/opt/understand-book/backups/mu13-nginx-20261001.conf`；Server 二进制、源文件及原网页 index 恢复点 `/opt/understand-book/backups/mu13-pdf-20261001`。服务重启前活动/未保存问答为 0；后端与前端均有独立部署回执。未迁移或覆盖用户笔记、聊天数据。
- PDF 大文件长度用真实宿主测试先复现失败，修复后 MU4 相关 12 项通过；PDF 组件 13 项、分段来源 3 项通过，覆盖指定字节读取、本地完整响应、后续授权失败和取消。Vue 类型检查、Vite 网络构建、Linux release 构建通过。
- 实际 PDF 请求 `Range: bytes=0-31` 返回 206、32 字节与 `Content-Range: bytes 0-31/2646721`；无登录返回 401，缓存仍 no-store。[协议回执](linux-reader-refresh-root-cause-20261001/range-probe.json)。
- 最终文字与 PDF 浏览器时间线无脚本异常。另一次 PDF 验收刷新为 4.098 s，跳到第 11 页再回第 1 页均可读，覆盖首屏以后按需取数。[翻页回执](linux-reader-refresh-root-cause-20261001/pdf-navigation-navigation.json)。

## 已知限制与部署记录

- 所有时间来自本次测试网络，各条件为单次样本，不能代替用户设备上那次约 8 秒的时间线。材料与阅读模式尚未由用户确认。
- HTTP/2 改善请求排队；正文仍使用原有按段接口，没有新增批量协议。
- PDF 当前实测仍约 4.08 s，第一页所需字体与内容约 0.81 MB，不能用文字书的 0.31 s 表示 PDF 已达到相同时延；论文地图的重复读取尚在，已压缩传输。代理分段仍从宿主完整 PDF 响应中截取，后端尚未改为按文件偏移读取。
- 首次尝试嵌套 Nginx 材料 location 时因缺少显式 proxy_pass 短暂返回 404，已回退并修正；后续对照确认材料 200。该失败样本不计入性能表。
- 初始诊断多次新建现场触发既有容量上限；后续复用诊断现场，未扩大限制或接管用户现有现场。
- 翻页验收后立即退出时出现一次 detach 409，账号退出仍完成；该事件发生在计时结束后。材料缺少 formula_semantics 的 404 与 paper_minimap.localize 的 501 也仍存在，首屏按已有缺省路径完成，未计为本次新增故障。
