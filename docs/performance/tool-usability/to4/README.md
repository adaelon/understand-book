# TO4：Presentation 操作示例

日期：2026-10-12。基线：`93b476d` 加主工作区 TO3、TO7–TO12。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 冷启动入口、TO4、固定版本 R3 和 TO/TB 所有权。
- [x] 保存改前实际工具定义、固定输入与失败证据。
- [x] 在 `presentation_author::spec` 发布九种 operation 的完整输入示例，验证最终 schema、解析及 Provider 投影。
- [x] 复用 authoring fixture，执行真实引用、来源／资产、修改、浏览器预览与交付链路。
- [x] 更新验证记录、方案、代码链路和 checkpoint。

## 范围

沿既有 AgentPresentation、制作阶段、来源引用和候选生命周期补操作示例，属于纯技术扩展。TO 所有权为工具定义与调用合法性；TB 的教学指导沿原分工维护。

## 验证计划

最终工具定义与真实 parser 检测引用／candidate 混用、operation 必填字段和预览互斥字段；失败修示例或相应原定义。真实 authoring 存储与浏览器检查修改发生在正确版本、候选不可提前交付、资源与来源保持、回执指向实际交付版本；失败修具体不一致的调用或描述。确定性结果不推断真实模型收益。

## 实现

`presentation_author::spec` 在原所有者中发布 14 个完整输入，继续由 `tool_schema::complete_spec` 施加 operation 必填、互斥与 nullable 约束。

| 操作 | 示例内容 |
|---|---|
| prepare | 当前局部目标、整体设计与 editing／state 需要；下一采样生效 |
| render_plot／render_animation | 实际 Python 入口、JSON 数据、画幅及动画 cue |
| read／search／patch | 每种各有已交付 reference 和本轮 candidate_id 两种输入；read 可单读 readable_content，patch 使用唯一精确匹配 |
| write | 显式 new_object 新建与 based_on 修改；完整 HTML／readable_content，正文 source_ref_ids 和图形 asset_refs 明确附加 |
| preview | width 与 viewport 分别示例；点击按钮、滚动并读取可见结果 |
| deliver | 使用最后候选 ID；实际完成状态及交付对象来自返回回执 |

示例的 ID 是标明来源的占位值，调用时替换整个实际 reference、当前候选和真实资产／来源引用。工具描述说明修改顺序、候选换代、预览后采样观察及显式 viewport 的环境完成条件。

原来源错误恢复提示曾建议 `patch.source_ref_ids`，该字段不在 parser 和最终 schema 中。现改为使用 `write.source_ref_ids`；patch 继续保留原来源，新增来源仍须来自实际观察或已有版本。错误复现见 [initial-tests.log](initial-tests.log)，缺少示例的改前失败见 [red.log](red.log)。

真实窄屏预览发现原 authoring fixture 的普通按钮只有约 41px 高，未达到项目触屏目标要求。两个 write 示例复用原 fixture 内容及交互，并补充按钮 `min-width/min-height:44px`；浏览器校验和对象生命周期保持原实现。原失败见 [tests.log](tests.log)。

## 验证结果

Runtime **50 项通过**，Server **5 项累计通过**。最终 Runtime 与修正后的浏览器链路见 [final-tests.log](final-tests.log)；其余四项 Server 用例（真实动画、来源错误恢复、补丁原子性、旧候选预览隔离）在 [tests.log](tests.log) 中通过，HTML 尺寸修正后沿用这些结果。

- 14 个发布示例全部经过最终 schema、registry 验证及 `AuthorRequest` 真实解析；Native／ReAct 保留相同示例。`prepare` 经真实 Runtime 循环，在下一采样更新设计与指导，没有创建或交付候选。
- [真实 authoring 链路](chain.json) 复用 `presentation-author.html`，完成新建 → width 预览 → 交付 → based_on 改写并绑定真实 Matplotlib 资产和规范书源 → read/search → reference patch → candidate read/search/patch → 三环境 viewport 预览 → 交付。存储中原版本逐值不变，最终回执指向同一 presentation 的下一 revision。
- 清空本轮图形缓存后，based_on 示例仍能从已交付版本保留资源与来源；patch 的完整内容比较只允许指定按钮文案变化。两次提前 deliver 均被拒绝，旧候选预览不能授权新候选。
- 三环境为 320×420 touch、640×240 touch、960×720 mouse；点击后实际页面读数为 `1`，截图绑定正确 candidate/environment。最终环境截图见 [preview-2.png](preview-2.png)。
- [动画调用](animation.json) 执行真实 Manim，得到 640×360、至少 1 秒的影片及解码帧；Matplotlib 产物随版本保存 SVG、代码和数据。
- 独立 Draft 7 [验证结果](validation.json)：14 个示例、19 个成功 Host 调用输入通过，11 个无效组合被拒绝；去除新增 examples 后，输入 schema 与改前逐值一致。

工具定义序列化体积从 **11,575 → 15,520 字节**（+3,945）；其中 parameters 从 **10,282 → 13,706 字节**（+3,424）。[改前](before.json) 与 [改后](after.json) 保存实际工具定义及 Native／ReAct 请求。字节数不是 billed token 或效果收益。

## 复现

在项目根目录使用既有 Rust 构建目录和本地渲染环境：

```powershell
$env:CARGO_TARGET_DIR='D:/codex-build/understand-book-no-progress-195'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
$env:TO4_DIR=(Resolve-Path 'docs/performance/tool-usability/to4').Path
$env:TO4_SPEC_SNAPSHOT=Join-Path $env:TO4_DIR 'after.json'
$env:UNDERSTAND_BOOK_ANIMATION_PYTHON=(Resolve-Path 'tmp/ex9a-manim-venv/Scripts/python.exe').Path
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime -p server --lib -- presentation_to4 --include-ignored --test-threads=1
python docs/performance/tool-usability/to4/validate.py
```

浏览器沿项目 `BrowserPreview::discover` 查找已安装 Edge／Chromium；Matplotlib 沿原 `UNDERSTAND_BOOK_PLOT_PYTHON` 或 `python` 配置。真实预览与动画测试默认忽略，显式 `--include-ignored` 才执行。

相关回归命令（沿用上面的构建环境）：

```text
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib -- tool_registry::contract_tests presentation_author::tests presentation_authoring presentation_author_requires_next_sampling_and_attaches_saved_reference presentation_plot_image_reaches_next_sampling_before_asset_write presentation_preview_at_turn_limit_gets_one_deliver_only_sampling --test-threads=1
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib -- ex12_source_list_error_explains_recovery_without_granting_sources presentation_edit_search_then_patch_preserves_original_and_requires_new_preview presentation_edit_invalid_patches_are_atomic_and_search_is_pageable --test-threads=1
```

## 已知限制

- 本片使用确定性调用、真实私有存储、浏览器和媒体渲染验证。没有真实模型采样、任务完成率提升结论或生产部署；后续实验沿 [模型约定](../to10/README.md#实验模型约定)。
- 浏览器链路使用 Host authoring 分派；Runtime 的后续采样观察与终答引用约束由原编排回归测试验证。完整真实模型行为由 TO14 验收。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R3 `toolContractHelpers.ts`、`toolContractExamples.ts`、`advertisedInputExamples.test.ts`：示例由输入解析验证、发布简短引用并去重。入口见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。
