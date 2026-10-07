# EX12.0 实际问题页与比较基线

2026-10-02。状态：**EX12.0 完成；下一入口 EX12.1。** 依据 [EX12 切片合同](../切片方案-演示页全局框架与分阶段制作.md)与 [ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)。

这份页面包含单位、工作量、时间下界、重叠条件、批内复用、实测和练习，主要交互也能工作。实际障碍集中在：基础概念引入太晚，操作与图形分离，交互步骤与正文步骤含义不一致，以及窄屏画布文字碰撞。前三项在独立页面同样存在，不能只归因于 Reader 嵌入。

## 确切对象与重新打开

| 项目 | 固定值 |
| --- | --- |
| 页面 | `presentation-1790855525959649112-18`，revision `1` |
| 标题 | 1.3 用几个数字估算一次模型执行：零基础版完整心智模型与逻辑链 |
| 候选 | `candidate-1790855525959653804-19` |
| 账号 / 书籍 | `adaelon` / `ai-infra-book-complete` |
| 书籍发布 | `01a0f58a-3382-7e03-890a-e3e36adfe89e` |
| 原会话 | `chat-01a0f704-ce2b-7403-85eb-831c5c2f7701` |
| 原回合 | `turn_01a0f745-9c37-7281-9810-c63b5d515815`，会话第 2 回合 |
| 阅读位置 | `3.6.1`，原选区标题“1.3 用几个数字估算一次模型执行” |
| 线上服务目录 | `/opt/understand-book/releases/mu12-20261001` |

