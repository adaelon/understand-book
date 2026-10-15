# YY4 助手空态与紧凑身份（2026-10-13）

YY4 已在本地完成。助手空态使用全身日常抱书页页与“这一节，想先弄明白什么？”，交谈身份使用专用头像。下一片为 YY5 首页情境融合。范围见 [实施切片](../../切片方案-页页品牌融入与动效.md#yy4-阅读助手空态与头像)。

## 实现

- [RightRail.vue](../../../packages/web/src/components/RightRail.vue):`showConversationWelcome` 仅在当前聊天无回合、无历史加载／错误、无发送或可停止运行且未打开演示工作区时展示欢迎。历史恢复和错误重试沿原入口呈现。
- 空态全身图桌面 144px、手机 128px；欢迎内容最大宽度 360px，放在可滚动对话区域。普通侧栏“问答”标签及手机“问这本书”使用 24px 头像，全屏标题使用 32px 头像。图片为装饰，不增加焦点位置或重复朗读。
- 示例问题仍只填写并聚焦输入框；用户发送后空态退出，正文回答拥有主要空间。历史入口、学习目标、作答与输入使用原组件和状态；手机发送后沿原行为收起键盘，输入框继续常驻。

## 验证

| 验证 | 检出的具体失败 | 结果 |
| --- | --- | --- |
| [RightRail 组件测试](component-tests.log) | 欢迎遮住恢复／错误，运行或演示仍显示欢迎，首问切换重建输入、丢失草稿／焦点，误用全身资源作头像 | 56 项通过，其中本轮新增 3 项并扩展历史恢复断言 |
| [Web 生产构建](build.log) | Vue／TypeScript 合同错误、生产资源导入失败 | 类型检查及构建通过 |
| [既有布局回归](browser-tests.log) | 长回答和全屏三栏挤压操作，手机选区、草稿、作答或模拟键盘回归 | 12 项通过：长讲解 4、手机交互 4、手机预览 4 |
| [首问与演示验证](browser-retry.log) | 首问后插画未退出、图片不可解码、顶栏溢出、返回阅读丢草稿、讨论入口不可用 | 4 项通过：1440／390／320px 首问各 1 项、手机演示讨论 1 项 |

浏览器共 16 个不同场景通过。首问用例挂载真实 App，使用固定接口数据，验证示例填写、显式发送、桌面焦点、手机收键盘、后续草稿、返回阅读、全屏切换和无脚本错误。已有布局回归同时覆盖 1907×879、1526×703、1440×900、390×669／844、320×568；模拟键盘可视高度为 350px。

首轮日志中的手机首问焦点失败来自测试误用桌面预期：`RightRail` 既有发送监听会主动收起手机键盘，最终断言保留该行为。演示首轮缺少 4175 验收服务；启动现有 Rust `presentation_browser_host` 后通过，见 [宿主日志](presentation-host.log)。本轮复用本机 2026-10-11 的 `target/debug/deps/server-e6140f0c20c22aad.exe`，运行结束已通过 `/stop` 关闭。

已查看实际桌面侧栏、全屏、390px／320px 空态和手机交谈截图：角色轮廓、头像和文字清晰，常驻输入可见，无横向溢出。截图：[桌面空态](welcome-1440.png)、[桌面全屏空态](welcome-fullscreen.png)、[390px 空态](welcome-390.png)、[320px 空态](welcome-320.png)、[桌面交谈](conversation-1440.png)、[390px 交谈](conversation-390.png)、[320px 交谈](conversation-320.png)。

从仓库根目录复现：

```powershell
pnpm --filter @understand-book/web test -- src/components/RightRail.test.ts
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e -- playwright/conversation-learning-entry.spec.ts --grep YY4 --workers=2
pnpm --filter @understand-book/web test:e2e -- playwright/conversation-learning-design.spec.ts playwright/mobile-chat-interactions.spec.ts playwright/mobile-learning-preview.spec.ts --workers=2
# 演示讨论需要先在另一终端启动有五分钟上限的本地验收宿主
cargo test -p server --lib presentation_browser_host -- --ignored --nocapture --test-threads=1
pnpm --filter @understand-book/web test:e2e -- playwright/agent-presentation.spec.ts --grep 'compact mobile toolbar' --workers=1
Invoke-RestMethod -Uri http://127.0.0.1:4175/stop -Method Post
```

## 已知限制

- 本轮为本地实现，未提交、推送或发布；首页、动效与完成反馈依 YY5–YY7 继续。
- 浏览器验收来自 Chromium 视口与键盘模拟；实体 iPhone／Android 软键盘及安全区域仍待验收。浏览器页面使用开发服务，生产资源由本轮构建验证。
- Web 构建保留既有 PDF 静态／动态混合导入与大 chunk 提示。
