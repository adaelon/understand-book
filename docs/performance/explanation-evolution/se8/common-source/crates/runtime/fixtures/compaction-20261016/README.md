# 网站对话压缩回归样本（2026-10-16）

`failures.json` 保存 LangSmith 项目 `understand-book-dev` 中两轮对话的压缩输入与四份原始草稿。来源为 10:25:25 和 10:29:20（香港时间）的运行：

- `01a14287-4d0c-7700-8c6d-f5f4bcfd66ec`：首次生成合并了证据范围；重试草稿引用了未定义的 `answer.readonlybase`、`answer.crosslang`。
- `01a1428a-e225-7113-a68e-eccf4cc2d801`：首次生成含中文 item_id；重试草稿同时含非法 item_id 和未获授权的合并范围。

每轮的 `request` 是线上生成请求中的六个语义输入字段；`attempts[].draft` 与 LangSmith 的 `outputs.parsed` 逐项一致。测试为运行时拥有的版本、history revision 和 raw 保留项补齐测试信封，不改语义输入。样本包含读者问题、书籍摘录、工具回执及摘要；来源运行 ID 保留在每条记录中。

`repairs.json` 是使用修订后的生成约束、完整拒绝草稿和聚合诊断，通过正式 `NativeAdapter` 调用同一 `deepseek-v4-flash` 模型得到的四份实际修复结果。每份只进行一次修复，全部通过正式 `validate_draft`；覆盖数量依次为 53、53、54、54。

离线回归：

```powershell
cargo test -p runtime --lib recorded_compaction_ --config profile.test.package.runtime.codegen-units=16
```

显式在线重放会使用 `.env` 中的 Provider 配置，模型固定为样本记录的模型，并将请求、结果和校验结论保存到指定目录：

```powershell
$env:COMPACTION_REPLAY_OUTPUT = Join-Path (Get-Location) 'tmp/compaction-replay'
cargo test -p runtime --lib recorded_compaction_live_ --config profile.test.package.runtime.codegen-units=16 -- --ignored --nocapture --test-threads=4
```
