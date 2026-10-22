# chart-reveal@3 候选验证记录

2026-10-22，本地完成；没有部署、提交、付费模型调用。此记录只证明候选示范的计算、交互和实际渲染，留出题收益由父任务另行判定。

## 候选范围

新增 skills/presentation/methods/chart-reveal/v3 下六个文件：card.md、counterexamples.md、checks.md、example/index.html、example/README.md、example/author.json。旧版本、catalog 和全局 prompt 均未改。

卡片按页面任务区分计数机制、同量纲比较、组成、完整推导和查表。原 chart-reveal@2 的正值／零值／负值数量比较与 part-whole@2 继续可读。示范使用独立教学模型，将位置、配对、输出三种对象及汇总联动；原数、公式、条件可展开。

## 原页证据修正

核对父任务提供的原页 reconstructed HTML，第 4 节已有 query–key 配对图及 S/P/B 控件；第 5 节表格下明确解释 m、N_pair、m_out 在两模式间怎样变化。候选反例据此限定为“汇总处与前文对象图的联动／衔接不足”，不把原页说成完全没有解释，也不把未读取方法的产出归因为旧模板失效。

具体 trace ID 与原始文件由 ../trace/ 保存，未放入共享方法正文。候选作者没有读取 heldout/，未获取留出题内容。

## 实际检查

执行 node tmp/rsi-author-transfer-20261022/candidate/verify-candidate.mjs，使用生产 presentationDocument 构造的 opaque iframe，而不是仅在普通 HTML 页面中注入假的状态桥。

- 4 个输入 prefill：4 个新投影位置、10 个有效格、6 个未来格、1 个输出；4 个缓存加 1 个新位置 decode：1 个新投影位置、5 个有效格、1 个输出。
- 8192 prefill：8192／33558528／1；8192 缓存后的 decode：1／8193／1。图中明确省略、非比例标注可见。
- 以上四种状态分别在 960×720、320×420、640×240 检查；模式来回切换恢复同样对象和文字。
- 三种视口均通过 Reader 保存重开、展开区恢复与静态导出；快照和导出不改变状态。
- 无横向溢出、按钮/select 最小边长达到 44px、键盘 Enter 切换模式；窄屏阅读顺序为 01→02→03；没有脚本错误和外部请求。

完整机器结果为 verification.json。已直接查看 candidate-960-prefill.png、candidate-960-prefill-scene.png、candidate-960-decode.png、candidate-320-prefill.png、candidate-320-matrix.png、candidate-640-prefill.png、candidate-640-matrix.png。

首次截图检查发现移动端“01”被换行，以及桌面配对格过大。已分别增加序号不收缩和矩阵宽度约束，并对改后作品重新执行同一检查。最终截图中文、标签、对象、图例和结论可读；短屏允许自然纵向滚动，不强行把整课塞进一屏。

2026-10-22 候选冻结，交由父任务选择是否进入隔离 B 构建及后续采用。没有宣称首次生成质量或真实学习效果已改善。

## 首屏布局修正与最终冻结

最终只调整 index.html 的标题区、场景提示排列和纵向间距，未改计算、状态恢复或导出逻辑。去掉重复的教学模型眉题，将场景问题与简短提示在宽屏并排呈现，保留正文和说明字号。针对这一布局变化执行 verify-layout.mjs：960×720、320×420、640×240 三个视口全部通过，无横向溢出，控件仍满足 44px 触控边长。

960×720 的默认首屏同时可见 4 个输入对象、新投影 4 行、10 个有效配对、最后位置的词表投影 1 行；求和式底部为 606.88px，总括结论底部为 695.70px。320×420 的首屏可见完整 4 个输入对象，其底部为 347.66px；配对与输出沿 01→02→03 路径继续滚动。短屏不压缩成难读小字。

直接查看改后 candidate-960-prefill.png、candidate-320-prefill.png、candidate-960-decode.png、candidate-320-matrix.png、candidate-640-matrix.png，确认文字、对象、配对和结论清楚。layout-verification.json 保存此次布局检查；既有 verification.json 的数学、恢复和导出检查不重复执行。

最终候选已冻结，可复制进入隔离 B 构建。正式采用与首次生成收益由父任务的新旧对照决定。
