# Linux 对话排序与闲置恢复

2026-10-02。

## 行为与根因

历史会话统一按 `updated_at / created_at / id` 倒序排列。原网络接口在尚未选择对话时直接返回存储顺序，漏掉已选中对话分支的排序；两条路径现在共用 `agent_session_summaries`。

问答正文按完整回合从旧到新显示，单轮内部是问题在回答之前。`RightRail` 按原数组顺序渲染，保留、撤销和轨迹操作使用对应回合的 `ti`。靠近底部时跟随最新内容，查看旧内容时保留位置，切换对话重新定位底部。2026-10-03 修正此前误加的正文倒序；历史会话列表仍按最近更新时间倒序排列。

服务端在现场闲置 15 分钟后回收常驻状态，恢复时递增 generation 并清除 attachment。原网页返回时只更新 generation，没有重新 attach；历史重试也没有绑定流程。`recoverWorkspaceBinding` 现在串行执行 GET → attach → install，复用本页写队列并合并重复恢复请求。外层 `NetworkApp` 完成认证与绑定后才通知 Reader 刷新，手动历史重试共用相同绑定过程。

未发送文字、引用和目标任务由 `NetworkApp` 暂存，同账号、现场、发布及对话的 generation 变化保留草稿；换账号、材料或对话清除。只核对原请求状态，恢复流程不提交问题。附属演示窗口继续遵循原主窗口连接边界。

错误响应保留真实分类；`WORKSPACE_STALE` 显示中文恢复提示。

同版本浏览器测试还发现，运行状态与流重建路径返回的是私人存储摘要，而网页需要完整 `AgentChatTurnView`，缺少 `effect_labels` 等字段会导致脚本异常。网络运行回执现在通过 `reader_run_view` 使用原发布内容和已有 `turn_view` 完成展示投影，覆盖接单、状态查询、原请求核对、停止/重试保存和无常驻流时的事件快照；原持久记录不变。

## 2026-10-02 发布验证（正文倒序已于次日纠正）

- 问答正文顺序、按钮原索引、顶部跟随与切换会话：新增失败测试先复现，修复后 `RightRail.test.ts` 27 项通过。
- `network-client.test.ts` 14 项、`App.startup.test.ts` 11 项、`NetworkApp.recovery.test.ts` 与 `NetworkApp.recap.test.ts` 各 1 项通过，覆盖重新绑定、重复恢复合并、草稿、重试及跨账号过期响应。
- 后端 `tests::mu5_tests` 13 项通过；新用例先复现未选中历史顺序不同，以及冷恢复后仅刷新 generation 仍拒绝历史请求、错误分类缺失。
- 已保存运行恢复的展示格式新增失败回归，修复后 `tests::mu8_tests` 5 项通过，浏览器宿主用例按原配置忽略并单独启动。
- `tests::mu6_tests` 45 项通过，进程杀断辅助用例按原配置忽略；覆盖接单、请求键核对、停止、保存失败重试、重启、事件流及多窗口权限边界。
- 当前工作区 Web 类型检查通过。基于实际线上源码单独应用本次补丁的候选网页，类型检查和生产构建通过。
- 同候选 Linux 隔离后端 + 候选网页：1440px、390px 两项真实浏览器测试通过，覆盖两轮问答顺序、断开后自动绑定、手机手动重试、原现场与草稿保留、无重复提交；只使用测试 Provider。
- 生产入口复验通过：读取 6 段历史会话并确认排序，恢复的对话 2 个回合按新到旧显示；桌面/手机模拟连接失效后恢复原历史与未发送文字；脚本错误 0、模型问题提交 0。见[复验回执](performance/linux-chat-recovery-20261002/live-verification.json)。

## 发布

已于 2026-10-02 发布至 `https://115.190.121.150`。以 `/opt/understand-book/releases/mu12-20261001` 实际源码为基线，只应用本次 8 个生产文件的变更；当前版本目录为 `/opt/understand-book/releases/chat-recovery-20261002`，Rust 五个运行入口 release 编译通过。发布前活动或未保存运行数为 0，Nginx 配置校验通过，服务状态 active。

旧版本目录保留，配置备份为 `/opt/understand-book/backups/chat-recovery-20261002`。切换 service、静态网页与演示 worker 路径；私人数据目录和数据库结构保持原有状态。见[发布回执](performance/linux-chat-recovery-20261002/deployment.json)。临时浏览器宿主和本地转发已关闭。

## 2026-10-03 正文顺序纠正

问答正文恢复为旧回合在上、新回合在下，自动跟随底部；向上查看旧内容时保持阅读位置，切换会话重新跟随最新回合。历史会话列表继续按最近更新时间倒序排列。

- `RightRail.test.ts` 两项顺序和跟随回归先失败，修复后全部 27 项通过，包括按钮原回合索引和查看旧内容时的位置保持。
- 以实际线上 `ex13-jl-20261002` 为基线，仅修改 `RightRail.vue`；Linux 类型检查和生产构建通过。发布新增静态资源并原子切换网页入口，旧资源和入口备份保留，服务未重启，已上线的 EX13/JL 功能保留。
- 正式 HTTPS 入口在 1440px 和 390px 下验证两轮正文从旧到新、最新回答定位底部，脚本错误 0、模型问题提交 0。验收账号在所选材料下没有可用历史，因此使用仅在测试浏览器内注入的两轮历史响应，未写入服务器；这不是既有私人历史数据的端到端验收。
- 当前 release：`/opt/understand-book/releases/ex13-jl-20261002`；本次备份：`/opt/understand-book/backups/qa-order-20261003`。见[发布回执](performance/linux-qa-order-20261003/deployment.json)和[浏览器验收](performance/linux-qa-order-20261003/live-verification.json)。

## 已知限制

浏览器旧标签页需要刷新一次加载新版。失效恢复验证由后端时间推进的回收测试与浏览器实际 detach/重新 attach 共同覆盖，没有在浏览器里等待真实 15 分钟。实际浏览器验收为 Chromium 桌面和手机宽度，未替代实体手机测试。
