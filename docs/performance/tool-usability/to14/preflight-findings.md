# 运行前确定性发现

## 带选区的保存请求没有激活笔记

`smoke-final/input.json` 使用 Reader 支持的 `question_quote` 和明确的“记为这一段的笔记”。真实 `/agent/chat` 把它包装成 `selection_provenance.v1`，其中 `user_question` 是 JSON 字符串。`orchestrator::run_outer` 对包装后的 question 调用 `classify_turn_intent`；`action_request_text` 排除引号内容，连用户保存要求也被排除。

实际 `tool.search` 返回 `explicit_effect_intent_required`，随后 `reader.note` 返回 `TOOL_NOT_EXPOSED`，重开存储为空。已保存的 `current_user_message` 仍有原始用户要求。这是生产支持的“选区＋保存”路径问题，不是笔记重开路径错误；TO5 的直接 Runtime 输入回归没有经过这个包装。

首次 smoke 的脚本未发现 deferred 工具即调用 reader.note；`smoke-final` 补上正常发现仍被拒绝，证据完整保留。真实任务继续使用原冻结输入，让 Agent 自行处置，不能通过改题或工具强制授权掩盖失败。TO14 的实验不修改这一生产行为；后续最小修法是在既有权限推断接点使用当前用户原话，并补带选区请求的回归，保留引用正文不授予权限的规则。

## 基线构建资产

首次源码快照缺少三个被既有测试 `include_str!` 引用的文档夹具，编译失败。采样前已从各自原 commit 补入，保留 compile-baseline.log；没有改基线产品代码。后续统一 freeze 入口包含这些构建资产。

## CLI 恢复后完成被执行器误判

baseline-tutor-continue-2 的第三次 CLI 记录先出现503重连提示，随后正常返回结构化答案和 `turn.completed`。旧 `summarizeEvents` 见任一 error 就判失败，将这次有效输出丢弃，真实 Server 因而返回502且没有 MessageDelivered。

已改为以最后的 turn.completed／turn.failed 判断终态，重连提示单独记录，并补恢复成功／恢复后失败的测试。原样本保留 unresolved/invalid_harness；用原先已经返回的三次决策做[无模型回放](diagnostic-replay/baseline-tutor-continue-2/result.json)，确认同一宿主可以正常交付。没有重抽模型，也没有用回放覆盖原样本。

## 目录测量的 HTTP 接线

早期独立 tools/list 记录器没有正确发送 Host、Accept 和协议版本，得到 CAPABILITY_FORBIDDEN；Codex 自身的 MCP 客户端发送正确请求，六次真实阅读均可完成。记录器已改用显式 HTTP headers；两版在同一冻结程序上进行无模型目录采集，结果保存于 catalogs/。早期 runs/ 内的失败目录记录保留，目录字节数只取 catalogs/ 中成功返回的原始定义。
