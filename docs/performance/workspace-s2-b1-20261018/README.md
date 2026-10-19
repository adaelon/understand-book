# S2 / B1 验收证据

日期：2026-10-18。S2共享使用及选择更新、B1运营构建到共享发布已完成；完整合同、115项去重验收与已知边界见[切片方案§14](../../切片方案-目标工作区与统一Agent额度.md#14-s2b1-实施与验收2026-10-18)。

## 实际流程

- S2：两个账号确认各自目标后复用同一公共版本；一人追加私人资料、预览并选择新版本，另一人保持原版。目标、私人资料及旧原文来源保留；撤权限制更新和阅读；共享及个人归档/恢复有效，账本未增加费用。
- B1：两份资料，一份复用，一份由原DSH/Engine准备。实际计划确认、平台预算、暂停/继续、服务重启恢复、PublishedLibrary导入及共享发布均经过真实Rust HTTP。79次受控模型请求归入13次执行回执，Pass1候选1份；恢复模型请求0，原13笔回执不增。
- 发布结果：标准Markdown/EPUB v1来源与导入/Tutor入口已统一；旧发布的incomplete快照保留，新v2快照为ready。后台可查看真实计划和待核算费用，读者确认目标后可以打开新成果的实际正文。
- 成本：13笔部分/未知费用，待核算占用13000 micro-CNY，账号扣减0；模型为本机受控替身，没有真实付款或外部付费调用。

## 截图

六张截图均已查看。S2流程6.2秒通过；B1最终流程6.5秒通过，正文截图等待实际段落可见。

| 画面 | 文件 |
| --- | --- |
| 共享更新预览，桌面 | [shared-update-desktop.png](shared-update-desktop.png) |
| 共享更新预览，390px | [shared-update-mobile.png](shared-update-mobile.png) |
| 私人实例与共享归档 | [shared-instance-desktop.png](shared-instance-desktop.png) |
| 运营任务、实际计划和待核算费用 | [operator-build-desktop.png](operator-build-desktop.png) |
| 运营任务详情，390px | [operator-build-mobile.png](operator-build-mobile.png) |
| 从真实新发布创建实例并阅读正文 | [operator-release-reader.png](operator-release-reader.png) |

## 执行证据

本地日志位于仓库target目录：`s2-browser-validation.log`、`b1-browser-validation.log`、`b1-http-acceptance-result.json`、`b1-http-acceptance-final.log`。服务端、组件和构建结果及复跑方式集中在切片§14.3；不需要依赖临时服务保持运行。
