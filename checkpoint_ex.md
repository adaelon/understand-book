# checkpoint_ex — 2026-10-08 21:39（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：`4e0b68e feat: integrate book structure, tutoring and multi-user reader`。
- 读入时对比 `git log -3`；工作树有其他任务的修改与未跟踪文件，HEAD 不能独立复原在途成果。
- 工作树未提交；2026-10-08 ADM10 同版发布已部署 EX14.0–EX14.3 / ex14.v4 / Manim，release `adm10-20261008`。EX14.4/5仍待验。根 `SESSION_CHECKPOINT.md` 继续承接视频任务。

## 当前在做什么
**EX14.2 / EX14.3 Manim 增补已完成；当前指导 ex14.v4，下一入口为 EX14.4 前置配置。**

用户明确本刀完成 Manim 检查指导、固定 runner 兼容方法、新媒体 B/C 全文及实际请求/计量。沿用 ADR-0157，无新增领域术语、架构、工具 schema 或媒体上限。

- 入口：`docs/performance/presentation-critical-moments-ex14/media-guidance/README.md`；总核对 `verification.json`，图像观察 `visual-review.json`。
- 正式媒体 B：`media-guidance/B-final/`，ex14.v3，历史 B＋Manim 检查；14 次请求计划、28 份 Native/ReAct，`B-run-3/`。`B/` 是测试修订前快照。
- 正式媒体 C：`media-guidance/C/`，ex14.v4，媒体 B＋既有 local 方法＋实测媒体方法；28 次计划、56 份 Native/ReAct，`C-run-1/`；压缩消息另存。
- 原 A/B/C 快照 `baseline/source/skills/presentation`、`checks/B`、`visual/C` 保持。新 B/C 都保存 14 份指导及相关源码；当前工作树为 C。
- 三个固定 runner 场景在 `compat-4/`：Transform 对象、TransformMatchingTex 等式两侧加 d、ValueTracker/DecimalNumber 联动与中文 Text＋MathTex。均 3 秒、640×360、30fps、无音频；270 帧独立解码通过。
- `layout-2/`：320×420、640×240、960×720 共九组真实解码图布局；已查看三组 320px 前/中/后画面。
- Runtime presentation **89 passed，0 failed，0 ignored**；B 另三项定向加载通过。完整 Manim 参考仅在所选 local/review 当前块进入实际请求；重复 prepare 不重复追加，压缩后保留一次。
- 固定官方 DeepSeek-V4-Flash-0731 tokenizer：Manim 正文 892→1117→1514 token；检查 +225，媒体方法再 +397。媒体 C 相对历史 C 的 Manim local 当前块 +622，未选 Manim 的本次媒体增量为 0；离线计量不等于 API usage/费用。
- 原媒体指导完整保留为前缀；生产 renderer/preview/author 与 EX14 基线逐字节一致。EX14.1-M 既有定位、解码、末帧、小数恢复证据保持有效，本轮未重复整套媒体矩阵。

## 下一步（可直接接手）
1. 按下方读序读取媒体指导报告、B-final/C manifest、verification 与 EX14 §8–9，确认本批工程结果和付费自然对照边界。
2. 进入 EX14.4 前，依据 `experiment.json:budget/shared_conditions/adoption` 向用户取得总额、单任务费用、模型/输出配置、停止条件与成本接受界限；EX13 的 9 元额度不沿用。
3. 获得配置后新建执行批次，冻结当时可运行的共同底座与本媒体 A/B/C；保留旧批次，不改写现有请求与成品。
4. 按 EX14 §8 对 N1/N2/N3 做独立自然请求对照，完整记录实际加载、未检验、失败及收益；采用结论后按 EX14.5 的相应路径验 Reader 连续使用。
5. 每刀同步 EX14 报告、方案状态、代码链路和本 checkpoint；EX14.C 仍按两任务证据条件另行准入。

## 未提交 / 未完成
- 本轮修改 Manim 参考、revision、指导 README、两个 Runtime 测试文件；新增 `media-guidance/` 兼容脚本、样例、B/C 全文与原始证据；ADR、方案、索引、experiment、代码链路和本 checkpoint 待提交。
- 原有 Runtime/Server/ADM/UI/视频与 EX 工作保持；提交只选本任务范围，不整体提交工作区。
- EX14.4/5 尚未执行；付费实验仍 `blocked: paid experiment not authorized`；没有自然模型收益、正式采用或学习效果结论。
- `compat-1` 捕获 Cairo 间接 updater 使实际画面静止；修为显示 VGroup 拥有 updater。`compat-2/3` 的首帧布局和误读路径已修，失败媒体及 B 两次夹具断言失败原始日志保留。
- 中文/公式兼容仅覆盖当前 Windows 字体/TeX；Linux 隔离 worker、实体手机、完整 Reader 连续使用仍需对应验收。EX14.1-B 圆盘 r₂=1.5 仍为 Author preview 未覆盖，独立补证不替代 Agent 观察。

## 冷启动读序
1. `docs/performance/presentation-critical-moments-ex14/media-guidance/README.md`、`verification.json`、`visual-review.json`、`B-final/manifest.json`、`C/manifest.json`：最新工程结果、输入开销、失败与限制。
2. `docs/adr/0157-obstacle-driven-visual-methods-and-critical-moment-review.md`；`docs/切片方案-EX14-视觉方法指导与关键解释时刻检查.md` §0–3、§6–9、§11 A13–A32。
3. `skills/presentation/references/manim.md`、`continuous-scene.md`、`skills/presentation/README.md`；Runtime `agent_prompt/presentation.rs`、`presentation_ex14_guidance_tests.rs` 及 `presentation_authoring_tests.rs:check_framework_survives_mid_turn_compaction`。
4. `docs/performance/presentation-critical-moments-ex14/experiment.json`、总 README；`media/README.md` 与 `media/verification.json`；`docs/代码链路.md` 的 EX14.2/3 Manim 增补条目。
5. `docs/Linux上线-EX12-EX13-JL.md` 的交付与限制；EX13 历史见 `docs/performance/presentation-context-ex13/ex13-6/README.md`，部署以较后的 Linux 记录为准。

## EX14.1-M 追溯
- 最终 `media/run-6/`、`browser-8/`、`verification.json`；候选 `candidate-1791463203478744400-1`。A04–A10/A12 通过，240 解码帧中 12 个故障帧被四帧初筛遗漏。
- 原生 18 次尝试、17 完成/1 拒绝、78 张页面 PNG；独立浏览器六组通过。产品四帧/四动作上限保持，提供方调用 0。

## EX12.5 历史接续入口
- EX13.0–6、EX12.4 原生交付与内容验收完成；2026-10-02 release `ex13-jl-20261002` 已部署 EX12/EX13/JL。
- 成果 `presentation-1790920818511542500-6` revision 2，27 来源、十站；见 EX13.6 `continuation-4/native/version.json` 与 `verification.json`。
- Linux 长页、保存重开、回顾、账号隔离、备份恢复和真实 B=8 现场追问已覆盖；线上同对象再修订、长期使用、实体手机与学习效果待验收。
- 账号 `adaelon`，书 `ai-infra-book-complete`，publication `01a0f58a-3382-7e03-890a-e3e36adfe89e`；聊天 `chat-ex13-release-20261002`，标题“1.3 交互讲解 · 已验收版本”。
- 原聊天 `chat-01a0f704-ce2b-7403-85eb-831c5c2f7701` 保留；恢复 EX12.5 时读 `docs/切片方案-演示页全局框架与分阶段制作.md` 的 EX12.5 合同。
