# MU8 前端登录、多现场与断线连续性

日期：2026-09-30。依据 ADR-0147 §7–§9、多人切片方案 MU8 与 RE §5.1。

## 入口与交付

多人 Web 显式以 `VITE_MULTI_USER=1 pnpm --filter @understand-book/web build` 构建。`main.ts` 选择 `NetworkApp.vue`；默认构建保留本地与桌面入口。Server 与 Web 必须来自同一 release。Nginx 示例 `scripts/linux/nginx-multi-reader.conf.example` 将静态构建置于 `/srv/understand-book/web`，`/api/` 转发至原多人服务；HTTPS Cookie、CSRF、原点约束仍由 MU4 承担。

登录页读取 `/auth/me`，显示制作 capability，列出当前授权发布包。选择材料才创建工作区；恢复页面不会自动新建聊天或提交问题。首次明确发送问题可创建该现场的聊天。

## 用户与现场生命周期

`network-context.ts` 保存内存身份、CSRF、workspace、attachment、generation 与 PublishedBookRef。`sceneKey` 包含服务器 generation，用于请求、聊天组件与附属页的现场隔离；`readerKey` 用于整个阅读应用的挂载。同一材料内成功的新建、选择聊天命令保留 Reader 的挂载代次，正文、PDF 与设备布局继续使用原实例；聊天子树重新挂载，旧 SSE 观察、来源浮层、输入草稿和演示选择被清理。首次提问自动建聊保留正在提交的问题。

`networkFetch` 仍核对请求开始和返回时的完整场景，旧响应拒收。聊天组件的 `bindSceneApi` 绑定 `sceneKey`；保留的 App 与 Tutor 控制绑定 `readerKey`，后续操作使用新的请求现场。旧提问的迟到接单结果不能切回旧聊天。换材料、换账号、连接恢复引起的换代仍重建整个应用；对话创建成功但历史读取失败时，在对话区提供恢复入口。

每页 attachment 都在登录恢复时重新生成。普通标签页使用自己的 sessionStorage，只保存当前用户的 workspace ID。刷新先读取服务检查点再 attach；复制标签页携带同一个句柄时，由 MU5 在仍有主连接的情况下 fork，不能借复制的浏览器状态接管原窗口。设备上的阅读表面偏好按 user 命名空间读取。

退出立即卸载私人页面，清除该用户的本页待核对请求和现场句柄；先尝试 detach，再撤销 Cookie。BroadcastChannel 通知同浏览器的其他页面清理身份。回到前台、pageshow、online 时复验身份和现场；登录会话改变、恢复连接导致的工作区换代或发布绑定改变都会重建场景。旋转、键盘和输入法事件只影响既有布局。

## 提交与观察

`network-client.ts` 在第一次 POST 前，把原 UUID 请求键、完整请求体与原 workspace 写入该用户的 sessionStorage。完整请求体包括聊天、发布包、attachment、generation 和 revision。响应丢失后按原键 GET 接单；刷新和后台恢复只查询。无法确认时保留原问题与“核对原问题 / 重试原提交”，显式重试先查接单再使用原键、原请求体。

同页工作区命令依次提交；每条命令先 GET 当前 revision，再做 CAS。代次改变时刷新现场并拒绝旧命令，不自动重放导航或私人写入。并发窗口的真正 CAS 冲突交给用户重做当前操作。

Run 描述带原 workspace、generation 和发布绑定。SSE 将 `Last-Event-ID` 作为 opaque 游标保存，`run.snapshot + live_buffer_reset` 原子替换旧缓存；跨服务器 epoch 的较小序号也可重置。运行结束继续保存到原聊天，旧 Reader 效果不能应用到新现场。排队、模型等待、取消、未保存、中断与确认过期分别显示；未保存按钮只调用原 turn 的 retry-save。

## 来源、PDF 与附属演示

正文、资源和 PDF URL 指向确切发布包。来源解析在原用户的 History / 演示版本中找原 turn，再核对材料；来源导航只写指定工作区。附属来源导航显式通知原窗口刷新 Reader 状态并定位原引用；普通前台恢复只核对身份、现场与运行，保留当前滚动位置。

“在附属窗口打开”使用父窗口显式创建的 WindowProxy，双方检查同源与 opener，交换服务端签发的 linked attachment。跨窗口只传普通的 ID、版本和现场投影；iframe 仍沿 MU7 每次装载独立通道。附属页的追问携带原 session/turn 的现场回执；父页切书、换聊天、退出或离开会使附属页失效，并提示重新打开。

演示读与 observe 经工作区绑定读取；只恢复该现场保存的 presentation receipt。另一个普通窗口打开同一旧回答时，不会偷取用户级最新滑块状态。显式“回到提问时现场”继续用历史回执恢复。

## 后端共用入口

`workspace_client.rs` 直接借用原 UserRuntime / ReaderWorkspace，承接 `chat/history`、`profile/manifest`、memory 命令、来源解析及 Tutor 展示/动作。画像读取、用户治理和 Goal 取消提取为共用的借用函数，本地路由与网络现场共用原 Store、历史保存与确认逻辑。独立后台复核仍沿已有能力边界。

`WorkspaceRegistry::persist` 使用 SQLite IMMEDIATE 事务，在读取检查点序号前获得写事务，避免与登录/运行终态写入并发时出现读事务升级失败。数据模型仍为 control schema 4、FrozenTurn v1。

## 验证与限制

验证命令、通过数与故障修正见 [MU8 验收记录](performance/linux-multi-reader-mu8-20260930.md)。浏览器测试运行真实 Vue 构建、真实 Rust HTTP 服务与固定模型替身，Cookie/CSRF、工作区和私人存储均走真实入口；测试夹具通过 Node HTTPS 服务器向真实 Rust 服务转发，浏览器自行接受 Secure/HttpOnly Cookie；仅使用本地测试证书。

真实 Nginx TLS、实体 iPhone Safari 的选择/键盘/后台行为、生产进程断电恢复和五分钟容量门槛仍属于 MU10/MU11。浏览器视口与 WebKit 验证不替代设备验收。本切片未部署生产服务、迁移个人数据或调用付费模型。翻译、本地化与独立后台复核保持原多人能力边界。
