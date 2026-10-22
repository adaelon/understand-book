# OM0–OM6 正式发布 — 2026-10-20

2026-10-20 20:34:32 HKT 已发布到 https://understandbook.top/，release `om6-20261020`，进程 PID `471137`。本地及公网 API、静态产物验收通过，停服至恢复入口耗时 14.84 秒。

## 发布内容

现网 `presentation-repair-20261019` 加 57 个 OM 源文件，包含实际页面观察与输入／已应用状态区分、持久化学习接续、当前请求优先的教学支撑、教师页面交互与取消／接手语义、共享讲解规约 `plain-explanation.v3`。网络 Reader 静态产物、Server 和沙箱 worker 同时切换，管理后台沿用现网产物。

[源文件清单](source.json)及[相对现网的补丁](overlay.patch)保存范围。数学结构、自动恢复阅读、登录期限和构建调度等并行工作未纳入此发布。本地原工作树保留。

## 验证

- Linux 六个 release 程序构建通过；[构建日志](build.log)。
- 独立发布源码的 Reader 类型检查、生产构建通过；交互执行、文档、组件测试共 30 项通过。
- 独立发布源码的四项 Server 回归通过：OM6 三组持久化场景、本地交互取消、网络保存回执、帮助记录在取消与重开后保持。[回执](isolated-verification.json)、[测试日志](isolated-tests-low-memory.log)。
- 从旧备份恢复独立数据副本，用新版程序启动；reader/admin 登录、授权材料及正文、学习工作区、四项制作能力通过。[副本验收](api-preflight.json)。
- Linux 实际 nobody/systemd/bwrap 沙箱，在 320×420、640×240、960×720 三种视口执行设置速度 2 与应用动作，已应用速度为 2、结果为 20 米，恢复检查及页面问题列表均通过。[交互验收](worker-preflight.json)、[桌面图](desktop-2.png)、[窄屏图](narrow-2.png)。
- 切换前后 reader/admin 的授权版本清单与能力一致；源站 HTML 逐字节比对通过，公网入口引用的资源清单一致，关联静态产物逐字节比对通过。[发布回执](deployment.json)。
- 公网浏览器完成 Reader 工作区、共享目录、额度、1440/390px PDF 阅读及章节入口、管理员构建／发布／用量页面共七条检查；脚本错误 0。[验收结论](acceptance.json)与[浏览器原始回执](public-browser.json)保留。

部署验证模型调用 0。教学效果和真实模型对照沿用 [OM6 实施记录](../om6/README.md)。

## 数据与恢复

数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 18 保持。切换前确认活动／未保存问答为 0，维护窗口停服后由旧版维护程序完成完整备份：

- 数据备份：`/opt/understand-book/backups/om6-20261020`。
- 旧配置：`/opt/understand-book/acceptance/om6-20261020/previous-configuration`。
- 旧 release：`/opt/understand-book/releases/presentation-repair-20261019`。
- 远端发布记录：`/opt/understand-book/acceptance/om6-20261020`。

已有标签页刷新一次加载新版。需要回退时先保留上线后数据，再按 MU9 判断旧程序可读性；不要用切换前备份覆盖后来的聊天与学习记录。

## 已知限制与构建调整

PDF 小地图中文标签接口 `reader/paper_minimap.localize` 返回 501，中文标签本地化不可用；PDF 正文与章节入口正常。10 月 19 日的[既有验收](../../full-release-20261019/acceptance.json)已记录同一问题。本次严格浏览器断言因此失败，原报告保留；这是本次七条检查中唯一的 HTTP 失败。

服务器未安装 pnpm，首次统一构建退出 127；随后采用既有发布流程，在独立本地源码目录构建前端、上传静态产物，在最终 Linux release 目录构建程序。

独立测试初次缺少四个旧夹具新增的 `action_contract` 字段，补齐 OM 对应夹具后，Windows 编译器发生内存分配失败。最终以单任务、关闭增量和调试信息构建，四项测试通过；[内存失败原日志](isolated-tests-retry.log)保留。组件测试首次受复用依赖目录的 Vite 读取范围限制，调整仅限验证目录的允许路径后 19 项通过；其余 11 项首次通过。产品源码未因这些环境问题修改。
