# EX13.2 已保存候选的源码投影

2026-10-02。**实现及确定性验证完成**。Runtime 443 项通过、0 失败、3 忽略；Server 编辑与真实存储测试 4 项通过、1 忽略。新增模型调用 0，usage=null。合同见 [EX13.2](../../../切片方案-EX13-Goal工作计划与演示修订上下文.md#ex132裁剪当前回合中已经有可靠落点的旧源码)，依据 [ADR-0155 §4](../../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md#4-历史裁剪与版本修订)。

## 保存与采样边界

`ActiveToolResultLedger::insert_call` 从本轮 `presentation.author` 的成功 `candidate_saved` 结果取得候选身份，只接纳 write/patch。没有候选落点、错误结果和其他操作不产生源码裁剪资格。

```text
write / patch 成功保存 → 首次后续采样保留完整调用和成功回执
  → 采样成功返回 → 后续请求裁剪 html / readable_content
  → saved_source 保留候选身份、index.html 和 read 参数
```

assistant 调用的 id、工具名、操作、标题、based_on/reference、来源、资源、假设和状态参数保持原义。patch 的输入 candidate_id 仍指基底，输出候选放在 saved_source 内，避免混淆。读取入口省略 file 时会返回源码及 readable_content；只需源码可按已保留的 index.html 定位读取。

最近 patch 的精确 edits 保留。仅在明确以该候选为基底的后继保存成功、且回执被采样后，早期 edits 才收敛为应用数量和读取定位。另一个独立 write、失败 patch、尚未观察的后继都不退休这份修改上下文。

裁剪只作用于模型消息副本，并要求调用和结果同时出现在窗口内；不改变原始消息、私有持久化裁剪规则、内容文件或工具回执。结果正文因现有 48 KiB 预算被淘汰后，assistant 中的 saved_source 继续有效。中途压缩保留当前回合原始后缀，投影仍由同一运行账本应用，source ID 与 required source 的生成和验证路径不变。

Server 的候选不可变，patch 保存新候选，旧候选和交付后的候选仍能由原运行读取；新运行继续拒绝旧 candidate_id。恢复定位不授予来源、预览或交付资格。

## 请求证据

受控循环使用 EX13.0 冻结的两次 write HTML 与 readable_content，顺序为发现 → write1 → preview1 → write2 → preview2 → review → 终答。测试端口采用 c1/c2 候选标识及空 source_ref_ids；真实来源字段保持和真实磁盘恢复分别由账本、Server 用例验证。两份 HTML 仍为 **18,486 与 31,850 字符**。

`before/saved-source/` 在生产改动前捕获，`after/saved-source/` 为相同输入的改动后捕获，各含 7 组计划、Native 和 ReAct JSON。改动前回归在“后续请求仍含 HTML”处失败，改动后通过。

| 末次请求统计（UTF-8 紧凑 JSON 字节） | 改动前 | 改动后 | 减少 |
| --- | ---: | ---: | ---: |
| 录制内容 | 128,457 | 52,654 | 75,803（59.01%） |
| assistant 调用参数 | 79,731 | 3,928 | 75,803（95.07%） |
| Native 序列化 | 128,150 | 52,347 | 75,803 |
| ReAct 序列化 | 145,423 | 62,994 | 82,429 |

共同 instructions、工具集及结果回执不变。首差位于 `messages[8].tool_calls[0].arguments`；两个裁剪边界分别是 request-02→03、04→05，首次成功回执所在 request-02、04 保留完整对应调用。裁剪完成后保持稳定，review 不再改写旧源码投影。完整分类、边界断言和两种协议统计见 [comparison.json](comparison.json)，末次同输入比较见 [before-after.json](before-after.json)。

## 验证与重放

- Runtime 新增 3 项：真实录制 HTML 的受控循环、失败/缺落点/未观察/无配对/结果淘汰、新运行隔离、独立作品与 patch 链；原压缩用例加入保存后裁剪与 required source 覆盖断言，指导前缀用例只允许明确的源码裁剪边界。
- `cargo test -p runtime --lib`：443 通过、0 失败、3 忽略；日志 `tmp/ex13-2-runtime.log`。
- `cargo test -p server --lib presentation_author_tests::editing::`：4 通过、0 失败、1 忽略。新增真实磁盘用例覆盖两个独立作品、连续 patch、交付资格、交付后原候选读取、跨运行拒绝和确切已交付版本读取；日志 `tmp/ex13-2-server.log`。
- `summarize.py` 断言裁剪发生的采样位置、首次完整观察、后续稳定性及 Native/ReAct 实际参数，`test_summarize.py` 覆盖录制重放和统计结果。

```powershell
$env:EX13_REQUEST_RECORDING_DIR = "$PWD/docs/performance/presentation-context-ex13/ex13-2/after"
cargo test -p runtime --lib ex13_saved_source_projection_keeps_first_observation_and_raw_history
Remove-Item Env:EX13_REQUEST_RECORDING_DIR
python docs/performance/presentation-context-ex13/ex13-2/summarize.py
python docs/performance/presentation-context-ex13/compare.py compare docs/performance/presentation-context-ex13/ex13-2/before/saved-source/request-06.json docs/performance/presentation-context-ex13/ex13-2/after/saved-source/request-06.json --output docs/performance/presentation-context-ex13/ex13-2/before-after.json
python -m unittest discover -s docs/performance/presentation-context-ex13/ex13-2 -p test_summarize.py -v
```

## 已知限制

以上是受控请求与离线序列化结果，没有发送模型 HTTP 请求；字节不是 token，真实缓存和费用收益待 EX13.6。保存后的首次采样仍带完整源码，最近 patch 的 edits 保留到明确后继被观察。本片不裁剪主动 read 的源码结果，它继续受现有结果预算管理。

本次未提交、未部署。EX13.3–EX13.6 待实施；EX12.4 最终原生验收仍受提供方 402 阻塞，EX12.5 尚未开始。
