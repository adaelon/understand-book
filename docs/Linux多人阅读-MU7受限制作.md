# MU7 受限制作接口

日期：2026-09-30。实现依据为 [ADR-0147 §8](adr/0147-linux-multi-reader-service-without-redis.md)；实机记录见 [MU7 验证](performance/linux-multi-reader-mu7-20260930.md)。

## 配置与能力

多人服务从自己的服务根读取 `presentation-sandbox.json`。配置由部署者写入，模型与 HTTP 调用者不能设置执行器、挂载或启动参数。

```json
{
  "instance": "reader",
  "worker": "/opt/reader/bin/presentation_worker",
  "python_root": "/opt/reader/render-runtime",
  "browser": "/usr/lib64/chromium-browser/chromium-browser"
}
```

`instance` 为独占名称，限 1–32 位 ASCII 字母、数字与连字符。服务重新打开时会回收 `ub-render-{instance}-*.service` 和 `/var/tmp/ub-render-{instance}`；不同服务根必须使用不同名称。

`worker` 是同版本源码构建的 `cargo build -p server --bin presentation_worker` 产物。`python_root` 内需要 `bin/python`、Matplotlib、Manim 0.21.0、av、Pillow 及其原生依赖。浏览器必须位于 `/usr` 或该运行时内；运行时中的路径会映射到 `/runtime`。执行器系统依赖 systemd、cgroup v2、bubblewrap 和可用的非特权用户命名空间。宿主需要创建、查询和停止这些 systemd 单元的权限，任务本身以 `nobody` 运行。MU7 验收使用专用目录和 root 管理端，部署权限安排归 MU11。

启动检查会实际启动隔离任务，验证身份、根目录不可写、宿主私人根不可见、断网、Python 依赖，并加载浏览器页面、读取观察和截图。配置缺失、无效、非 Linux、依赖或探针失败时，所有网络 `presentation.author` 操作均返回 `PRESENTATION_SANDBOX_UNAVAILABLE`，普通阅读仍可用。

`GET /api/auth/me` 的 `capabilities.presentation` 返回：

```json
{"authoring":true,"plot":true,"animation":true,"preview":true,"reason":"ready"}
```

不可用时四项均为 false，`reason` 为 `not_configured`、`invalid_configuration`、`linux_required`、`probe_failed` 或 `execution_cleanup_failed`。该状态只控制多人制作；完整页面接入归 MU8。

## 输入、输出与私人权威

`NetworkRunPort.author_presentation` 复验原 Run 的用户和发布材料，再调用共用 `AuthorSession`。候选和版本的短暂存取借用当前用户唯一 `UserRuntime`，渲染期间不持私人存储锁。绘图、动画、预览分别取得 MU6 的独立资源许可；实际执行、进程树回收和输出读取完成后释放。模型调用继续使用 MU6 的 `LimitedAdapter`，制作工具本身不调用模型。

每个任务使用独立的文件系统、PID、用户和网络命名空间。只读挂载 `/usr`、指定 Python 运行时、固定 worker、当前任务 JSON，以及字体和动态链接缓存；新建 `/proc`、`/dev` 和可写临时空间。环境变量采用白名单；不挂载用户资料、生产凭据、服务根或管理 socket。默认网络命名空间不能访问宿主、外网或元数据服务；浏览器控制连接只存在于任务内部。

宿主接收有界 JSON，不接收生成代码指定的宿主输出路径。worker 对绘图和动画文件验证单层文件名、普通文件类型、大小、PNG 尺寸，以及动画元数据和 MP4 类型。失败不返回生成进程原始日志或宿主路径。候选写入、演示版本和状态保存沿原私人 Store，旧版本保持不可变。

## 资源边界

| 边界 | 单任务或单回合上限 |
|---|---|
| CPU | 每任务 100%；实例合计 150% |
| 内存 | 每任务 768 MiB；实例合计 1200 MiB；不使用 swap |
| 进程与线程 | 绘图/动画 64；预览/探针 256；实例合计 384 |
| 临时工作盘 | 每任务独立 256 MiB tmpfs |
| 单文件 | 绘图/动画 40 MiB；浏览器由 tmpfs、内存与输出边界约束，见下文 |
| 返回数据 | 总 JSON 40 MiB；任务 JSON 输入 100 MiB |
| 执行时间 | 绘图 25 秒；动画 185 秒；预览/探针 35 秒；内部渲染器有更短期限 |
| 文件描述符 | 每进程 256 |
| 回合临时媒体 | 最多 8 项，累计媒体/源码/数据载荷 32 MiB，新增结果超限不进入回合缓存 |
| 私人落盘 | 默认每用户 512 MiB / 4096 文件，候选、版本和状态合计 |

服务根 `service-limits.json` 的 `presentation_bytes` 和 `presentation_files` 可调整私人落盘限额，均必须大于零。写入前在持有用户私人权威期间检查，超限返回 `PRESENTATION_STORAGE_LIMIT`，不替换旧版本。

Linux `TasksMax` 包括线程。实机 Chromium 在 64 个任务的上限下无法启动，因此预览独立设为 256。Chromium 分配器还会创建逻辑长度 16 GiB 的稀疏 `memfd`；`RLIMIT_FSIZE` 连这种内存文件也限制，40 MiB 或 256 MiB 都会触发 `SIGXFSZ` 并使页面渲染进程崩溃。因此预览任务设置 `LimitFSIZE=infinity`，实际内存仍计入 cgroup，临时工作盘仍限制为 256 MiB，返回数据仍限制为 40 MiB。未禁用 Chromium 自身沙箱。

## 取消、恢复与消息桥

取消、超时和超量输出会停止该任务 systemd 单元，`KillMode=control-group` 回收全部后代；普通退出也完成回收再释放许可。服务重新打开时先清理本实例遗留单元及任务输入，再运行探针。

生成演示继续使用既有受限 iframe 和 CSP。每次装载生成新通道值，宿主接收时检查原 iframe 的 `WindowProxy`、不透明来源、通道及消息结构；动作通过该组件原有用户/现场/版本绑定执行。错误窗口、旧通道、未绑定来源和未授权动作不触发宿主调用。账号切换后的整个页面状态和连接清理归 MU8。

## 已知限制

本接口交付执行器与网络制作接线；现网发布、服务身份/权限配置、备份恢复和停服演练归 MU11。未安装依赖的环境保持能力关闭，不自动下载依赖。Windows 本地可信制作沿原入口执行；多人受限制作只支持已通过探针的 Linux 环境。
