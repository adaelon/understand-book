# 有界解释案例整理

`explanation-case.mjs` 从维护者明确选定、获准读取的导出材料整理一个案例。复用现有 Provider 证据解析、工具轨迹和真实用量统计；输出保存事实与解释，留待诊断是否应形成方法候选。

```powershell
pnpm eval:case:test
pnpm eval:case <私有 manifest.json> <新的私有输出目录>
```

在仓库内使用已忽略的 `.understand-book/evolution-cases/` 保存清单和输出。`case.json` 包含任务、原文、评论、完整演示、模型工具结果和机器路径，是私有材料。`index.json` 只包含固定状态、数量和用量，可以复制到 `docs/performance/explanation-evolution/`。输出使用新目录；已有 `case.json` 不覆盖。

## 输入合同

```json
{
  "schema_version": "explanation-case-input.v1",
  "case_id": "local-case-1",
  "origin": "actual_agent_revision",
  "obstacle": "要解释的关系或观察到的使用障碍",
  "materials": {
    "original": {"path": "exports/version-1.json"},
    "repaired": {"path": "exports/version-2.json"},
    "history": {"path": "exports/history.json"},
    "task": {"path": "exports/input.json"},
    "source": {"path": "exports/source-version.json"},
    "feedback_current": {
      "path": "exports/feedback.json",
      "observed_at": "2026-10-16T00:00:00Z"
    },
    "feedback_scene": {"path": "exports/exact-saved-state.json"},
    "provider_records": {"path": "exports/run.json", "select": ["qa", 0, "requests"]},
    "before_check": {"path": "exports/before-check.json"},
    "after_check": {"path": "exports/after-check.json"},
    "diff": {"path": "exports/source-diff.txt", "format": "text"},
    "parent_candidate": {"path": "exports/candidate.json"}
  },
  "method_basis": ["parent_candidate"],
  "observations": [{"fact": "原版出现的具体差异", "evidence": ["before_check"]}],
  "interpretations": [{"explanation": "根据代码对原因的判断", "evidence": ["diff", "before_check", "after_check"]}],
  "hypothesis": "下一步待验证的改进假设",
  "replay": ["打开原版并执行具体操作", "按同样操作打开修正版"]
}
```

路径相对清单所在目录，绝对路径也可使用。JSON 为默认格式；`select` 按数组中的键/索引选取已有导出的一个字段。`format: "text"` 保存逐字文本。材料可显式为 `{"status":"missing","reason":"no_reader_vote"}` 或 `{"status":"unavailable","reason":"access_revoked"}`；后者即使另带可读路径也不会尝试读取。没有提供、文件不存在、无权读取和 JSON 无法解析分别保留状态及原因。

`origin` 使用 `actual_agent_revision`、`actual_reader_use` 或 `engineering_fixture`，由材料来源决定。测试票只能使用工程夹具身份。task 保存当时实际请求，source 保存当时版本/来源及可用原文；用户直接提供的任务材料可以作为 source，不能补写一个不存在的书源版本。

| 材料 | 已有格式与关联要求 |
|---|---|
| original / repaired | `AgentPresentation` 完整版本。保留 content_files、source_bindings、method_refs、method_observations、owner、created_by_turn_id；修复的 based_on 必须指向原版 |
| history | 选定用户的导出历史；在相同 book/session 的 completed 回合中确认 answer_view 实际交付了该确切版本 |
| method_basis | 明确列出的历史候选或内容版本；递归解析 SE3 的 candidate/version/read 依据，不从当前目录猜测旧方法 |
| feedback_current | SE4 `PresentationFeedback`。可以是本地/多人/工作区已有授权 read 接口的导出；工作区外层 result 通过 select 选取 |
| feedback_previous | 可选的先前观察记录；只用于显示改选、更新或撤回，不能代替不可用的当前记录，也不累计票数 |
| feedback_scene | `SavedPresentationState {receipt, owner, state}`。必须匹配反馈附带的完整回执，不能选用 latest state |
| provider_records | `agent-run` 的 requests 数组；复用 `toolMessages`、`reportToolTrace`、`measuredUsage`。历史分文件导出可在私有目录转换为该既有结构，保留原文件定位；缺用量不估算 |
| 其他证据 | 检查结果、图片定位、差异或报告；observations/interpretations.evidence 引用材料键。程序标记材料可读性，语义支持仍须按对应证据判断 |

## 反馈、方法与归因

反馈必须对应原版的 session、内容 revision 和实际交付回合。有效选择、从未选择、已撤回、材料不可用分别表达。原现场另核对完整回执和 owner；没有现场仍可保留有效票。公开索引不包含评论、scope、文件名、机器路径、案例或演示身份，额外材料也只列状态。

`observed_at` 是材料导出时刻；`collected_at` 是案例整理时刻。反馈的 `freshness: export_snapshot` 明确这是导出快照，不能作为服务当前状态的保证。需要刷新时，经原授权入口重新导出当前反馈，再收集到新目录。撤权/删除聊天后记录不可用；整理器不会扫描私有存储或使用旧票绕过失效入口。若有独立的合法历史归档权限，归档仍只能作为历史观察。

方法追溯区分未记录、明确空声明、看过目录、读卡、读代码及声明使用。read 依据核对原回合、确切方法版本与所有 tool_call_id，并保留实际非空字符范围；继承追到所选旧候选/版本的原始读取记录。只读局部仍显示局部范围，不称作读完源码。历史共同指导如 ex11.v11 可放入解释和请求证据，不伪装成 SE3 方法引用。

页面反馈始终保留页面级归因，`method_credits` 为空；程序不自动分配每张卡的收益，也不把可读证据判作因果证明。案例保持 `pending_diagnosis`，事实、原因解释和假设分别保存。SE7 可依据具体观察决定提炼、缩小范围、已有方法足够或证据不足。

## 验收入口

首个真实案例及私有接手路径见 [SE6 记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/explanation-evolution/se6/README.md)。12 项整理器用例覆盖确切关联、继承、无现场、未读方法、改选/撤回、失效材料、选定运行读取及公开投影；相关证据解析和用量回归一并运行。历史页面重放通过现有 `presentationDocument` 与快照桥接，不运行新模型请求。
