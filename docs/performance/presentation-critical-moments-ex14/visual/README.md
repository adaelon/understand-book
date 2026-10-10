# EX14.3：条件性视觉方法指导

2026-10-08，**浏览器/静态路径工程接入完成，C 组 revision 为 ex14.v2**。A19–A21 通过确定性表示样例，A22 通过加载与原生静态修订工程验证，A23 完成按需加载与离线 tokenizer 计量。自然模型选择、N3 行为对照及采用结论留给 EX14.4。提供方调用 0。

## 指导与版本

`phases/local.md` 新增三个短段，按当前障碍选择方法：

- 对象对应：保持名称、形状等身份线索，同步强调对应部分；前后变化可保留旧位置或变换同一对象，先满足数学条件。
- 比较和边界：交代不变条件，使用可比尺度，从同一状态更新图形、数值、标签和单位，必要时用边界或反例限定结论。
- 注意重点与可读性：用位置、对齐和视觉强度突出相关内容，检查 Reader 实际尺寸的中文布局；小修订保持原例子和版式。

[C/presentation](C/presentation/) 保存全部 14 份指导，[manifest](C/manifest.json) 记录版本与相关源码。相对 [B](../checks/B/presentation/)，指导文件差异仅为 local、共同版本标识和 README；共同方法正文、global/review、工程与全部技术参考保持。Manim 参考仍与 A 逐字节一致。

未增加方法资源、PresentationNeed、具体 Manim API 示例或工具 schema。回退 C → B 或 C → A 时整体恢复对应指导与 Runtime revision，保留工程测试和历史成果；正式对照前仍需冻结可运行的共同底座。

## 验收事实

| 验收 | 本轮结果 | 证据 |
|---|---|---|
| A19 对象与公式 | 同一 DOM 圆点持续表示当前位置，前一步以空心方块保留；离散更新与独立闭式公式一致，文本说明变换前提和示意边界 | [relationship.html](relationship.html)、[浏览器记录](browser-1/verification.json) |
| A20 比较与联动 | 固定起点、函数、步号及共同横轴；η=0.5/0.8/1/1.1 × 0–3 步 × 三环境，共 48 组；几何、位置、距离、损失与标签逐项对应 | [observe.mjs](observe.mjs)、浏览器记录 |
| A21 中文与视觉层次 | 三环境无水平溢出、文本裁切或标签/图形重叠；身份同时由文字及形状表达；深色文字与浅色底可读 | 6 张真实 PNG；人工查看 320px 跨越画面与 640px 远离画面 |
| A22 静态小修订 | C local 实际进入脚本化 Resident 后续请求，随后只执行 patch/preview/review/deliver；独立 Server 从冻结 N3 内容导入隔离对象，原生修订并交付 revision 2，旧版和其余内容保持 | [static-revision 请求](run-1/static-revision/)、[原生 N3 回执](n3-1/verification.json)、[交付版本](n3-1/version.json) |
| A23 体积与兼容性 | 原选择器继续按阶段与依赖装配；新 local 全文进入 Native/ReAct 实际消息，global/review 新增指导块不含 local 方法；压缩后 local 全文保持；离线计量见下节 | [verification.json](verification.json)、[实际载荷](run-1/)、[压缩恢复](run-1/local-compaction/messages.json) |

表示样例是手工编写的有限浏览器样本，沿用 N1 的教学模型，仅用于确认这些表示可以正确实现。样例、独立核对公式、参数与选择器没有进入产品指导。人为错位和错标签共 6 次均被核对捕获；实际截图见 [320px 跨越](browser-1/320-eta-0.8-step-1.png) 与 [640px 远离](browser-1/640-eta-1.1-step-3.png)。这些定向浏览器操作不产生 Author 三环境资格。

N3 的原生测试使用冻结 revision 1 的完整 content，导入隔离测试所有者后取得新的本地对象 ID；来源对象与导入对象映射保存在回执。只替换一处 HTML 措辞及对应 readable_content，当前候选真实取得 320×420 touch、640×240 touch、960×720 mouse 回执后交付；逐字段比较确认例子、样式、状态、资源与其他字段保持。脚本化 Resident 验证加载和操作路径，Server 验证实际保存与交付；两者分别留档，不冒充一次自然模型运行。

## 新增输入开销

