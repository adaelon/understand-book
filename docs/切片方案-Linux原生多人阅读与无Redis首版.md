# Linux 原生多人阅读与无 Redis 首版切片方案

日期：2026-09-29；合同修订：2026-09-30。状态：MU0、MU1a–MU1c、MU2–MU6d 已完成本地实现与定向验证；MU7 已完成实现及 Linux 实机隔离验收；MU8 已完成前端接线与本地 HTTPS 双浏览器验证；MU9 已完成离线工具与本地隔离迁入 / 恢复验证，见 [MU9 记录](performance/linux-multi-reader-mu9-20260930.md)；MU10 实现与报告已交付，完整发布门槛未通过，见 [MU10 记录](performance/linux-multi-reader-mu10-20260930.md)；MU11 已完成优化构建、可信 IP HTTPS、生产迁入与入口切换，预构建失败按用户决定不挡此次部署，见 [MU11 记录](performance/linux-multi-reader-mu11-20261001.md)。MU1、MU6 分别按三个、四个子切片交付。

决策入口：[ADR-0147](adr/0147-linux-multi-reader-service-without-redis.md)。已交付范围和证据见 [MU0/MU1a 记录](performance/linux-multi-reader-mu0-20260930.md)、[MU1b/MU1c 记录](performance/linux-multi-reader-mu1bc-20260930.md)、[MU2 记录](performance/linux-multi-reader-mu2-20260930.md)及下列各片记录。MU2 建立服务元数据基础表，MU3 补发布登记与默认目录，MU4 接通鉴权，MU5 接通现场生命周期与用户级 Tutor；活动接单转换及后续端点仍按对应切片实施。

## 0. 已确认范围与交付边界

**FrozenIntent**：在单台 Linux、单个 Rust 主服务内，让受邀用户独立阅读同一已发布书库；每人拥有自己的 Agent、笔记、画像、聊天、学习证据和私人演示。一个人可使用多个设备 / 窗口，长期数据连续，当前阅读现场互不覆盖。

**已确认的技术取舍**：首版不使用 Redis；服务级 SQLite + 现有用户私有持久存储 + 进程内有界缓存、调度与事件通道。保留现有读时 Agent、来源证据闸、Rich Presentation、Tutor、预构建插件与桌面产品。

**交付边界**：本方案规定多人服务的实施合同、依赖与验收；当前已交付 MU0–MU5、MU6a–MU6d 的持久接单、取消/删除、公平调度与观察恢复；MU7 受限制作已通过四组 Linux 实机验收；MU8 已完成登录、多现场和断线连续性。MU9 离线迁入、备份与回滚已交付；当前收口 **MU10 完整发布门槛**，接手入口见 [SESSION_CHECKPOINT](../SESSION_CHECKPOINT.md)；[RE1–RE6](切片方案-阅读体验排版批注与动效.md) 已实现，RE7 正式验收继续保留，MU10 / MU11 尚有 Core 全量稳定性、正式容量与实体设备的未验范围；入口已上线。MU8 接口与证据见 [前端连续性](Linux多人阅读-MU8前端连续性.md)、[验收记录](performance/linux-multi-reader-mu8-20260930.md)。制作接口与验证见 [MU7 接口](Linux多人阅读-MU7受限制作.md) 和 [MU7 证据](performance/linux-multi-reader-mu7-20260930.md)。运行接口见 [MU6](Linux多人阅读-MU6运行接口.md)，验证范围见 [MU6c–MU6d](performance/linux-multi-reader-mu6cd-20260930.md)。

**ChangeType**：`[边界重构] [权限边界新增] [持久化扩展] [迁移]`。

**设计默认值**：管理员创建账号；应用级 Cookie 会话；用户共享获授权的公共材料，不共享私人数据；全局 Provider 由管理员配置。首版限制并发、排队、单轮模型预算与常驻内存，并记录实际 usage。现场数量、Book 常驻容量、待确认操作有效期、会话有效期和性能门槛分别在对应切片验收前冻结。

**非目标**：实时共读、协作编辑、共享聊天 / 私人笔记分享、团队与组织租户、开放注册、邮箱验证链路、BYOK、读者自主上传与预构建、分布式 Worker、多后端高可用、离线合并、桌面本地与服务器双向同步。本次不以“顺便升级”为由重写 HTTP 栈、Agent 循环、Learning 数据模型或预构建任务系统。

## 1. 当前实现与查阅边界

### 1.1 查阅基线

`project_overview` 于 `2026-09-29T15:30:13.233Z` 返回 HEAD：

```text
5e105166573c50ce294f2fa7d01544884f229fce
```

这是实时工作树，不是该 HEAD 的冻结快照；包含未提交与未跟踪修改。查阅发现 ADR-0141–0146 已存在，其中 0146 为已接受设计、实现待切片。`working_changes` 首批 100 项尚有续页，因此本次不声称已完整列出所有变更。实施者必须重新记录 HEAD、相关 diff、依赖和测试基线。

以下行号是本次 / 本轮架构讨论读取的定位依据，工作树继续变化时应以符号重新定位，不把行号当作不可变 API。

### 1.2 证据与实施接点

| 编号 | 当前事实 | 代码 / 文档定位 | 本次影响 |
| --- | --- | --- | --- |
| E01 | AppState 同时持 Book、Reader、Memory、messages、History 和活动流 | `crates/server/src/lib.rs:91–124` | 必须拆归属，不能只加用户表 |
| E02 | Req 只有 method、url、body、now | `crates/server/src/lib.rs:2387–2394` | 在受信 Host 边界形成 Principal / AuthorizedContext |
| E03 | book.open 接收 dir，并替换当前 Reader、Book、messages | `crates/server/src/lib.rs:2811–2878` | 多人端使用授权书库对象，作用于指定现场 |
| E04 | Host 从同一个 memory 目录派生 session 与 history | `crates/server/src/host.rs:1176–1185` | 每用户显式路径；窗口位置另行归属 |
| E05 | Host 创建一份 AppState、RunCoordinator、ReviewCoordinator | `crates/server/src/host.rs:1262–1296` | 分解服务、用户和现场；后台任务不再全站共享一个运行布尔值 |
| E06 | 运行读取、SSE、取消在 Host 单独处理 | `crates/server/src/host.rs:1389–1412` | 必须纳入统一对象授权 |
| E07 | 统计、翻译、地图本地化、观测存在 Host 旁路 | `crates/server/src/host.rs:1360–1387,1452–1480` | 权限、资源额度及路径注入需覆盖旁路 |
| E08 | 文件资源使用 guard.book_dir | `crates/server/src/host.rs:1482–1492` | 资源按发布包寻址，不依赖全局当前书 |
| E09 | Coordinator 的 Slot 只有一个 active | `crates/server/src/agent_run.rs:401–465` | 按聊天 / 现场保留冲突边界，新增服务级公平调度 |
| E10 | PreparedAgentChat 已固定 Book、turn_ref、messages | `crates/server/src/agent_run.rs:120–130` | 扩展归属，复用执行主循环 |
| E11 | RuntimeStatePort 通过权威 AppState 操作 Memory / Reader | `crates/server/src/agent_run.rs:24–77` | 替换为固定 owner 的受限状态端口 |
| E12 | MemoryStore 是内存文档 + 文件快照，路径可由进程环境覆盖 | `crates/memory/src/lib.rs:452–511,523–550,553–585` | 同用户统一写入口；禁止按请求切进程环境变量 |
| E13 | learning.db 位于 memory.json 的相邻路径 | `crates/memory/src/lib.rs:644–648` | Learning 跟随用户级根，不能跟随窗口复制 |
| E14 | TutorControl / TutorSession 有 revision，TutorMutation 带操作 ID 与期望版本 | `crates/memory/src/learning.rs:9–15,60–78,108–114` | 复用 CAS / 幂等合同，不另建教学状态真相 |
| E15 | PresentationStore 从 history_path 派生私人根 | `crates/server/src/presentation_store.rs:58–88,114–120` | 保留原成果路径布局，补用户 owner 与对象授权 |
| E16 | 私有目标成果根可由全局环境变量覆盖 | `crates/server/src/intent_build_store.rs:374–386`；`crates/server/src/host.rs:1270` | 显式注入用户路径，审计所有调用者 |
| E17 | 绘图执行生成 Python，并普通启动子进程 | `crates/server/src/presentation_plot.rs:46–58,89–122` | 多人模式必须沙箱化或禁用 |
| E18 | 前端使用统一 /api 和按 turnId 的事件地址 | `packages/web/src/api.ts:97–119` | 请求绑定现场 / 书籍，事件响应检查账号与现场 |
| E19 | App.vue 已读写设备侧来源偏好 | `packages/web/src/App.vue:444,2404` | 增加用户命名空间，不重建移动布局 |
| E20 | 当前文档仍限定单读者，HTTP Basic 不提供传输加密 | `docs/Linux阅读器部署.md:118,139–154` | 新增 HTTPS / 应用登录上线门槛；不推断线上已更新 |
| E21 | 历史权威、锁外执行、SSE 快照、重启 pending 中断已有明确合同 | `docs/adr/0127-resident-agent-streaming-and-runtime-activity.md:17–18,23–29,47–53,72–78` | 只扩展作用域与接单，不复制另一套 Agent |
| E22 | Tutor 按人全局、演示属于原聊天、更新生成独立基座 | `docs/adr/0141-global-tutor-control-and-session-ownership.md:7–37`；`docs/adr/0144-shared-presentation-conversation-workspace.md:7–37`；`docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md:9–15,55–72` | 避免窗口化后分裂教学目标或内容版本 |
| E23 | Agent 会读取实时 Reader 构造上下文；现端口一起借用 Memory 与 Reader | `crates/runtime/src/orchestrator.rs:5455,5582`；`crates/runtime/src/run_context.rs:239` | 分开私人写入、实时现场读取与效果应用，冻结必要现场输入 |
| E24 | 待确认画像操作仅保存在进程内，确认消息执行时消费它们 | `crates/server/src/lib.rs:1035–1041,11733–11792,20175–20216` | queued 恢复须判断临时依赖是否仍存在；用户回收须处理待确认操作 |
| E25 | 接单准备会写教学回应 / 绑定；同事件 ID 不允许载荷变化 | `crates/server/src/lib.rs:11638`；`crates/server/src/teaching.rs:196–215,278–291`；`crates/memory/src/teaching.rs:150–157` | 教学准备进入接单协议；按冻结载荷与既有回执续接 |
| E26 | 当前删除活动聊天会先取消并等待 Run 退出 | `crates/server/src/host.rs:1511–1517`；`crates/server/src/agent_run.rs:644–666` | 拆分 Coordinator 后保留聊天删除与运行生命周期的互斥边界 |

本节保留设计审阅时的 E01–E26 证据；全量入口、隐式状态和子进程清点已由 MU0 冻结，见基线报告及其 CSV 附件。

## 2. 领域归属与必须保持的不变式

### 2.1 四层对象

| 范围 | 拟议对象 | 权威职责 | 不允许持有 |
| --- | --- | --- | --- |
| 服务 | ServerServices | 认证、书库授权、发布包缓存、调度、受限 Worker、运维配置 | 某个“当前用户 / 当前书 / 当前聊天” |
| 用户 | UserRuntime | Memory、AgentHistory、Learning 访问、私人演示 / 成果、画像复核 | 其他用户的状态；唯一全局视口 |
| 现场 | ReaderWorkspace | owner、Book 绑定、Reader、selected_chat、generation、位置检查点 | 独立拷贝的同用户 Memory / 掌握度 |
| 运行 | ResidentRun / RunScope | 固定 owner、turn、原聊天、原现场、材料、必要现场快照和执行取消 | 动态读取新代次现场，模型自选 owner |

