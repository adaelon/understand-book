# 来源与核对记录

读取日期：2026-10-12。上游版本：[50db387dd1218f5a0578183bdfd031cdb9375573](https://github.com/Deeki-Sora/last_hit_positioning/tree/50db387dd1218f5a0578183bdfd031cdb9375573)。本书使用该提交的公开内容，不包含上游未提交材料。

上游材料入口：

- [README](https://github.com/Deeki-Sora/last_hit_positioning/blob/50db387dd1218f5a0578183bdfd031cdb9375573/README.md)
- [数学原理](https://github.com/Deeki-Sora/last_hit_positioning/blob/50db387dd1218f5a0578183bdfd031cdb9375573/补刀站位原则_数学原理.md)
- [计算核心](https://github.com/Deeki-Sora/last_hit_positioning/blob/50db387dd1218f5a0578183bdfd031cdb9375573/last_hit_positioning_v3.py)
- [交互入口](https://github.com/Deeki-Sora/last_hit_positioning/blob/50db387dd1218f5a0578183bdfd031cdb9375573/补刀站位_交互输入版_v3.py)
- [现有测试](https://github.com/Deeki-Sora/last_hit_positioning/blob/50db387dd1218f5a0578183bdfd031cdb9375573/test_last_hit_positioning_v3.py)
- [MIT 许可证](examples/upstream/LICENSE)

## 章节与关键结论

| 章节与结论 | 来源位置 | 依据性质 |
| --- | --- | --- |
| 导读、第 1 章：已知敌方与两只关键兵，选择己方位置 | 数学原理 §1；`solve_geometry(enemy, own_minion, target_minion, ...)` | 已读原文与实现 |
| 第 1 章：补兵圆内与敌方普攻圆外的交集 | 数学原理 §2–3；`solve_geometry` 中有效射程与 `rho` | 已读；A–D 点另外复算 |
| 第 2 章：对手与其目标确定局部朝向，交换角色需更换目标 | 数学原理 §1、§12；`u=(F-E).unit()` | 已读；正文为自主组织的教学解释 |
| 第 3 章：切线距离、区间端点、三个代表点 | 数学原理 §4–7；`reachI`、`Geo.safeAaI`、`aggressive_point`、`midpoint`、`conservative_point` | 已读；G1 端点与直接距离复算 |
| 第 4 章：最大施法范围与具体消耗距离独立 | 数学原理 §2.2–2.3；`pressure_range`、`print_risk`、`recommendedI` | 已读；明确 `pressure_range` 不筛选候选 |
| 第 4 章：普通补刀、技能与其他方向的候选 | 数学原理 §6、§10；`Geo.recommendedI` | 已读；G2–G4 与 G3 线外 P 点复算 |
| 第 4 章：圆形范围与矩形碰撞 | 数学原理 §8；`Skill.target_reach`、`aimed_at_q_hits` | 已读；随书技能数值复算 |
| 第 5 章：出手起算的剩余窗口，时间分段与追击 | 数学原理 §9；`simulate_window`、`minimum_hit`、`move_toward`、`move_away` | 已读；W1–W4 实际执行 |
| 第 5 章：W1 的保守距离下界支持 | 数学原理 §9.3 的三角不等式下界；本书 W1 参数 | 解析下界与数值结果分别记录 |
| 第 6 章：固定射程时目标位置改变机会 | 数学原理 §13 的分析方式；本书短手静态例和 W1、W2 | 本书练习数据，非官方英雄属性 |
| 第 7 章：提前观察、回看片段与比较后续代价 | 由前述局部模型组织的实践建议 | 编辑性推论；没有声称实战效果数据 |
| 附录 A：数学定义与代码语义对应 | 上游数学原理；核心向量、区间、技能、模拟实现 | 已读；特殊输入与默认值按实现说明 |
| 附录 B：函数调用、参数表、结果 | `examples/book_cases.py`、`results.json`、`quickstart.py` | 可运行资料，结果由本次执行生成 |
| 附录 C：切线搜索范围、起点、伤害、时序与近似限制 | 数学原理 §10、§14；核心与交互入口的提示和输入合同 | 按源码能力收窄结论 |
| 附录 D：可独立运行代码 | 上游核心从文件开始至 `simulate_window` 结束的连续定义；文末新增 W1 入口 | 保留原代码与 MIT 声明，删去后续控制台输出函数 |

## 练习材料与配图

G1–G4、W1–W4、第六章静态例、A–D 点及 P 点为本书构造的练习参数。参数定义和计算结果分别位于 [案例程序](examples/book_cases.py)与[结果 JSON](examples/results.json)。正文保留足以理解的数字，附录保留完整输入和更多小数位。

八幅配图由 [draw_figures.py](tools/draw_figures.py) 依据相同坐标绘制。追击图调用 `window_trace` 记录与上游相同的更新过程，其终点与 `simulate_window` 对照。所有图均为本次生成，没有使用游戏截图或未取得的上游图片。

已发现上游 README 和数学文档引用的 `examples/` 目录不在所读提交中。本书没有声称运行这些缺失的示例，也没有引用其未提供的测试报告。

## 本次验证的范围

定向复算检查关注书中可被算错的结论：普攻与施法区间端点、四个候选点的距离、线外可行反例、短手静态例、四种窗口结果、技能尺寸口径和两幅轨迹的终点一致性。

附录 B 的代码块抽取为 `quickstart.py` 实际执行；附录 D 的完整代码保存为 `standalone_model.py` 实际执行。组装时由 KaTeX 解析正文和附录公式，交付检查核对本地图片、代码、目录链接与单文件正文一致性。版面另通过本地浏览器检查。

具体验证结果与阅读版状态见 [SESSION_CHECKPOINT.md](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/books/last-hit-positioning/SESSION_CHECKPOINT.md)。未执行上游整个测试集；未进行实际游戏录像标定或教学效果实验。
