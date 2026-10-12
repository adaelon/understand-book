# TC5 — 建议宿主与低门槛纠偏

2026-10-11；**TC5 已完成，未提交、未部署。** 沿已确认的 ADR-0163 §4–§7 和 TC5 范围；HEAD `12b765b`，保留 TC0–TC4 及其他在途修改。

## 实施步骤

1. [x] 单一建议宿主读取原交付投影，接通可见选项展示回执、原 head 选择与 consumed 定位。
2. [x] 整段／片段纠偏、演示现场、原题帮助及两宿主原请求恢复；草稿独立保留。
3. [x] 组件／真实 API、键盘／移动端与真实材料纠偏验证；同步代码链路、架构和 checkpoint。

## 验证目的

组件测试检测不可见曝光、重复按钮、错误 head、场景换绑和草稿丢失；失败则修正对应入口。服务端测试检测展示事实越界、未知动作重复接纳及原题帮助条件遗漏；失败则修正原协议。浏览器验证实际操作与小屏布局；真实材料验证纠偏是否改变讲法，失败保留原记录并定位指导或输入断点。

## 实现与操作边界

- `TutorContinuation` 从原 `/tutor/activities` 读取已持久交付的集合；读取入口依据持久教学关联或现有教学轨迹，不从 Markdown 猜按钮。普通宿主读取无演示引用的集合，演示宿主读取同一版本，避免重复操作与曝光。
- 推荐项在前、备选在后，并展示各自用途；真正进入可见区域的选项才提交 `ContinuationDisplayed`。服务端校验公开选项、顺序子集及推荐标识，按交付与子集顺序幂等保存。回执失败不会阻断合法选择。
- 选择保留原 delivery／option／head；演示先保存同版本不可变现场。双击、保存期间暂停／换版本／新 head 不会发出第二个选择。旧建议只读，已消费集合可以定位原接纳回合。
- 原回答及连续片段可直接“这一步没跟上”；摘录复用现有回答源映射，携带 `answer_ref` 和点击时当前 head。`chatTurnFromHistory` 保留解析输入，界面可查看“接续的原回答”并返回原处。自由改问不继承 choice；草稿不随按钮动作清空或发送。
- 原题分流继续定位原提示／揭示；无这两项帮助时明确说明。界面定位不写 HelpDisplayed，实际提示、揭示、作答仍沿 TutorActivities 原回执。
- 本地标签页持久保存类型化原请求；刷新后先调用只查重／补事实的 `reading-action-status`，用户显式重试才提交原载荷。多人继续使用 PendingSubmission。两边遇到未核对请求都先保留自由草稿，禁止按新身份盲重提。

## 确定性验证

| 验证 | 结果 | 证据 |
|---|---|---|
| TC5 展示子集、查询不执行、多人展示授权 | 3 通过 | [server.log](tutor-continuation-tc5/server.log) |
| TC4 接纳恢复＋TC5＋原活动分流／摘录合同 | 16 通过；2 实模／浏览器入口默认忽略并另行执行 | [server-regression.log](tutor-continuation-tc5/server-regression.log) |
| 新宿主、请求恢复、网络客户端、活动及原组件／启动 | 163 项覆盖；最后受影响的 123 项通过，其余 40 项在收口组通过 | [closure.log](tutor-continuation-tc5/closure.log)、[closure-fix.log](tutor-continuation-tc5/closure-fix.log) |
| Markdown／PDF、桌面／390px、键盘、草稿、自由改问、刷新原身份与原题导航 | 17 通过 | [browser-final.log](tutor-continuation-tc5/browser-final.log) |
| 普通回答／演示／多人浏览器到真实 Rust API | 3 通过 | [api-closure.log](tutor-continuation-tc5/api-closure.log) |
| 原回答历史投影可见 | 普通回答／演示两条路径 | [history-projection.log](tutor-continuation-tc5/history-projection.log) |
| Web 类型检查及生产构建 | 通过；既有大 chunk 提示保留 | [build-final.log](tutor-continuation-tc5/build-final.log) |