`ReaderWorkspace` 是本次新增的服务器侧独立阅读现场概念；现有 Vue `ReaderWorkspace.vue` 是界面组件，不因重名就已经实现服务器隔离。`AuthSession`、`AgentChatSession`、`TutorSession`、构建工作区各自保留领域语义。

### 2.2 红线

| ID | 不变式 |
| --- | --- |
| I01 | UserId 只来自受信身份；所有外部对象引用均验证归属 / 授权 |
| I02 | 所有私人 Store 与缓存键带用户范围，不能回退到默认用户 |
| I03 | 同一用户同一持久文档只有一个权威写入口；修改不靠多副本整体覆盖 |
| I04 | 当前书和 Reader 属于现场；切书只影响该现场 |
| I05 | Run 固定原聊天、原材料与现场快照；失效代次的实时 Reader 读写均不触及新现场 |
| I06 | 同一 `user_id + client_request_id` 最多接受一个逻辑用户回合，内容不同必须冲突 |
| I07 | 202 前接单、完整 Pending Turn 及必要教学准备回执均持久化；历史与索引可对账 |
| I08 | 只恢复未领取且输入可完整重建的 queued 回合；临时依赖丢失时明确结束，不解释性重做 |
| I09 | 缓存、实时事件、调度索引不成为回答、学习证据或预构建的新真相 |
| I10 | 已发布包不可原地改写；旧引用不静默重绑新版 |
| I11 | Tutor 控制按用户统一；教学提交受原会话 / revision 约束 |
| I12 | 生成代码不能取得宿主密钥、用户根或管理权限；缺隔离时不执行 |
| I13 | Windows 本地与 Book MCP 的既有权限边界不因多人兼容兜底而扩大 |
| I14 | 常驻现场、全部存活 Book 引用、连接、队列、子进程、缓存与磁盘占用均有上限和回收规则 |
| I15 | 聊天删除与接单 / 领取互斥；已接单及未保存回合不因另一窗口删除而失去归属 |

## 3. 首版拓扑与存储选择

```text
独立浏览器窗口 / iPhone
          │ 同源 HTTPS，应用会话 Cookie
          ▼
Nginx：TLS、入口约束、无私有资源公共缓存、SSE 转发
          │ 只到 loopback / 本机 socket
          ▼
单个 Rust 主服务（继续使用现有 HTTP / Agent 核心）
  ├─ 认证 + 对象授权 + 书库目录
  ├─ UserRuntime 注册表 → 每用户唯一私人写入口
  ├─ ReaderWorkspace 注册表 → 每窗口独立 Reader
  ├─ Resident 接单 / 有界公平调度 / SSE 观察
  ├─ SQLite 服务元数据 + 既有私人文件 / learning.db
  └─ 受限执行器 → 每任务沙箱（绘图 / 动画 / 预览）

现有预构建插件 / Harness → 完整就绪产物 → 管理员校验发布
```

不新增独立 Redis、队列服务器或对象存储依赖。受限 Worker 可以是主服务调用的本机执行器，并不意味着分布式任务服务。

### 3.1 目录布局（拟议）

```text
<service-root>/
  control.sqlite
  service.lock                    主服务写入者独占；迁移同样遵守
  library/
    published/<book_id>/<publication_id>/
  shared-cache/                   仅无个人上下文的可重建派生数据
  users/<opaque_user_id>/
    memory/
      memory.json
      agent-history.json
      learning.db
      agent-history.presentations/  沿用 history_path 派生的既有布局
      ...                         既有辅助文件，不靠手列文件名漏迁移
    private/                      沿用私人目标成果 / 统计内部布局
  sandbox-jobs/                   受限输出区，不能装入其他用户的数据根
```

迁入的旧 `session.json` 保留原始副本，由迁移映射投影到该用户的新现场检查点，不作为多个活动窗口的共同写入文件。

多用户应用授权与 POSIX 文件权限互补：同一个服务账户读取多个用户文件时，`0700/0600` 不能自动提供应用用户之间的隔离。用户选择权必须在 Rust 访问上下文中封闭；沙箱不可继承该服务账户的文件访问范围。

### 3.2 哪些数据放在哪里

| 数据 | 权威位置 | 说明 |
| --- | --- | --- |
| 账号、登录令牌摘要、账号禁用状态 | control.sqlite | 无开放注册；令牌明文不落日志 / 数据库 |
| 书籍发布绑定与访问授权 | control.sqlite | 路径为管理员可信配置，不由读者传入 |
| 现场 owner、绑定、generation、持久检查点 | control.sqlite | 视口实时态在内存；必要更新节流提交 |
| 请求幂等索引、接单阶段、调度领取、容量占位 | control.sqlite | 占位由非终局接单记录确定；不复制业务正文 |
| 规范化请求、冻结输入、准备完成标记、聊天与回答 | 用户 AgentHistory | Pending Turn 复用原 turn_id；持有幂等比较所需字段与必要准备载荷 |
| 画像、笔记、高亮、阅读记录 | 用户 MemoryStore | 统一写入口；原单源标注规则不改变 |
| Tutor / Trace / Assessment / Evidence | 用户 learning.db | 原 schema 和操作语义优先；不复制到窗口表 |
| 私人演示、候选、版本、目标成果 | 既有用户私人根 | 原始 owner 再包入 user 范围，不重编号旧成果 |
| 运行事件分片、Book 缓存、排队唤醒信号 | 有界内存 | 丢失后由历史 / 接单 / 发布包重建可恢复部分 |

服务数据库拟议最小表集合：`users / auth_sessions / book_publications / book_grants / reader_workspaces / run_admissions`。队列占位由接单状态派生，活动资源槽在进程内管理。实现可合并等价表，但必须保留各领域权威边界；不新增预构建 task、goal 或 mastery 真相表。

关键唯一约束为 `(owner_user_id, client_request_id)`、`(owner_user_id, turn_id)`。对象间可用复合外键校验同用户关联；私人 JSON 的 chat/session 存在性通过 UserRuntime 核实。遗留 ID 不必全局重新生成，所有查找先限制 owner。

### 3.3 SQLite 与文件提交

服务库建议 WAL、`foreign_keys=ON`、有界 busy timeout、短事务；接单与身份等要求掉电耐久的服务库提交使用 `synchronous=FULL`。具体配置读取实际结果，不假设 PRAGMA 必然生效。已有 learning.db 的事务与权限设置先核对，不顺便全量改写。[S5]

使用本机可靠文件系统；不放在网络共享盘。运行时记录实际 SQLite 版本，而不只记录 rusqlite 版本。官方 WAL 文档披露的 WAL-reset 问题已有修复；启用多连接 WAL 前，核对所链接版本包含修复，官方列出的修复为 3.51.3 及以后，或明确的已修复回移版。该项是依赖核验门槛，不代表已证明当前二进制有问题。[S5]

服务 SQLite 与私人 JSON / learning.db 不共享跨存储原子提交。使用稳定对象 ID、已有原子文件提交、分阶段写入和对账，不把多次写入包在代码块里就称为事务。MU2 同时核对文件替换后的崩溃 / 掉电耐久语义，不能把仅进程内测试当作完整耐久证据。

## 4. 身份、路由和材料绑定合同

### 4.1 Principal 与授权上下文

认证入口产生 `Principal { user_id, auth_session_id, auth_epoch }`。授权解析器根据 Principal 与请求对象取得用户、现场、聊天、Run 和材料的受限句柄；普通业务函数不接收可任意解释的全局路径。

普通读者读取自己的私人对象，以及有权访问的书库材料。管理权限允许管理账号 / 书库 / 服务配置，不默认开放其他用户私人聊天的读取 API。服务器管理员的操作系统权限不在应用层保密承诺之内。

多人模式未登录返回 401；已登录但对象不存在 / 不归属的请求统一返回不泄露对象存在性的 404；可见对象上的管理操作禁止返回 403。CSRF 拒绝与普通业务冲突分开。错误响应不包含真实路径、另一个用户 ID、问题正文或密钥。

### 4.2 首版建议 API 形状

以下为 MU4/MU5 冻结的默认接口方向，可在不改变边界的前提下沿用项目命名；不得让两套路由执行不同核心。

| 端点类别（拟议） | 范围与要求 |
| --- | --- |
| `POST /api/auth/login`、`POST /api/auth/logout`、`GET /api/auth/me` | 登录有失败限流，退出撤销服务器会话；登录也验证可信 Origin |
| `GET /api/library` | 只返回授权材料的公开元数据与 opaque 资源身份，不返根路径 |
| `POST /api/workspaces`、`GET /api/workspaces/{id}` | 身份来自 Cookie；新独立页面默认新现场 |
| `/api/workspaces/{id}/book/open` | 使用 book_id + 服务端确认的发布绑定，不接受 dir |
| `/api/workspaces/{id}/reader/*` | 检查 owner、generation 与必要的 expected_revision |
| `POST /api/workspaces/{id}/agent/runs` | 请求包含 client_request_id、原聊天与现场版本，身份不由 body 决定 |
| `GET /api/agent/runs/{turn_id}`、`.../events`、`POST .../cancel` | 先按用户查找 Run；SSE 与取消不能绕过授权 |
| `/api/books/{book_id}/publications/{publication_id}/...` | 正文、图、PDF、映射和资源均绑定发布包与当前授权 |
| `/api/me/...` | Memory / Tutor / 历史 / 私人演示，不接受切换成其他 user 的参数 |
| 聊天删除命令及旧别名 | 按 owner + chat 查找；与接单 / 领取互斥，有未结束或未保存回合时返回 409 |

受保护的 PDF / 图片 / 产物不走 Nginx 公共目录直出。若以后采用内部重定向，也须先通过应用鉴权，且内部位置不能从公网直接请求。Range、HEAD、条件请求和 304 仍先授权；未实现的传输方法可返回明确限制，不得绕鉴权兜底静态文件。

外部请求头不能伪造 Principal。客户端提交的 forwarded / identity 头由代理清除或覆盖；后端仅信任已配置的本机代理来判断原始 HTTPS 等传输信息。`Host / Origin` 使用配置的站点列表，不从任意请求头派生可信源。

### 4.3 会话与前端安全

采用高熵 opaque 会话令牌；数据库保存摘要；可使用 `__Host-` Cookie，要求 Secure、Path=/、无 Domain，设置 HttpOnly 和 SameSite=Lax，并对状态修改执行 CSRF token 与 Origin 验证。登录后轮换会话、密码重设 / 账号禁用撤销相关会话。密码使用 Argon2id 和独立盐，不使用普通 SHA-256 作为密码哈希。[S1][S2][S3][S4]

登录页与不含私有内容的基础静态包可匿名加载，数据接口不可匿名。账户切换清理内存缓存、草稿、请求恢复状态、PDF 缓冲、演示现场和相关设备存储；已有连接必须被拒绝继续推送或主动关闭。普通断网或刷新不取消已接受的 Run；退出登录不把 Run 改归新账号。

书库授权撤销 / 账号禁用应取消受影响的活动任务、清除授权缓存并关闭相应观察连接；任务领取和每次新的材料访问重新验证权限。已发给模型或已经被用户看到的内容不能“追回”。已保存的私人历史按保留政策存在，但来源原文的重新读取仍检查授权，不借旧引用绕过权限。

### 4.4 发布包身份

`PublishedBookRef = { book_id, publication_id }` 是概念合同。继承 ADR-0146，正文 / 来源附件改版使用新的 book_id。`publication_id` 由受信发布入口生成，固定该内容的一次不可变发布；同内容补齐语义 / readiness 后登记新的发布对象，旧对象继续可读。

