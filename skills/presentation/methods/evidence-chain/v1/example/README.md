# 运行与适配

浏览器直接打开 [index.html](index.html)，无需依赖和网络。通过 Author 制作时，将完整 HTML 作为 `write.html`，以 [author.json](author.json) 提供其余内容字段，选择 `state`、`continuous_scene` 指导。实际读取本版本后，在 `method_refs` 中声明 `{"method_id":"evidence-chain","revision":1}`。三环境 preview 后 deliver。

## 内容与状态

用三组构造的耗时记录练习核对结论：先看记录，再算均值，最后检查比较条件。

实际书籍任务须将记录替换为可回到原文的来源。三次示例测量只能支持这三次记录的描述，不建立总体优势。

初始参数：`{"controlled": true, "semantic_state": 0, "transition_progress": 0}`。参数定义见 author.json。讲解有 3 步，`semantic_state` 记录已完成步，`transition_progress` 记录下一步过渡。参数改变暂停并回到第 0 步；重置保留参数。前后步、拖动、预览 `presentationScene.seek` 和现场恢复共用同一渲染路径。播放状态由 snapshot 返回；恢复总是暂停。

单独打开用于观察，现场持久化由 Reader 负责。示例的显式模型或构造数据已写在页面；真实书籍任务重新填写来源。

## 改写位置

替换原始材料、来源引用、计算与成立条件；正文和 source_ref_ids 一起重新建立，不能保留构造记录当真实证据。

源码中的 `DEFAULTS`、`renderAt` 和界面控件组成最小改写面。改变参数含义时同步修改 author.json 的 `initial_state`、`state_contract` 和内容说明。改变模型时重新推导 [预期观察](../checks.md)。本例自包含，不依赖模板库的其他文件。
