# EX13.6 有限真实接续

2026-10-02。**原生交付与内容验收完成，EX13.0–EX13.6 收口，EX12.4 最终验收完成。** 在用户授权的原 DeepSeek 配置和 9 元人民币总限额内完成；成功后已停止模型调用。Linux Reader 连续使用属于 EX12.5，尚未开始。

## 交付与验收

`presentation-1790920818511542500-6` **revision 1 → revision 2**，保留同一对象、27 个来源绑定、十站完整范围、资源和参数。实际交付见 [version.json](continuation-4/native/version.json)，验收见 [verification.json](continuation-4/verification.json)。

从 feedback5 的原聊天、未完成 Goal 和已安装检查点恢复，读取 feedback3 的确切版本；保存现场来自该版本已录制的 `960/default_calculator` 浏览器观察，经原保存和 follow-up admission 路径装入。当前运行重新发现工具、读取正文和源码、更新四项工作计划、prepare(local)、patch、原生 preview、后续模型图片观察、prepare(review)、deliver，最后更新 Goal。原要求完整保留，原候选资格未复用。

原 `local-correction/patch.json` 的 11 项替换原样保留。验收准备发现一个额外可达错误：读取量减半后，静态推导仍写 `494.7 ÷ (2 × 2.345)`，动态结果却变成 52.7。新增 [supplemental-edit.json](supplemental-edit.json) 改为实际读取量通式，并分开原条件与减半条件；本次原生 patch 共 12 项。旧证据没有回写。

独立浏览器重放三环境、每环境七条操作路径：默认值、B=8、权重减半、练习原值/减半、控件聚焦、四档实测汇算。三个环境共 78 张沿页与操作截图，无脚本错误、无水平溢出。实际交付内容与“原修正内容 + 补充公式”逐字段一致；27 个来源绑定和 Goal requirements 保持，四项工作已完成。

- [320×420 控件聚焦](continuation-4/native/independent/320-24.png)：标签、滑块、就地读数可见，总览不遮挡。
- [640×240 控件聚焦](continuation-4/native/independent/640-24.png)：控件与结果同屏可读。
- [四档实测表](continuation-4/native/independent/960-25.png)：原始值、倍数和增幅齐全。
- [观察记录](continuation-4/native/independent/observations.json)：绑定实际 revision 2；独立观察与原生交付资格分别保留。

## 实验入口与恢复修复

`run.py --binary <server test exe> --run-dir <new directory>` 启动现有 `prepare_agent_chat → execute_prepared` 原循环。目录已存在时拒绝覆盖；遇到明确的准备失败后，`--prior-run` 承接此前累计费用和 HTTP 次数。本次仅完成一个冻结输入的接续，没有重新跑对照矩阵或追加机制样本。

原 native、`deepseek-v4-flash`、CatalogMatch、压缩水位、演示输出 131,072 和摘要输出 65,536 保持；摘要使用当前 EX13.5 提示。提供方实际响应模型为 `deepseek-flash`。原来不指定输出上限的工具发现/辅助判断，在本次实验请求上限为 16,384。

费用入口在每次实际 HTTP 请求前，按整个 1M 上下文（向上取 1,048,576）全未命中输入和请求最大输出预留高峰价费用。收到实际 usage 后结算；上游无自动重试，错误或缺失 usage 即停止并保留全额预留。累计最多 32 次 HTTP 请求，累计预算 9 元；运行中观察到的传输尝试均经过此入口。

| 目录 | 结果 | 提供方调用 |
| --- | --- | ---: |
| continuation-1 | 本地拒绝无上限的辅助请求，补齐实验输出限制 | 0 |
| continuation-2 | 辅助判断后发现旧检查点来源身份被历史裁剪改变 | 1 |
| continuation-3 | 恢复通过；本地拒绝无上限的首个工具发现请求，统一补齐线路上限 | 1 |
| continuation-4 | 原生接续与交付成功，379.231 秒 | 20 |

