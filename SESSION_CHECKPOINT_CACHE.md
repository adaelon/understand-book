# SESSION_CHECKPOINT — 请求缓存优化

2026-10-21 Asia/Hong_Kong。用户授权：补请求差异定位、移动回合可变上下文、稳定 Tutor 示例，再以原 JSONL 问题与连续追问做真实缓存／费用／质量对照，不能牺牲质量。

## 当前状态
- 请求差异记录完成，15 项 Node 测试通过；runner 双题入口 7 项通过。
- Context/Tutor 两项生产改动完成；Runtime 11 项定向测试通过，1 项另需实际压力输入的既有 ignored 实验未跑。
- Server 的 5 项 tutor_to3_ 集成与 1 项契约导出全部通过；日志 `tmp/web-cache-eval-20261021/server-tests.log`。
- candidate 三个二进制已成功构建，两组各两题的真实对照全部完成并结清。已于 2026-10-21 14:01 HKT 随 `search-cache-20261021` 部署；保留其他未提交修改，未带入发布。发布与验证见 `docs/performance/web-search-jsonl/release-20261021/README.md`。
- 独立费用与匿名质量评估均完成。A=候选、B=基线，质量代理评估时未接收映射和费用；候选未见相对基线实质退步，已有过强表述明确保留在报告限制中。

## 文件与所有权
- request diff worker 完成：`evals/semantic/request-diff.mjs/.test.mjs`、`provider-recorder.mjs/.test.mjs`；只输出差异元信息，原生 reasoning 比对在剥离前执行，不落原文。
- jsonl_eval_impl：`evals/semantic/web-search-case.mjs/.test.mjs`、实验私有目录和汇总；两组均完成，服务和 recorder 已停止。
- Context worker 完成：`crates/runtime/src/context_fragment.rs`、`orchestrator.rs`。
- Tutor worker 完成：`crates/runtime/src/tutor.rs`、`tool_contract_tests.rs`、`crates/server/src/tests/tutor_example_tests.rs`。
- root：文档、统一 Cargo、候选冻结与质量交付已完成。质量报告 `docs/performance/web-search-jsonl/cache-quality-review.md`，总报告 `cache-optimization.md`，可保存指标 `cache-metrics.json`。

## 实验协议与已发生费用
详见 `tmp/web-cache-eval-20261021/protocol.json` 和 [实施报告](docs/performance/web-search-jsonl/cache-optimization.md)。

两条件均开 Search/Fetch、同一书和原始 547 行历史、同一真实 DeepSeek 直连模型，各条件隔离账号 4 元共享额度。每组两题同一会话，逐题等待后台结清。

原题：好的，jsonl我知道是什么了，那为什么用jsonl呢，还有什么其他选择吗

追问：那联系这个项目的模型调用流程，采用这种格式会怎样影响上下文缓存？如果改成 SQLite，这些收益还能保留吗？请区分项目已经实现的机制和一般可能性。

基线程序已从上一轮 bin-r2 冻结到 `tmp/web-cache-eval-20261021/bin-baseline`；本轮修改前文件在 `source-before/`。

基线两题已完成并自动结清，无后台请求／未知费用：原题 ￥0.278105，追问 ￥0.288162，合计 **￥0.566267**。真实 offpeak 费率；模型 ￥0.556267、Search ￥0.010000、Fetch ￥0。基线服务已停止。原文与完整统计见 `baseline/summary.json`。

基线跨回合首请求：previous_ordinal=4，首变 messages[1].content（system），仅共同前缀1消息／1091序列化UTF8字节，缓存512/115274。Web工具首次显露／新回合收回也改变工具数组；不能将全部失效归因于状态位置，不能把数组索引处description差异误当同名描述改写。

## 已完成交付
1. Runtime、Server、Node 定向验证已完成；候选构建成功。
2. 已用 `contracts/rust.json`、`contracts/tutor.json` 更新两份派生工具文档。
3. candidate：原题 ￥0.263055、追问 ￥0.097013，合计 ￥0.360068，全部为模型费；本次两组共 ￥0.926335，无未知费用。供应商缓存输入占比 57.49%→70.67%。
4. 匿名 A/B 与独立账本核对均完成，报告记录了调用路径差异和供应商缓存不可重置等局限。不能因一次样本宣称稳定节省 36.41%。
5. 未部署；没有额外付费运行。后续若继续优化，可优先使用已实现的诊断定位工具显露／收回引起的定义变化。

## 构建约束
机器提交内存接近上限。先前两次 Runtime 编译（codegen16，分别debug2/debug0）LLVM OOM。成功配置：`cargo --config profile.test.package.server.debug=0 --config profile.test.package.runtime.debug=0 --config profile.test.package.runtime.incremental=false --config profile.test.package.runtime.codegen-units=256 test -j 1 ...`。Server 当前也关闭该包 incremental。

不要并行运行多个 Cargo。构建 dev 时采用对应 profile.dev 包配置。未关闭用户应用、未改页面文件。旧基线二进制不受编译失败影响。
