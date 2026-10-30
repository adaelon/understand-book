# 实时监控验收 — 2026-10-28

本轮完成后台 `/admin/monitor`、服务资源采集、在线心跳及 AI 对话统计。本地源码交付，未发布或重启正式服务。接口与指标定义见 [实时监控实现](../../运营后台-实时监控.md)。

## 自动验证

- `cargo test -p server --lib monitor_ -- --nocapture`：6 项通过，见[终端结果](backend-tests.txt)。覆盖管理员与 CSRF 边界、心跳不装载私人现场、运行/排队/未保存状态、内存可用量、磁盘预留、网络接口去重/时间差/计数重置、在线账号去重/退出/停用/过期、香港日期边界、共享缓存及趋势上限。
- `pnpm -C apps/admin test`：17 个文件，73 项通过，包含监控刷新/暂停/恢复、断线保留与空值展示。
- `pnpm -C packages/web test -- src/account-presence.test.ts src/NetworkApp.guest.test.ts src/NetworkApp.recovery.test.ts`：14 项通过，覆盖可见页面、退出/卸载、失败重试，以及现有身份恢复、草稿保留与密码重置流程。
- `pnpm -C apps/admin build` 与 `pnpm -C packages/web build:multi`：生产构建通过。

本机 C 盘空间不足曾导致测试临时文件写入失败，临时目录切到仓库 `tmp/monitor-os-temp` 后完成验证。一次后端编译被其他正在修改的运行时代码阻塞，工作区该处修复后重新执行通过；本轮未修改该运行时模块。既有包体积与依赖构建提示保持。

## 页面与交互

运行 Admin 开发服务后执行 `node apps/admin/scripts/monitor-browser.mjs`。浏览器加载真实 React 页面，以受控 HTTP 样本验证显示；这些截图为测试数据，不代表线上业务人数。结果见 [browser-results.json](browser-results.json)。

- [桌面 1440px](desktop.png)：四项业务统计、资源卡片、四组趋势、采样滑块和统计口径。
- [手机 390px](mobile.png)：双列业务卡片、单列资源与趋势，无页面横向溢出；[图表局部原图](mobile-chart.png)保留坐标和采样时间的清晰细节。
- [深色模式](dark.png)：图线、数值和控件对比度可读。
- [连接错误](connection-error.png)：保留上次采样、明确提示错误，可恢复刷新。
- [部分指标不可用](partial-unavailable.png)：内存与网卡不可用不影响磁盘、在线及对话统计。

人工直接查看上述真实页面截图，调整了趋势坐标字号与滑块样式，手机坐标不再随 SVG 整体缩小。自动操作覆盖定时刷新、暂停、暂停后手动刷新、恢复、错误恢复、主题切换、采样选择和统计口径展开。

## Linux 实机采集

从 `crates/server/src/server_monitor.rs` 原样抽取资源结构及采集/差值函数，放在服务器的独立临时探针中编译运行；没有装载业务数据、修改部署配置或重启服务。原始结果见 [linux-collector.json](linux-collector.json)。

本次采集到约 3.63 GiB 总内存、39.08 GiB 文件系统及 `eth0` 入站/出站速率。一次系统资源采集约 0.28 ms，仅说明本次独立 OS 读取耗时，不代表完整 API 响应、长期平均或并发承载测量。后端业务统计由本地真实 SQLite/HTTP 分发集成测试验证；尚未做 Linux 正式发布包的整站联调。

## 2026-10-28 上线补记

20:28 已随 `editable-mind-map-20261028` 合并发布，20:33 公网复核通过，原有记录保留。先前未上线描述指当时的本地验证。见[完整发布记录](../combined-release-20261028/README.md)。
