# YY3 书架接续（2026-10-13）

YY3 已在本地完成。真实阅读接续卡右侧显示 144px 的日常抱书页页；900px 及以下收起装饰，书名、版本、已保存位置、摘录、上次问题和继续入口沿原数据呈现。下一片为 YY4 助手空态与紧凑身份。范围见 [实施切片](../../切片方案-页页品牌融入与动效.md#yy3-书架接续)。

## 实现

- [NetworkApp.vue](../../../packages/web/src/NetworkApp.vue) 在现有 `resumption && resumptionBook` 条件下使用 `YeyeMascot`，默认 `idle` 和装饰语义；内容列占据可用宽度，页页位于独立的右侧空间。
- 装饰不进入键盘焦点和可访问性树；继续按钮仍调用 `continueReading → recoverWorkspaceBinding`，按原版本、聊天及保存读位恢复。返回书架再继续时保留已有阅读器与未发送草稿。
- 首次进入显示正常书架，读取失败保留明确重试入口，无授权材料显示“账号已创建，等待管理员开通材料。”。已有接续在后台刷新时保留卡片；无效记录沿原行为清除。

## 验证

- [11 项组件测试](component-tests.log) 通过：覆盖初次加载、后台刷新、暂时失败与记录失效、准确恢复现场、草稿保留、无保存记录、无材料、授权撤销及版本变化。本轮补充初次读取失败后的重试和无材料账号场景。
- [普通生产构建](build.log) 与 [多用户生产构建](build-multi-user.log) 均通过，包含 Web 类型检查。
- [12 项浏览器回归](browser-tests.log) 在多用户生产构建的真实阅读器入口上通过。320／390／768／901／1024／1440px 各覆盖普通及长书名、长问题；生产 WebP 成功解码，宽屏装饰与文字无重叠，窄屏收起装饰。卡片、标题、问题和页面无横向溢出，书名与问题无裁切；继续按钮可滚动到视口内并由键盘激活，装饰不增加焦点停靠点。查看和重载书架只读取状态，继续操作提交原现场的恢复请求，冲突后保留重试能力；无页面脚本错误。
- 已查看桌面、390px 书架和桌面／320px 长内容卡片截图。窄屏长内容自然增高，原摘录继续最多显示三行，完整书名与上次问题保留。

截图：[桌面书架](bookshelf-1440.png)、[390px 书架](bookshelf-390.png)、[320px 书架](bookshelf-320.png)、[桌面长内容](card-1440-long.png)、[390px 长内容](card-390-long.png)、[320px 长内容](card-320-long.png)。

从仓库根目录复现；生产测试配置自动启动并关闭独立的 6195 预览服务：

```powershell
pnpm --filter @understand-book/web test -- src/NetworkApp.resumption.test.ts
$env:VITE_MULTI_USER = '1'
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e --config ../../docs/performance/yeye-brand-yy3-20261013/playwright.config.ts
```

[浏览器用例](../../../packages/web/playwright/reading-resumption.spec.ts) 也可沿 Web 默认配置在多用户开发服务器运行；上述 [独立配置](playwright.config.ts) 将相同用例指向生产产物，截图保存在本目录。

## 已知限制

- 本地实现尚未提交、推送或发布；YY4–YY7 继续按实施切片推进。
- 浏览器证据使用固定接口数据及 Chromium 视口模拟，未连接真实账号，也未在实体手机上验收。
- 构建保留既有 PDF 静态／动态混合导入与大 chunk 提示。
