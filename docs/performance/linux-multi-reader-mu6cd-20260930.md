# MU6c–MU6d 调度与观察恢复

基线：HEAD `5e10516`，MU0–MU6b 实时工作树；保留其他工作线。

## 实施步骤

- [x] MU6c：统一容量配置、按用户轮转领取、模型许可和单轮预算、同步等待隔离。
- [x] MU6c：双用户并发、多窗口份额、嵌套调用、取消与 usage 验证。
- [x] MU6d：观察配额、排队到终态连续观察、进程游标与持久快照。
- [x] MU6d：断线、溢出、重启、撤销与未保存验证。
- [x] 更新接口、架构、代码链路与 checkpoint。

## 实施合同

沿 ADR-0147 与切片方案 §6.5/§6.6；既有 UserRuntime、ReaderWorkspace、RunScope、授权与 History 权威不变。用户明确要求顺序实现 MU6c → MU6d。模型网络槽只覆盖一次实际调用，工具及其嵌套调用在释放模型槽后执行。当前未开放的网络制作、翻译和后台复核入口保持原 capability 行为。

## 验证记录

定向测试检测份额越界、HTTP 等待饥饿、预算旁路和观察错序；共享核心回归检测本地 Agent 和历史序列化行为变化。所有测试使用独立 TEMP/TMP `tmp/mu6cd-tests`，没有付费模型调用或生产部署。

| 验证阶段 | 结果 |
| --- | --- |
| 初次 `cargo check -p server --all-targets` | 发现 RunUsage 缺 Debug 派生和两个容量整数类型不一致；已修正 |
| 初次 MU6 回归 | 19 通过、2 失败：旧测试按单槽同步阻塞；usage 缺状态投影 |
| `cargo test -p server --lib -- mu6 --test-threads=1` | 23 通过，72.56 秒 |
| `cargo test -p server --lib -- mu6 mu4_ agent_stream:: --test-threads=1` | 38 通过，104.45 秒；含四个同步等待的真实 HTTP 用例 |
| 新增 MU6d 专项 | 4 通过，7.52 秒；epoch/溢出、观察配额、排队至终态及重启 HTTP 旧游标、排队取消 |
| `cargo test -p server -- --test-threads=1` | 427 通过、0 失败、43 原有忽略；库 419 通过/33 忽略，565.56 秒；Book MCP/CLI 7，预览取消 1 通过/10 忽略 |
| 领取前容量拒绝的原观察流复现（修复前） | 预期失败：旧实现清理 queued 流，使原观察者失去后续事件，2.73 秒 |
| 最终生产源码 `cargo test -p server --lib -- mu6 agent_stream:: --test-threads=1` | 33 通过、1 新增测试准备错误，89.58 秒；包括容量拒绝修复、preparing→queued→终态与真实 HTTP 断线/配额 |
| 修正预算测试输入后单独复验 `mu6c_configured_budget_stops_nested_round_without_losing_committed_note` | 1 通过，2.56 秒；配置生效、模型只调用一次、预算终态、已发生笔记保留、现场容量 |

完整输出见 [日志目录](linux-multi-reader-mu6cd-20260930/)。不重复计算全量内由写入者锁测试启动的子进程用例。全量运行在最后的领取前流保留和 preparing 观察修正之前；最终生产源码由上述受影响组及单独预算用例覆盖，不声称最终文件状态一次全量全绿。

新增预算测试最初重开服务后仍使用旧现场版本，随后又用“你好”触发未获授权的笔记写入；两者均被已有合同正确拒绝。测试改为重新挂接并明确请求“请在 1.1 保存笔记”；生产实现与断言未放宽。预算用例重跑成功，最终受影响的 34 项均有通过记录。

领取前容量拒绝修复：ActiveGuard 只在确实存在已领取活动执行时清理实时流，queued 因容量/存储暂缓时保留同一流；回归验证恢复容量后原观察者收到 saved 终态。preparing 流按真实接单阶段推进，不提前显示 queued。

MU6b 的未保存测试也扩展为检查已打开观察流收到失败，retry-save 后同一原结果变为 saved。初次失败的旧测试仍执行了新并发实现却等待单槽拒绝，因此阻塞 Probe 超时；新断言要求 A/B 两个调用均进入且停服后不再领取，保留原测试的生命周期目的。

## 实现

- `RunAdmissions` 先按用户轮转取得活动份额，再加载原私人输入并 CAS 领取；同用户 FIFO，多窗口共用份额。领取准备、运行退出和停止共用状态边界。
- `Resources` 对模型请求按用户轮转发许可；每次调用结束后释放，外层执行不会持模型槽等待工具。当前可达的画像判断、教学评估、query/synthesize、压缩/修复共用 adapter 和单轮预算。
- `LimitedAdapter` 保存每次最后一份累计 Provider usage，失败/超时无 usage 记录 null；次数和已知 Token 总数达到阈值后禁止下一次调用，并收紧单次输出上限。预算故障沿原 History 终态入口保存。
- `ServiceLimits` 从 `service-limits.json` 一次读取并接通运行、模型、制作池、HTTP/SSE、常驻用户/现场/全部 Book 容量。三种制作槽独立，网络实际制作仍未开放。
- 同步 chat 接单后由有界等待线程处理，超额返回原 turn 的 202；普通 HTTP 工作线程继续读写。停止后停止领取、协作取消、等待同步退出并冲刷所有常驻用户 pending reads。
- 队列与活动运行共享一个观察流；epoch 游标不跨流拼接，重启/终态流重建只读取 History。缓冲溢出用原子 snapshot(N) / cursor N 恢复；用户/全局连接配额在创建观察线程前取得。
- 观察持有精确登录会话和原材料身份，逐批复验；慢客户端不持业务锁，也不固定服务写根。取消、未保存与重存保留原回合身份。

## 已知限制

证据为 Windows 隔离服务根、本地 fake Provider、真实 HTTP/SSE、提交点故障及服务根重开。Linux kill/掉电、真实 HTTPS/iPhone、MU0 五分钟完整容量门槛、生产部署仍按 MU10/MU11 验收。

网络绘图/动画/预览、翻译/本地化与独立后台复核入口沿原 capability 拒绝；三个制作资源池已验证独立许可，实际执行器与私人产物/临时磁盘限制属于 MU7。完整 Vue 观察客户端和设备生命周期属于 MU8。

Token 预算按已报告总量阻止下一次调用，最后一次调用可能越过阈值；未知 usage 不伪装为零，不构成金额硬额度。事件不逐 token 持久化，运行崩溃后仍沿原 claimed 不重放合同。

保留原 ts-rs 属性警告和 NetworkRunPort 仅测试使用的两个方法警告；三个制作 Resource 变体在实际 MU7 执行器开放前有 dead_code 警告。

