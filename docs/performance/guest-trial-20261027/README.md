# 内测码游客体验，2026-10-27

2026-10-27 12:03:30 HKT 已随 `board-chain-guest-20261027` 与画板链路合并上线。数据库结构沿用 schema 20。真实现网副本兑换与公网入口结果见[合并发布记录](../board-chain-guest-20261027/README.md)。

## 行为

首页“输入内测码，开始体验”进入 `/?account=guest`。填写未使用的内测码后，创建独立游客身份并直接进入资料库，同时一次性发放 5 元体验额度。会话与额度期为 30 天。

材料来自发码管理员已获授权的默认发布，排除项目书 `understandbook`。同一本书的旧发布和其他读者私有材料不自动授权。每位游客的笔记、对话与扣费沿用现有用户隔离机制。

码的核销、身份、会话、额度和材料授权在一个事务中提交。失败全部回滚；同一码在原游客有效会话中重试不会再发额度，其他浏览器不能凭已使用的码接管记录。赠送使用既有额度账本，实际收款记录为零。

## 实现入口

- `crates/server/src/guest_trial.rs:redeem`：兑换事务、资料范围和一次性赠送。
- `crates/server/src/auth.rs:write_session/is_guest`：复用持久会话、标记游客身份。
- `crates/server/src/multi_user_host.rs:dispatch`：`POST /api/auth/guest` 与身份查询。
- `packages/web/src/components/GuestAccess.vue`、`NetworkApp.vue:enterGuest`：单字段入口、断线恢复与进入书库。
- `AccountSettings.vue`、`AccountAllowance.vue`、`ProductHome.vue`：游客说明、人民币余额与首页入口。新增文案包含英文翻译。

## 验证

| 范围 | 证据 |
| --- | --- |
| 后端 25 项通过 | [回归日志](server-regressions.log)：游客兑换、默认发布与排除、独立身份、扣费后重试不重置、并发一次赠送、授权失败回滚、原邮箱注册与密码登录 |
| 前端 27 项通过 | [首次 27 项](web-tests.log)；后续受影响的 22 项[复验](web-final.log)：单字段提交、重复点击、已有会话、断线重试、书库进入与刷新、设置及额度面板、完整翻译目录 |
| 多用户生产构建通过 | `pnpm -C packages/web build:multi` 包含类型检查；后续样式调整通过生产构建 |
| 真实浏览器流程通过 | [浏览器日志](browser-tests-final.log)：Rust HTTP 服务与正式前端产物，通过测试代理模拟 HTTPS 反向代理；使用真实会话 Cookie、数据库、授权和额度。覆盖 1440 px 桌面与 390 px 手机、中文和英文、无效码、兑换、余额、刷新、不同游客和实际打开阅读页 |

验收没有调用付费模型；扣费后余额测试使用确定性用量记录。浏览器中的书籍为隔离测试材料。入口、余额、书库、阅读页的实际渲染截图均已逐张检查。

![桌面入口](guest-desktop.png)

![手机入口](guest-mobile.png)

![游客余额](guest-allowance.png)

复跑真实浏览器：先运行 `cargo test --offline --locked -p server --lib guest_trial_browser_host -- --ignored --nocapture`，再在 `packages/web` 用 `vite preview --outDir dist-multi --host 127.0.0.1 --port 4195 --strictPort` 服务正式产物，最后运行 `playwright test --config playwright/guest-trial.config.ts`。测试后请求 `http://127.0.0.1:4197/stop` 停止隔离后端。

本机大型 Rust 测试库默认编译栈曾溢出；将本次验证进程的 `RUST_MIN_STACK` 设为 `33554432` 后正常编译。前端构建与 Rust 大型编译分开执行，避免本机同时编译时的内存不足。未修改项目编译配置。

## 使用边界

游客暂不提供自助绑定邮箱、跨设备登录或会话丢失后的自助恢复。清除浏览器数据、主动退出或会话到期后，原码不能再登录；退出前已有确认提示。额度用尽后，有效会话仍可读取已有授权内容与记录。合并验收已核对实际可授权默认材料为 13 份，正式入口已发布；没有消耗正式内测码。

合并时发现既有新账号默认授权会与游客授权重复，已在新游客同一建号事务内替换继承授权，再按游客材料选择授权。新增真实默认授权回归及原游客回归 5 项通过；详见[合并问题与证据](../board-chain-guest-20261027/README.md)。