分组有重叠，不累计为独立总测试数。API 浏览器使用确定性模型替身，实际经过本地 RunCoordinator 或多人授权／工作区／RunAdmissions，使用真实 SQLite／JSONL；页面素材载入使用同源固定夹具。普通与演示各接纳一个选择、保存一条 chosen、一次执行中采样 3 次并完成交付。多人同样只产生一个新回合。网络测试桥使用夹具账号身份，不代替生产登录／TLS 验收。

产物：[普通录屏](tutor-continuation-tc5/api-ordinary.webm)、[演示录屏](tutor-continuation-tc5/api-presentation.webm)；[普通界面](tutor-continuation-tc5/api-ordinary.png)、[演示工作区](tutor-continuation-tc5/api-presentation.png)、[390px](tutor-continuation-tc5/choice-390.png)、[多人界面](tutor-continuation-tc5/api-network.png)。逐路径原请求、接纳回合与事实在 [api 目录](tutor-continuation-tc5/api/)；[多人原请求与历史](tutor-continuation-tc5/api/network.json)保存该路径的接纳和交付记录。

## 真实材料纠偏

模型 `deepseek-v4-flash`；沿 TC3 平均速度材料及 v3 已交付解释，不增加材料范围。先以无补充文字的 `clarify_answer` 指向“平均速度 × 总时间 应当还原出总路程”；再用一句自由输入提出停顿时间障碍。每个场景执行一次，保存原请求、工具调用、公开回答和结果；[配置](tutor-continuation-tc5/live/configuration.json)、[运行日志](tutor-continuation-tc5/live.log)。

| 场景 | 结果 | 模型采样／累计 usage token | 内容观察 |
|---|---|---|---|
| [回答片段求助](tutor-continuation-tc5/live/clarify/answer.md) | completed，incomplete=false | 4／72,231 | 拆成等式变换、逆运算、回代与时段对齐，未要求补写操作指令 |
| [自然纠正](tutor-continuation-tc5/live/redirect/answer.md) | completed，incomplete=false | 6／189,596 | 转向总时长和移动时长的差别，解释停顿为何属于所选行程；未继续执行原两段行程建议 |

本轮真实模型共 10 次采样，provider usage 合计 **261,827 token**。这是累计使用量，不是单次上下文或货币费用。两次都改变解释重点，没有再次要求用户选择原教法。第二次根据用户明确的停顿疑问分析分母含义；没有把纯按钮点击当成具体知识缺陷证据。

## 失败与修正

- 新测试服务程序初次构建误用 TraceStep 字段，已按真实结构修正；[fixture-build.log](tutor-continuation-tc5/fixture-build.log)保留错误。
- API 测试初次从错误层级读取 interaction，并可能在旧回合 completed 时提前结束等待；改从 domain.interaction 取原输入，并等待第二回合终态。[初次记录](tutor-continuation-tc5/api-browser.log)保留。
- 多人测试业务断言通过后有待完成路由遇到测试上下文销毁；结束时先离开页面、再释放路由。中间日志保留于 network-browser*.log，最终三路径通过见 api-closure.log。
- 一律为普通回答读取建议时，来源弹窗回归发现多余请求。入口改为持久教学关联或旧教学轨迹，保留无完整工具轨迹的多人历史支持；两项来源测试恢复，未放宽其请求数断言。
- 历史映射遗漏 domain.interaction 已补齐，并增加真实 API 界面的原回答引用断言。

## 已知限制

真实读者人数仍为 **0**；本片为脚本化真实材料纠偏观察。两次输出仍偏长，不能据此声明整体学习收益或通用教学准确率。TC2 的既有公开内容错误继续保留，TC6 防污染轨迹和 TC7 完整读者闭环尚未完成。

本地原请求恢复限原标签页会话存储；明确关闭标签页后的恢复沿服务端历史。浏览器保留原动作不保证外部 Provider exactly-once。发布、旧二进制兼容与降级演练仍归 TC8，本轮没有上线。
