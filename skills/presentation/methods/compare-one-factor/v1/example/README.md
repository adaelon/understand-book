# 运行与适配

浏览器直接打开 [index.html](index.html)，无需依赖和网络。通过 Author 制作时，将完整 HTML 作为 `write.html`，以 [author.json](author.json) 提供其余内容字段，选择 `state`、`continuous_scene` 指导。实际读取本版本后，在 `method_refs` 中声明 `{"method_id":"compare-one-factor","revision":1}`。三环境 preview 后 deliver。

## 内容与状态

两条轨道从同一起点出发，经过同样的 10 秒。A 固定为 1 m/s，只改变 B 的速度。

显式匀速模型 d=v·t。轨道高度仅用来分组，不表示距离或速度。改变 B 的速度后，时钟回到 0。

初始参数：`{"speed": 2, "semantic_state": 0, "transition_progress": 0}`。参数定义见 author.json。讲解有 1 步，`semantic_state` 记录已完成步，`transition_progress` 记录下一步过渡。参数改变暂停并回到第 0 步；重置保留参数。前后步、拖动、预览 `presentationScene.seek` 和现场恢复共用同一渲染路径。播放状态由 snapshot 返回；恢复总是暂停。

单独打开用于观察，现场持久化由 Reader 负责。示例的显式模型或构造数据已写在页面；真实书籍任务重新填写来源。

## 改写位置

替换关系式、参数和单位；保留共同条件、共用坐标和同步时钟。实际实验需要重新建立控制条件与来源。

源码中的 `DEFAULTS`、`renderAt` 和界面控件组成最小改写面。改变参数含义时同步修改 author.json 的 `initial_state`、`state_contract` 和内容说明。改变模型时重新推导 [预期观察](../checks.md)。本例自包含，不依赖模板库的其他文件。
