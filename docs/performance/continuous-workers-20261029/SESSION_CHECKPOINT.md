# 跨批次持续领取 — 2026-10-29

## 目标

完成 Codex 书籍同阶段跨批次持续领取与 BookStructure 动态任务接续，保持 full 质量及并发 8。用户最新要求不重跑对照，直接让聊天 `01a182f6-204a-79e2-95d7-10b5b1cc69dd`（用 understand-book 构建深度学习书）使用新版接续原构建，并用其现有与新增记录观察实际速度。

## 基线与工作区

- 工作区：`E:/allwork/download/agent/understand-book`，包含其他线程的未提交修改，禁止回退或覆盖。
- HEAD：`d791eea51009a4c36150528de50feb5d081c1281`，不能单独用 HEAD 代表当前基线。
- 修改前构建相关源码已冻结：`tmp/continuous-workers-20261029/baseline-source/`，874 个源文件，包含当时相关未提交修改。
- 基线程序：`tmp/continuous-workers-20261029/benchmark/baseline-engine.exe`。

## 当前进展

- 已确认原实现只覆盖当前批次，BookStructure 单项 dispatch 之间仍返回主控重建执行器。
- 已实现稳定 worker 身份、当前 assignment、同 invocation 串行调度、批次结束后的下一项领取，并接入生产 CLI/MCP。
- 最终新增验证：调度 6 项、动态 BookStructure 2 项、两个 MCP 连接跨批次 1 项、工具内部等待 6 项全部断言通过；原执行器/MCP 回归 73 项通过；Core typecheck 通过。
- 调度联跑曾全部断言通过但退出码为1；其长时间同步夹具可能饿死测试框架通信。已在真实提交之间让出事件循环，调度6项单跑退出码0（111.45秒），生产代码未为此改变。
- 额外 Driver/refill/adapter 回归原先99/101通过，剩余两个用例最终均定向通过、退出码0（29.70秒）。策略变更测试改为基于实际值的 `.retry-test`，生产策略未改。
- 最终角色与提示发布合同34项通过；本次所有新增/修复断言均已完成验证。
- 新二进制已安装至既有 operator 路径，旧文件保存为 `understand-book-build-before-continuous-workers-20261029.exe`，原进程保留。插件缓存的 build/executor skill与role副本已同步。
- 《失败的逻辑》对照在用户中断后保留于 `benchmark/baseline-run`：94项已接纳，58个discovery均完成，章节链尚未关闭；child096已按中断登记，返回的恢复ref没有启动。用户要求不重跑，因此不再接续这条专用对照，也不启动已准备的candidate-run。
- 两份隔离副本和全部原始记录保留；尚无这一实验的整阶段加速结论。
- 基线前段与本地回归有时间重叠；这一混杂因素必须记录，不能把阶段耗时差直接全部归因于持续领取。后续测速期间不运行重型回归。
- 基线已完成全部58个discovery/1822个来源单元，已进入动态章节组织；只有一次可重试失败，原引擎自动恢复成功。最新原生child和终态以control-state为准。
- 基线暴露的 `AutomaticBuildDispatchSettledError` 已由worker按WAIT处理并通过真实调度回归；两章前驱提交后动态创建任务、原连接等待唤醒的测试也已通过。
- 工具内部20秒有限等待、每秒轻量同伴状态检查、只在进展/窗口边界重做领取已实现；实际WAIT回包计入上下文预算。全部已打包。
- 指标脚本已增加并发峰值和真正完成多批任务的worker数。result.created_at为submit入口记录时间，接续间隔包含提交内部处理，不应称为纯空等；两路完整阶段计时口径不变。
- 2026-10-29香港时间15:29:01激活最终程序，并逐字节确认编译候选与安装文件、技能与角色副本相同。前版和原始版备份均保留，未终止现有进程。证据见 activation.json。
- 已按用户要求向深度学习书聊天发送具体续建指令：原invocation/计划、full、Pass2 disabled、上限8；稳定worker ownership跨普通提交和WAIT保留，空槽约30秒refill，新child用新鲜连接。
- 深度学习书15:28:10观察：主控181/732，24个注册worker中6个已完成多批任务、累计完成26份assignment。该聊天早先已使用第一版持续领取；此次是补齐等待与并发settled处理，不应把此前所有耗时当作旧批次实现。
- 目标聊天已明确确认采用最终新版继续。15:35:15观察为186/732，完成多批任务的worker增至8个、累计完成assignment33份。首个窗口含新旧连接交接和2个新增可重试失败，尚不报告稳定加速倍数。

## 责任边界

- scheduler worker：driver/refill、worker 状态及领取、调度测试。
- executor worker：执行会话/MCP 接入及连接测试。
- dynamic test worker：真实动态 BookStructure 路由回归。
- benchmark worker：隔离数据、基线程序和测量工具，不修改原书。
- root：协议发布副本、整体验证、实际接入、原生执行器调度、记录和交付。

## 当前接续

深度学习书的原聊天继续拥有其构建和执行器，本聊天不并行修改该书控制检查点或分配任务。用 `observe-live-book.mjs` 读取无正文的时间、计量和worker记录，保存安装前后观察。完整书籍由原聊天沿原授权继续；吞吐结论以实际新增样本为准，不补跑独立书籍对照。
