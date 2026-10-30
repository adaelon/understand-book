# SESSION_CHECKPOINT — 2026-10-29

## 当前目标
继续原《动手学深度学习 PyTorch 版》standard_deep 构建，Pass2 disabled、并发上限 8、无 token 限制。用户已授权查根因、修复和继续，完成后在原聊天通知。

## 新鲜度与状态
- 根聊天：01a182f6-204a-79e2-95d7-10b5b1cc69dd。
- 原 invocation：abinv1_1a5a81af398505b190cf4a91981b7bb8848fdf93d7bc0d10336788a231fcb7b3。
- 原已确认计划保持 opaque；只使用机器检查点保存的路径，不读取、重建或修改计划。
- Pass1 complete；profile_sidecar 2999/2999 complete；BookStructure 最新 Engine 投影 226/732，后续 formal_objects / cognitive_materials / teaching_publish 尚待依赖。
- 真实总纲已通过 46 个章节批次和 4 个主题动作，后续发现 682 个结构任务。732 只是当前阶段已发现总量。
- 性能改进聊天于香港时间 15:29:01 激活新版持续 worker 引擎；已沿原构建续跑，没有另开书籍对照。
- 15:28:10 观察点为 181/732；15:39 元数据观察中，9 个 worker 已跨多个 dispatch，共完成 37 份 assignment。只据 Engine 投影报告正式进度。

## 当前故障与修复
- Engine 返回 NEEDS_USER(build_engine_failed)，无 choices；request_id：abreq1_b77e3ba9c91273e201f2279329ea6125fb857de6edbfd9594020765dd16802b5。
- 已检查原 refill 请求，字段与类型正确。
- 有界诊断：EPERM / open / readFileSync / withAutomaticBuildWorkerSchedulerLock，发生在争用者读取已有调度锁 PID 时。
- 已在 packages/core/src/automatic-build-worker.ts 修复：已有锁的 PID 读取遇到 ENOENT、EPERM、EACCES 时继续争用等待；仍只有成功 wx 创建锁者可进入调度；其他错误继续抛出。
- 新增 packages/core/test/automatic-build-worker-lock.test.ts，两个占用错误回归先失败，修复后 3/3 通过。
- Core TypeScript 检查通过；worker、worker-lock、driver-failure 三文件 23/23 全部通过，进程正常退出 0。
- v7 已打包（614 模块）并激活到原 operator 路径；protocol-doctor 8 项 compatible、invalid_artifacts=0。前版备份 understand-book-build.before-d2l-worker-lock-fix-20261029.exe，原 MCP 进程保留。
- 香港时间 16:24 后恢复 refill 成功：Engine 推进到 226/732 并发放 8 个新 worker，book_exec_1029–1036 均已启动并登记。1031 已终态，正在通过 refill 进程 64507 消费；其余 ownership 保留。

## 下一步
1. 等待 refill 进程 64507；如 app 恢复导致进程号失效，从机器检查点用精确拥有的 child 身份重新 refill。
2. 收到 SPAWN_EXECUTORS 后从 book_exec_1037 起立即启动新 child，使用新鲜连接并登记 ownership。
3. 保留原 ownership、completed_refs、terminal_observations、invocation；消费新终态时保持原计划、并发与质量门。
4. SPAWN_EXECUTORS 后立即启动全部 ready_executors，并登记 ownership；只在实际终态释放 child，普通提交与 WAIT 保留同一 worker。
5. 持续到 Engine DONE 或新的真实边界；当前故障已获维护授权，无需重复询问。

## 权威控制状态
- .understand-book/d2l-pytorch-20200313/.build/controller-inputs/root-checkpoint.json。
- 每次引擎返回、登记执行器、提交续接请求时保存；含 ready_executors、terminal_observations、原请求和进程号。
- 进程号在 app 恢复后可能失效；用精确拥有的 child 身份重新 refill，不能据聊天记忆宣告任务成功。
- 当前最近启动 child 为 book_exec_1036；后续新 child 从 1037 编号。
- 恢复时只有 ownership 不确定才 list_agents；不读取任何 child 原始 rollout、语义输入或候选内容。
- build.prepare 本阶段先前两次因 outline_pending 跳过，重试已完成；不以此重建或改变语义成果。

## 证据与代码
- 本目录 README.md：v1–v6 来源加载、Pass1 复用、阶段关闭、总纲容量、提示映射及诊断路径修复。
- docs/performance/continuous-workers-20261029：持续 worker 性能修复与有界元数据观察。
- 本次仅在现有 worker 模块增加锁读取竞争处理；保留同目录其他任务原有未提交/未跟踪改动。
