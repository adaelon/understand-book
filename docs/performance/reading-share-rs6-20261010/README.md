# RS6 已交付回答分享

2026-10-10。功能／Web 完成，接续 RN1–RN4、RS1–RS5 未提交工作树；未提交、未发布。

## 已交付

单条已完成回答的“回答操作 → 生成理解卡”与回答划词浮层共用分享入口。App 从当前聊天中的原 turn 取得已交付正文，固定该回合文字和来源；流式、失败、已停止及 incomplete 回答不提供导出。演示对象保持原入口，回答分享只选择 Markdown 文字。

分享面板显示“回答文字全文／回答节选”，支持从只读原回答中重新选取连续文字或包含全部文字。扩大范围保留标题与感想、立即作废旧预览；选中的助手解释与“我的感想”分别呈现。回答固定使用理解卡，保持助手身份标签。原回答、笔记、学习事实与聊天输入不被改写。

来源按原回答实际引用的 source_ref_id 去重，沿原 turn 调用 agent/source.resolve；图片标明“本回答来源”。原接口新增 material_title，由服务端在材料绑定相符后从元数据／输入清单取得，本地材料沿原书架名称兜底。其他材料的绑定不使用当前材料解析原文或填入当前书名。来源失效或读取失败保留原标签并显示不可用。网络发布仍经过原发布匹配校验。

分享草稿保存固定的正文、聊天和回合，标题／感想只用于本次图片；切换聊天、身份、材料、发布或关闭后清除草稿与图片，忽略迟到来源。预览和下载继续使用同一 PNG Blob，关闭分享保留未发送问题并恢复菜单入口焦点。

## 验证

| 检查 | 实际结果 |
| --- | --- |
| Web 定向 | 5 文件 122 项通过：新入口、历史回答、多来源、无来源、跨材料标签、来源失败、未完成回答、选段范围、作用域清理；含原分享／选区／RightRail 回归，见 `web-tests-final.log` |
| 身份标签 | 固定理解卡后面板 5 项通过；覆盖重新选段保留编辑、作废预览及不可切到隐藏身份的摘录版式，见 `panel-final.log` |
| 服务端来源 | 6 项通过；新增用例验证本地／网络来源读取的真实材料名称、跨材料回退及历史只读；原来源绑定、旧回答、来源失效和归属回归，见 `server.log` |
| Chromium | 320／390／768／1440 px 共 4 项通过：真实菜单、划词选段、重新选取、身份与来源、PNG、焦点和未发送问题保持，见 `browser.log` |
| 图片输出 | 四宽度下载与对应预览逐字节相等；绘制文本含助手解释／条件／感想／原来源，不含未选段落、其他回答或内部回合标识；无分享写入／模型／阅读跳转请求 |
| Web 类型与构建 | vue-tsc 与 Vite 生产构建通过；保留既有 PDF 混合导入／大包提示，见 `web-build-final.log` |

实际成图：[回答理解卡](answer-320.png)。界面：[320 px](answer-panel-320.png)、[390 px](answer-panel-390.png)、[768 px](answer-panel-768.png)、[桌面](answer-panel-1440.png)。已查看成图和窄屏界面。

初轮新增测试有两处样本错误：网络现场缺少完整 viewport，Markdown 预期遗漏段落换行；补齐测试样本后通过，见 `web-tests.log`。没有通过修改业务行为迁就这些断言。

复现入口：

```text
cargo test -p server agent_source_ --lib
pnpm --filter @understand-book/web exec vitest run src/reading-share.test.ts src/components/ShareImagePanel.test.ts src/App.startup.test.ts src/components/RightRail.test.ts src/answer-note-selection.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep RS6 --workers=1
pnpm --filter @understand-book/web build
```

## 已知限制与接续

- 来源按原回合提供，图片不声明每个节选字句与每条来源一一对应。选段是否包含足够适用条件由读者在原回答中选择，面板可扩大范围。
- 原来源接口要求确切发布可读；RS6 不自动切换材料／发布。不可读取的跨材料或旧发布只保留已记录标签，不推断缺失书名。
- 重新选段使用只读 Markdown 文字；内嵌资源及过宽／不可分内容沿 RS2 给出明确错误，长内容复用既有分页。复杂图解导出由 RS7 接入。
- 浏览器使用固定业务响应测试实际组件、字体、图像与下载；服务端绑定由真实存储测试覆盖，未做线上发布验收。
- Windows 原生保存、实体手机及 RN4 平台待办沿 RS3 保留；本轮未恢复 Computer Use。
- 下一切片为 [RS7 图解静态分享](../../切片方案-阅读分享与安静呈现.md#rs7-图解静态分享与专注呈现)。
