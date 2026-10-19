# BookStructure 工具调用提速

本次修复针对 Understand Book 项目书的现有 BookStructure 构建。保留来源、已接受动作、模型输入及接纳规则，减少工具调用重复处理既有上下文的成本。

## 执行步骤

1. 采集现有工作区只读阶段重放的耗时与 CPU 分布，保存当前全部任务描述及模型输入作为对照。
2. 消除已确认的重复工作；用真实任务输入逐项对照、来源变化及接纳回归验证正确性，再测相同重放。
3. 编译 Build Engine，验证打包后的执行器入口，安装新引擎并重装插件，记录安装位置和版本。

## 实现

- `book-structure-generation.ts:validateBookStructureGenerationTask` 用结构比较核对规范化后的任务。规范化保留同一个 `input` 对象，因此比较可直接跳过完整上下文；仍检查目标、描述符、来源范围、证据顺序和实际交付内容的现有证明。
- `readBookStructureOrganizationState` 保留产物身份与策略检查，把动作应用一次所得的状态直接交给路由器。动作应用本身完成当前上下文校验，省去原先“应用以校验，再应用以推进”的重复工作。
- `routeStructureOrganization` 每步浅拷贝主题上下文，保留该步的 `prepared` 引用；目录、章节和来源摘录在同一次路由内不变，不再逐动作深拷贝。

## 测量

测量入口为 `tmp/book-structure-speed-20261015/measure.ts`。只读 `understandbook` 工作区，通过 `readAutomaticBuildTaskStage` 重建当前 BookStructure 状态；不生成候选。输入含 2,595 个候选、35 个已完成章节、9/11 个已完成主题，共 1,286 个当前任务。

| 版本 | 阶段重放耗时 | 校对 |
| --- | ---: | --- |
| 修改前，带 Node CPU 采样 | 88,702.7 ms | 保存基线描述符和交付输入 |
| 结构比较与浅拷贝 | 37,074.5 ms | 1,286 个描述符与交付输入逐项相同 |
| 再消除重复动作应用 | 32,709.5 ms | 1,286 个描述符与交付输入逐项相同 |
| 最终版，单独运行并启用 Node CPU 采样 | 33,344.8 ms | 1,286 个描述符与交付输入逐项相同 |

相同 CPU 采样设置下，阶段重放耗时减少 **62.4%**，约为原来的 **2.66 倍速度**。基线 CPU 样本中，递归序列化任务比较约占 37.8 秒，主题输入重建约占 18.6 秒，深拷贝约占 6.0 秒；包含调用时间存在重叠，不能相加。最终采样中 `validateBookStructureGenerationTask` 不再是主要热点，组织阶段路由的包含调用时间从约 70.7 秒降至 21.3 秒。

另一次与执行器测试并发的采样为 71,222.0 ms，当时系统可用内存约 1 GB，文件读取和未修改的执行预算计算同时变慢。该原始样本保留为 `optimized.json` / `optimized.cpuprofile`，不作为优化倍率的对照。

复现命令（仓库根目录执行）：

```powershell
node --cpu-prof --cpu-prof-dir=tmp/book-structure-speed-20261015 --cpu-prof-name=optimized-serial.cpuprofile --import tsx tmp/book-structure-speed-20261015/measure.ts optimized-serial tmp/book-structure-speed-20261015/baseline.json
```

## 验证

- 任务身份特征测试 5 项通过；包含字段顺序变化、非规范头、额外字段、证据顺序与磁盘耗尽后的恢复。
- 全书引擎流程（词法与语义检索）、主题修订、恢复回归中 13 项通过。
- 剩余恢复回归的旧夹具按 24 项分页假定第 26 项未显示，生产路由实际为 64 项分页。改用 66 项，并断言被修改项确实未显示；该项通过，覆盖保留其他章节、旧收据和来源变化后的写入拒绝。
- Core TypeScript 检查通过。
- 执行器会话、工具适配和 MCP 的 72 项用例中，首次 70 项通过；另外两项策略变化用例在并发负载下超过默认 5 秒。单工作线程复跑这组 4 项，全部通过；`start` 和 `submit` 分别约 2.2 / 2.4 秒，未修改测试源码的超时或任何产品行为。
- 上述 BookStructure 与执行器回归合计覆盖 86 项不同用例。
- `build-sidecar.mjs` 编译通过；`smoke-automatic-build-parity.mjs` 验证薄插件、仓库外 Node/Bun 一致性、调度租约与收据恢复；`smoke-t7-executor-release.ts` 验证实际编译版的四个 MCP 工具、输入交付、提交、幂等恢复和错误路径；`assert-plugin-release.mjs` 发布检查通过。
- 安装后再次通过 `--sidecar` 指向正式引擎、`--installed-plugin-root` 指向 Codex 缓存运行 T7，结果 `status=passed`、`installed_launcher_executed=true`、`final_status=committed`。该运行使用独立合成工作区，不提交原项目书的模型动作。

## 安装

- 2026-10-15 18:56:14 +08:00 安装引擎：`E:\allwork\Understand Book\understand-book-build.exe`，106,909,184 字节；与通过验证的编译产物逐字节相同。
- 原引擎保存在同目录 `understand-book-build.before-book-structure-speed-20261015184655.exe`。
- `codex plugin marketplace add` 复用已配置的本地市场；`codex plugin add understand-book@understand-book --json` 成功安装版本 **`0.1.0+codex.20261015184655`**。
- 插件路径：`C:\Users\Lenovo\.codex\plugins\cache\understand-book\understand-book\0.1.0+codex.20261015184655`。`codex plugin list` 确认 `installed=true`、`enabled=true`；全部 10 个发布文件与安装文件逐字节相同。
- 本地原始记录：`tmp/book-structure-speed-20261015/engine-install.json`、`installed-files.json`、`compiled-executor.json`、`installed-executor.json`、`parity.log`，以及各轮测量 JSON 与 CPU profile。

## 已知限制

以上耗时是单次本机阶段重放，不能直接换算为整本书的完成时间。工具仍会重新读取当前阶段；动作仍使用现有单步执行器协议，主题调度仍串行。模型生成、执行器启动和网络等待未包含在此测量中。

安装验收时仍有 6 个安装前启动的执行器 MCP 进程。替换磁盘程序不会热更新已有进程。原对话需在当前执行器收尾后重新建立 MCP 连接；重启 Codex 再继续原构建可确保使用新引擎。现有工作区和已接纳产物保持原样。
