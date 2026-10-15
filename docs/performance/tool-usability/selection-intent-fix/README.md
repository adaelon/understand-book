# 选区保存意图识别修复

2026-10-12。修复TO14实证的“带选区明确保存笔记，却未授权”问题。用户已在解释问题和最小修法后确认“修了吧”；沿用既有阅读笔记、选区证据和工具权限语义，属于纯技术接线修复。

## 原因与改动

`agent_run::execute_model` 已把未经选区包装的当前用户请求存入 `RunContext.current_user_message`。`orchestrator::run_context_inner` 却把包装后的 `question` 交给 `classify_turn_intent`；其中 `action_request_text` 会排除引号内容，于是JSON字符串形式的 `user_question` 被一起排除。

**决策**：动作识别使用已有原始当前用户请求。

**否决**：
- 解析selection_provenance取回用户请求：重复已有结构化字段的职责。
- 放开引号过滤：会把被引用的正文命令当作用户动作。

选区包装继续用于模型输入和正文证据；没有提供原始用户字段的直接Runtime调用继续使用其question参数。生产改动仅位于原动作识别接点。

## 回归路径

复用TO14使用的TC2A真实Server、选区包装、Reader及MemoryStore。模型边界由固定工具决策驱动，实际工具发现、权限检查、笔记分派及持久化均使用生产代码。测试保存在 `crates/server/src/tests/selection_intent_tests.rs`：

- 带选区明确“记为笔记”：检查首轮工具暴露、正常发现、CREATED回执、笔记ID、正文放置和磁盘重开后的原文字记录。
- 带选区只解释且明确不保存：即使模型尝试发现并调用reader.note，仍拒绝写入，Reader状态和磁盘记录不变。
- 所选真实正文是“Save a note.”、用户只要求解释：正文命令不授予笔记权限。

## 验证结果

旧逻辑先运行新增保存用例，发现返回explicit_effect_intent_required，reader.note返回TOOL_NOT_EXPOSED，重开存储为0条，测试按预期失败。证据见 [red.log](red.log) 和 [red/save-note.json](red/save-note.json)。修复后，同一输入返回CREATED，重开为1条，内容、note_id、anchor及正文放置均正确。

| 用例 | 笔记结果 | 重开记录 | Reader效果 |
|---|---|---:|---:|
| 明确保存 | CREATED，正常发现可见 | 1 | 1 |
| 只解释、不保存 | TOOL_NOT_EXPOSED | 0 | 0，状态不变 |
| 正文含保存命令、用户只解释 | TOOL_NOT_EXPOSED | 0 | 0，状态不变 |

三个实际结果见 [results.json](results.json)，完整请求、发现和回执分别在 [green/save-note.json](green/save-note.json)、[green/explain-only.json](green/explain-only.json)、[green/source-command.json](green/source-command.json)。

共38项通过：新增Server回归3项、既有Tutor三种UI动作入口1项、Runtime工具暴露／意图／权限31项、直接Runtime保存／Memory／只读3项。日志为 [green.log](green.log)、[server-ui.log](server-ui.log)、[runtime-exposure.log](runtime-exposure.log)、[runtime-write.log](runtime-write.log)。后两组也覆盖原始用户字段缺省时的直接Runtime调用，以及否定、引用文本、note与highlight分别授权的既有规则。本次没有调用真实模型；TO14原始请求、结果、源码快照和冻结程序保持原样。

## 复现

沿主工作区既有构建环境：RUSTUP_TOOLCHAIN=1.95.0，CARGO_TARGET_DIR为 `D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、CARGO_BUILD_JOBS=1，TEMP/TMP为 `D:/codex-build/to3-temp`。使用下列定向入口：

```powershell
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib selection_intent:: -- --test-threads=1
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib tutor_to3_projected_ui_examples_execute_with_runtime_bound_fields -- --test-threads=1
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib tool_exposure:: -- --test-threads=1
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib write_examples:: -- --test-threads=1
```

需要导出新证据时，将SELECTION_INTENT_DIR设置为已创建的新目录。所有测试使用临时材料和真实本地存储，不接触生产数据。

## 构建说明

首次编译混用了此前基线构建留下的read-tools产物，当前源码已有的方法和schema导出被报为缺失，日志见 [red-build-cache.log](red-build-cache.log)。清理read-tools和book-tool-contracts包的编译缓存后重新运行；编译失败不算行为红测。后续同时构建两个源码版本应分别指定各自CARGO_TARGET_DIR，避免缓存混用。

## 已知限制

TO5的EXISTING笔记回执与Goal完成判断是另一个已记录问题，本修复未改变其行为。当前变更尚未提交Git或部署生产服务。
