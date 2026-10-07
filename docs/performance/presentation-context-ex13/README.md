# EX13.0 离线请求基线

2026-10-02。**EX13.0 已完成**：现有录制可以重复生成 [baseline.json](baseline.json)，并用 [compare.py](compare.py) 比较后续录制。8 项离线测试通过。本片新增模型调用为 0。实施合同见 [EX13 方案](../../切片方案-EX13-Goal工作计划与演示修订上下文.md#6-实施切片与依赖)。

## 重放与比较

在仓库根目录执行；只需 Python 标准库：

```powershell
python docs/performance/presentation-context-ex13/compare.py baseline --output docs/performance/presentation-context-ex13/baseline.json
python -m unittest discover -s docs/performance/presentation-context-ex13 -p test_compare.py -v
```

比较任意两份同类型录制，不指定输出文件时直接打印 JSON：

```powershell
python docs/performance/presentation-context-ex13/compare.py compare docs/performance/presentation-staged-authoring-ex12-4/mechanism-staged-catalog/request-12.json docs/performance/presentation-staged-authoring-ex12-4/mechanism-staged-catalog/request-13.json
```

需要附带真实 usage 时传入 `--left-response <response.json>`、`--right-response <response.json>`；没有传入的那侧为 `usage: null`。`compare` 同时支持两份 `completion-*-request.json`，用于摘要初次请求与修复请求。`--output <file.json>` 可保存比较结果。`baseline --source-root <EX12.4目录>` 可重放移至其他位置的原始材料。

`baseline` 固定验收当前 EX12 录制；后续 EX13 改动使用 `compare` 比较新旧请求，并另存结果。EX13.1 已完成，[受控录制、Native/ReAct 比较及重放入口](ex13-1/README.md)单独保存。脚本只读引用原始材料，输出写到指定文件。

## 样本与记账口径

输入来自 [mechanism-staged-catalog](../presentation-staged-authoring-ex12-4/mechanism-staged-catalog/) 的 request/response **12、13、28、29、30**；write 证据另读 response-13/request-14、response-18/request-19 及对应已保存候选。摘要使用 [mechanism-staged-feedback5](../presentation-staged-authoring-ex12-4/mechanism-staged-feedback5/) 的两组 completion 及实际恢复采样。纠错入口引用 [local-correction/patch.json](../presentation-staged-authoring-ex12-4/local-correction/patch.json)、origin 与既有 verification。

录制层是 `RecordingAdapter` 保存的 `AgentRequestPlan` 字段，测量按以下规则进行：

- **有序内容比较**：依次比较 instructions → tools → messages → images；摘要比较 system → user。保留数组与字符串顺序，逐内容给出第一个不同字段及字符串内的零基字符/UTF-8 字节偏移。对象键顺序不作为语义变化。
- **序列化体积**：上述字段采用紧凑 JSON、`ensure_ascii=False`、对象键排序、UTF-8；不计录制 elapsed_ms、instruction_assets、runtime_profile 等旁路元数据。它是固定口径的录制内容体积。
- **分类闭合**：instructions 按录制内 `Presentation phase:` 至下一项宿主 policy 的边界分出 phase/needs，冻结 EX12 样本的下一项为 `Source presentation:`；没有该工具的受控样本使用 `Capability discovery:` 或 `Navigation and guided reading:`。EX13.1 的 `sampling_guidance.v1` 后部消息单列 phase/needs，当前 `presentation_guidance_state.v1` 归动态状态；动态快照与 dynamic fragment、assistant 参数、tool 正文、其他历史分别统计转义后的字符串内容。工具定义、图片引用、协议续接字段单列，JSON 键名、引号和结构符号归入 serialization_overhead。各项之和等于总量。
- **原文体积**：`writes[].html` 与摘要 system/user 同时给出未转义原文的 Unicode 字符数和 UTF-8 字节数；字符不是 token。协议续接只统计体积，不在报告中输出其正文。
- **真实消耗**：`usage` 原样取自录制响应；reasoning 是 output 的子集。缺失 usage 保持 null。

五个请求均为 `resident-agent-deepseek-flash-v1` / `deepseek-v4-flash` / `CatalogMatch`。现有制作路径将 admission 的输出预留 **8,000 → 131,072**、压缩水位比例 **0.096 → 0.219072** 同步提高；脚本按这一已实现规则逐字段比较，其余档案字段必须一致。五个请求的冻结上下文、原用户请求、指导 `ex12.v5`、phase/needs 和相邻 response→assistant→tool 配对均有断言。

## 已保存的请求基线

| request | phase / needs | 序列化字符 | UTF-8 字节 | 真实输入 token | 真实缓存输入 token |
| --- | --- | ---: | ---: | ---: | ---: |
| 12 | global / 空 | 223,999 | 275,812 | 70,791 | 68,992 |
| 13 | local / state | 232,778 | 289,740 | 74,406 | 9,984 |
| 28 | local / state | 463,814 | 558,006 | 149,450 | 146,304 |
| 29 | review / 空 | 466,132 | 561,174 | 150,402 | 9,472 |
| 30 | 交付后默认 global / 空 | 468,828 | 563,562 | 150,769 | 24,960 |

下表为各组成部分的 **UTF-8 序列化字节**；字符统计及历史调用类型在 JSON 中：

| 组成 | 12 | 13 | 28 | 29 | 30 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 固定 instructions | 26,111 | 26,111 | 26,111 | 26,111 | 26,111 |
| phase / needs 指导 | 1,204 | 3,255 | 3,255 | 588 | 1,204 |
| 动态状态 | 25,969 | 31,059 | 62,709 | 64,306 | 63,760 |
| assistant 调用参数 | 11,896 | 14,702 | 104,893 | 107,186 | 107,269 |
| tool 结果 | 63,727 | 64,127 | 81,861 | 82,253 | 82,788 |
| 图片引用 | 2 | 2 | 2 | 2 | 2 |
| 其他历史 | 30,972 | 33,816 | 33,816 | 33,816 | 33,816 |
| provider_continuation | 88,963 | 89,293 | 211,738 | 212,947 | 214,368 |
| 工具定义 | 15,754 | 15,754 | 15,754 | 15,754 | 15,754 |
| JSON 结构开销 | 11,214 | 11,621 | 17,867 | 18,211 | 18,490 |

五个采样均无图片，图片引用的 2 字节为 `[]`；整个运行其他采样包含实际图片。所选请求的 10 个工具及完整 schema、固定指导内容均相同。

## 前缀变化与源码落点

| 比较 | 首个差异 | 录制中的变化 |
| --- | --- | --- |
| 12 → 13 | instructions 字符 10,846 / 字节 11,987 | response-12 prepare(local, needs=[state])，下一次采样替换阶段并增加状态技术指导。 |
| 13 → 28 | messages[18].content 字符/字节 67 | 跨 15 次采样的区间；instructions 与工具完全相同，9 个旧工具结果的 model_body 已移除。 |
| 28 → 29 | instructions 字符 10,846 / 字节 11,987 | response-28 prepare(review, needs=[])；当前 authoring context 随新阶段替换。 |
| 29 → 30 | instructions 字符 10,846 / 字节 11,987 | response-29 deliver；当前 authoring context 移除，指导回到默认 global。 |

三个相邻切换对中未观察到旧调用参数或旧工具结果改写。13 → 28 单独标记 `sampling_gap: 15`，只定位该区间内的结果裁剪，不把它归为某一个中间采样的事件；每个变更回执的 call_id 与体积在 `transitions` 中可查。

两次 write 均有配对的成功 `candidate_saved` 回执，脚本直接比较候选 `index.html` 与原 write HTML：

| write 响应 / 回执所在请求 | HTML 字符 | HTML UTF-8 字节 | 保存候选 |
| --- | ---: | ---: | --- |
| response-13 / request-14 | 18,486 | 21,582 | candidate-1790920374957418000-1 |
| response-18 / request-19 | 31,850 | 40,890 | candidate-1790920503082827200-5 |

两份共 **50,336 字符 / 62,472 字节**的 HTML，在 request-28、29、30 中都仍以原 write 参数携带。EX13.2 可据此比较成功保存后的源码投影；当前基线仍保留原样。

## feedback5 摘要与纠错入口

两次摘要 user 完全一致，包含 237 个 eligible source、84 个 required source，原文为 **131,674 字符 / 152,535 字节**。初次 system 为 4,268 字符，修复增加到 4,669；首个差异位于 system 字符/字节 **4,268**，处于长 user 材料之前。

| 摘要请求 | 录制内容序列化字节 | 真实输入 token | 缓存输入 token | 输出 token | 其中 reasoning |
| --- | ---: | ---: | ---: | ---: | ---: |
| completion-00 | 181,737 | 54,780 | 54,528 | 37,269 | 19,651 |
| completion-01 修复 | 182,141 | 54,861 | 768 | 47,810 | 24,791 |

两次输出上限均为 65,536、reasoning_effort=low。原诊断为 `item.fact.quote_receipts invented or duplicated a source ID`。直接比对输入后，具体错误是该项引用了不存在的 `source.186.fnv1a64-d42637f088cc882f`，正确输入为 `source.186.fnv1a64-d42637f089cc882f`；本项没有重复 ID。此前文字报告称为“重复 source ID”，本片按原始录制更正。脚本核对修复的 source_coverage 与输入一致，且修复结果完整出现在 request-00 的已安装 checkpoint 中；不另造 Runtime 的语义验收器。

局部补丁仍指向 **presentation-1790920818511542500-6 revision 1**，含 11 项替换与 5,209 字符的 readable_content，完整补丁为 13,043 个序列化字节。脚本只核对它与 origin、feedback5 continuation 的版本关联及既有 `local_verified_not_native_delivered` 状态；三环境内容验收沿用原 [verification](../presentation-staged-authoring-ex12-4/local-correction/verification.json)。

## 验证与已知限制

[test_compare.py](test_compare.py) 的 8 项测试覆盖实际录制重放与统计一致性、phase 前缀与源码保留、摘要拒绝及修复安装、错误 profile/上下文/phase、响应错配和缺回执、有序内容/字符与字节偏移/缺 usage、裁剪报告、跨工作目录 CLI；全部通过。

本片记录的体积不是最终 HTTP 请求大小，图片只有引用元数据，provider_continuation 在适配时也可能转换。因此有序内容的差异位置不能直接当成提供方缓存块位置。真实 usage 是既有调用结果，不能仅由这些样本将全部未命中归因于阶段切换；本片不产生新的缓存收益或费用估计。

EX13.1 已实现并通过[受控验证](ex13-1/README.md)，比较脚本新增 2 项用例后共 10 项通过；原冻结统计不变。EX13.2 已完成[保存后源码投影](ex13-2/README.md)：原两份 HTML 的受控末次请求减少 75,803 字节，Runtime 443 项、Server 编辑/存储 4 项、专属录制测试 1 项通过。EX13.3 已完成 [Goal 工作计划](ex13-3/README.md)的原子更新、日志重放、同聊天接续与压缩投影，Runtime 447 项、Server 相关 10 项、Web 27 项及类型检查通过。EX13.4 已完成[确切版本修订](ex13-4/README.md)，EX13.5 已完成[摘要请求与修复前缀](ex13-5/README.md)，Runtime 453 项及 Native/ReAct 本地 HTTP 验证通过。EX13.6 待明确真实调用范围和费用边界。EX12.4 最终原生预览、图片检查与交付仍受 402 余额不足阻塞；EX12.5 尚未开始。
