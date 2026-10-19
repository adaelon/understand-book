# 运行与适配

浏览器直接打开 [index.html](index.html)，无需依赖和网络。通过 Author 制作时，将完整 HTML 作为 `write.html`，以 [author.json](author.json) 提供其余内容字段，选择 `state`、`continuous_scene` 指导。实际读取本版本后，在 `method_refs` 中声明 `{"method_id":"chart-reveal","revision":1}`。三环境 preview 后 deliver。

## 内容与状态

固定坐标和单位，依次展示四批数据，再聚焦首末差异。切换数据可观察负值与零值。

坐标范围固定为 −10 到 30 份。同一组数据共用尺度；起点为负时，只报告首末差值。

初始参数：`{"dataset": "positive", "semantic_state": 0, "transition_progress": 0}`。参数定义见 author.json。讲解有 3 步，`semantic_state` 记录已完成步，`transition_progress` 记录下一步过渡。参数改变暂停并回到第 0 步；重置保留参数。前后步、拖动、预览 `presentationScene.seek` 和现场恢复共用同一渲染路径。播放状态由 snapshot 返回；恢复总是暂停。

单独打开用于观察，现场持久化由 Reader 负责。示例的显式模型或构造数据已写在页面；真实书籍任务重新填写来源。

## 改写位置

替换序列、标签、单位、来源及坐标域。柱形必须保留零基线，负值从零向下。涉及百分比时重新判断分母与解释口径。

源码中的 `DEFAULTS`、`renderAt` 和界面控件组成最小改写面。改变参数含义时同步修改 author.json 的 `initial_state`、`state_contract` 和内容说明。改变模型时重新推导 [预期观察](../checks.md)。本例自包含，不依赖模板库的其他文件。
