# 当前工作区、内测联系与页脚发布

2026-10-15。正式入口：[Understand Book](https://understandbook.top/)。当前产品改动已发布至 `current-20261015`，首页新增内测群二维码及网站信息页脚。用户已明确暂停 FP7 剩余验收并要求直接上线；FP7 的性能联合验收仍未完成。

## 发布内容

- 15:15:29 HKT：发布 `6612932` 加工作区中的产品源码，包含正文/PDF专注分页、精确读位与来源接续、相关阅读器修复、Tutor `tb4.v5` 出题指导及已有构建等待修正。发布前与原现网源码比较：43个变化文件、76个新增文件、1583个相同文件，见 [源码比较](source-comparison.json)。
- 15:30:19 HKT：首页内测区域加入用户提供的微信内测群二维码。原图完整保留，点击可查看原图，手机可保存后识别；展示“10月22日前有效”，原注册/登录入口继续可用。见 [二维码发布回执](contact-deployment.json)。
- 15:53:32 HKT：首页页脚加入产品说明、当年版权与服务条款、隐私说明、联系我们、关于我们四个入口。三个说明在可滚动的原生对话框中阅读，可按 Escape 或关闭按钮退出；联系我们回到 `#begin`。见 [页脚发布回执](footer-deployment.json)。

网站说明按现有产品写作：账号邮箱与内测权限、阅读和对话记录、AI 所需上下文、浏览器偏好及运行记录；个人信息相关请求通过内测群联系管理员。对应事实来自 `account_registration.rs`、`auth.rs`、`network-client.ts`、`reader-typography.ts` 及已有阅读/对话流程。

## 验证与证据

| 范围 | 结果与证据 |
| --- | --- |
| 生产构建 | 最终 Linux release 目录构建成功，5分41秒，`build.exit=0`；多人版 Web 的类型检查与生产构建通过。见 `build.log`、`build-receipt.txt`、`web-build.log`，后续首页变更分别为 `web-contact-build.log`、`web-footer-build.log`。 |
| 服务切换 | 切换前进行中或未保存问答为0；运行程序为新 release，包含 `tb4.v5` 与精确读位实现；服务 active、PID 101669。见 [部署回执](deployment.json)。 |
| 账号及可用能力 | 源站和公网登录、11本授权材料，以及 authoring / plot / animation / preview 四项能力均通过；模型请求0。数据根与schema13未改变。 |
| 桌面实际阅读 | 1440px正文与PDF各前进2屏、后退2屏、回到原屏及退出通过。正文证据为 `public-browser-first.json` 已完成检查，PDF为 `pdf-diagnostic.json`。冷PDF等待见下方限制。 |
| 手机实际阅读 | 390px补充线上检查未完成，最终在登录页加载前连接中断，见 `public-browser-final.json`；此前曾完成登录和打开材料，脚本需先从助手返回正文并等待材料加载，该过程保留在 `public-browser-mobile-entry.*`、`public-browser-mobile-loading.*`。已有本地移动功能证据沿FP7记录。 |
| 二维码 | 本地和公网1440/390/320px均验证加载、与用户原图逐字节一致、原图链接、注册入口及无横向溢出；`contact-local.json`、`contact-public.json`。 |
| 页脚 | 本地1440/390/320px全部通过；公网1440/390px全部通过。覆盖版权、三个文档、关闭/Escape/焦点返回、滚动后重开回到顶部、联系跳转及二维码仍可用。见 `footer-local.json` 与 `footer-public.json` 中已完成结果，截图已人工检查。 |

## 运行与恢复

- 服务：`understand-book-multi.service`；当前 release：`/opt/understand-book/releases/current-20261015`。
- 前一 release：`/opt/understand-book/releases/tutor-question-discrimination-20261014`，完整保留。
- 数据：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema13；用户数据及发布后的新写入继续保留。
- 原服务/Nginx/演示配置：`/opt/understand-book/acceptance/current-20261015/previous-configuration`，对应 `understand-book-multi.service`、`understand-book-multi.conf`、`presentation-sandbox.json`。
- 二维码更新前的首页源码与HTML：`/opt/understand-book/acceptance/current-20261015/contact-before-073015`。
- 页脚更新前的首页源码与HTML：`/opt/understand-book/acceptance/current-20261015/footer-before-075329`。

仅回退某次首页更新时，恢复相应备份中的 `home.html`、`index.html` 到当前 release 的 `packages/web/dist-multi/`，并恢复 `ProductHome.vue` 与 `home.css`；旧静态资源均保留，无需重启服务。完整回退先确认无进行中/未保存问答，在短暂维护期间恢复原三份配置，重载服务及Nginx，再核对原程序、登录和材料读取。本次不涉及schema迁移，不恢复旧数据库覆盖新写入。

## 已知限制

- FP7 剩余 WebKit 严格正常阅读、两组正文长读、触摸帧及最终联合汇总暂停，自动化 `fp7` 为 `PAUSED`。既有失败、首轮基准和有效证据全部保留，冻结门槛未改变；详见 [FP7记录](../focus-reading-fp7-20261015/README.md)。
- 公网冷PDF首次前进曾超过30秒检查等待；诊断观察到邻页准备持续约20秒后完成，随后双向翻页与退出正常。`pdf-diagnostic.json` 保留准备状态时间线，后续就绪路径的功能检查不代表冷启动或连续阅读性能通过。
- 本机浏览器访问公网间歇出现 `ERR_CONNECTION_CLOSED`，同期HTTP读取正常。显式使用现有本机HTTPS代理后，页脚电脑与390px手机检查完成，320px新页面连接仍中断；该尺寸的本地检查通过，公网未完成。原失败见 `footer-public-connection-closed.*`、`footer-public.*`、`public-browser-connection-closed.json`，未作为应用脚本错误或性能合格证据。
- 首次远端增量构建使用了时间戳为0的源码归档，不能作为新源码已编译的证据；纠正文件时间后在最终目录重新编译成功，并核对运行程序中的新指导。原日志 `build-initial-timestamp.log` 保留，旧候选未部署。
- 本次部署核对未调用模型生成新答案；Tutor实际生成质量沿原验收记录的范围。二维码有效期以用户原图为准，过期后需替换。

工作区既有未提交改动全部保留；本次未提交Git。发布入口脚本位于 `tmp/current-release-20261015/`，凭证不进入运行证据或仓库。