**生产修复**：EX13.4 的 `redact_history` 无条件添加 `new_object:null`，使旧消息字节与已安装检查点的 source ID 不一致。现仅在原调用存在该字段时保留它；原有明确新建语义保持。字节回归先红后绿，原 feedback5 检查点在当前历史裁剪后通过校验。没有清洗摘要或重造来源。

## 用量与费用

成功接续发生一次自然自动压缩，第一次输出即通过严格来源覆盖校验，未触发修复。真实阶段包含 local 和 review；review 后采样缓存输入 132,864 / 136,365，约 97.43%。该样本同时发生工具激活、压缩安装、图片及结束时工具集合变化，不能单凭一个比例归因。

| 成功接续类别 | 调用 | 输入 | 缓存输入 | 未缓存输入 | 输出 | 其中 reasoning | 空闲价估算/元 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 主线 | 18 | 1,762,154 | 1,260,928 | 501,226 | 51,430 | 40,111 | 0.73216456 |
| 摘要 | 1 | 55,513 | 0 | 55,513 | 35,657 | 18,379 | 0.19814100 |
| 辅助判断 | 1 | 5,060 | 4,864 | 196 | 292 | 249 | 0.00146128 |

主线缓存输入占 71.56%。计入两次准备失败的辅助判断，**新增提供方调用共 22 次**：输入 1,832,847、缓存输入 1,270,784、输出 88,009（含 reasoning 59,283），输入加输出 1,920,856 token。没有重复计入 reasoning。

价格来源：[DeepSeek 官方价格](https://api-docs.deepseek.com/zh-cn/quick_start/pricing/)，2026-10-02 核对；每百万 token 空闲 cached/uncached/output 为 0.02/1/4 元，高峰为 0.04/2/8 元。本次成功运行北京时间 22:25:56–22:32:16，按空闲价估算，**累计 0.93951468 元；用于限额的高峰价保守累计为 1.87902936 元**。首末余额 9.11→8.52 元，观察净减 0.59 元。余额读数与 usage 估算分别记录，未取得逐笔账单，不将两者等同。

[累计统计](cumulative-metrics.json)、[逐请求与操作](continuation-4/metrics.json)、[费用限制记录](continuation-4/wire/budget.json)、[原生结果](continuation-4/native/outcome.json)可重放核对。

## 验证入口

- `python -m unittest discover -s docs/performance/presentation-context-ex13/ex13-6 -p test_run.py -v`：3 项通过；本地 HTTP 验证发送前拒绝、缺省输出限制、usage 替换而非累加、跨尝试累计、缺 usage/失败停机。
- `cargo test -p runtime --lib`：454 通过、0 失败、3 忽略；日志 `tmp/ex13-6-runtime.log`，失败复现 `tmp/ex13-6-red.log`。
- Server 原始存档接续用例：1 通过；checkpoint 相关：6 通过、1 忽略。日志 `tmp/ex13-6-admission.log`、`tmp/ex13-6-checkpoints.log`。
- 真实运行：`presentation_author_tests::ex12::ex13_live::ex13_live_continuation` 通过；请求、响应、图片、实际 HTTP 请求与 usage 保存于 continuation-4。
- 独立页面重放使用 EX12.4 的 `observe.mjs` 与原 `local-correction/actions.json`，再运行 `verify.py <continuation-4>`；3 环境 × 7 路径通过。
- `summarize.py <continuation-4> --cumulative` 重算统计；本地拒绝的请求不计为提供方调用。

## 已知限制

本次证明选定内容的原生修订、恢复和交付；不是新旧版本的同输入性能对照，不承诺一般性费用下降。自动压缩一次通过，本次没有真实摘要修复样本，修复前缀结论沿用 EX13.5 的确定性验证。

原模型收尾文字将一处练习 1-3 的计算误称为“练习 1-4 卡片”，且对公式替换范围概括过宽；原始回答保留。交付页面及其确切补丁已独立验证，公式修订对象为练习 1-3 和共享计算函数。

未提交、未部署；EX12.5 Linux Reader 连续使用、实体手机和学习效果仍未验收。主 SESSION_CHECKPOINT.md 属于其他工作，本片更新 checkpoint_ex.md。
