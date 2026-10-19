# 运行与改写：整体与部分保持对应

直接打开 index.html 即可操作，无外部依赖。正式 Author 按顺序读取 card.md、完整 HTML、本说明、author.json；HTML 作为 write.html，其余内容字段以 author.json 为起点重写，libraries 和 asset_refs 均为空。声明实际读过的 part-whole@2，三环境 preview 后 deliver。

## 本例的关系

条形拆分、100 单位格、整体比例与剩余比例。q∈[0,1] 表示从一条连续整体展开成三行的程度；不改变任何份额。

水平拖动改变 A，保留当前展开程度；百格图按行列定位 A 的数量。

## Agent 改写入口

完整 HTML 保留两个清晰区域：共同 createDemo 与主题 createDemo({...})。新主题先重写 derive 的分配规则、条形位置/长度、百格填充和百分比的分母。 再同步改写题目、操作提示、三个观察预设、讲解段落和模型范围。

- derive(state) 是纯函数，从参数和位置一次计算完整结果；不读取 DOM，不累加上一帧。
- drawMain(model,state) 与 drawDetail(model,state) 只投影这份结果为 SVG。保持身份颜色与数值单位一致。
- metrics、explain、detailReading 与 positionLabel 将同一个结果写成可读 DOM；读者与 Agent 均可在暂停时读到当前结论。
- pointer、detailPointer、key 把直接操作映射为 {values,q}，与滑杆、预设共用 apply/seek。修改对象时判断是改变模型参数，还是只改变观察选择。
- parameters 定义原生控件；defaults 中的每个标量都进入保存/恢复。新增参数必须同时更新 author.json 的 initial_state、state_contract 和可读说明。

## 状态、播放与导出

共同状态只有主题标量、semantic_state、transition_progress 和临时 playing。seek 任意位置都暂停并从模型重画，终点进位；参数控件修改后暂停并回起点，预设使用各自指定的位置。重置只回起点，保留参数。选择、高亮、平移等直接操作的行为以本例说明为准。

Reader 的 registerStateReader 返回标量与当前步骤；registerStateRestorer 从 saved.values.page 恢复并暂停。原生控件输入、操作结束调用 commitState，不逐帧提交。动画仅在可见且页面激活时推进；隐藏后恢复不跳过时间。图解使用 SVG，静态输出保留矢量图与同一现场的文字。

本地维护源为 topic.js 和 ../../_shared/linked-v1.js、linked-v1.css；仓库根执行 node scripts/build-linked-methods.mjs 生成全部八个 HTML 和元数据。Agent 使用完整 HTML 时无需该构建环境。

## 核对新主题

[预期观察](../checks.md) 给出本例独立公式。改公式后重写对应预期，实际检查主图坐标和细节图，不能仅比较同一个 derive 的两次输出。检查手机、短屏、桌面中的直接操作、反向定位、正式恢复与静态输出；正式浏览器入口是 node scripts/verify-linked-methods.mjs part-whole。

## 已知限制

本作品证明当前模型、图形和交互的确定性对应，尚不代表 Agent 已独立迁移任意主题或读者学习效果已被验证。新任务需要重新建立材料依据与解释范围。
