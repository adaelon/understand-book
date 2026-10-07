# MU11 Linux 多人阅读发布运行单

使用同一源码目录构建 Server、维护工具、执行器和网络 Web；部署配置、数据恢复点和验收结果共同确定可开放的版本。当前实施及门槛见 [MU11 记录](performance/linux-multi-reader-mu11-20261001.md)，既有矩阵见 [MU10](performance/linux-multi-reader-mu10-matrix.md)。

现网已于 2026-10-01 部署到 `https://115.190.121.150`，实际版本、账号、证书续期与恢复点见 [现网运行说明](Linux多人阅读-现网运行说明.md)。

## 版本与目录

在最终 release 目录运行 `bash scripts/linux/build-multi-reader.sh`。脚本安装锁定依赖，生成独立的 `packages/web/dist-multi`，构建 `server / manage_reader / publish_book / reader_maintenance / presentation_worker`，成功后写 `multi-reader-build.txt`。构建收据包含实际目录、时间、commit、工作树状态、工具链和 profile；工作树有修改时，交付须同时保留源码，不能仅靠 commit 重建。

服务使用 [多人 unit](../scripts/linux/understand-book-multi.service)，环境使用 [多人配置](../scripts/linux/multi-reader.env.example)，代理使用 [多人 Nginx 模板](../scripts/linux/nginx-multi-reader.conf.example)。将两个 unit 路径及 Nginx 的静态根中的 `MU11_RELEASE` 替换为同一最终目录。

| 对象 | 选定路径 / 配置 |
|---|---|
| 新服务数据 | `/opt/understand-book/data/multi-reader`，独立于旧 lx6 根 |
| 环境文件 | `/opt/understand-book/multi-reader.env`，root 所有、0600 |
| 新服务 | `understand-book-multi.service`，运行用户 `understand-book` |
| 后端 | `127.0.0.1:8788`，仅 loopback |
| 模式 | `UNDERSTAND_BOOK_MODE=multi-user` |
| 旧资料唯一归属 | `reader`；其他账号从空私人空间开始 |
| 备份 | `/opt/understand-book/backups/<operation-id>`；不可覆盖已有恢复点 |

`start-reader.sh` 只按显式模式选择入口；多人模式要求绝对服务根和 HTTPS origin，缺失则退出。后端校验 origin、loopback、写锁、schema 和能力。原单读者配置继续使用默认 single-user 模式。

## HTTPS 与代理

设置 `UNDERSTAND_BOOK_ORIGIN` 为浏览器真正使用的 HTTPS origin。Nginx `server_name` 为相应主机，`proxy_set_header Host` 必须含 origin 中的非默认端口。证书 SAN 必须覆盖主机；TLS 私钥放在 Nginx 管理目录，不能放入 Web 根。

没有域名时可使用 IP HTTPS，但证书须被客户端信任且覆盖该 IP。隔离测试可使用单独的测试 CA / 证书及 SSH 隧道；浏览器忽略证书错误只能证明应用与代理路径，不能签署公网可信 HTTPS 或实体 iPhone 验收。

Nginx 只公开网络 Web 静态目录，所有 `/api/` 原样转发到同一后端；包含 PDF、来源、私人演示和 SSE。模板关闭 Basic Auth、代理缓存及响应缓冲，清除上游身份头，保留应用 Cookie / Origin / CSRF。普通读者 API、别名和禁止入口的权威清单为 `crates/server/src/authorization.rs:capability`；完整路由对抗证据见 [MU4](performance/linux-multi-reader-mu4-20260930.md)。

用 `nginx -t` 检测证书、语法和路径错误，通过后再 reload。匿名 API 必须 401；有效账号可登录；缺 CSRF 或错误 Origin 的写入必须 403；B 读取 A 的现场、聊天、来源和演示必须拒绝。确认 HTTPS 响应的 Cookie 含 Secure / HttpOnly / SameSite，实际 SSE 逐批到达。

## 迁入与恢复点

旧数据根以实际 `systemctl show understand-book` 和其 EnvironmentFile 为准。本次查得 lx6 的 memory/private 和 `/opt/understand-book/books`，旧 release 见实施记录。书库登记及 session 还包含书库外材料；每本都应保留并明确映射。

