# 本机后台运营引擎接入核查

核查日期：2026-10-28。只读观察和接入说明；没有修改后台宿主配置、终止进程或调用远端服务。原始限定字段见 `operator-routing-observation.json`。

## 结论

准备中的独立 Codex 插件 `env.UNDERSTAND_BOOK_BUILD_EXE` 和项目 `shell_environment_policy.set.UNDERSTAND_BOOK_BUILD_EXE` **只覆盖本项目的新 Codex 会话**。视频、音乐后台的 `operator-build-host` 使用 DSH 插件自己的 `config.executable`，不会读取这两个 Codex 设置，因此不能凭 Codex 激活宣称后台视频、音乐已接入新版。

本轮检查时，本机 20 个 `understand-book-build.exe` 都从 `E:/allwork/Understand Book/understand-book-build.exe` 运行，命令类型均为 `executor.mcp`。没有观察到 `video-operator`、`music-operator`、DSH / DeepSeek Harness 或加载 `operator-build-host` 的匹配进程。

标准 DSH desktop/web profile 的依赖中均未发现 `@understand-book/dsh-plugin`，对应配置文件也没有 Understand Book / operatorBuild 条目；标准 node_modules 路径没有该插件。当前检查进程以及 User/Machine 层的 `UNDERSTAND_BOOK_DSH_ENGINE`、`UNDERSTAND_BOOK_OPERATOR_BUILD_CONFIG`、`UNDERSTAND_BOOK_BUILD_EXE`、`UNDERSTAND_BOOK_BUILD_SIDECAR` 均未设置。这不等同于读取其他进程的私有环境，但配合进程与标准 profile 观察，目前没有可确认的本机活跃运营宿主。

所以，**目前没有可据以声称“已在后台生效”的运行实例**。若按当前默认发现规则新启动 DSH 构建插件，会从 HKCU `Software/UnderstandBook` 的 `InstallDir` 得到旧安装路径 `E:/allwork/Understand Book/understand-book-build.exe`。

## 代码依据

- `packages/dsh-plugin/src/config.ts:25–33`：启动时解析，优先级为插件 `executable` → `UNDERSTAND_BOOK_DSH_ENGINE` →注册表安装路径。`UNDERSTAND_BOOK_OPERATOR_BUILD_CONFIG` 仅提供运营连接、原根 Agent id 和媒体工具配置，不选择 Engine。
- `packages/dsh-plugin/src/index.ts:22,38–41`：`apply` 时冻结解析结果；有 `operatorBuild` 才启动后台循环；插件 dispose 会取消当前宿主工作。因此不能为即时生效而随意重载正在执行的插件。
- `packages/dsh-plugin/src/operator-build-host.ts:111,149`：音乐、视频都把同一 `config` 传给各自引擎调用。
- `packages/dsh-plugin/src/engine-client.ts:11–17,51–66`：实际 `spawn(config.executable, ['video-operator'/'music-operator'])`；不会读取 `UNDERSTAND_BOOK_BUILD_EXE`。启动环境经过明确挑选，也不把 Codex plugin env 当作运营配置。
- `skills/build/sidecar-entry.ts:96,146`：打包引擎把自身路径设为 `UNDERSTAND_BOOK_SIDECAR_SELF`，视频 Python 和音乐资源由实际被启动的引擎及其相邻资源提供。

## 三种生效范围

| 范围 | 如何接入新版 | 本轮状态 |
| --- | --- | --- |
| 本项目新 Codex 会话 | 独立插件启用；插件 executor env 与项目 shell env 同指最终 exe；新会话加载角色与 MCP | 已准备，待最终引擎测试后激活 |
| 已有 Codex 会话 / 已连接 MCP / 已运行引擎 | 原连接和进程继续用原可执行映像；配置文件改动不替换运行中进程 | 保持旧版，不中断 |
| DSH 视频、音乐后台 operator | DSH 插件 `executable` 或启动前 `UNDERSTAND_BOOK_DSH_ENGINE` 指最终 exe；保留原 driverRoot、rootSessionId、连接与媒体工具配置，下一次安全初始化后生效 | 未发现活跃本机宿主，也未配置新版；不列为已激活 |

最终 exe 约定：

`E:/allwork/Understand Book/operator-builds/20261028-prebuild-reuse/understand-book-build-x86_64-pc-windows-msvc.exe`

## 后台最小可逆动作

若已有运营 DSH profile：备份该插件配置，仅把其 `executable` 改成上述最终路径；如果现配置没有显式 `executable`，可只在下一次启动 DSH 的启动脚本里设置 `UNDERSTAND_BOOK_DSH_ENGINE`。显式 `executable` 优先级更高，已有值时仅设环境变量不会覆盖它。保持现有 driverRoot、rootSessionId、连接凭据和 video 工具路径，完整保留新 exe 相邻的 `resources/music/`。

已有宿主在运行时不重载；等当前任务自然结束、运营方原定安全重新初始化时加载新配置。回退只需把该路径设置恢复为备份值，旧安装和旧工作区继续保留。

本机标准 profile 没有发现该插件和运营配置，因此这里还不能执行“一项路径更新”。若要启用本机后台，需要先确定实际使用的运营 profile / 原根 Agent / 连接配置，再通过已有 DSH 插件安装流程接入；本轮不凭空创建宿主或接管队列。远端后台也不受本机 Codex 配置影响，本轮没有部署远端。

## 验收

新 Codex 会话应由 `codex mcp get understand_book_build_executor --json` 显示独立插件缓存与新 env，随后新启动 MCP 的 `ExecutablePath` 应为最终路径；项目 shell 的指定变量应为同一路径。

后台验收需要观察**实际**由运营宿主发起的 `video-operator` / `music-operator` 子进程路径为最终 exe，或者记录其完整启动配置。仅检查文件存在、Codex MCP 路径或引擎协议协商成功都不足以证明后台已切换。无需为了核查而创建付费任务。

另有 Reader 本地意图/音乐校准入口使用 `UNDERSTAND_BOOK_BUILD_SIDECAR` 或 Reader 相邻 exe（`crates/server/src/build_intent_api.rs:1734`、`music_calibration.rs:68`）；它也不受 Codex plugin env 自动覆盖。该入口与 operator 队列不同，本轮保持原配置。
