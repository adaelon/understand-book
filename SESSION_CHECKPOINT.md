# SESSION_CHECKPOINT — 2026-10-15，当前工作区与首页更新已上线

## 新鲜度自检
- 写入时最新 commit：`6612932 ops: publish approved logo and latest reading updates`；大量既有未提交改动保留。
- 读入时对比 `git log -3`；产品运行版本为该提交加工作区发布快照，见发布记录。
- 用户已明确“先不用做了直接上线吧，把目前没上线的给上线”；随后要求首页加入内测群二维码及类似参考图的网站信息页脚。

## 当前在做什么
- 本轮发布与首页增量已完成，正式入口 https://understandbook.top/ 。
- 15:15:29 HKT 发布 `current-20261015`：正文/PDF专注阅读、精确读位与来源接续、相关阅读器修复、Tutor `tb4.v5` 及现有构建等待修正。
- 15:30:19 HKT 上线首页内测群原图；15:53:32 HKT 上线产品说明、版权、服务条款、隐私说明、联系我们与关于我们页脚。
- 现网 `/opt/understand-book/releases/current-20261015`；服务 `understand-book-multi` active、PID101669；数据根 `multi-reader-to-tb-20261012`，schema13，均沿用。
- 原 release `tutor-question-discrimination-20261014`、静态资源和恢复配置保留；本次未提交Git。
- FP7 heartbeat `fp7` 为 PAUSED；不自动续跑长测、不等待或干预其他构建、不向其他聊天发消息。

## 下一步（可直接接手）
1. 本轮无待发布内容。先读 `docs/performance/current-release-20261015/README.md` 核对现网、证据与恢复点，再处理用户新请求。
2. 用户提供新二维码时，替换 `packages/web/src/assets/home/beta-group-20261015.png` 并同步 `ProductHome.vue` 有效期；现图10月22日前有效。
3. 仅在用户重新要求FP7验收时，读原FP7 README和 `tmp/fp7-finish.ps1`，先核对当前源码、构建及环境再续跑，保留已有成功与失败原件。

## 未提交 / 未完成
- 工作区所有既有未提交源码与材料保留；本轮产品改动已上线，相关文档已更新。
- FP7 完整性能联合验收未完成，不标记通过。待测：WebKit严格本地/弱网各200次、WebKit与Chromium各20分钟且500次正文长读、修正后的触摸帧、最终联合汇总及资源曲线。
- 原Chromium长读字号回返不一致、WebKit外部内存负载与PDF首轮52/50/60ms长任务未归因记录均保留。
- 公网冷PDF邻页准备曾超过30秒功能等待；诊断约20秒后完成，随后往返/退出正常，详见 `pdf-diagnostic.json`。
- 公网桌面正文/PDF各前后2屏及退出通过；390px补充阅读检查受连接中断影响未完成。源码侧既有功能证据沿FP7记录。
- 二维码本地/公网三尺寸通过；页脚本地1440/390/320px、公网1440/390px通过。公网320px页脚新连接中断，未完成；相关失败保留。

## 冷启动读序
1. `docs/performance/current-release-20261015/README.md` — 发布、首页改动、验证范围与恢复点。
2. `docs/Linux多人阅读-现网运行说明.md` 的最新状态及“入口与版本” — 现网配置与数据。
3. `docs/performance/focus-reading-fp7-20261015/README.md` — 既有证据与暂停的剩余验收。
4. `docs/代码链路.md` 末尾两节 — 首页内测联系与网站信息页脚实现。

## 本会话决策摘要
- 用户暂停FP7剩余验收并授权发布，已上线状态与性能验收状态分别记录；ADR-0166保留冻结门槛。
- 首页联系入口采用用户原二维码；网站信息使用现有页面中的可访问说明窗口，见发布README与代码链路。
- 旧数据、旧release及回退配置保留；其他书籍构建继续自然运行。
