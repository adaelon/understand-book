# MU7 受限制作与浏览器边界

基线：2026-09-30，HEAD `5e10516` 与 MU0–MU6d 实时工作树。

## 交付

- `presentation_sandbox.rs` 与固定 `presentation_worker`：systemd cgroup + bubblewrap，实际启动探针与 fail-closed capability。
- `NetworkRunPort` 接共用 `AuthorSession`，私人读写沿原用户权威；预览、绘图和动画分别占用 MU6 独立许可。
- 生成输出文件验证、回合临时媒体上限、每用户候选/版本/状态落盘配额。
- iframe 每次装载使用新通道，校验原 WindowProxy、来源和消息结构。
- 配置、资源数值与恢复接口见 [MU7 接口](../Linux多人阅读-MU7受限制作.md)。

## 实施合同

沿 ADR-0147 §8 与多人方案 §7/MU7。任务只获得授权内容副本；生成进程使用非特权身份、独立文件系统/网络/PID 命名空间及 cgroup 限额。缺失依赖或隔离探针失败时不开放网络制作。

## 验证目的

制作回归检测共用 author 入口变化；Linux 探针检测文件/凭据可见、网络可达、资源越界和进程残留；消息桥测试检测错误 WindowProxy/通道的消息是否产生宿主动作。失败时修正对应边界并保持能力关闭。

## Linux 实机验收

使用 [Linux 部署文档](../Linux阅读器部署.md) 所列主机，在 `/opt/understand-book/acceptance/mu7` 建独立源码、运行时和服务配置。没有替换或重启现网服务，没有生产数据迁移，也没有付费模型调用。系统为 CentOS Stream 9，Linux 5.15.120、systemd 252、bubblewrap 0.4.1、Chromium 138.0.7204.49；Python 3.12 运行时安装 Manim 0.21.0、Matplotlib 3.11.0、PyAV 17.1.0。

最终命令：先构建同源码 `presentation_worker`，再用 `MU7_SANDBOX_ROOT` 指向隔离服务根，运行 `cargo test --offline --locked -p server --lib mu7_linux -- --ignored --test-threads=1 --nocapture`。**4 通过 / 0 失败，58.03 秒**，见 [完整日志](linux-multi-reader-mu7-20260930/linux-final.log)。

| 用例 | 实际验证 |
|---|---|
| `mu7_linux_files_credentials_network_and_renderers` | 非 root 身份；测试环境密钥、真实其他用户哨兵文件和宿主根不可见；外网、本机服务、元数据地址不可连；真实 Matplotlib、Manim 和 Chromium 产出 |
| `mu7_linux_memory_pids_disk_output_and_symlink_bounds` | 2 GiB 内存分配失败；fork 触及进程边界；临时工作盘填满返回 ENOSPC；输出 symlink 和过大 SVG 拒绝；独立超量 worker 的 45 MiB 管道输出命中外层 40 MiB 边界；普通绘图仍可用 |
| `mu7_linux_cancel_timeout_and_restart_cleanup` | 取消连同子进程回收；两个忙进程的实际 cgroup `cpu.max` 为 `100000 100000` 且 `nr_throttled > 0`；超时终止；服务重开清除模拟中断单元和遗留输入；最后无运行任务 |
| `mu7_linux_network_author_renders_previews_and_saves_only_owner_version` | 真实 NetworkRunPort 绘图、保存候选、三种规定视口预览和交付；A 的私人 Store 能读含 SVG 的版本，B 的私人 Store 无该版本；实际 capability 为 true |

上述输出管道测试使用独立的超量 worker fixture，前三类真实渲染器和完整私人交付均使用正式 worker。没有用 fake 代替正式执行器验收。结束后无 `ub-render-mu7-*.service` 遗留任务，任务暂存目录为空，见 [环境及清理记录](linux-multi-reader-mu7-20260930/linux-environment.log)。

## 本地与浏览器验证

- 最终 Windows 受影响回归：使用本次独立 TEMP/TMP，包含 MU4/MU5/MU6/MU7、制作、编辑、私人 Store 和资源池，**84 通过 / 0 失败 / 29 按原条件忽略，173.18 秒**，见 [最终日志](linux-multi-reader-mu7-20260930/isolated-final.log)。
- Vitest：**7 通过**，覆盖文档装配和 WindowProxy/来源/通道/消息结构，见 [日志](linux-multi-reader-mu7-20260930/web-tests.log)。
- `vue-tsc --noEmit`：通过，见 [日志](linux-multi-reader-mu7-20260930/web-types.log)。
- Playwright：**6 通过**，使用 Rust 4175 测试宿主，验证错误窗口/通道/来源/宿主动作拒绝以及有效来源按钮、布局、主题、动态错误。见 [日志](linux-multi-reader-mu7-20260930/frame-browser-2.log)。
- 既有 `ts-rs` 属性解析警告和 NetworkRunPort 两个测试专用方法警告保留；没有新增编译错误。

## 失败原因与修正

1. 首轮 Linux 预览触及 `TasksMax=64`。该限额包括线程，Chromium 报 `pthread_create: EAGAIN`、`Zygote could not fork`。预览/探针改为 256，绘图/动画保持 64，实例合计 384。
2. 提高进程数后仍在 `Page.enable` 超时。Chromium 崩溃记录为信号 25（SIGXFSZ）；查到其内存分配器创建三个逻辑长度 17179869184 字节的稀疏 memfd。统一 `LimitFSIZE=40 MiB` 误伤了这些内部内存文件，256 MiB 也不足。预览取消该单文件逻辑长度限制，实际 cgroup 内存、tmpfs、返回输出、时间与进程边界保持。改后真实页面加载和截图通过；Chromium 自身沙箱保持启用。见 [崩溃信号、内存文件及成功截图记录](linux-multi-reader-mu7-20260930/chromium-limit-diagnosis.log)。
3. 原输出超限用例试图从生成 Python 重新打开宿主的 root 所有管道，实际被权限拒绝，未命中所测边界。改为独立超量 worker fixture；同时保留真实渲染器的过大文件拒绝用例。
4. Windows 第一次广域回归有一项 MU5 测试在建材料时返回存储失败，独立重跑通过。后续制作组的两项失败指向共享系统临时目录中的 `learning.db` schema；只读检查得到 `user_version=1`、`application_id=0`，与当前要求的 `0x55424c4e` 不符。既有 `state_named` helper 把不同测试的 Memory JSON 放在同一临时父目录，LearningStore 取同父目录固定 `learning.db`。最终回归改用本次独立 TEMP/TMP，未删除或修改共享临时数据库。
5. 首次 Playwright 未启动 Rust fixture，连接 4175 失败；启动规定 fixture 后 6 项通过。通过日志仍有一次页面结束后的默认 8787 保存请求代理告警，不影响该组断言。

## 已知限制

真实现网发布归 MU11；完整账号切换与多现场 Vue 生命周期归 MU8。Linux 测试覆盖执行器层和网络端口，尚未开展 MU11 主服务进程断电/生产恢复演练。未具备 Linux 执行器的环境继续关闭网络制作。本地旧测试 helper 的共享临时 Learning 数据库问题仍存在，回归需要独立临时根。
