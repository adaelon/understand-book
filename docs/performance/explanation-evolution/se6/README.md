# SE6：从真实使用形成可追溯案例

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树。下一入口 SE7：判断现行方法是否已覆盖该案例，再决定候选。

## 实施步骤

- [x] 实现 `evals/semantic/explanation-case.mjs`，整理明确选定的私有材料，连接交付版本、方法依据、当前反馈、确切现场、观察与修复。
- [x] 整理 EX11.8 圆盘 revision 2 → 3 的真实修复，并从保存的完整版本复现差异；真实读者反馈和历史方法引用保持缺失。
- [x] 验证缺失、未读、继承、改选/撤回、失效材料与公开索引；更新方案、代码链路及 checkpoint。

## 案例整理合同

入口为 [explanation-case.mjs](../../../../evals/semantic/explanation-case.mjs)，完整清单格式、材料来源与调用方式见 [EXPLANATION_CASE](../../../../evals/semantic/EXPLANATION_CASE.md)。首版按明确选定的授权导出材料运行，输出私有 `case.json` 和可分享的 `index.json`，每次保存到新目录。

`原任务/来源 → 原交付与修复交付 → 方法读取/继承依据 → 反馈导出快照与确切现场 → 观察/原因解释/假设 → 待诊断案例`。交付必须出现在原书/会话已完成回合的 answer_view；修复 based_on 指向确切原版；内容直接逐字段/逐文件比较。能力引用递归解析 candidate/version/read，保留原回合、工具调用与实际字符范围。方法目录、卡、源码、声明分别表达。

反馈当前导出、先前观察和现场分别保存。改选只保留当前有效选择的含义，撤回不形成有效票，先前反馈不填补当前不可用状态。现场必须匹配完整回执和所有者；无现场不丢有效选择。`observed_at` 标记导出时间，`collected_at` 标记整理时间；旧快照不声称是线上即时状态。页面反馈不分摊给方法，程序不自动批准候选。

Provider 工具证据与失败调用使用现有 `toolMessages` 和 `reportToolTrace`，用量沿 `measuredUsage`。材料只能提供元数据时不补造原文；显式不可用材料不尝试读取其路径。原始任务、评论、完整版本、来源和机器路径留在私有案例；公开索引仅投影固定状态、数量和用量。

## 真实案例与接手位置

选用方案 §7.1 已指定的 [EX11.8 圆盘精确恢复修复](../../explorable-explanation-ex11-8-live-20260929.md#圆盘页)。来源是当时任务直接提供的理想圆周运动材料，历史页面无书源绑定。revision 2 与 3 均从原归档完整版本读取，并在原 completed 回合确认交付。

私有接手目录为仓库根目录下 `.understand-book/evolution-cases/se6-disk-restore/`，沿既有 Git 忽略规则保留：

- `verified-manifest.json`：完整材料定位、任务/来源、原检查、差异、真实模型记录和本次重放。
- `verified/case.json`：最终案例包，含原版/修正版完整源码、事实、解释、假设及全部选定材料。
- `collected/replay.json`：本次三视口的原始快照、恢复结果和确切版本；已纳入最终案例包。
- `prepare.mjs`、`manifest.json`、`provider-records.json`：从既有 batch13 分文件归档形成清单及既有 Provider 记录格式的本地准备入口；不发起模型请求。

公开 [案例索引](index.json)保留 `actual_agent_revision`、两次交付 confirmed、一个源码文件变化和四条观察。历史修复共 10 次聊天请求与 1 次压缩，355,433 tokens，沿历史真实用量归档；本次新增模型请求为 0。

## 验证结果

| 验证 | 结果与证据 |
|---|---|
| 案例单元验收 | 最终 12 项通过：[日志](case-final.log)；[测试源码](../../../../evals/semantic/explanation-case.test.mjs) |
| 既有证据/用量回归 | 首轮合并 26 项通过，其中案例 11 项、既有 agent-core/agent-report/provider-recorder 15 项：[日志](unit-regression.log)。之后新增导出选择/坏 JSON 用例并重跑全部 12 项案例测试 |
| 确切关系 | 原交付/修复、内容差异、当前反馈与现场身份；错版本/会话/回合/快照拒作有效关系；未交付和非 based_on 修复分别标出 |
| 缺失与归因 | 无现场仍有票；无票/撤回/改选区分；失效材料不读旧路径；未记录、空声明、只见目录、只读卡、未读源码、候选/版本继承区分；两方法页面不分票 |
| 私人输出 | case 完整回读且不可覆盖；公开投影排除原文、评论、scope、私有材料名和路径 |
| 真实 SE3 保存格式 | SE3 D1 版本实际解析到 discrete-update@1、四文件六段读取，卡/代码分别识别：[结果](stored-method-check.json)。该数据明确为 SE3 工程 Provider 夹具 |
| 历史页面重放 | 1 项浏览器用例通过，包含两版本 × 三视口：[日志](replay-browser.log)、[非私有结果](replay-summary.json)、[测试入口](../../../../packages/web/playwright/presentation-se6-case.spec.ts) |

重放经当前 `presentationDocument` 装配完整历史版本，用真实 iframe 与快照桥接保存/恢复，六次打开结果如下：

| 视口 | revision 2 恢复 | revision 3 恢复 |
|---|---|---|
| 960×720 | θ=6.283185307179586，8+0 | θ=6.282399909016188，7+.999 |
| 320×420 | θ=6.283185307179586，8+0 | θ=6.282399909016188，7+.999 |
| 640×240 | θ=6.283185307179586，8+0 | θ=6.282399909016188，7+.999 |

每次原快照均为 θ=6.282399909016188，重开均暂停。差异与历史检查一致；具体原因是输入端点吸附误用于恢复，修复后恢复赋值保留精确 θ。历史 Konva 资源逐字相同；本片没有重跑未受影响的 84 组数学检查。

## 复现

在仓库根目录运行；输出目录须为新目录：

~~~powershell
pnpm eval:case:test
node --test evals/semantic/agent-core.test.mjs evals/semantic/agent-report.test.mjs evals/semantic/provider-recorder.test.mjs
pnpm eval:case .understand-book/evolution-cases/se6-disk-restore/verified-manifest.json .understand-book/evolution-cases/se6-disk-restore/recheck
$env:SE6_CASE = Join-Path (Get-Location) '.understand-book/evolution-cases/se6-disk-restore/recheck/case.json'
pnpm --filter @understand-book/web exec playwright test playwright/presentation-se6-case.spec.ts --workers=1 --reporter=line
~~~

## 已知限制

- 真实案例发生在反馈功能和 method_refs 之前，没有实际读者喜欢/不喜欢或反馈现场回执；历史共同指导 ex11.v11 的输入证据保留，不能称作 discrete-update@1 的使用或收益。重放现场是本次工程验证，不是读者反馈。
- 案例状态为 pending_diagnosis。已证实历史作品的具体失败和修复；跨任务能力归因、候选和学习收益由 SE7–SE9 的独立证据决定。下一步先读当前 continuous-scene 指导和 discrete-update@1，判断已有方法是否足够。
- 整理器读取获准导出，不直接访问多人线上服务；撤权/删除的实时判定沿 SE4 授权入口。这里验证不可用导出不会回退读旧材料，不声称重新验证了服务授权。
- 本次重放覆盖页面装配与 bridge 的快照恢复，没有再次运行服务器状态持久化、宿主反馈 UI、滑块/播放全回归；这些保留各自原有证据。浏览器仅有既有颜色环境警告。未发布、部署或新增付费模型实验。
