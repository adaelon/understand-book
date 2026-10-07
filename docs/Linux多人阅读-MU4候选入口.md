# Linux 多人阅读 MU4 候选入口

MU4 提供应用登录、授权书库与私人对象读取入口；MU5 已增加 [现场与 Tutor 接口](Linux多人阅读-MU5现场接口.md)。它是隔离候选服务，后续接单、执行隔离、Reader 前端和部署门槛沿 [切片方案](切片方案-Linux原生多人阅读与无Redis首版.md) 执行。既有单读者部署继续使用原命令和配置。

## 账号和材料准备

服务与维护工具独占同一个服务根。以下操作在该根的服务停止后执行；不指向旧单读者数据目录。

```bash
cargo build --locked -p server --bin server --bin manage_reader --bin publish_book
# 输入密码不放在命令参数或 shell 历史中。密码为 12–1024 字节。
read -rsp 'New reader password: ' reader_password
printf '%s' "$reader_password" | target/debug/manage_reader /srv/understand-book create reader_a
unset reader_password

target/debug/publish_book /srv/understand-book import /srv/prebuilt/book
# 使用 import 返回的确切 book_id / publication_id。
target/debug/publish_book /srv/understand-book grant reader_a BOOK_ID PUBLICATION_ID
```

`manage_reader <root> password <user>` 同样从标准输入读取新密码。`disable / enable / revoke <user>` 分别禁用、重新启用、撤销全部登录。改密与重新启用均不会恢复旧 Cookie。`publish_book <root> revoke <user> <book> <publication>` 撤销材料授权。管理操作只在受信 CLI 中提供；普通应用账号都是读者。

## 启动与代理

```bash
target/debug/server --multi-user /srv/understand-book \
  --origin https://reader.example --addr 127.0.0.1:8787
```

`--multi-user` 必须作为第一个参数，服务根为绝对路径，站点 Origin 必须是 HTTPS。后端仅允许 loopback 监听。它不读取本地默认 Memory/History，也不从 Header 或请求正文选择用户。

使用 [多人 Nginx 模板](../scripts/linux/nginx-multi-reader.conf.example)，填入自己的域名与证书；Origin、固定 Host 和实际 HTTPS 地址须一致。所有资源都反向代理至应用，不设置发布包或用户目录的公共静态 location。模板关闭代理缓存、SSE 缓冲和包含搜索参数的访问日志。代理清除外部身份头；应用不会信任这些头。生产 TLS、SSE 代理和真实设备验证归 MU11，本轮未部署。

访问根页面登录，查看获授权目录并退出。页面由同一服务内置，无需将旧 Reader SPA 当作已接入多人协议。会话 Cookie 为 `__Host-ub_session`，Secure、HttpOnly、SameSite=Lax、Path=/，有效期 12 小时，每账号最多 8 个有效会话。登录轮换当前浏览器的旧会话；数据库只保存令牌摘要。密码使用 RustCrypto Argon2id，参数 m=19456 KiB、t=2、p=1，并独立生成盐，API 依据 [argon2 0.5.3 官方文档](https://docs.rs/argon2/0.5.3/argon2/)。

## 请求合同

| 入口 | 权限与本片行为 |
| --- | --- |
| `/`、`/login`、`/auth.js`、`/auth.css` | 仅固定、不含私人信息的登录壳可匿名读取 |
| `POST /api/auth/login` | 验证配置 Origin/Host；按账号每分钟 5 次、全站每分钟 60 次尝试，密码哈希同时最多 1 个 |
| `GET /api/auth/me` | 返回当前身份与 CSRF token，不返回会话令牌 |
| `POST /api/auth/logout` | 可信 Origin + X-CSRF-Token；撤销会话并清空 Cookie |
| `GET /api/library`、旧 `/book/library` | 仅当前账号获授权发布的目录元数据 |
| `/api/books/{book}/publications/{pub}/{leaf}` | 白名单正文/结构/映射/PDF/图片；先验证材料授权，再处理 GET/HEAD；Range 返回完整 200，条件请求不返回绕过授权的 304 |
| `GET /api/me/chats`、`.../{id}`、旧 `/agent/history?session_id=...` | 仅当前用户私人历史；不暴露原始模型消息、Provider continuation 或文件路径 |
| `POST /api/me/presentation.read/observe`、旧 `/agent/presentation.read/observe` | 按用户、原聊天/回合、确切发布和已交付演示版本读取；无公开候选文件目录 |
| `GET /api/agent/runs/{turn}` | 当前用户回合状态；MU6a–MU6b 增加持久接单及未保存状态 |
| `.../{turn}/events` | 已保存终态的 SSE 快照；使用共同授权观察器。后续活动流每次推送前复核会话与原材料，撤销关闭观察 |
| `POST .../{turn}/cancel` | 他人回合返回 404；已终局回合幂等返回原状态；MU6b 已接通排队取消和在途协作取消 |
| `/api/workspaces/{id}` 及现场子路由 | 先检查 owner/材料，再按 MU5 的 attachment/generation/revision 处理现场创建、挂接和操作 |
| 旧聊天 select/delete | select 使用 MU5 现场合同；delete 使用 MU6b 同聊天占位与全现场清理 |
| `/api/me/tutor/state`、`/api/me/tutor/mutate` | 用户级原 Tutor 控制与 revision/operation_id；兼容旧 `/tutor/*` 控制端点 |
| Provider、预构建、raw dir、原始观测及其他未开放旁路 | 读者能力白名单拒绝，旧别名与 `/api` 路由一致 |

所有写请求要求可信 Origin 和当前会话对应的 `X-CSRF-Token`，登录也检查 Origin。客户端不能通过 `user_id / owner_user_id / dir / book_dir / service_root` 切换身份或路径。响应禁用缓存；所有私人物件查找限定用户后进行，不存在和不归属统一 404。HTTP 与内部存储错误不回显操作系统路径。

## 已知限制

本片在 Windows 临时服务根验证，HTTPS 由配置合同约束，未验证真实 Linux 代理、证书、只读挂载或 iPhone。管理命令需停服务并取得写锁；尚无在线管理 UI。

MU6a–MU6d 已接通持久接单、公平并发、指定取消、聊天删除占位和连续观察，见 [MU6 运行接口](Linux多人阅读-MU6运行接口.md)。生成代码执行隔离归 MU7；公平调度与跨重启观察见 MU6c/MU6d 记录。当前登录壳有账号切换与晚到响应保护，完整 Reader 缓存/设备连续性清理由 MU8 接入。