**实际 Reader**：打开 [Linux Reader](https://115.190.121.150/)，使用上述账号和书籍，进入本书历史中“这节讲的是什么，怎么理解呢，给我可交互的演示吧”会话，找到“把我当成初学者”的第二回合，打开其“演示 · 版本 1”，核对上表完整标题。第一回合也有一个“版本 1”，但它的 presentation ID 不同，不能混用。账号与服务器访问资料仍按本机访问说明取得。

**独立页面**：完整版本保存为 [version-1.json](presentation-staged-authoring-baseline/version-1.json)，其中三个 `content_files` 原样展开到 [page/index.html](presentation-staged-authoring-baseline/page/index.html) 与 `page/libraries/`。从仓库根运行：

```powershell
python -m http.server 18791 --bind 127.0.0.1 --directory docs/performance/presentation-staged-authoring-baseline/page
```

然后打开 `http://127.0.0.1:18791/index.html`。独立页面可以运行本页交互；原书引用跳转及现场保存需要 Reader 桥接。它用于隔离布局因素，不代替 Reader 验收。

Linux 原文件位于 `/opt/understand-book/data/multi-reader/users/adaelon/memory/agent-history.presentations/versions/presentation-1790855525959649112-18/1.json`。归档未修改该版本，也未生成新候选或调用模型。

## 原请求与可复用材料

原请求是两轮对话，第二轮的“再做一版”依赖第一轮，不能把它单独当成一个无上下文的新请求：

> 这节讲的是什么，怎么理解呢，给我可交互的演示吧

选区：“1.3 用几个数字估算一次模型执行”。收到第一版后，用户继续：

> 不对了啊，没有带完整的心智模型，有很多默认的，把我当成初学者，给我完整的心智模型和逻辑链条呀，再做一版

| 保存文件 | 用途 |
| --- | --- |
| [request-context.json](presentation-staged-authoring-baseline/request-context.json) | 两轮逐字请求、第一轮选区、上一轮回答及第二轮实际用户消息。 |
| [previous-version-1.json](presentation-staged-authoring-baseline/previous-version-1.json) | 第一轮交付 `presentation-1790851114402727998-8` revision 1，供“再做一版”读取。 |
| [original-turn.json](presentation-staged-authoring-baseline/original-turn.json) | 第二轮冻结输入、模型配置、来源绑定、交付结果、调用和活动记录。 |
| [original-book-reads.json](presentation-staged-authoring-baseline/materials/original-book-reads.json) | 原会话六条 book 读取回执；包含两轮对 `3.6.1`、`3.6.2`、`3.6` 的实际读取。 |
| [section-1.3.md](presentation-staged-authoring-baseline/materials/section-1.3.md) | 第二轮 `3.6` 回执的可读展开。原始转义内容保留在上项。 |
| [published-section-1.3.md](presentation-staged-authoring-baseline/materials/published-section-1.3.md) | 从确切发布目录 `source.txt` 截出的 1.3 正文，含 1.3.1–1.3.4 与练习。 |
| [publication.json](presentation-staged-authoring-baseline/materials/publication.json)、[source_manifest.json](presentation-staged-authoring-baseline/materials/source_manifest.json)、[asset_manifest.json](presentation-staged-authoring-baseline/materials/asset_manifest.json) | 原发布与 LID、来源、图片映射；原有字段原样保留。 |
| `materials/assets/ch01/` | 上述正文引用的七张 SVG，从原发布的 `stored_path` 复制，按正文引用名放置。 |
| [readable-content.md](presentation-staged-authoring-baseline/readable-content.md) | 作者提供的可回读说明；它不是画面观察记录。 |
| [guidance-baseline/SKILL.md](presentation-staged-authoring-baseline/guidance-baseline/SKILL.md)、[agent_prompt.rs](presentation-staged-authoring-baseline/guidance-baseline/agent_prompt.rs) | 线上 release 目录中的 `ex11.v11` 指导与注入源文件，保存旧指导对照。 |
| [historical-run-metrics.json](presentation-staged-authoring-baseline/historical-run-metrics.json)、[deployment.txt](presentation-staged-authoring-baseline/deployment.txt) | 原生成消耗和本次读取的实际服务目录；历史生成不等于同底座对照。 |

用户提供的五张《信任的进化》截图均已找回：[故事引入](presentation-staged-authoring-baseline/references/trust-1.png)、[收益矩阵](presentation-staged-authoring-baseline/references/trust-2.png)、[策略预测](presentation-staged-authoring-baseline/references/trust-3.png)、[进化过程](presentation-staged-authoring-baseline/references/trust-4.png)、[群体组成](presentation-staged-authoring-baseline/references/trust-5.png)。这些是原有参考材料的归档，本次未重跑参考网站。

## 实际观察环境与串读位置

浏览器使用本机 Codex 内置浏览器，访问实际 HTTPS Reader。初始外层视口 620×620，演示 iframe 约 602×396；随后将外层设为 1280×900，页面内容区约 778×654，保存现场提示出现后约 778×616。页面保持现有主题与字体。独立原文件分别在 960×720 和 320×420 观察，未注入 Reader 样式。

[observations.json](presentation-staged-authoring-baseline/observations.json) 记录 Reader 截图时间、内容滚动位置、各标题相对位置及控件值；[standalone-observations.json](presentation-staged-authoring-baseline/standalone-observations.json) 记录独立页面。纵向位置是文档滚动位置，受视口与 Reader 样式影响；跨环境定位应同时使用段落名称。截图保留完整视口，未裁剪或重排。

| 阅读位置 / 动作 | Reader 纵向位置约值 | 截图 |
| --- | ---: | --- |
| 初始窄宿主首屏 | 0 | [01](presentation-staged-authoring-baseline/screenshots/01-reader-initial.jpg) |
| 宽宿主首屏：开场与画布 | 0 | [02](presentation-staged-authoring-baseline/screenshots/02-reader-wide-top.jpg) |
| 下一步、B=8 预设 | 510 / 530 | [03](presentation-staged-authoring-baseline/screenshots/03-reader-next.jpg)、[04](presentation-staged-authoring-baseline/screenshots/04-reader-batch8.jpg) |
| B=8 实时读数与七参数 | 1068 | [05](presentation-staged-authoring-baseline/screenshots/05-reader-results-parameters.jpg) |
| 七词表 → 第 0 步舞台 | 2257 | [06](presentation-staged-authoring-baseline/screenshots/06-reader-vocabulary-stage.jpg) |
| 第 1 步：三个量与单位 | 2783 | [07](presentation-staged-authoring-baseline/screenshots/07-reader-counts.jpg) |
| 第 2 步：两条时间下界 | 3645 | [08](presentation-staged-authoring-baseline/screenshots/08-reader-times.jpg) |
| 第 3 步：max 与重叠条件 | 4048 | [09](presentation-staged-authoring-baseline/screenshots/09-reader-overlap.jpg) |
| 第 4 步：四种改动的解释 | 4458 | [10](presentation-staged-authoring-baseline/screenshots/10-reader-four-changes.jpg) |
| 第 5 步：转折点与算术强度 | 5019 | [11](presentation-staged-authoring-baseline/screenshots/11-reader-turning-point.jpg) |
| 第 6 步：实测、KV 与容量边界 | 5612 | [12](presentation-staged-authoring-baseline/screenshots/12-reader-measurement.jpg) |
| 第 7 步 → 练习 1-3 | 6233 | [13](presentation-staged-authoring-baseline/screenshots/13-reader-exercise.jpg) |
| 假设、误解 → 页底总结 | 7290 → 页底 | [14](presentation-staged-authoring-baseline/screenshots/14-reader-end.jpg)、[15](presentation-staged-authoring-baseline/screenshots/15-reader-bottom.jpg) |
| 练习预设、重置、q=4 比较图 | 见记录 | [16](presentation-staged-authoring-baseline/screenshots/16-reader-exercise-preset.jpg)、[17](presentation-staged-authoring-baseline/screenshots/17-reader-reset.jpg)、[18](presentation-staged-authoring-baseline/screenshots/18-reader-final-diagram.jpg) |

从开场到页底已逐段观察主要局部；这不是每个滚动像素的全覆盖截图。正文中的单位解释、2N 来源、重叠假设、吞吐与单请求延迟的区别、容量限制、实测及练习都存在。缺陷应按下面的具体位置讨论，不能概括为“内容缺失”或“交互不能用”。

## 已确认的理解障碍

**1. 先使用后解释，七词表出现得太晚。**

实际顺序是：开场 → 舞台/时间图 → 操作 → 实时公式 → 七参数 → 四改动表 → 七词表 → 正文第 0 步。初学者先遇到 `R_W / β_eff`、`F / Π_eff`、算术强度、机器平衡和 B*，才读到权重、token、带宽、算力与下界的基本定义。Reader [05](presentation-staged-authoring-baseline/screenshots/05-reader-results-parameters.jpg) 与 [06](presentation-staged-authoring-baseline/screenshots/06-reader-vocabulary-stage.jpg)，独立 [27](presentation-staged-authoring-baseline/screenshots/27-standalone-vocabulary.jpg) 均支持这个顺序。保存的可回读说明却写“开头的词表”，说明只回读文字描述会漏掉真实顺序。

后续修订应先安排理解所需前提，再让相关公式与操作出现；保留完整内容。源码中 `$('core').appendChild(gloss)` 会将词表移至交互区末尾，属于对画面结果的实现解释，不是仅凭源码得出的视觉结论。

**2. 图、操作、推导分处多屏，阅读必须来回寻找。**

Reader [03](presentation-staged-authoring-baseline/screenshots/03-reader-next.jpg) 显示操作后只有画布下方空白区、说明、按钮与状态，舞台和时间条已经离屏。独立 960×720 的 [20](presentation-staged-authoring-baseline/screenshots/20-standalone-next.jpg) 同样如此。初始画布为后续比较图预留较大空白，按钮又排在图下的说明之后；独立 [19](presentation-staged-authoring-baseline/screenshots/19-standalone-top.jpg) 可见按钮尚在首屏之外。

正文第 4 步又引用“上面那张实时表”，Reader [10](presentation-staged-authoring-baseline/screenshots/10-reader-four-changes.jpg) 所在位置约 4458，前面的参数区约 1068；正文没有就近图表。后续应围绕当前关系安排图文与操作，而不是要求所有图和控件先集中在开头。Reader 可用高度会放大距离，但独立页面复现，不能用宿主扩宽替代内容组织修订。

**3. “第几步”同时指不同过程，标注容易误导。**

交互第 1 步是“把权重读一遍”，第 2 步是“做矩阵乘加”，共 4 步；正文第 1 步却是“翻译成三个量”，第 2 步是“两条时间下界”，并继续至第 7 步。再加上 q=步号+进度，点一次“下一步”得到 q=2、文字“第 1 步·进度 1.00”；从 q=4 回退得到 q=3、文字“第 2 步·进度 1.00”。证据为 Reader [03](presentation-staged-authoring-baseline/screenshots/03-reader-next.jpg)、正文 [07](presentation-staged-authoring-baseline/screenshots/07-reader-counts.jpg)，独立 [20](presentation-staged-authoring-baseline/screenshots/20-standalone-next.jpg)、[22](presentation-staged-authoring-baseline/screenshots/22-standalone-previous.jpg)。

这些不是按键未执行。后续应区分讲解段落、动画阶段与真实 token 生成步，让按钮和位置文字使用同一套清楚的对象约定；无需因此引入新的时间轴系统。

**4. 320px 独立页面的画布标签发生拥挤与碰撞。**

在默认参数、q=4 下，[28](presentation-staged-authoring-baseline/screenshots/28-standalone-narrow-diagram.jpg) 中“两条 lane……”的说明换成多行，挤入下方瓶颈标注附近；四改动的左侧名称也因区域过窄而拆行。蓝色长条与橙色短条的数量差仍可由数值辨认，但图内解释已不易跟读。960px [21](presentation-staged-authoring-baseline/screenshots/21-standalone-final-diagram.jpg) 可对照同一参数的完整比较图。

该窄屏结果来自独立原文件，因此属于生成页图形排版。后续应调整此局部的标签位置、换行及图形组织，以实际窄屏复看决定；不以缩小全页字体处理。

## 操作结果与恢复

| 环境与操作 | 实际结果 | 证据 |
| --- | --- | --- |
| Reader：下一步 | q=2；语义第 1 步，进度 1.00 | 03 |
| Reader：B=8 预设 | B=8、q=0；读取 20.90 ms、计算 1.13 ms、分摊 2.61 ms、吞吐 382.9 token/s | 04–05 |
| Reader：练习预设 | B=16、有效带宽 0.7、有效算力 0.5；读取下界 29.85 ms | 16 |
| Reader：重置 | B=1、有效比例均为 1、q=0 | 17 |
| Reader：位置滑块 End | q=4，显示四改动比较图 | 18 |
| 独立：上一步 | q=4 → 3，语义第 2 步、进度 1 | 22 |
| 独立：位置 Home → PageUp | q=0.40，语义第 0 步、进度 0.40 | [23](presentation-staged-authoring-baseline/screenshots/23-standalone-fraction.jpg) |
| 独立：播放 → 暂停 | 播放时按钮变为暂停、q 前进；暂停后按钮恢复播放，观测 q=0.27 | [24](presentation-staged-authoring-baseline/screenshots/24-standalone-playing.jpg)、[25](presentation-staged-authoring-baseline/screenshots/25-standalone-paused.jpg) |
| 独立：B 滑块 End | B=512，计算下界/一步下界 72.45 ms；表格正确显示算力×2有效、带宽×2和权重减半无改善 | [26](presentation-staged-authoring-baseline/screenshots/26-standalone-batch512.jpg)及[操作补充记录](presentation-staged-authoring-baseline/operation-results.json) |
| Reader：最终重置 | 恢复开始时 B=1、N=70、bW=1、β=3350、有效比例均为1、q=0，界面显示现场已保存 | [29](presentation-staged-authoring-baseline/screenshots/29-reader-restored.jpg) |

“现场已保存”是这次界面观察，不等于已完成 EX12.5 的重开恢复验收。播放截图的 DOM 采样与截图之间动画仍在前进，因此即时 q 可能略有差异；暂停、预设和静态段落使用固定状态记录。

## 后续同底座对照输入

EX12.4 两组均从同一书籍发布、同一首轮交付与回答、同一第二轮逐字请求出发；先读取 `request-context.json` 和 `previous-version-1.json`，提供同样的原文及来源定位。若改为首轮从头生成，应另记实验，不能混称原请求复现。不要在自然请求中补入固定画面、控件数量或布局答案。

原生成配置为 native / `deepseek-v4-flash`，runtime profile 为 `resident-agent-deepseek-flash-v1`，完整冻结配置见 `original-turn.json`。历史记录包括 19 个模型—工具循环、40 条工具 trace、21 条 usage 记录、2,831,235 个已知总 token；活动记录结束约 650.44 秒。`outcome.turns` 另记为 20；工具 trace 的循环编号为 1–19，与界面显示的 19 对应。usage、终态回合计数与有工具循环数口径不同，比较时分别保留。上述历史消耗只作溯源。

两组重新生成时共用相同模型、来源、工具、候选与预览底座，包括 EX12.3 的 scroll；一组使用归档旧指导，另一组使用阶段指导与框架交接。每组先运行一次并保存失败产物。旧 Linux 成品用于定位问题，不能直接与新底座成品作提示收益比较。

对照首先回答：基础定义是否先于使用，当前操作能否同时看到目标变化，段落和动画对象是否一致，窄屏标签能否读清；同时保留原有单位推导、重叠条件、batch/延迟区别、实测与练习范围。记录实际修订和制作消耗，不合成美观分数。

## 验证与已知限制

本切片只新增证据和更新文档。校验针对三个具体失败：归档页漏资源导致无法重开、请求或发布身份错配导致比较输入不同、报告链接指向不存在的证据；发现时补齐相应文件或修正索引。结果见 [verification.json](presentation-staged-authoring-baseline/verification.json)。没有改运行时代码，未运行全量功能测试。

Reader 图文串读与关键预设已实际执行，独立页补充播放、暂停、回退和参数变化。未进行实体手机、所有参数组合、Reader 播放与重开状态恢复的完整矩阵；320×420 是桌面浏览器缩窄视口，不是实体触摸验收。没有读者学习表现证据，不据此宣称教学效果改善。

Reader 引用按钮比独立页面更长，占据额外行高；宿主相关修复入口为 `packages/web/src/components/AgentPresentation.vue`、`RightRail.vue`、`presentation-document.ts` 与 `source-chip.js`。工作树已有独立的[演示缩放与来源阅读修复记录](../修复方案-演示缩放与来源阅读.md)，本切片不重复实施。当前证据中的主要问题在独立页面也存在，EX12.1 可以继续。

线上 release 目录的服务路径与指导文件已保存；本次未取得该目录的 Git 提交号，不将本地 `5e10516` 当成运行中二进制的版本证明。完整原始版文件和必要材料是本基线的复查依据。
