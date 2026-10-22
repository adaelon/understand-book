# 云端学习浏览器联合验收

此目录保存实际浏览器与 Rust HTTP 服务的联合验收结果。服务使用临时数据库，价格、额度和空间全部为测试输入；课程复用仓库原创材料，不代表首发课程或正式销售配置。

## 实测结果

2026-10-20 运行通过：Playwright **1 passed**，场景操作耗时 17.9 秒；含启动两个前端开发服务总计 59.1 秒。页面错误为 0。实际保存的结果见 [results.json](results.json)。

| 场景 | 观察结果 |
| --- | --- |
| 套餐选择 | 1/3/7 天和月度均显示三档，未定价商品没有进入读者目录 |
| 核款与续期 | 两笔核款分别开通当前一天及下一月；当前期 86400 秒，下一期紧接当前到期 |
| 私人课程 | 原创文本与视频共 2 份公共资料，个人考试日期、每天 45 分钟和修改后的范围已保存 |
| 存储 | 公共资料占用 0；中断上传预留 65536 字节，取消后归零；完整上传计入 81920 字节 |
| 准入 | 超过单文件上限和账号剩余量的文件被拒绝，没有增加占用 |
| 账号隔离 | 第二账号不显示第一账号目标、上传或套餐；读取第一账号工作区返回 404 |
| Windows 授权 | 实际浏览器确认后，真实 PKCE 交换产生 A 账号的独立会话 |
| 费用与公共构建 | 模型调用 0 → 0，公共发布 4 → 4，核款回执 0 → 2 |

页面证据：

- [套餐与手机布局](plans-mobile.png)、[当前及下一期](plans-current-next.png)
- [运营核款回执](admin-purchase-receipt.png)
- [考试冲刺安排](private-course-sprint.png)
- [已用空间及超限提示](storage-used-limit-and-cancel.png)
- [第二账号隔离](second-account-isolation.png)
- [Windows 浏览器授权](windows-browser-consent.png)

## 最终前端构建

2026-10-20 在 C4、C10 界面及桌面授权调整完成后，两端生产构建均成功。2026-10-21 完成视频字幕通用文案后，Reader 再次生产构建成功；每次构建均包含 TypeScript 检查：

```powershell
$env:VITE_MULTI_USER='1'
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/admin build
```

- Reader 最新输出至 `packages/web/dist`，Vite 构建耗时 26.59 秒，使用云端多用户入口。完整输出见 [reader-production-build.log](reader-production-build.log)。
- Admin 输出至 `apps/admin/dist`，Vite 构建耗时 14.95 秒。完整输出见 [admin-production-build.log](admin-production-build.log)。
- 两端均有 Vite 的分块体积提示，构建返回码为 0。

桌面授权最后改为按账号完成请求，避免后台恢复阅读现场时丢弃已签发的授权码。对此新增组件回归，连同原 URL 与取消绑定测试 **4/4 通过**：同一账号切换阅读现场仍返回桌面；账号切换后阻止原授权跳转。运行命令：

```powershell
pnpm --filter @understand-book/web exec vitest run src/desktop-auth.test.ts src/components/DesktopAuthorization.test.ts
```

2026-10-21 的视频阅读器通用文案修订只调整“视频字幕／视频转写”显示与无障碍标签，未更改转写语言配置或处理策略。对应 `VideoReaderPane.test.ts` **9/9 通过**；Admin 无本轮变更，沿用上次构建结果。

## 运行

先启动 Rust 测试宿主：

```powershell
cargo test -p server cloud_learning_browser_host -- --ignored --nocapture
```

出现 `CLOUD_LEARNING_BROWSER_READY` 后运行：

```powershell
pnpm --filter @understand-book/web exec playwright test --config playwright.cloud-learning.config.ts
```

配置会启动本地 Reader（4204）和运营页（4205），产品请求转发至真实宿主（4200）；4201 只提供测试资料及停止入口。每次重新验收需新的宿主临时数据库。完成后可请求 `http://127.0.0.1:4201/stop` 结束宿主。

## 验收范围

- 无有效套餐账号查看全部时长和三档商品，未定价草稿不出售。
- 运营页人工核款，一天为连续 24 小时，下一期从当前期结束接续。
- 公共原创课程创建个人实例，保存考试日期、范围及每天可用时间。
- 公共资料不占私人空间；单文件及账号空间超限拒绝；中断上传保留预占，取消释放，完整上传计为已用。
- 切换账号后私人目标、资料与权益隔离。
- Windows 登录的浏览器确认与真实 PKCE 交换产生独立会话。

## 证据与限制

成功运行后，`results.json` 保存断言结果、API 路径和状态，不保存会话密钥或授权码。PNG 来自实际页面。失败运行保存 `failure.png` 与 `failure.txt` 供定位。

上传中断通过浏览器测试工具在第二个上传块模拟断网，其他产品接口不替换响应。桌面回调接收器由测试模拟，授权确认、一次性码交换和会话读取使用真实服务；原生 Windows 安装包启动由桌面验收负责。本场景不调用模型，语义质量与真实成本证据另行记录。