1. 在新根离线创建 `reader`，导入完整材料，保存每本 `publish_book import` 返回的 PublishedBookRef。密码通过 stdin 输入，不能写进命令行或报告。
2. 按 [MU9 计划](Linux多人阅读-MU9迁移恢复.md) 填写实际源根、唯一账号、独立备份目录及全部书籍映射，执行 `reader_maintenance preview`，处理逐项未知文件与缺失引用。
3. 停止旧实例和其他写入者，确认 `systemctl is-active understand-book` 为 inactive，再执行 `reader_maintenance migrate <plan> --stopped`。记录条数、revision、版本和映射，保留 operation 与 snapshot。
4. 在隔离入口启动新根；核对原笔记、聊天、学习、演示、目标成果和阅读位置。只有迁移完成且 MU10 / MU11 必要验收通过才切入口。

迁移中断时按同一计划续接，不手工删除 maintenance marker。不要复制正在写入的 SQLite 主文件作为备份。空间须容纳旧资料、一致性备份、发布副本和恢复演练副本；空间不够时先扩容或明确转移无用验收产物。

## 停止、撤权和回滚

`systemctl stop understand-book-multi` 触发 SIGTERM；服务停止接单和领取，协作取消在途工作，冲刷各用户阅读记录。unit 用 180 秒截止和 `KillMode=mixed`。核对实际退出状态和残留任务；超时强杀记为中断，重开后按原 turn 恢复，不把它写成有序成功。

账号禁用 / 改密 / 撤销会话及发布授权管理均为离线工具：先停服务，再执行 `manage_reader <root> disable|enable|revoke <user>` 或 `publish_book <root> revoke <user> <book> <publication>`，之后启动。普通用户退出只撤销当前会话。

停服后用同 release 的 `reader_maintenance backup <root> <new-backup>`，再 `restore <backup> <empty-independent-root>`；在隔离后端打开恢复根，核对原私人记录、发布绑定和接单状态。快照目录保持私人权限，工具不修改已有父目录权限；Nginx 必须能遍历 release 的父目录并读取网络 Web，私人数据根继续只允许服务用户访问。恢复演练成功后才能把该目录作为切换目标。

没有新写入时可回原独立实例。产生多人写入后，优先回兼容当前 schema 的版本；若只能恢复备份，先保留现根并明确备份后数据差额。旧单读者回滚使用 `export-user <root> reader <empty-export-root>`，用选定旧 release 读取导出的 memory/private/books；验证格式兼容后才切入口。

## 沙箱与交付证据

普通服务账户未获得受限执行器所需 systemd 权限时，保持网络制作关闭；从 `/api/auth/me` 记录实际 capability。启用制作时按 [MU7](Linux多人阅读-MU7受限制作.md) 配置与实测文件、网络、CPU、内存、进程、输出、取消及重启清理；权限安排改变后重新验证。固定服务账户不应仅为通过探针就整体改为 root。

制作部署同时安装 `scripts/linux/presentation-sandbox.json.example` 与 `49-understand-book-render.rules`：前者填入同 release 的 worker、独立只读运行时及浏览器路径后放入服务根，后者由 root 安装到 `/etc/polkit-1/rules.d/`，权限 0644。专用服务账号获得 `org.freedesktop.systemd1.manage-units`；这项管理权限属于受信任宿主，不授予读者或生成任务。systemd 252 的 `StartTransientUnit` 不提供 unit 详情，不能把按单元名过滤的规则当作可用部署方案，见 [systemd 252 实现](https://github.com/systemd/systemd/blob/v252/src/core/dbus-manager.c#L947)。不授予 `manage-unit-files`。生成任务继续以 nobody、原有命名空间与资源限额运行，不能访问宿主 D-Bus。权限和配置就绪后先以实际服务账号完成 MU7 验证，再重启服务，核对 `reason=ready`。

交付记录保留：源码及构建收据、实际 unit / Nginx 路径、系统依赖、SQLite 实际版本与 WAL/FULL/schema、资源预算、迁移 operation / 恢复点、授权目录与账号测试结果、Provider 请求数及实际 usage、SIGTERM / 强杀 / 恢复 / 旧程序回滚结果。证据只记录状态与数量，不保存密码、Cookie、Provider Key 或个人正文。

实体 iPhone 单独记录机型、系统、证书信任、竖横屏、中文输入、选区、来源返回、后台恢复和退出。Playwright Chromium / WebKit 用例另列。MU0 指定硬件与完整混合压力仍按原门槛判定。

浏览器部署冒烟可运行 `node scripts/linux/smoke-multi-reader.mjs`：设置 `READER_URL`、`READER_EVIDENCE_DIR` 与 `READER_BROWSER=chromium|webkit`，从 stdin 提供隔离账号 reader / reader-b 的密码 JSON；测试证书另设 `READER_TEST_CERTIFICATE=1`。脚本不提交模型问题，检查实际登录、版本区分、多标签、尺寸切换、账号隔离与刷新；真实问答需独立预算并记录请求数。