每条发布记录关联不可变目录和完整清单，清单列出正文 / base、来源附件、映射、被消费的 sidecar、各能力版本及 readiness 回执，保留既有 `source_fingerprint` 和来源校验。完整清单由 MU3 沿实际加载依赖确定。发布入口先在暂存区按现有结构与就绪合同验证，再原子登记；发布身份固定后，目录与清单均不可替换。

书库默认条目只影响新打开的现场；活动现场、Run 与新生成的历史引用保存完整绑定。遗留引用没有 publication_id 时通过迁移映射解析；无法唯一确认则显示缺失 / 待核对，不猜最近版本。正文可读不等于正式学习就绪。

视频伴读的同源渐进成果接入另按 [ADR-0147 §3](adr/0147-linux-multi-reader-service-without-redis.md#3-书库只消费不可变发布包) 与 [VQ36](grill-video.md#vq36-同源补齐成果自动接入2026-10-02)：窗口无提问草稿且无在途回答时自动接入新发布，保留观看现场和旧引用。该视频扩展于 2026-10-02 确认设计，尚未包含在已交付的 MU3 实现中。

包路径由管理员目录映射产生，校验路径穿越、symlink / 特殊文件和归档逃逸。发布过程不执行包内脚本；读取校验沿既有结构闸。预构建凭据、运行临时目录、私人信息不作为 HTTP 可取资源公开。

## 5. 私人存储、现场和 Tutor 的并发规则

### 5.1 用户级写入

同用户的所有窗口和 Agent / 复核任务共享一份 UserRuntime 权威实例。不能分别 `MemoryStore::open` 同路径后各自维护整份文档。Learning 连接可按既有方法打开，但写入依旧核验用户、revision 和幂等操作，使用短事务。

`UserStoragePaths` 一次解析后显式注入 Memory、History、Presentation、IntentArtifact、统计与辅助程序；不能在请求期间 `set_var` 修改进程环境。需要子进程配置时只设置该次 Command 的允许字段，不继承全站用户根。

每用户 Store / 缓存有惰性加载与闲置回收策略。存在在途 Run、pending read、未保存终态、迁移或有效的待确认画像操作时不能驱逐。待确认操作的有效期在 MU2 固定；到期后明确失效，后续确认返回需要重新确认，不能消费同聊天内另一个操作。候选正文沿既有进程内存储规则处理。满足回收条件后先冲刷、释放引用再回收；调度 / 存储日志不得输出私有正文。

锁顺序固定，建议需要同时访问时采用用户写入口 → 指定现场短锁；调度锁在进入私人持久化之前释放，不允许持现场锁反向获取用户锁。优先将跨对象修改编排成短命令，禁止在锁 / 数据库事务中等待模型、文件下载、SSE 写入或渲染。对必须原子检查的聊天与现场占位，统一在调度器一次完成。

### 5.2 阅读现场

每个普通独立标签页 / 设备获得独立 workspace_id。刷新通过该标签页保存的恢复句柄重新挂接；每个新页面实例生成 attachment_id，服务器 CAS 确认有效挂接与 generation。复制标签页导致恢复句柄重复时，若已有活动挂接则默认克隆为新现场，或经用户显式接管；不能悄悄让两页拥有同一 Reader 写权。

无法确定旧页面是否仍活动时不抢占：可先新建现场并恢复检查点，同时只观察原 Run 的聊天结果。接管会增加 generation，旧页面及旧 Run 的实时 Reader 读写被拒绝。旋转、键盘弹起、布局重排不创建新现场、不增加业务 generation。

现有演示附属窗口是受控例外：由宿主显式关联原 workspace 与聊天，继续复用原上下文，不成为新的课程或聊天。其消息通过宿主验证、不能伪装成任意 workspace。

同用户窗口可同时查看同一聊天，但每聊天同一时刻仅有一个待执行 / 活动 Run；另一个窗口提交不同请求返回忙或提示另建聊天，不悄悄 fork。一个现场也最多一个待执行 / 活动的可变 Reader Run。不同用户的相同遗留 session_id 必须互不影响。

位置检查点按用户 + 现场持有。另有用户按材料的“继续阅读”候选，采用服务端确认序列更新；它仅用于新打开时恢复，不推送滚动到其他活动窗口。不得把客户端时钟最大值作为覆盖权限。

常驻现场数量按用户和全局设上限。无在途 Run、未保存结果或待冲刷操作的闲置现场可卸载 Reader 与 Book，仅保留持久元数据和检查点。重新挂接时验证身份、授权及原发布绑定，重载后增加 generation，使旧页面请求重新获取现场状态。达到预算且无可回收现场时明确拒绝新的常驻分配；不回收在途 Run 正使用的对象。

### 5.3 Run 的现场读取与私人提交

接单时冻结必要的 `ReaderInputSnapshot`：原发布绑定、视口锚点、已验证选区及本问题引用的演示版本 / 状态。它是输入证据，不是可在终局整体写回的 Reader 副本。运行中的三类操作使用不同端口：

```text
read_live_reader(scope) -> ReaderView | WORKSPACE_STALE
apply_reader(scope, command) -> Applied | NotApplied(reason)
submit_private(scope, command) -> Receipt
```

同一 generation 内，实时读取和效果应用继续服从 Reader revision 与用户操作优先规则。切书、换聊天或接管后，`read_live_reader` 返回现场已失效；导航、布局和演示恢复返回未应用，不读取或修改新现场。Run 可用原材料与已冻结快照继续回答，不能将快照伪装成当前视口。

私人提交按原 owner、聊天 / 回合、发布绑定和当前权限检查，独立于现场 generation；原书笔记和原聊天终态可继续保存。教学提交另核对 Tutor 会话 / 控制版本。Book、教学素材及私人演示读取均从原绑定解析；不借新现场的 `book_dir` 读取旧 Run 的依赖。既有 `reader.note` / `reader.highlight` 中私人写入与界面效果分别处理，不能因共享一个闭包而一起拒绝或一起放行。

### 5.4 聊天删除与运行生命周期

多人模式下，删除与同一 owner + chat 的接单、领取共用占位边界。存在 preparing、queued、claimed 或 unsaved 回合时返回 `409 CHAT_BUSY`，保持聊天不变；界面提示先取消 / 等待运行或处理未保存结果。取得删除占位后释放调度锁，再经用户权威入口持久化删除；删除结束前不接受该聊天的新回合。

删除成功后，清除所有引用该聊天的现场选择并增加 generation；现场持久元数据与历史跨存储提交时，恢复及后续访问也会清除已不存在聊天的选择。旧页面请求返回聊天已删除，不自动改投另一个聊天。原请求键保留为已关闭映射，不因正文删除而可再次执行。其他用户、其他聊天和已发生学习事实不受影响。本地单用户在 MU1 阶段保持现有取消后删除行为，多人入口在 MU6b 启用本合同。

### 5.5 Tutor 与私人演示

Tutor 全局开关和当前教学会话属于 UserRuntime / learning.db。多窗口对同一控制版本执行互斥修改时，只有符合 expected_revision 的操作成功；冲突方刷新状态。旧运行的教学提交核对原 TutorSession、活动和控制代次；关闭 Tutor 后不继续记为正式推进。已发生证据不撤销或伪造为未发生。

演示内容版本仍由原 owner / conversation / turn 持有；当前选中的版本与滑块现场属于窗口。打开旧来源、在演示内追问、回到提问时现场，都沿 ADR-0144 的显式行为执行，不因新版本出现而自动替换当前画布。跨聊天读取私人演示也先验证同一用户及明确引用。

## 6. 接单、调度、持久化与恢复

### 6.1 冻结内容与权威分工

每个逻辑用户提交生成 / 复用既有 turn_id。Pending Turn 保存规范化请求与冻结输入：owner、原 workspace / generation、chat_session_id 与期望聊天头、PublishedBookRef、用户原文、ReaderInputSnapshot、明确的任务 / 教学引用、Provider / 模型配置绑定及原接单时间。API Key 和无关全站配置不进入回合。

依赖进程内待确认画像操作的请求，冻结其非敏感操作身份及依赖类别，候选正文保持原有临时存储边界；执行时必须取得确切原操作。教学准备另冻结必要事件的 ID 与载荷，包括原教学绑定、回应次数及帮助引用，已有事实从 learning.db 读取。这些字段使恢复能够判定输入是否完整，不能只依据问题文本重做准备。

请求键在第一次提交前创建，网络重试使用原键。幂等比较直接比较已保存的规范化请求字段，覆盖问题、选区、材料、聊天、现场版本及显式任务动作；服务端绑定按原接单记录解析，不随当前书库默认项或 Provider 配置重新选择。同一句话选择不同来源是不同内容。同键 preparing 尚无私人输入时返回 `409 ADMISSION_PREPARING`，等原准备结果明确后再比较，不生成第二个 turn。

确定性失败或用户主动重试使用新键，可通过 retry_of 引用原回合。幂等映射至少保留到对应历史保留期结束；删除历史后，原接单保留不含正文的已关闭请求键映射，迟到请求返回 `409 REQUEST_KEY_CLOSED`。已失败且无私人输入可比对的旧键同样关闭；账号彻底删除另按保留政策执行。

`run_admissions.dispatch_state` 只表达接单 / 调度阶段，主路径为 `preparing → queued → claimed → settled`；准备失败走 `preparing → admission_failed`。queued 被取消或依赖失效时，先保存原回合终态，再直接进入 settled。是否回答完成、失败、取消、未保存继续由 History / Run 合同决定；settled 只表示调度已与持久结果对齐。

### 6.2 202 前的接单与教学准备提交

采用以下单机可恢复流程，不要求分布式事务：

1. 认证后先按 owner + 请求键查询原接单，重复请求按 §6.1 的原绑定、规范化内容和当前权限返回原状态，不重复占资源。新请求验证材料、聊天和现场，在短调度临界区取得同聊天 / 同现场占位；不能越过删除占位。
2. control.sqlite 事务创建带稳定 turn_id 的 `preparing` 记录，并检查用户 / 全局队列容量；记录归属与状态，业务输入留在私人存储。事务成功后释放调度锁，保留逻辑占位。
3. 经 UserRuntime 原子保存 Pending Turn、规范化请求和完整准备载荷，准备完成标记初始为 false。重复提交只比较同一 turn 的字段，不重复追加用户问题。原聊天头、现场快照、临时操作引用与教学事件载荷在本阶段冻结。
4. 按冻结的事件 ID 读取已有教学准备回执，载荷相符则复用；缺失项按原载荷幂等补写，全部完成后在私人 Pending Turn 中持久化准备完成标记。该阶段仅记录本次用户提交及教学关联，不调用模型或执行 Agent 工具。无教学准备的普通问题可直接将标记保存为完成。
5. 核对私人输入、准备完成标记及必要教学回执后，服务库将接单推进为 `queued`，提交成功才返回 202。唤醒消息丢失可由有限轮询 / 启动对账发现。
6. 准备失败时，若已有私人 turn，先保存明确失败，再将接单置 admission_failed 并释放占位；若没有私人 turn，可直接结束接单。无法保存失败状态时保留 preparing / 未保存故障与占位，返回存储错误或待核对状态，不返回 202。

同一服务根只允许一个遵守锁协议的主写进程；迁移 / 修复工具使用同一锁。重启发现 preparing 但没有私人 turn 时，结束为 admission_failed 并释放占位；已存在私人 turn 时按冻结输入续接准备，不重新调用会读取当前现场并产生新载荷的 prepare_agent_chat / teaching::prepare。旧二进制使用独立根，与迁移通过停写切换隔离。

### 6.3 领取和终局

领取前再次核对账号 / 材料权限、原聊天、冻结输入与教学回执、临时依赖及计算资源槽。只有输入完整可重建的未领取 queued 回合可执行；所依赖的确切待确认操作丢失或到期时，将原回合持久化为需要重新确认的失败，再 settled 并释放占位，期间不调用模型。仅现场 generation 改变不取消原问题，实时读写按 §5.3 返回失效，原快照仍可用于回答。

用服务库 CAS 将符合条件的 queued 置 claimed，写入 boot_id / attempt 身份；**持久领取标记成功之前，不发模型请求、不执行工具。** 领取成功与下一条指令之间崩溃，也按“执行情况可能不明”处理，不自动重试。

执行继续复用现有 orchestrator / RunContext / RunEventSink。模型历史从原聊天构造，并核对冻结的聊天头。私人提交进入用户权威入口；实时 Reader 读取和效果应用分别核验 generation，失效时返回结构化回执。临时确认操作在使用时再核对原操作身份和有效期，不能消费排队期间被替换的操作。

运行结束先通过原历史入口提交最终状态、摘要、来源与交付结果，再把调度记录更新为 settled、释放占位并记录已知 usage。历史已完成而服务索引尚未更新时，只修复索引。最终内容未保存时保持现有 unsaved 语义，可释放已退出的计算资源，但聊天仍不能被覆盖或删除；修复 / 显式处理之前要能观察该故障。

跨存储涉及的已有 Note / Teaching / Presentation 操作沿各自稳定操作身份和回执保持幂等。整个 Agent 不宣称 exactly-once；首版通过禁止自动重放已开始运行，减少重复副作用风险。

### 6.4 崩溃窗口表

| 崩溃点 | 磁盘可见事实 | 恢复行为 | 禁止 |
| --- | --- | --- | --- |
| 接单事务前 | 无记录 | 新请求可正常接受 | 猜测已执行 |
| preparing 后，Pending Turn 前 | 有容量占位，无私人 turn | 结束为 admission_failed，释放占位并关闭原键 | 从缺失正文猜请求或永久占位 |
| Pending Turn 后，教学准备未完整 | 私人冻结载荷已保存，教学回执缺失或部分存在 | 复用已有回执，仅按冻结载荷补齐缺项，再保存准备完成标记 | 重新计算回应次数或当前教学绑定 |
| 私人准备完成后，queued 前 | 同一 turn 的完整输入与必要回执已保存 | 核对后推进 queued，原键查回同一 turn | 再追加一个用户问题 |
| queued 后，响应丢失或重启 | 已接受、无领取，必要输入完整可重建 | 原键查回；继续一次调度 | 客户端换新键自动重提 |
| queued 所依赖的临时确认操作丢失 / 到期 | 确认消息与依赖身份存在，原临时操作不可用 | 原回合保存需要重新确认的失败，settled 后释放占位 | 持久化原禁止保存的候选或让模型猜确认内容 |
| claimed 后，模型调用前后不明 | 已有领取标记 | 标记该回合中断 / 待核对，不自动调用模型 | 根据“没写结果”推断没调用过 |
| 工具副作用后，最终回答前 | 部分领域回执存在 | 保留已发生事实与中断状态 | 回滚整份 Memory，或自动重放工具 |
| 历史终态后，settled 前 | 最终历史已提交 | 只修复服务索引与占位 | 重新生成答案 |
| 终态保存失败 | 无持久终态，可能有内存结果 | 显示未保存；重启只能报告已知事实 | 伪造持久完成 |

现有 `recover_pending` 必须与新接单清单协同：无新接单记录的遗留 pending 沿原规则中断；新 queued 同时满足未领取与输入可重建时恢复，其余按具体缺失依赖结束或报告存储故障。已领取 / 执行情况不明的回合一律中断。对账先读取私人终态，再判定恢复动作，不能将全部 pending 一律修改或跳过遗留恢复。

### 6.5 公平调度与资源限制

有界队列按用户轮转，兼顾用户级和全局活动上限。同一用户不能通过开窗口无限扩大份额。默认每用户活动 Run 上限可先设为 1，但作为可配置策略，不写死到状态模型；不同用户应能真实并发。所有等待状态可观察、可取消。

模型请求槽、预览槽、绘图槽和动画槽分别管理。Run 占位不意味着从开始到结束一直持有模型网络槽；避免父任务持有唯一子资源配额后等待子任务的死锁。翻译、地图本地化、画像复核、教学评估、压缩 / 修复等旁路按用户或明确服务任务归属计入资源预算；已有后台复核状态留在原 Store，服务调度只提供执行许可。

首版资源治理包括用户 / 全局并发上限、排队上限、单轮模型调用 / Token 预算和实际 usage 记录。Provider 超时且 usage 缺失时记录未知，不记为零；计算资源槽在请求实际退出后释放。纯内容缓存命中不产生新的 Provider 调用；共享生成的 usage 记录在实际发起请求上，等待者只记录命中关系。

限制每用户 / 全局 SSE 数、请求体、排队数、常驻现场数、全部存活 Book 的字节 / 数量、渲染并发、私人产物和临时磁盘。慢客户端丢弃实时缓冲后恢复快照，不能阻塞 Agent。优雅停服停止接单和领取，取消或等待有界在途执行，冲刷每用户 pending reads，清理任务进程树。

### 6.6 观察恢复

同进程 SSE 继续使用运行序号、有限缓冲和原子快照边界。不将每个 token 写入 control.sqlite。服务重启后从持久历史 / 接单状态构造新观察快照，明确 live buffer 已重置；客户端清理旧游标而不是拼接不同生命周期的序号。

401、无权限、临时断线、运行中断、需要重新确认、聊天已删除和终态未保存分别展示。取消只影响该 owner 的指定 turn。断线不产生第二次 POST。以后 Redis 可用于通知或调度适配，但 Pub/Sub 的离线丢消息特性不能改变以上权威规则。[S6]

## 7. 不可信代码、缓存和运维边界

### 7.1 受限执行器最低合同

Rust 主服务拥有凭据与私人 Store；模型生成的 Python / 动画 / HTML 不能拥有相同权限。现有普通 Command 启动方式不能作为安全沙箱；Rust 默认继承环境，需要 `env_clear` 后显式白名单，并辅以文件系统和进程权限隔离。[S7]

每次任务只传入已经授权、大小受限的输入副本及输出目录。启动程序、镜像、依赖和挂载由管理员固定，模型仅提供受限制的内容 / 代码，不提供任意启动参数。首版默认断网，字体与依赖预置，不在运行时下载模型指定的软件包。

执行身份非特权；只读根文件系统、受限输入挂载、独立临时空间；不挂载用户根、主服务环境文件、宿主 `/proc` 或容器管理 socket。清理环境、继承文件描述符、代理变量和凭据。设置 CPU / 内存 / 进程数 / 输出大小 / 超时限制，取消与宿主重启应清理整个任务进程组或 cgroup，不能只杀父 Python。

采用经 MU7 实机验证的 rootless 容器 / Linux 沙箱组合；文档不把某个运行时名称等同于强隔离证明。沙箱即使被生成代码“占满”，也不能影响服务进程的资源保留或读取其他用户数据。[S8]

输出作为不可信输入重新验证：路径不得逃逸 / symlink 回指；尺寸、数量、类型受限；HTML / SVG / frame 内容按现有富呈现合同处理，不能把文件直接当宿主同源脚本执行。无必要权限的 iframe 不加入 allow-same-origin / 顶层导航 / 任意弹窗；CSP 默认禁止网络连接、表单提交与外部基础地址，只允许本次呈现经过验证的必要资源，不放行任意远端 URL。opaque origin 场景下不能仅检查 `event.origin === "null"`：还应核对消息来自指定 WindowProxy、会话随机通道标识、消息 schema、原演示版本与现场，且从不传认证 Cookie / token。[S10]

沙箱不可用、配置不完整或探针失败时，后端直接禁止所有相关制作工具，并向前端暴露真实 capability 状态；不允许 UI 关闭而后端仍可被直接调用。可继续普通阅读 / 问答；已有可安全展示产物是否启用也须单独通过浏览器隔离测试。完整富呈现功能恢复依赖 MU7 验收，不自动退回宿主执行。

### 7.2 缓存隔离

Book 按 PublishedBookRef 共享。内存预算统计全部存活 Book，包括缓存、常驻现场、在途 Run 及未保存结果持有的引用；同一对象只计一次。从 LRU 移除但仍有引用的 Book 继续计入常驻占用。常驻现场按 §5.2 回收；分配前先回收可释放对象，仍无容量则明确拒绝，不以删除缓存条目冒充释放内存。

释放内存不删除底层发布包。发布包清理须核对活动引用与保留的历史引用，首版不做未经验证的自动旧版删除。

跨用户可共享的派生缓存必须只消费公共材料和确定参数，例如材料绑定、locale、规则版本；不能夹带用户画像、对话或选区问题。个性化召回、ProfileContext、私人产物访问快照与查询结果均带 user 范围。授权检查在命中缓存时仍执行。

现有读时地图翻译、访问统计和注册表等如有写书籍目录行为，迁入 shared-cache 或用户目录后再将 published 挂为只读。缓存生成只有一个权威写者或 single-flight，失败不影响原文可读；缓存中断不能把半文件发布给其他读者。

### 7.3 可观察性与数据保护

沿 ADR-0134 保持观测可完全关闭、异步有界、隐私白名单先于导出。新增的 user/workspace/run 关联默认用内部 opaque 标识；不能把密码、Cookie、路径、笔记、全量问题或生成代码当指标 label / 普通错误日志。访问状态接口只返回当前用户所需信息；全站队列、用户消费和运维诊断限管理员。

记录低敏指标：活跃用户 / 现场数、队列等待、各资源槽利用率、SQLite busy / 提交耗时、用户写锁持有时长、SSE 连接 / 丢缓冲、缓存命中 / 字节、崩溃对账、未保存故障、沙箱拒绝 / 超时、磁盘余量。真正的源内容与运行证据仍保存在授权的私人历史中，不以日志代替。

## 8. 切片顺序与实施合同

### 8.1 依赖与发布阶段

```text
MU0 基线与全量清点
  → MU1a 用户权威状态 → MU1b 阅读现场归属 → MU1c Run 状态端口
  → MU2 用户私有持久化 + 服务元数据
  → MU3 不可变书库发布 / 资源绑定
  → MU4 登录与统一对象授权
  → MU5 多阅读现场 / Tutor 范围
  → MU6a 单槽接单恢复 → MU6b 取消 / 删除 / 未保存生命周期
  → MU6c 多用户公平调度 → MU6d SSE 与跨重启观察
  → MU8 前端端到端连续性

MU7 受限执行（MU1c 后可设计；启用时接入 MU4/MU6c）
MU9 迁移 / 备份 / 回滚（依赖 MU2/MU3/MU6）

MU6 + MU7 安全能力状态 + MU8 + MU9
  → MU10 对抗测试 / 故障测试 / 回归 / 容量基线
  → MU11 Linux 真实入口发布验收
```

A 阶段（MU0–MU3）只在本地 / 隔离环境联调，不向新读者开放。B 阶段（MU4–MU9）构成完整候选版本，但仍需全量安全与恢复测试。C 阶段（MU10–MU11）是对外开放门槛。

MU1、MU6 是阶段编号；实际实施单元为 MU1a–MU1c、MU6a–MU6d，均依序单独提交和验收。每一步以仓库文件和测试为接手依据，前一小片通过后才进入下一片。

MU7 尚未完成时，只能以“相关能力在前后端均确定性禁用”的安全模式参与普通阅读验收；不能宣告多人富呈现已完成。每个切片自行补测试，不把所有授权工作推迟到 MU10。

### 8.2 与 RE 阅读体验切片的并行边界

RE 的排版、批注与动效可与 MU 分段并行，完整交叉合同见 [RE §5.1](切片方案-阅读体验排版批注与动效.md#51-mu-与-re-的并行与集成边界)。MU 继续维护身份、私人状态、workspace 与请求归属，RE 消费这些入口；本节不改变 §8.1 的 MU 实施顺序。

- RE1 的字体资源与正文局部样式可先行。RE2–RE6 可先准备独立显示及组件行为；用户偏好、重排恢复、批注请求和 PDF 资源的完整链路，消费 MU3 / MU4 / MU5 接口并与 MU8 联合接入。
- 设备侧排版偏好按当前用户命名空间保存，跨书复用。退出或换用户时清理旧内存状态；读位恢复和来源返回同时沿用户、workspace、generation 与材料绑定失效。RE 的本地恢复代次继续独立取消过期重排任务。
- `App.vue`、`ReaderWorkspace.vue`、`useReadingContinuity.ts`、`reader-surface.ts`、PDF / 来源组件及共用样式的同一片段依序修改。MU 维护授权、资源和清理入口，RE 维护显示与交互；共同生命周期由一个执行者集成，保留当前工作树中所依赖的未提交成果。
- MU8 不以整条 RE 完成为前置。MU10 / MU11 验收实际纳入已完成 RE 切片的同一候选版本，复用共同验收记录；后续新增 RE 改动补验受影响路径。未完成的 RE 留待后续候选版本，不替代 MU 的发布门槛。

### MU0 — 基线、威胁模型和清单

**状态**：2026-09-30 完成；128 项入口、107 处隐式状态/进程引用、原工作树差异、依赖与测试基线、夹具和部署预算见 [MU0/MU1a 记录](performance/linux-multi-reader-mu0-20260930.md)。

**目标**：在改动前冻结真实入口、隐式全局状态和可验证验收基线。

**输入 / 产出**：当前实时工作树 → 变更清单、路由能力表、存储 / 子进程清单、测试基线、目标部署预算。不得把未提交修改回滚掉再声称基线干净。

**实施接点**：E01–E26；`crates/server/src/lib.rs`、`host.rs`、`agent_run.rs`、`mcp.rs`、`packages/web/src/api.ts`，现有 Linux 与桌面启动 / 测试入口。

逐条列出规范路由、旧别名、Host 旁路、二进制资源、SSE、PDF、演示资产、统计与观测接口的调用者、权限、读写根、耗时类别。列出所有读取环境 / HOME、static cache、default_path/default_root、spawn/Command 和共享 current-session 的位置；记录受只读工具限制无法核验的点。

冻结测试维度：两个不同用户、同用户至少两个独立现场、同一本书与不同书、同名遗留 ID、普通技术材料与含 PDF 材料、已就绪与未正式学习就绪材料。记录并发 / p95 / 内存 / 数据库 / 测试机器预算；测试门槛事先确定，不用失败后修改门槛制造通过。

**验收**：所有路由可归入明确能力类别；所有持久根有归属；既有失败列表、命令、依赖与 SQLite 实际版本可重现；身份 / 状态 / 版本 / 任务权威边界表完整。

**不做 / 回滚**：不加 Redis，不改线上端口，不重排其他开发中的功能。此片仅文档 / 测试基线，可独立提交。

### MU1 — 状态拆分与本地兼容阶段

**阶段目标**：桌面 / 本地宿主逐步使用新归属结构，每一步保持原单用户行为。网络多现场能力在 MU5 启用。

**共同接点**：`crates/server/src/lib.rs`、`host.rs`、`agent_run.rs`、`crates/runtime/src/run_context.rs`。可按现有风格引入 `service_state.rs`、`user_runtime.rs`、`reader_workspace.rs`。各子片沿用旧持久格式，回滚在隔离数据根进行。

### MU1a — 用户权威状态归属

**状态**：2026-09-30 完成本地实现与验证；UserRuntime / ServiceState 已落地，完整 Server 回归 346 通过、0 失败、43 原有忽略，最后补强的 MCP 访客断言单独通过。详细范围及后续门槛见 [MU0/MU1a 记录](performance/linux-multi-reader-mu0-20260930.md)。

**输入 / 产出**：现有 AppState 的私人持久对象 → 一个本地固定身份下的 UserRuntime，以及独立服务配置。Memory、History、Learning 访问及私人演示 / 成果只保留一个权威写入口。

**边界**：本片只搬归属，不改私人路径、schema、聊天选择和运行调度；路径注入留给 MU2。

**完成判据**：本地启动、笔记、聊天、Tutor、私人演示的既有定向回归通过；模型等待不持用户写锁，后台复核访问同一权威对象。

### MU1b — 阅读现场与聊天选择归属

**状态**：2026-09-30 完成本地实现与验证；验收范围、首次失败与修复复验见 [MU1b/MU1c 记录](performance/linux-multi-reader-mu1bc-20260930.md)。

**输入 / 产出**：MU1a 的用户状态 + 原 Book / Reader → ReaderWorkspace 持有 Book、Reader、selected_chat 和 generation；本地宿主显式使用一个现场。

**边界**：审计 ensure_agent_history_for_book 及 active_by_book 的隐式选择，将持久聊天归用户、当前选择归现场。保留本地切书和删除的现有运行边界，不开放多窗口挂接接口。

**完成判据**：单用户切书、来源、会话新建 / 选择 / 删除行为等价；两个内存现场原型可选不同书和聊天而不互相覆盖；服务级对象不持有 current_book / current_chat。

### MU1c — Run 的受限状态端口

**状态**：2026-09-30 完成本地实现与验证；验收范围、首次失败与修复复验见 [MU1b/MU1c 记录](performance/linux-multi-reader-mu1bc-20260930.md)。

**输入 / 产出**：MU1a / MU1b 的明确归属 → 固定 RunScope、ReaderInputSnapshot 和 §5.3 的私人提交 / 现场读取 / 效果端口。

**边界**：替换一起借用 Memory 和实时 Reader 的入口；Book、教学素材及演示依赖按原 Run 解析。沿用 Agent 主循环，不克隆整份 Store 后写回。真实多现场启用仍由 MU5 负责。

**完成判据**：原单用户 Agent、带读、来源、Tutor / 演示定向回归通过；受控现场换代后，旧 Run 的实时读取失效、导航未应用，仍可按原绑定写笔记与保存原聊天终态；旧代码不能借新现场拼装旧 Run 上下文。

### MU2 — 用户私人路径与服务数据库

实施状态（2026-09-30）：本地实现与验证完成，见 [MU2 验收](performance/linux-multi-reader-mu2-20260930.md)。确认有效期 10 分钟，闲置回收门槛 30 分钟，常驻用户上限 10；实际 SQLite 3.53.2。合并不同用例验证 490 项通过、43 项原有忽略，首次全量的 24 项失败已在隔离临时根复验通过。网络身份/现场接入与 Linux 实机验收仍归后续切片。

**目标**：建立可持久化、可复用的用户范围，而不是“每个窗口一份文件”。

**输入 / 产出**：显式 user_id 与服务根 → 唯一 UserRuntime、路径映射、control.sqlite 基础 schema、权限 / 迁移版本检查。

**实施接点**：E04、E12–E16；`crates/memory/src/lib.rs`、`learning.rs`、`private_storage.rs`、`presentation_store.rs`、`intent_build_store.rs`、Host 的 ReviewCoordinator / read-ledger worker。拟议 `control_store.rs` 与 `user_storage_paths.rs`。

禁止多人模式调用全局 default root 作为用户兜底。将 Memory、History、Learning、私人 Presentation 和 IntentArtifact 的路径一次注入；保证所有后台任务携带 owner，复核协调由全站布尔值转为用户范围。建立服务与维护工具共同遵守的写入者锁、私有目录检查、短事务、JSON 原子提交和闲置回收规则；固定待确认操作的有效期与到期回执。

**验收**：A/B 使用相同 note/session/turn 字符串仍物理和逻辑隔离；同用户不同笔记均保留；复核不覆盖后写数据；各私人 Store 从正确用户根解析；故障不回退到别人的库；不兼容 schema 拒绝写入。有效待确认操作阻止闲置回收，到期后明确失效；两个遵守锁协议的写入者不能同时进入同一根。

**不做 / 回滚**：不把 learning.db 合并为全站掌握表，不迁移真实用户。此片新增 service schema 与路径注入，不自动重写旧原始文件。

### MU3 — 只读发布书库与资源寻址

**状态**：2026-09-30 完成本地实现与验证，见 [MU3 验收](performance/linux-multi-reader-mu3-20260930.md)。离线 import/default/grant、完整材料清单、readiness、现场/Run/历史绑定与存活引用容量已接通；schema 2 新增默认发布表。缺省 20 个存活 Book、2 GiB 估算计费；HTTP 身份/现场挂接与 Linux 只读挂载实机验收留在后续阶段。

**目标**：把工作目录式阅读入口变成授权材料对象读取，消除全局 book_dir 和原地更新竞态。

**输入 / 产出**：管理员提供的已就绪构建成果 → 校验后的不可变发布包、发布目录与完整材料绑定。

**实施接点**：E03、E08、E20、E22；Book 加载、source manifest、来源解析、书库索引、`route_book_asset_file` 和读时本地化路径。拟议 `published_library.rs`。

实现管理员离线导入 / 发布入口，沿现有 readiness 和完整性检查。按暂存 → 清单及依赖验证 → 原子登记生成 publication_id；发布对象不可被阅读账户修改。注册表、统计和纯内容缓存迁出只读目录，正文 / PDF / 图片 / 映射 URL 固定 PublishedBookRef。Book 注册表跟踪全部存活引用，提供缓存与现场共同使用的容量入口。

**验收**：两人读不同书的并发资源请求不串包；读同书共享纯内容缓存但不共享用户结果；新包发布不中断旧 Run；内容改版沿新 book_id；只读挂载下普通阅读 / 问答所需链路通过；路径穿越 / symlink / 未声明资源被拒绝；教学未就绪不被升级为可正式学习。

**不做 / 回滚**：不等待 ADR-0146 全部增量复用算法完成，首版可发布现有完整包；不公开上传接口。回滚仅撤销新默认条目，保留旧包及其引用。

### MU4 — 应用登录与全入口授权

**状态**：2026-09-30 完成本地实现与验证。显式多人 Host、离线账号管理、schema 2 会话、CSRF/Origin/限流、统一能力白名单、材料/私人对象读取和授权 SSE 已接通；最小登录页有账号切换与异步结果失效保护。网络现场和活动 Run 接单分别沿 MU5/MU6 接入，未启用操作不回退本地路径。真实 HTTP、定向/完整回归和浏览器证据见 [MU4 记录](performance/linux-multi-reader-mu4-20260930.md)，命令见 [候选入口](Linux多人阅读-MU4候选入口.md)。

**目标**：所有可触及用户或材料数据的请求都有可信身份和一致的权限检查。

**输入 / 产出**：管理员创建账号、HTTPS 入口 → 可登录 / 退出 / 撤销的会话与 AuthorizedContext；最小登录界面。

**实施接点**：Host 请求入口与全部旁路、Req / route、资源 / SSE 处理、`scripts/linux/nginx-reader.conf.example`。拟议 `auth.rs`、`authorization.rs`。

实现会话令牌摘要、密码哈希、失效 / 禁用、CSRF、Origin、基本失败限流。统一路由能力 allowlist，拒绝多人端 prebuild / raw dir / Provider 变更；旧别名同样检查。Nginx 只负责 TLS / 代理等边界，不把同一个 Basic Auth 用户作为应用身份。

**验收**：匿名 API / 私人资产均被拒绝；A 不能 GET/POST B 的 chat/run/workspace/presentation；SSE / cancel / PDF / HEAD / Range / 304 和旧别名没有旁路；伪造 user/forwarded 头失败；退出与禁用会话生效；本地兼容身份不能从公网启用；错误无内部路径。

**不做 / 回滚**：不接开放注册、SSO 或邮件流程。候选部署始终隔离，认证链不完整时入口关闭，不回退成匿名模式。

### MU5 — 多窗口现场、失效读写与 Tutor 范围

**状态**：2026-09-30 完成本地实现与验证。授权 HTTP 已接通创建、恢复、挂接、分叉、接管、Reader、演示附属页及用户级 Tutor；schema 3 的版本化检查点使用 CAS 和服务端提交序列。常驻现场每用户 4、全局 20，闲置 15 分钟卸载，活引用与未结接单阻止回收；全部材料仍经 MU3 容量入口。受限 `NetworkRunPort` 已验证旧运行私人归属、失效 Reader 效果和 Tutor 控制版本，活动执行接线留 MU6。证据见 [MU5 记录](performance/linux-multi-reader-mu5-20260930.md)，使用合同见[现场接口](Linux多人阅读-MU5现场接口.md)。

**目标**：同一人多个窗口不互相抢书，且私人学习仍连续。

**输入 / 产出**：已登录用户 + 材料绑定 → workspace 创建 / 恢复 / 挂接 / 分叉，generation 与检查点合同。

**实施接点**：Reader / `route_open_book`、History 选择、`agent_run::RuntimeStatePort`、`teaching.rs` / `tutor_api.rs`、Presentation 跟随与来源跳转。

实现 workspace owner 检查、attachment 冲突处理、CAS 位置检查点与失效读写回执；切书 / 会话替换更新 generation，普通滚动仍沿 Reader revision / 用户优先合同。接入 MU3 的 Book 容量入口，固定常驻现场上限与闲置卸载 / 重载规则。Tutor 的全局范围明确为 UserRuntime，正式操作复用原 operation_id 和 revision。

**验收**：A 手机读 X、电脑读 Y 均可持续操作；刷新不新增聊天；复制标签页不共享写权；B 切书不影响 A；旧 Run 不读取或操控新现场，结果只入原聊天；另一窗口关闭 Tutor 后旧教学推进被拒绝；演示附属页仍属原现场。反复打开 / 关闭不同材料时常驻引用受限，闲置卸载后可按原检查点重载。

**不做 / 回滚**：不做实时共读或并行课程。新现场 schema 版本化；未知版本拒绝错误恢复，不整体覆盖用户记忆。

### MU6 — 接单、生命周期、调度与观察阶段

**阶段目标**：先证明单槽接单和恢复合同，再扩展多用户并发及观察。MU6a–MU6d 全部通过后，本阶段完成。

**共同接点**：`agent_run.rs`、`agent_stream.rs`、Pending Turn / History、`teaching.rs`、Host 模型入口及 `host_lifecycle.rs`；拟议 `run_admission.rs`、`run_scheduler.rs`。turn / goal 仍使用原身份，历史和教学事实仍由原 Store 持有。

**回滚边界**：存在新格式接单时，旧恢复代码不读取新根；使用兼容版本或停服核对。各子片只在隔离候选环境启用。

### MU6a — 单执行槽下的持久接单与恢复

**状态**：2026-09-30 完成本地实现与验证。schema 4、owner/request key 直接比较、冻结 Pending Turn/教学回执、queued 后接受、单槽领取、启动对账和旧 chat 共用核心已接通。真实 Host、本地 fake Provider 与提交点故障注入见 [MU6a–MU6b 记录](performance/linux-multi-reader-mu6ab-20260930.md)，接口见 [MU6 运行接口](Linux多人阅读-MU6运行接口.md)。

**输入 / 产出**：MU5 的受限上下文 + 请求键 → §6.1–§6.4 的准备、queued、claimed、终态提交与对账；旧 /agent/chat 等待相同接单核心。

**边界**：固定一个执行槽验证状态协议；把教学准备改成冻结载荷与回执续接，不增加公平调度或新 SSE 协议。既有取消和观察路径继续可用。

**完成判据**：同键只产生一个 turn、不同内容冲突；§6.4 各提交点故障后可核对；输入完整的未领取 queued 可恢复，claimed 不自动重放；临时确认丢失明确结束；教学准备重试不改变事件载荷；终态保存失败保持 unsaved。

### MU6b — 取消、聊天删除与未保存结果

**状态**：2026-09-30 完成本地实现与验证。queued 取消与领取共享竞争边界，claimed 协作退出；unsaved 保留原结果和占位，retry-save 不调模型；删除清除所有相关现场选择并关闭旧键，History 已提交后的清理中断可恢复。验证和平台限制同 [MU6a–MU6b 记录](performance/linux-multi-reader-mu6ab-20260930.md)。下一片为 MU6c。

**输入 / 产出**：MU6a 接单状态及终态入口 → 指定回合取消、§5.4 多人聊天删除和 unsaved 占位处理。

**边界**：接单 / 领取 / 删除共用同聊天占位；queued 取消与领取只能一方成功，已领取则沿既有协作取消退出。未保存结果先处理再解除聊天占位，不把窗口关闭当作取消。

**完成判据**：跨窗口删除 preparing / queued / claimed / unsaved 聊天返回忙且不丢回合；正常删除后所有相关现场清除选择，重启可修复残留选择；迟到旧请求键关闭，其他聊天及用户不受取消或删除影响。

### MU6c — 多用户公平调度与资源限制

**状态**：2026-09-30 完成本地实现与定向验证。统一配置、用户轮转份额、实际模型许可、单轮预算/usage、同步 HTTP 等待隔离已接通；未开放制作入口和 Linux 容量验收边界见 [MU6c–MU6d 记录](performance/linux-multi-reader-mu6cd-20260930.md)。

**输入 / 产出**：已通过恢复和生命周期测试的单槽核心 → 按用户轮转的有界队列、用户 / 全局运行上限及独立模型 / 预览 / 绘图 / 动画资源槽。

**边界**：旁路模型调用接入相同资源限制，原业务 Store 保持权威；接入实际 usage 记录和单轮模型预算。运行及缓存容量使用统一配置。

**完成判据**：A 长请求不阻塞 B 普通读写或长期独占执行份额；A 开多个窗口不能放大用户份额；嵌套模型 / 渲染不因持槽等待死锁；旁路不无限 spawn；请求退出后释放计算槽，未知 usage 保持未知。

### MU6d — SSE 与跨重启观察恢复

**状态**：2026-09-30 完成本地实现与定向验证。排队至终态连续授权 SSE、用户/全局观察容量、epoch 游标和重置快照已接通；完整 Vue 连续性仍归 MU8。证据见 [MU6c–MU6d 记录](performance/linux-multi-reader-mu6cd-20260930.md)。

**输入 / 产出**：MU6a–MU6c 的持久状态和原运行事件 → 多用户授权观察、有界缓冲 / 快照及重启游标处理。

**边界**：只扩展观察归属与恢复，不改变已验证的接单或执行恢复条件；慢客户端不反向阻塞 Agent。

**完成判据**：SSE 断线不重复 POST；缓冲溢出从同一序号边界恢复；重启后清理旧游标，展示持久结果 / 中断 / 需要重新确认 / 未保存；指定用户退出或取消不影响其他观察者。

### MU7 — 受限绘图、动画与浏览器预览

**状态（2026-09-30）**：实现与定向验收完成，未部署现网。Linux 4 项、本地相关回归 84 项、Vitest 7 项、Playwright 6 项通过；配置、边界及限制见 [MU7 接口](Linux多人阅读-MU7受限制作.md) 与 [MU7 证据](performance/linux-multi-reader-mu7-20260930.md)。

**目标**：保留丰富演示能力，但不让模型生成代码成为跨用户入口。

**输入 / 产出**：授权输入包 → 隔离执行任务 → 校验后的私人输出；明确 capability 与拒绝状态。

**实施接点**：`presentation_plot.rs`、`presentation_animation.rs`、`presentation_preview.rs`、`presentation_author.rs`、相关 Node / 浏览器启动链路、`AgentPresentation.vue` 及宿主消息桥。

实现 §7.1 的环境 / 文件 / 网络 / 资源 / 进程树隔离及输出验证，生产执行器与测试 fake 分开。未验收分支在多人模式 fail-closed。已有 HTML 演示也不能持有宿主会话或请求任意用户 API。

**验收**：带测试哨兵的环境变量和其他用户文件无法读取；网络 / 元数据地址访问被拒绝；fork / 大内存 / 超大输出受到边界；取消无孤儿进程；输出 symlink 不被读取；生成 frame 不能调用未授权宿主动作；没有隔离器时所有相关直接 API / 工具调用均拒绝。

**不做 / 回滚**：不开放任意网络与依赖安装，不以改提示词代替隔离。回滚为关闭制作能力，不回滚到宿主直接执行。

### MU8 — 前端登录、多现场与断线连续性

**状态**：2026-09-30 完成实现和本地 HTTPS Chromium/WebKit 验证。原键恢复、多标签页、附属演示、来源和 PDF 账号切换已覆盖；真实 iPhone/Nginx 与生产恢复仍按 MU11 门槛验收。见 [接口](Linux多人阅读-MU8前端连续性.md) 与 [证据](performance/linux-multi-reader-mu8-20260930.md)。

**目标**：让后端隔离在真实浏览器使用中成立，不只停留在 API 测试。

**输入 / 产出**：身份与现场 API → 登录恢复、作用域化客户端、多窗口阅读、原 Run 重连和私人缓存清理。

**实施接点**：`packages/web/src/api.ts`、`App.vue`、`agent-run-state.ts`、`agent-submission-recovery.ts`、`ReaderWorkspace.vue`、`AgentPresentation.vue`、PDF 与来源组件、设备偏好读写。

客户端所有有状态请求从当前授权上下文产生；网络响应也检查 owner / workspace generation / book binding，晚到响应不覆盖新页面。请求恢复键在第一次发送前创建，响应丢失时先查原接单。登录和多窗口 bootstrap 与旋转 / 键盘布局分离；设备缓存加 user 命名空间。

**与 RE 集成**：向排版偏好、批注和重排恢复提供同一用户、现场及清理入口；字号或专注呈现变化不触发 bootstrap、不推进 workspace generation。纳入 RE 的版本按 [共同场景](切片方案-阅读体验排版批注与动效.md#51-mu-与-re-的并行与集成边界)覆盖换用户、双窗口、延迟字体与 PDF / 来源晚到结果。

**验收**：同浏览器退出 A 后登录 B 无 A 内容闪现；后台恢复不重提问题；不同标签页互不切书；旧问题回答保持原聊天；现场重载会更新 generation；聊天已删除 / 忙和需要重新确认均明确展示；PDF、来源及演示追问现场可核对；iPhone 的选择、输入法、旋转与后台恢复继续工作。

**不做 / 回滚**：不重写移动 UI、不新增离线 Service Worker。Web / Server 必须同 release 构建；旧 Web 不得静默用无上下文 API 写多人状态。

### MU9 — 旧单读者数据迁入、备份与回滚

**实施状态**：`reader_maintenance` 已提供 preview / migrate / backup / restore / export-user；参数与操作记录见 [MU9 说明](Linux多人阅读-MU9迁移恢复.md)，T60–T64 的本地隔离证据见 [MU9 记录](performance/linux-multi-reader-mu9-20260930.md)。MU11 已查明生产源并选定 reader 为唯一迁入账号，隔离导出已由实际旧发布二进制读取；完整生产映射与入口切换现已执行，原资料与恢复点保留。

**目标**：将现有使用者的数据无歧义迁入一个明确账号，保留恢复路径。

**输入 / 产出**：指定旧 memory/private/书库根 + 显式目标账号 → dry-run 清单、不可变备份、迁移映射、可重复验证的新服务根。

**实施接点**：既有 Linux 数据迁入与路径重定位工具、Memory / History / Learning / Presentation、私人目标成果、旧 session.json，以及服务 schema version。

先停写并建立一致性恢复点；SQLite 使用 backup API 或可靠停服备份，不孤立复制活跃主库。[S9] 保留未知辅助文件并审核；来源路径按结构化映射解析。迁移记录 operation_id、指定源 / 目标、逐项映射和完成状态；重跑核对已写入目标，跳过已完成项，只续接未完成部分。同一 operation_id 的源 / 目标或映射不符时返回冲突。

**验收**：旧资料只出现在指定账号；新用户为空；原 note / chat / evidence / presentation ID 与历史引用保留；书籍路径移位不改正文；learning.db 与历史关联可读；迁移中断不切入口；重复执行不新增副本；从备份实际恢复通过。

**不做 / 回滚**：不把旧目录设置成新旧服务共享写根。开始多人写入后只允许兼容版本回滚；需要回旧单读者程序时必须停服、显式导出目标用户并核验，不能把多用户根交给旧程序。

### MU10 — 对抗、故障、跨平台与容量验收

**实施状态（2026-10-01）**：测试设施、真实进程终止 / ENOSPC / 非 root 权限 / busy / 慢 SSE、跨平台回归和五分钟容量诊断已完成，见 [MU10 记录](performance/linux-multi-reader-mu10-20260930.md)、[I01–I15 / T01–T79 矩阵](performance/linux-multi-reader-mu10-matrix.md) 与 [操作说明](Linux多人阅读-MU10验收.md)。修复普通用户发布包、首次聊天恢复选区与网络 Reader 来源复核异常。**完整发布门槛未通过**：MU11 已补齐教学夹具、DSH 35 项与一次真实 Provider；Core 全量 1061 通过 / 1 超时，该文件单独 25 通过。指定容量机器 / 完整渲染混合负载及正式发布条件仍缺，后续证据见 MU11 记录。

**目标**：用可重现证据证明隔离和恢复，而不是用两张登录截图交付。

**输入 / 产出**：完整候选版本 → §9 矩阵报告、故障注入回执、性能对照、已知限制、release gate 状态。

**实施接点**：Rust 定向测试与 Host 集成测试、现有 Web Vitest / Playwright、Windows / Tauri 和 Linux 实机流程、Book MCP 与预构建插件回归。

使用 fake Provider 稳定控制阻塞、响应丢失、失败及 usage 未知；测试必须走真实 Host 边界而非仅绕过认证调用 route。另以隔离账号和授权测试预算跑最少的真实 Provider 冒烟，结果分开报告。数据故障包括 disk full、权限错误、数据库 busy、指定提交点 kill / restart、慢 SSE 和 Worker 失联。

**验收**：所有 I01–I15 有正反例与明确断言；必须失败的越权确实失败；旧单用户、来源真实性、教学证据和富呈现合同不降级。测出纯阅读延迟、含现场固定引用的常驻内存、并发资源占用、队列公平性、恢复耗时及数据库 / 锁指标；结果对照事先冻结的门槛。

**不做 / 回滚**：不删除旧测试、放宽断言或隐瞒既有失败；未通过的安全 / 恢复项阻止开放。性能不达标先定位热点，不自动升级 Redis。

### MU11 — Linux 发布、真实 HTTPS 与交付记录

**实施状态（2026-10-01）**：显式启动、同目录构建、systemd / Nginx 模板、[运行单](Linux多人阅读-MU11发布运行单.md) 与真实 Linux 隔离部署已交付。两账号、TLS / SSE、一次真实 Provider、备份恢复、旧 release 读取、禁用账号、切发布包保留旧引用及强杀重启通过；修复备份修改共享父目录权限、目录同名发布版本不可区分的问题。前期隔离候选使用 dev 构建；随后已用优化 release 完成 9 份材料、449 条记忆、17 段对话的生产迁入，入口为 https://115.190.121.150。现网操作见 [运行说明](Linux多人阅读-现网运行说明.md)，剩余未验范围见 [MU11 记录](performance/linux-multi-reader-mu11-20261001.md)。

**目标**：把通过隔离测试的同一版本发布到真实入口，并能安全停止、回滚和恢复。

**输入 / 产出**：MU10 通过记录 + MU9 可恢复数据根 → 同 release 的 Server/Web、systemd/Nginx 配置、真实入口验收和最终运行说明。

**实施接点**：`scripts/linux/start-reader.sh`、service / nginx 模板、Linux 部署与迁入文档。新多人运行模式必须显式声明，不靠 OS / User-Agent 猜测，具体参数名在实现时更新文档。

先发布隔离实例验证迁移副本；确认旧实例停止写目标根后再切入口。核验 TLS、Cookie、CSRF、后端不可公网直连、无 Basic Auth 身份混用、SSE 无代理缓冲、PDF / 私人资源经同一受控入口。服务启动检查写入者独占、依赖和沙箱 capability；关闭生产调试与预构建端点。

**验收**：真实两个账号 / 多设备完成阅读、问答、笔记、来源、重连、退出 / 禁用；切发布包后旧引用仍准确；有序停止冲刷所有用户，不留子进程；实际恢复和兼容回滚演练通过。iPhone Safari 实机结果与 Playwright WebKit 结果分别记录。

**交付文件（实施时新增）**：部署说明、路由权限清单、配置示例、版本与依赖清单、迁移 / 恢复记录、矩阵结果、已知限制。不得把真实账号密码、会话令牌或 Provider Key 放入交付文件。

## 9. 端到端验收矩阵

本表定义完整候选版本的验收矩阵。分阶段执行证据见 MU0–MU5 的报告；[MU4 记录](performance/linux-multi-reader-mu4-20260930.md)列明 HTTP/身份/观察边界，[MU5 记录](performance/linux-multi-reader-mu5-20260930.md)列明多现场、持久恢复、失效读写和 Tutor 证据。活动执行、完整浏览器连续性及真实部署项仍待后续切片。测试 ID 用于结果追踪，不自动表示通过。至少采用用户 A/B、现场 A1/A2/B1、材料 X/Y、两个不兼容内容版本与同内容两次能力发布包；测试材料不得含真实私人资料。

| ID | 场景 / 注入 | 必须观察到的结果 | 责任切片 |
| --- | --- | --- | --- |
| T01 | 无 Cookie 访问数据 API | 401，无私人数据 | MU4 |
| T02 | 伪造 body user_id / 身份头 | 不能切换 Principal | MU4 |
| T03 | A 查询 B 的 workspace / chat / turn | 统一不泄露对象存在性，不返回片段 | MU4 |
| T04 | A 订阅或取消 B 的 Run | 无事件、无取消效果 | MU4/6 |
| T05 | 未授权用户请求 PDF / 图片 / 映射 | 正文 API 与二进制资源同样拒绝 | MU3/4 |
| T06 | 对资源使用 HEAD / Range / ETag 条件请求 | 先授权；无 304 / 长度旁路 | MU4 |
| T07 | 使用旧路由别名或 Host 旁路 | 不绕过身份、目录与资源限制 | MU0/4/6c |
| T08 | A 读取 B 的演示、候选、私人目标成果 | 不泄露文件 / 元数据 | MU2/4/7 |
| T09 | POST 缺 CSRF 或 Origin 不可信 | 在业务副作用前拒绝 | MU4 |
| T10 | 登录失败洪泛 | 限流；不无限触发密码哈希资源消耗 | MU4/6 |
| T11 | 退出 / 改密 / 禁用账号后旧 Cookie | 不再授权；既有观察连接不继续泄露 | MU4 |
| T12 | 请求内部目录、..、编码穿越、symlink | 在授权 / 路径闸拒绝，无真实路径回显 | MU3/4 |
| T13 | 从多人公网入口触发本地身份回退 | 明确拒绝，无默认读者数据 | MU1/4 |
| T14 | 普通用户访问 Provider / prebuild / 全站观测 | 后端拒绝，不仅隐藏按钮 | MU4 |
| T15 | A1 读 X，B1 读 Y，同时切书 | 各自 Reader / 资源 / 对话不变异 | MU3/5 |
| T16 | A1 与 A2 读不同书 | 私人数据共享，当前现场不共享 | MU2/5/8 |
| T17 | 刷新、复制标签页、旧连接仍存活 | 正确恢复或分叉，无双写 Reader | MU5/8 |
| T18 | 手机旋转 / 键盘 / 选区变化 | 不重建业务 workspace 或提交问题 | MU8 |
| T19 | 同用户两窗口同时写不同笔记 | 两条均保留，不丢更新 | MU2 |
| T20 | 画像后台复核与用户显式修改并发 | 依 revision / 原治理规则，不覆盖新事实 | MU2/6 |
| T21 | 两用户使用相同遗留对象 ID | 授权、缓存、文件和事件完全隔离 | MU2/4/6 |
| T22 | 私人缓存命中 / 用户 runtime 驱逐重载 | 无串用户；未冲刷数据不被驱逐 | MU2/7 |
| T23 | A1 关 Tutor，A2 旧运行尝试推进 | 用户控制一致，旧教学提交拒绝 | MU5 |
| T24 | 同用户同时改 Tutor 的同 revision | 一次成功，其余冲突并刷新 | MU5 |
| T25 | 显式打开演示附属视图并追问 | 原聊天 / 现场连续，不新增课程 | MU5/8 |
| T26 | A 退出后 B 在同一浏览器登录；纳入 RE 时含排版切换及展开批注 | 无 A 草稿、偏好内容、PDF 或演示闪现；排版读取 B 的选择或默认值，旧批注及晚到结果不出现 | MU8，纳入 RE2/RE3 时联合验收 |
| T27 | 同键相同请求并发提交，包括 preparing 阶段 | 一个 turn、一次容量占位；准备未完成不提前返回 202 | MU6a |
| T28 | 同键但问题 / 选区 / 材料 / 任务动作改变 | 比较规范化内容后冲突，不覆盖旧问题 | MU6a |
| T29 | 202 响应丢失，重连查回 | 找到同一 turn，不二次 POST 新回合 | MU6/8 |
| T30 | 队列满、单轮请求超限、账号被禁用 | 接单前拒绝或取消未执行项，不偷跑 | MU4/6c |
| T31 | A 长模型任务，B 读写 / 问答 | 无全站 AppState 长锁，B 获得份额 | MU1/6 |
| T32 | 同聊天跨窗口同时提交不同请求 | 明确串行 / busy，不并行改历史 | MU5/6 |
| T33 | 不同用户请求集中，A 大量开窗口 | 公平调度，不按窗口无限放大 A 份额 | MU6 |
| T34 | 翻译、画像复核、压缩等旁路集中触发 | 计入资源池和确定归属，不无限 spawn | MU0/6 |
| T35 | 第一次 SQLite 提交后 kill，私人 turn 尚无 | admission_failed、释放占位并关闭原键，无模型调用 | MU6a |
| T36 | 私人准备完成后、queued 前 kill | 同 turn 及准备回执对账，无重复 append | MU6a |
| T37 | 普通问答 queued 后、领取前 kill | 未领取且输入完整可重建，原输入只调度一次 | MU6a |
| T38 | claimed 后、是否发模型请求不明时 kill | 中断 / 待核对，不自动重放 | MU6 |
| T39 | 工具已写笔记，终局前 kill | 已发生笔记保留，不重复、不整体回滚 | MU6 |
| T40 | 最终 History 已写、settled 前 kill | 只修复索引，不再调模型 | MU6 |
| T41 | 历史终态提交 disk full / 权限故障 | 显示未保存，聊天仍占位；不伪造持久完成 | MU2/6a/6b |
| T42 | SQLite busy / JSON 替换失败 / WAL 恢复 | 有界等待、真实错误、不丢已接受归属 | MU2/6/9 |
| T43 | 慢 SSE、事件缓冲溢出、断线 | 快照恢复，Agent 不被网络反压阻塞 | MU6/8 |
| T44 | 重启后使用旧 SSE 游标 | 新观察快照，无错序拼接或重新执行 | MU6/8 |
| T45 | 取消 A 的 queued / claimed Run，并与领取交错 | queued 取消与领取只成功一方；已领取协作退出，只影响指定回合 | MU6b |
| T46 | 提问后 A1 切书 / 换聊天再收到旧回答 | 答案入原聊天，过期 Reader 效果不应用 | MU5/6/8 |
| T47 | 提问后手动改变当前演示版本 / 滑块 | 回答仍解释冻结现场，不自动回滚画布 | MU5/8 |
| T48 | 发布同内容新能力包，旧 Run 继续 | 旧包固定，新的默认不替换旧证据 | MU3/6 |
| T49 | 内容更新新 book_id，旧引用点击 | 仍定位旧版；无旧版则明确不可用 | MU3/9 |
| T50 | 准备中的包 / readiness 不完整 | 不发布未声明能力，正式学习不提前开放 | MU3 |
| T51 | 只读挂载 published 后走全阅读链路 | 不写书籍目录；派生缓存 / 统计进正确根 | MU3/7 |
| T52 | 包缓存驱逐 / 旧包清理提议 | 在途引用仍可读且计入常驻量；保留历史包不被误删 | MU3/5 |
| T53 | 沙箱代码读取测试密钥与其他用户哨兵 | 都不可读，输出不含哨兵 | MU7 |
| T54 | 沙箱访问外网 / 本机服务 / 元数据地址 | 默认拒绝，无宿主凭据外传 | MU7 |
| T55 | 沙箱 fork / 内存耗尽 / 超时 / 大输出 | 在配置边界终止，服务可继续阅读 | MU7 |
| T56 | 取消与主服务重启时遗留子进程 | 整个任务进程树被回收 | MU7/11 |
| T57 | 沙箱输出 symlink / 路径逃逸 / 恶意 HTML | 输出验证拒绝或安全隔离，不获得宿主权限 | MU7 |
| T58 | 未安装 / 配置失败的沙箱，直接调用制作工具 | 后端确定性禁用，不降级成普通 Command | MU7 |
| T59 | 生成 frame 伪造来源或宿主消息 | owner / WindowProxy / 通道 / schema 验证拒绝 | MU7/8 |
| T60 | 迁移一次、重跑、在中间失败 | 只到指定账号，数据不重复，入口未误切 | MU9 |
| T61 | 两个遵守锁协议的服务 / 维护工具打开同一写根 | 第二写入者拒绝；无并发 JSON 覆盖 | MU2/9 |
| T62 | 备份并恢复多用户数据与包绑定 | 真实恢复成功，历史 / Learning / 演示可核对 | MU9 |
| T63 | 旧二进制回滚及新旧入口切换 | 旧程序只获得独立旧根或指定用户导出根；切换前停写 | MU9/11 |
| T64 | 遗留 pending 与新 queued 混合恢复 | 分别按原规则和新接单对账，无错误重放 | MU6/9 |
| T65 | Provider 超时且 usage 不明 | usage 记录未知；请求实际退出后释放计算槽 | MU6c |
| T66 | 禁用用户 / 撤销材料权限时运行仍活动 | 停止新读取和观察，取消受影响任务 | MU4/6 |
| T67 | Windows / Tauri 正常启动与来源 / 演示 | 本地模式行为等价，无多人隐式登录要求 | MU1/10 |
| T68 | Book MCP / Codex / DeepSeek 预构建回归 | 访客仍不读私人库，不新增用户执行权 | MU10 |
| T69 | 日志、观测、错误和计量检查 | 无密码、Cookie、密钥、私人正文或他人路径 | MU4/7/10 |
| T70 | 真实 Nginx HTTPS + iPhone Safari | 登录、PDF、SSE、后台恢复和退出可用 | MU8/11 |
| T71 | 并发读 / 模型 / 渲染混合压力 | 满足事先冻结指标，资源上限有效 | MU6/7/10 |
| T72 | 有序停止与强制终止后启动 | 停接单、用户记录冲刷、恢复状态真实 | MU6/9/11 |
| T73 | X 书 Run 排队 / 运行时切到 Y，再读取 reader.state / 构造定位上下文 | 实时读返回 WORKSPACE_STALE；仅原快照可用于原问题，原书私人提交仍有效 | MU1c/5/6a |
| T74 | 待确认画像操作的确认消息 queued 后重启 | 原临时操作丢失时保存需要重新确认的失败，不调用模型、不保存候选正文 | MU6a/8 |
| T75 | 教学准备回应已写、绑定或完成标记未写时 kill / restart | 复用原事件及载荷，仅补缺项；回应次数、绑定与已发生事实不变 | MU6a |
| T76 | A1 提问，A2 删除其 preparing / queued / claimed / unsaved 聊天 | 409 CHAT_BUSY，原聊天和回合保留；先处理运行后才可删除 | MU6b/8 |
| T77 | 正常删除被多现场选中的聊天，在历史删除后、现场清理前重启 | 清除残留选择并换代；旧页面不改投其他聊天，旧请求键关闭 | MU5/6b/8 |
| T78 | 多现场持有不同 Book，LRU 驱逐后继续开页；闲置现场卸载再恢复 | 所有存活引用仍计量，预算满时拒绝分配；卸载释放引用，按原检查点恢复 | MU3/5/6c |
| T79 | 用户 runtime 闲置但仍有待确认操作；到期后回收再确认 | 有效期内不驱逐；到期明确失效，重载后不消费其他操作 | MU2/8 |

### 9.1 测试组织建议

拟议新增测试模块分为：`multi_user_authorization`、`workspace_isolation`、`run_admission_recovery`、`published_library`、`sandbox_boundary`、`legacy_user_migration`；可按当前 crate 单元测试 / integration 约定落位。Web 对应增加认证、晚到响应、请求幂等和多窗口 Playwright 用例。名称不代表已有文件。

当前已知可用的仓库级测试 / 构建入口如下；实施时在真实仓库执行，本次未执行：

```bash
cargo test -p memory -p runtime -p server
pnpm test
pnpm build
```

依据：根 `package.json:7–17` 及已核验 crates。真实 Linux / Windows 验收必须分别运行现有平台流程并补多人用例；不以本地 fake Provider 或 WebKit 模拟替代生产入口和 iPhone 实机结论。涉及真实 Provider 的用例使用隔离数据和事先批准的测试额度。

测试报告每项至少记录：测试 ID、代码 / 配置版本、数据 fixture 身份、实际路径类别（不含秘密）、执行命令、断言结果、故障注入点、失败证据、是否重跑。不能把“设计覆盖”记为“测试通过”。

## 10. 迁移与上线运行单

### 10.1 迁移前

明确旧实例、目标服务根、唯一目标用户、书库发布映射和回滚版本；确认当前操作者有权处理该单读者数据。停止旧实例写入后生成一致性备份，数据库连同所需日志一致处理；保留文件清单、数据版本和源 / 目标映射。启用日志脱敏，不把原个人内容复制到测试截图。

先在隔离副本运行 dry-run：核对 Memory 条数与 revision、聊天 / 回合数、学习会话 / 证据数、私人演示版本 / 引用数、目标成果和阅读位置。未知文件或无法映射引用标为待核对，不静默丢弃。迁移状态未明确完成不能让新服务使用部分数据。

### 10.2 切换入口

确认完整候选版本已通过 MU10；从同一 release 构建 Server 和 Web。新多人服务根启用单写入者锁；Nginx 只切换受控后端，不改变其他站点。检查认证、权限与实际 capability，未验收的代码执行路径保持禁用。

记录真实二进制与 Web release、系统依赖、SQLite 实际版本、journal / synchronous 配置、数据 schema、TLS / Cookie 策略、资源预算和已完成沙箱探针。旧实例和旧数据可保留，但不得同时写新服务的数据。

### 10.3 上线后与停止

用两个真实测试账号检查独立读书 / 笔记 / 提问 / 来源 / 私人演示权限，并用手机完成后台恢复。分别演示撤销会话、运行取消和服务停止。全部成功后才邀请普通用户，不能用匿名接口降低手机登录难度。

SIGTERM / SIGINT 后先停止接受和领取任务，再等待 / 协作取消在途执行、持久化可保存终态、冲刷所有用户已读记录并关闭数据库；达到截止后强制终止应在下次启动显示中断，而不是记为有序完成。

### 10.4 回滚和数据保留

如果没有产生新写入，可回到原实例与原数据副本。已经产生多人写入后，优先回退到兼容新 schema 的前一服务版本；不存在兼容版本时保持停服并恢复到明确一致的备份，向操作者列出会丢失的备份后数据，不能默默恢复旧数据覆盖新记录。

旧单读者程序只可读取独立导出的目标用户数据。历史材料包按明确保留策略存档；任何清理都不能把仍被保留聊天引用的内容替换为“最新”。首版不引入自动共享 / 公共发布私人产物的逻辑。

### 10.5 已知限制

首版仍有单机故障域和私人 JSON 整体提交的容量限制。已领取回合不自动重放；依赖已丢失临时确认操作的回合需要重新确认。usage 缺失时只能记录未知；按用户累计金额 / Token 硬额度、消费预留和对账在需求明确后独立设计。旧二进制不识别新写入者锁，运行安全依赖独立数据根和停写切换。

## 11. 未来何时再评估 Redis 或其他基础设施

| 可观测触发 | 应讨论的变化 | 仍不改变的边界 |
| --- | --- | --- |
| 需要两个以上独立后端共同服务同一用户 | 先改存储并发权威和分布式归属，再评估共享会话 / 配额 / 事件总线 | 不允许多个 UserRuntime 独立覆盖同一 JSON |
| 多台 Worker 自主领取 / 接管任务 | 评估持久队列、租约、幂等副作用；Redis Streams 是候选之一 | 原 turn / history / learning / prebuild 各自权威不变 |
| 实测跨后端重复纯内容计算显著 | 评估共享缓存 | 用户个性化内容不进入公共缓存 |
| SQLite 写等待达到冻结容量门槛 | 优化事务 / 索引 / 分区，必要时迁移 PostgreSQL | Redis 不是关系持久化的自动替代品 |
| 需要跨服务器高可用 | 重做文件存储、任务恢复、会话和发布包一致性设计 | 不是“加 Redis 后开第二进程”即可成立 |

只在需要替换的位置保留窄模块边界，不预先实现多种 adapter 或通用消息中台。Redis Pub/Sub 仅通知，不保证断线回放；Streams 的消费组、确认和待处理消息可以帮助任务分工，但不自动实现业务副作用 exactly-once。[S6][S11]

## 12. 外部技术依据

2026-09-29 查阅。项目现状由 §1 的代码 / ADR 支撑；以下资料仅支撑数据库、安全和消息机制的外部事实，本文的具体架构与切片属于设计决策。

[S1]: https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html "OWASP Authorization"
[S2]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html "OWASP Session Management"
[S3]: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html "OWASP CSRF Prevention"
[S4]: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html "OWASP Password Storage"
[S5]: https://sqlite.org/wal.html "SQLite WAL：并发、耐久、同机约束和 WAL-reset 修复说明"
[S6]: https://redis.io/docs/latest/develop/pubsub/ "Redis Pub/Sub delivery semantics"
[S7]: https://doc.rust-lang.org/std/process/struct.Command.html "Rust Command 环境继承"
[S8]: https://docs.docker.com/engine/security/ "Docker Engine security"
[S9]: https://sqlite.org/backup.html "SQLite Backup API"
[S10]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/iframe "MDN iframe sandbox"
[S11]: https://redis.io/docs/latest/commands/xreadgroup/ "Redis XREADGROUP：消费组与待确认消息"
