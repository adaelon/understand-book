# 方法目录到 Author 的选择路径

调查范围仅为 `common-source` 冻结源码与 skills。未读取题目、runs、候选结果，未访问模型或线上服务。以下文件行号均以 `common-source/` 为根。

**结论：拿到目录后直接 `prepare → write` 是当前契约允许的正常路径。方法库的使用是可选的；只有主动声明一个新方法引用时，才触发读取前置条件。目录已提供不等于作者被要求选用，也不等于正文已读。**

## 最短路径与事实

1. **编译目录和可读文件来自同一 manifest。** `crates/runtime/build.rs:5–18` 从 `UNDERSTAND_BOOK_METHOD_CATALOG` 或默认 catalog 读取目录；`:21–46` 将该目录登记的文件编入 `FILES`。`crates/runtime/src/presentation_methods.rs:8–17` 将整个 catalog 序列化到目录提示，正文仍按需读。因此不是 `prepare` 返回目录，也不需要先读方法才能得到目录。

2. **目录首次随 Author 工具可见而出现，早于显式 prepare。** Author 默认 deferred（`crates/runtime/src/tool_exposure.rs:1479–1482`）；成功能力发现激活后，下次采样暴露完整工具（`:1413–1420`、`:585–612`）。只要该采样含 `presentation.author`，`crates/runtime/src/agent_prompt.rs:151–152` 就装入 presentation modules；其中 common modules 无条件包含方法目录（`crates/runtime/src/agent_prompt/presentation.rs:83–112`）。没有已有 authoring context 时，普通作者阶段默认为 global（`crates/runtime/src/orchestrator.rs:5725–5738`）。这是工具可见性的条件，不是选了 global/local/review 后才解锁的方法目录；实验 policy 也调用同一装配函数（`crates/runtime/src/experiment.rs:21–28`）。

3. **选择是明确可选，读取是采用方法后的条件义务。** `crates/runtime/src/presentation_methods.rs:17` 明写 “Choose zero or more methods”，并将技术 needs 与方法选择分开；同一行要求适配前读 example source 与 adaptation guide、实际使用时声明 `method_refs`。但基础 skill 要求的是按内容关系选择表达方式，文字、图形、例子和交互都可用，没有要求每次从注册方法中选一个（`skills/presentation/SKILL.md:8–18`）。

4. **prepare 的输入和返回不包含方法选择。** Author `prepare` 分支只要求 `phase, needs`，可选 `framework, focus, teaching_binding`；`read_method` 是同一个工具的独立 operation，只要求 exact `method_id, revision`（`crates/runtime/src/tool_schema.rs:138–144`）。`prepare` 保存上述当前设计并返回 `status=authoring_context_prepared, phase, needs, changed`，没有推荐方法、待完成读取项或 method 字段（`crates/runtime/src/orchestrator.rs:7987–8001`）。它只能独立调用，下一采样使用指导（`:7946–7947`）。全局阶段末尾的明确动作链是 `prepare(local) → 等下一采样 → write/patch`，未插入 `read_method`（`skills/presentation/phases/global.md:13`）；工程提示同样突出该链（`skills/presentation/engineering.md:3,9`）。

5. **write 无方法引用是合法的。** schema 的 write 必填只有 `title, html, readable_content`，`method_refs` 位于可选字段（`crates/runtime/src/tool_schema.rs:172–187`）；工具自带的新建 write 示例也没有 `method_refs`（`crates/runtime/src/presentation_author.rs:292`）。服务端在 write 处调用 `resolve_method_refs`（`crates/server/src/presentation_author.rs:279–283`）。该函数对新建且未声明引用的 write 生成空数组；显式 `[]` 同样允许（`:534–544`）。只对重写已有非空引用的版本要求显式重新声明，patch 省略则继承（`:536–548`）。

6. **代码强制的是“新引用有观察证据”，不是“每次至少读一个方法”。** 新声明方法会检查 exact id/revision 存在，且当前观察记录中至少有一个同 id/revision、`characters > 0` 的读取；没有则报错，要求 read_method 后经过下一采样（`crates/server/src/presentation_author.rs:550–555`）。检查不要求某一文件、读完全文或同时读 card/example/adaptation。目录与有效正文分别记录：只有目录 asset 进入采样才记 catalog；只有当前 run 的 tool result 实际进入请求且正文非空才记 read（`crates/runtime/src/presentation_methods.rs:66–110`，调用点 `crates/runtime/src/orchestrator.rs:7081–7096`）。因此有真实的引用前置校验，也有独立观察记录；没有“目录已见但没读方法就拒绝 prepare/write”的校验。它也不根据 HTML 的实际设计反推漏报的方法使用。

## 能力搜索与暴露边界

- **没有独立的 `read_method` 能力开关。** 路由能力属于整个 `presentation.author`，名为 `presentation_authoring`（`crates/runtime/src/tool_registry.rs:1096–1099`），发现操作是 explain/compare/summarize（`:1413–1424`）。基础提示明确指导用 `presentation_authoring + explain/compare + read_only` 发现（`crates/runtime/src/agent_prompt.rs:45`）。用 `source_read + read_source` 去找它不会匹配这个路由卡；已经有 Author 时不需要再发现 read_method。
- **整体 Author 仍会受普通暴露条件影响。** 环境不可用会隐藏整个工具（`crates/runtime/src/tool_exposure.rs:535–538`）；未激活或 schema 预算不足也会不暴露（`:585–603`）。暴露时复制整个 spec（`:606–612`），不会只删除 read_method 分支。schema 明列 read_method，描述还说明它无需 presentation reference 或 candidate（`crates/runtime/src/presentation_author.rs:234,271`）。所以这些整体条件不能单独解释“同一次采样能 prepare/write，却不能 read_method”。
- **既有操作权限例外：** 当前采样如果处于已有教学设计的 inspection-only 状态，allowed_operations 只有 read/search（`crates/runtime/src/orchestrator.rs:5751–5762`，执行校验 `:7461–7465`）。此时 prepare/write 也不可用。这是现有 Tutor 边界，未重做其调查。
- **读取结果还有输出预算。** `read_method` 使用 source read 的结果投影预算，可能截断；只有实际进入后续采样的非空正文才算读过（`crates/runtime/src/tool_result.rs:1417–1427`；`crates/runtime/src/presentation_methods.rs:66–90`）。这能解释调用后观察记录缺失，不能解释模型根本没有调用 read_method。

## 事实与推测的分界

**代码能支持的解释：** 当前显式工作流把 prepare/local/write 设为制作主线；注册方法允许零选，且省略 method_refs 时不存在读取阻断。作者即使看到目录，也可以合法地自行设计并继续制作。

**只能作为推测：** 这些提示的相对强调可能使作者把目录看成参考资源，优先沿 prepare/write 主线前进。但仅凭源码不能判断某次作者是否忽略了目录、认为方法不匹配、受上下文负担影响，或实际上采用了某方法却未声明；本调查没有读取运行证据，不能把其中任何一种写成已证实原因。
