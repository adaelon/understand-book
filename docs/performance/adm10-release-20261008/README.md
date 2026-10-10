# ADM10 Linux 发布验收 — 2026-10-08

正式入口 https://115.190.121.150，后台 https://115.190.121.150/admin/。release `/opt/understand-book/releases/adm10-20261008`，服务 active，schema 8。发布的是 `4e0b68e` 基线上的工作树，尚未提交。源码范围见 `source-files.txt`；同目录构建收据见 `multi-reader-build.txt`。

## 范围与首批配置

ADM1–ADM10、EX14.0–EX14.3（含 Manim、ex14.v4）及近期 Reader 目录、手机问答/键盘、表格/引用/划选、运行活动折叠已同版发布。EX14.4 自然 A/B/C 对照和 EX14.5 完整连续使用仍待独立验收。

reader、puff、adaelon、admin 各赠送 10 元模型成本额度，2026-10-08 22:26:37 至 2026-11-07 22:26:37（香港）。9 项默认材料和 10 个发布版本保留；原读者授权不变。售价为官方成本加 10%，收款 11 元授予 10 元额度。联系入口 19847595883。赠送未制造收款记录，演练模型调用留在独立候选根。

## 证据

| 证据 | 结果 |
| --- | --- |
| `migration.json` | 实际旧快照 schema 4→8；3账号、6会话、10发布、30授权、9默认、39现场、8接单、169私人文件保留；运营新事实为空 |
| `restore-current.json` | schema 8 候选备份恢复；16张表、176私人文件、费率配置一致，含21次真实模型费用 |
| `deployment.json` / `production-accounts.json` | 正式切换、活动进程、发布范围及首批额度回执 |
| `production-browser/release-browser.json` | 真实 HTTPS 四组通过，无脚本错误；后台刷新/资源/认证/CSRF/退出同步/普通账号拒绝/手机额度，模型请求0 |
| `live-calls.json` | 21次发送的身份、预占、真实用量、费率快照、结算及回执；均 settled，无 pending |
| `presentation-verification.json` | 长任务完成并交付 presentation-1791469383816308218-2 revision1；3次发送包含实际图片观察 |
| 本目录6份测试日志 | 恢复12、装配3、额度5、Reader92、目录/表格浏览器7、EX14 Runtime89通过 |

新建 `crates/server/src/tests/adm10_tests.rs` 覆盖真实维护入口的旧快照升级、新账目/私人数据恢复与费用恢复幂等；`scripts/linux/smoke-admin-release.mjs` 可重跑两套前端的正式部署检查，不调用模型或变更金额。生产截图已查看，额度与联系方式正确，无横向溢出。

## 真实费用核对

短问答及含交互呈现的长任务完成，共21次 Provider 调用；本地按真实 usage 与官方人民币费率逐次向上取整，合计 **0.721157 元**（短答0.008703元）。当前官网再次直接读取仍为闲时缓存命中/未命中/输出每百万 **0.02/1/4 元**。费率未加入10%销售加价。来源：https://api-docs.deepseek.com/zh-cn/quick_start/pricing/ 。

Provider 余额首次查询只减少0.20元，后续减少 **0.55 元**；用户提供同小时 deepseek-flash 图表显示 **0.54 元**。本地折算与最新余额差 **0.171157 元**，尚无逐请求账单可以解释或分配该差额。`live-verification.json` 保留初次观测，最新核对见 `billing-note.json`。不将余额差当逐次账单，不据此猜测折扣或改变费率。各口径均低于本次3元预算。

## 恢复点与已知限制

正式切换前最新备份 `/opt/understand-book/backups/adm10-production-20261008/service-snapshot`；同级 configuration 保存环境、Nginx、systemd 与 worker 配置。旧 release 和演练快照全部保留；候选进程及仅回环访问的候选站点已关闭。回退依照发布运行单，新写入发生后先保存新库，不用旧余额覆盖。

费率区间到2026-11-08 00:00香港，续期前须更新适用区间并重启装载。费用差额待逐请求账单核对。EX14自然对照、完整连续使用、实体手机及Linux Manim专项兼容仍未补做；本次实际交互呈现不替代这些实验。既有MU4/MU10夹具失败和构建体积提示见切片方案第10节，不声明全仓测试通过。