使用 DeepSeek 官方 [DeepSeek-V4-Flash-0731 tokenizer](https://huggingface.co/deepseek-ai/DeepSeek-V4-Flash-0731/tree/7872f01b1d1fe23eabc4c98b48bffcef5a386062)，固定该仓库版本并保存原始 tokenizer、配置及许可证；来源见 [source.json](tokenizer/source.json)。本地 `tokenizers 0.23.2` 对实际文本执行 `encode(add_special_tokens=False)`。

local 正文：B **344 token** → C **572 token**，增加 **228 token**；另记 UTF-8 增量 1085 字节，字节不作为 token。下表为实际采样追加的指导事件，含相应包装和选中参考。

| 当前选择 | B 事件 token | C 事件 token | 增量 |
|---|---:|---:|---:|
| global | 349 | 349 | 0 |
| static local | 407 | 635 | 228 |
| local + continuous_scene | 1355 | 1583 | 228 |
| review + continuous_scene | 1240 | 1240 | 0 |
| local + Manim | 2267 | 2495 | 228 |
| 返回 global | 349 | 349 | 0 |

历史指导仍保留原追加位置；重复 prepare 没有新事件。在本次选择序列中，三次不同 local 选择累计增加 684 token，返回 global 的当前块没有新增方法，但累计历史仍保留增量。压缩后仅重建当前选择，已保存 local 与 review 两条路径的真实消息。

这里是所固定公开 tokenizer 的离线文本计量，未包含提供方消息封装、图片 token、缓存计费或真实 API usage。EX14.4 批次的模型配置尚待批准，启动时须核对最终模型/tokenizer；不得把本轮计量写成已发生费用或节省。

## 测试与重跑

| 执行 | 结果 | 记录 |
|---|---|---|
| Runtime EX14 注册 | 5 项列出，新增静态修订测试挂入原模块 | [registration.log](registration.log) |
| Runtime presentation | 88 passed，0 failed，0 ignored | [runtime-run-1.log](runtime-run-1.log) |
| 追加 local 全文压缩断言 | 1 passed，0 failed | [local-compaction-2.log](local-compaction-2.log) |
| Server N3 注册与显式真实浏览器执行 | 1 passed，0 failed，0 ignored | [server-registration.log](server-registration.log)、[server-n3-1.log](server-n3-1.log) |
| 三环境表示样例 | 48 组、6 次负向故障识别通过 | [browser-1/verification.json](browser-1/verification.json) |
| C 全文、载荷、schema、token 与交付内容核对 | passed；23 次计划、46 份 Native/ReAct 请求 | [verification.json](verification.json) |

运行目的：请求回归发现阶段串位、全文丢失或重复注入则修装配；N3 发现修订范围、旧版本或交付资格变化则修对应存储/交付路径；样例发现图文和公式不一致则修样例；归档核对发现正文、源码或版本错配则修归档后才进入后续对照。

```powershell
cargo test -p runtime --lib ex14_ -- --list
$env:EX14_GUIDANCE_OUTPUT = '<new absolute request directory>'
cargo test -p runtime --lib presentation -- --nocapture --test-threads=1
cargo test -p server --lib ex14_visual_ -- --list
$env:EX14_VISUAL_N3_OUTPUT = '<new absolute N3 directory>'
cargo test -p server --lib ex14_visual_static_revision_previews_and_delivers -- --ignored --nocapture --test-threads=1
node docs/performance/presentation-critical-moments-ex14/visual/observe.mjs '<new browser directory>'
python -m pip install --target tmp/ex14-tokenizer-runtime tokenizers==0.23.2
python docs/performance/presentation-critical-moments-ex14/visual/verify.py
```

录制入口拒绝覆盖旧证据；`verify.py` 核对本轮冻结目录，新批次须另存并修改其核对入口。新增压缩全文断言首轮将原始多行指导与 JSON 序列化文本直接比较，导致测试误报；改为从实际消息 content 核对后通过，产品装配未改，失败见 [local-compaction.log](local-compaction.log)。编译存在既有 ts-rs serde 属性告警。

## 已知限制与接续

EX14.1-M 未执行，Manim 指导增补及其固定 runner API 兼容样例继续待办。A22 的自然 N3 负向对照、指导表示收益、正式采用与学习效果均未评估；付费实验仍 `blocked: paid experiment not authorized`，EX13 的历史额度不沿用。

未新增 Linux 隔离 worker、实体手机或 Reader 连续使用验收；EX12.5 历史状态见 [Linux 发布记录](../../../Linux上线-EX12-EX13-JL.md)。本轮未提交、未部署。下一入口为 EX14.4 的费用/执行配置及共同底座冻结，或独立 EX14.1-M；EX14.C 保持条件准入。主视频 checkpoint 保持，接续信息写入 [checkpoint_ex.md](../../../../checkpoint_ex.md)。
