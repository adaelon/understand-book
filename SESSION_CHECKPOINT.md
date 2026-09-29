# SESSION_CHECKPOINT — 2026-09-29 集成基线

## 新鲜度自检
- 写入前最新 commit：`c2ff3e1 feat: ship reader, agent, viewport, and observability updates`。
- 本页随 `feat: integrate agent goals, presentation, build harness and reader updates` 提交；实际提交身份以 `git log -1` 为准。

## 当前在做什么
现有源码、测试、固定资源及实施文档整合为一个可继续工作的基线。主要在途工作是 EX11.7 跨题真实生成与验收；EX11.1–6 工程完成，制作方法为 ex11.v9。各条工作线的验收状态保持原结论。

## 下一步（可直接接手）
1. 阅读 EX11.6–7 报告和 EX11 方案 §8–10；检查本地 batch8 的 summary/outcome，先确认已有运行终态，再决定是否启动后续样本。
2. 对已交付原稿运行独立数学/图元或真实解码帧核对，按实际页面适配 reader-selectors.json；不要人工改模型页补算成功。
3. 使用 `EX10_CONTENT=<run>/content.json` 启动 `presentation_ex10_browser_host --ignored --nocapture`，通过 `playwright.ex10.config.ts` 执行 Reader 使用链，完成后 POST /stop。
4. 按冻结合同完成剩余重复题和短答；工程或方法修复另开批次，完整合同实际通过后才标记 EX11.7 完成。
5. 下一项实现按独立切片提交；DH6 和调用成本后续分别从专用 checkpoint 接手。

## 未提交 / 未完成
- 本次集成提交纳入当前实现、测试、依赖、固定资源、切片方案及报告；本机配置、临时文件、书籍、handoff 和大批原始运行产物保留本地。
- EX11.7 尚未完成完整六例与短答使用链；batch1–7 的原始失败及归因保留。batch8 的最新终态以本地运行目录为准。
- G6 整体自然请求/发布验收、CQ 范围表达稳定性、DH0 人工作答以及 DH6 真实模型质量/业务恢复/回滚仍有未完成项；DH7 属于后续扩展。
- DeepSeek 续接、状态追加和压缩预算工程已完成；真实缓存命中和费用收益尚待实际任务观测。
- ED3、EX2、EX10 保留其未通过的实验结论；EV7 候选已撤回。TutorSession 等教学领域 ADR 仍为设计决策。

## 当前代码与验证
- EX11.6：页内交互归 content，宿主 Reader 动作归 ReaderAction；原 Goal 误分类复现已修复，不提高回合上限。
- Flash Author 输出预留为 131072，输入压缩水位保持原范围；普通请求和显式配置保持原合同。
- 压缩补全既有 v1 结构；最终历史改写/截断已保存前缀时失效派生 checkpoint；严格加载校验保持。
- 成功预览按实际请求区分新进展；重复输入/失败不增进展。AnimationCue 覆盖实际 Provider JSON 小数解析路径。
- ex11.v9 增加最小 160px Reader 内容区的媒体控件布局指导；实际解码帧、播放、恢复与追问仍需逐题验收。
- 集成前重新执行本地 Rust 回归、Core/Web 定向回归、前端构建、评测脚本、观测层/只读 MCP 测试和 DSH 类型检查/打包；最终结果见本次提交说明。

## 冷启动读序
1. `docs/performance/explorable-explanation-ex11-6-7-20260928.md` — 真实失败、修复和批次结论。
2. `docs/切片方案-EX11-Konva通用能力与局部动态演示.md` §8–10 — 当前状态、验收合同。
3. `docs/performance/ARTIFACTS.md`，然后本地 `docs/performance/ex11-local-demonstrations/ex11.7/batch8/` — 证据保存边界及已有结果。
4. `docs/代码链路.md` 自 2026-09-20 选区采样修复起的新增记录 — 集成改动归属。
5. 按接手任务读取 `SESSION_CHECKPOINT_DSH.md` 或 `SESSION_CHECKPOINT_KV_CACHE.md`；EX11 环境见 `docs/Manim-部署.md` 和 EX11.3–5 报告。

## 本会话决策摘要
- 用户确认将交织的现有改动筛选后整体提交；保持未完成验收和失败实验的真实状态。
- 原始模型记录、截图、视频、trace 和逐批源码副本保留在原位置；报告、复现脚本与必要夹具进入仓库。
- 提交后以这份基线继续，后续按独立切片落 commit。
