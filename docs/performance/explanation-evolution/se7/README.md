# SE7：现有方法已覆盖精确恢复案例

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树。下一入口 SE8：完整能力单元的同条件对照与独立迁移。

## 实施步骤

- [x] 读取 SE6 的事实、原因解释与待验证假设，对照现行指导、完整作品及已验收记录。
- [x] 逐字比对当前资产与 SE1/SE3 已保存内容，确认可沿用原验收证据。
- [x] 保存诊断与 SE8 接手条件，更新方案、代码链路及主/专线 checkpoint。

## 诊断结论

**已有方法足够，本次不产生新版本。** D2 历史圆盘的精确恢复问题已由当前合同、`discrete-update@1` 的可运行实现和既有恢复验收覆盖。该单元继续保持 SE1 候选身份，正式方法目录仍为空；后续 SE8 比较现行资产集合与加入该完整单元的组合。

沿方案 §5.2，此案例分支为 `待诊断 → 不提炼：已有方法已覆盖`。SE6 的 `verified/case.json` 保留整理时的 `pending_diagnosis` 快照；本次后续诊断另存于仓库根 `.understand-book/evolution-cases/se6-disk-restore/se7/diagnosis.json`，通过 case_id、collected_at 及确切原版/修正版引用连接原材料。

## 事实、原因与判断

**事实**：SE6 用历史完整 revision 2/3 在三个视口重放，原版将 θ=6.282399909016188（7+.999）恢复为 2π（8+0），修正版保持原 θ 且暂停。历史及本次重放的原始记录分别保留在 SE6 案例的 `original_check`、`repaired_check`、`reader_check`、`replay`；公开结果见 [重放摘要](../se6/replay-summary.json)。

**原因**：原版 `index.html:506` 的恢复赋值为 `clip(snapEndpoint(v.theta), 0, TWO_PI)`，修正版 `index.html:509` 为 `clip(v.theta, 0, TWO_PI)`。`snapEndpoint` 把距终点不足 .001 的值吸附到 2π；滑块量化输入和模型精确存档具有不同含义。案例 `repair_diff` 和两份完整源码支持这个原因判断。历史圆盘使用 ex11.v11，没有历史 method_refs。

**待验证假设的处理**：SE6 提出的“当前方法是否仍缺精确恢复支持”已完成诊断；没有发现本案例需要补充的当前实现缺口。完整能力单元能否改善独立任务的选用、制作或修复仍由 SE8 判断。

| 案例要求 | 当前实现与合同 | 已有证据 |
|---|---|---|
| 合法的近终点不能变成整步 | [作品](../../../../skills/presentation/methods/discrete-update/v1/example/index.html) `position/seek` 仅按 0..8 限界并分离整数与小数，没有邻近终点吸附 | SE1 Reader 三环境各恢复 7+.999，保存值与恢复值一致 |
| 恢复不以滑块显示值为准 | `registerStateReader` 保存模型字段；`registerStateRestorer` 从 `saved.values.page` 取 η/k/p 后调用 seek；滑块只是 renderAt 的输出及用户输入入口 | 同一 Reader 验收保存 2+.35、7+.999、8+0，共 9 次存储重开 |
| 恢复与定位应暂停 | `seek` 先 pause 再定位和渲染；[continuous-scene](../../../../skills/presentation/references/continuous-scene.md) 要求共享暂停定位路径 | 9 次 after.state.playing 均为 false |
| 数学关系、图形和状态一致 | 单一模型驱动 renderAt；[checks.md](../../../../skills/presentation/methods/discrete-update/v1/checks.md) 第 1、4 项检查图形与恢复，第 5 项拒绝图形冻结坏例 | SE1 的 250 组独立数学/几何检查及坏例识别 |
| 迁移不能照搬主题内容 | [卡片](../../../../skills/presentation/methods/discrete-update/v1/card.md)与 [适配说明](../../../../skills/presentation/methods/discrete-update/v1/example/README.md)要求重建公式、域、边界与来源；连续物理过程使用自身时间模型 | 此处是已明确的适用边界，独立迁移效果尚待 SE8 |

当前例子的滑块范围是 0..8，终点可由 .001 步长精确表示；圆盘滑块的 2π 终点不具备这个条件。可复用部分是模型状态与控件量化分开、确定性定位和暂停恢复；θ、π/4 分段、半径和弧长关系属于圆盘内容，不能将圆盘主题实现加入离散更新配方。`continuous-scene` 的“通过同一 seek 定位”约束允许输入先换算，不能据此把控件专属吸附带进恢复。

## 证据与复现

当前资产与既有验收的对应结果见 [baseline-verification.json](baseline-verification.json)。比对入口为 [verify-baseline.mjs](verify-baseline.mjs)：从 SE3 已保存的实际 Provider 输入重组四文件六段正文，逐字比较当前文件；再比较当前 HTML 与 SE1 最终交付，并核对正式目录为空。

```powershell
node docs/performance/explanation-evolution/se7/verify-baseline.mjs
```

若比对失败，停止沿用对应文件的旧验收，先识别资产变化，再按受影响行为重验。原有工程结果保留 [SE1](../se1/README.md)、[SE3](../se3/README.md) 和 [SE6](../se6/README.md) 的运行身份。SE7 没有改动运行代码、资产、验证合同或模块边界。

## 下一入口

[SE8 接手合同](se8-handoff.md)明确 A/B 资产、D/H/R 材料用途、比较与失败处理。保留 SE1 候选，结束本次 D2 派生新版本的提案。SE8 开跑前冻结独立 H1/R1 具体输入及同模型、工具、预算、顺序和停止条件。

## 已知限制

- 结论只覆盖本案例的精确恢复缺口。历史页面没有实际读者投票、反馈现场或方法使用记录；页面修好不构成方法收益或学习效果证据。
- 本次沿用已验收作品的工程结果，没有重跑浏览器、数学全量或服务权限测试；新增运行仅做资产比对，0 次模型请求。未发布、部署或采用候选。
- 首次比对脚本将能力入口 `example/index.html` 误作交付入口，导致比较失败；按交付自身 `entrypoint=index.html` 取文件后通过。两份 HTML 正文逐字一致，运行资产没有因此修改。
- SE8 的运行配置和成本边界尚未冻结；历史修复的 355,433 tokens 只属于旧运行，不决定新批次预算。
