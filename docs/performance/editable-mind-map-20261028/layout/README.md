# 主题排布与导出验证

2026-10-28，Windows 10，AMD Ryzen 7 5800H，Chromium 149.0.7827.55。实现位于 `mind-map-layout.ts`、`mind-map-outline.ts`、`mind-map-export.ts`；Reader、候选预览和导出共用主题文字测量、语义树排布与主题外观规则。

## 输入与实测

从本机 Rust 材料取 500 个不同中文短标签，使用可重复的五孩子层级，测量浏览器真实 Noto Sans SC 字体下的布局。这个层级仅用于规模测试，不代表模型生成的章节提纲或教学质量。1000 主题复用这些标签并加编号，属于容量探索。

| 场景 | 首次测量 | 之后 10 次局部改名的中位数 | 最大值 |
| --- | ---: | ---: | ---: |
| 500 主题 | 241.4 ms | 10.3 ms | 15.4 ms |
| 1000 主题，已预热 500 主题字体宽度 | 23.4 ms | 17.1 ms | 20.7 ms |

字体准备单独耗时 177.4 ms。以上只测主题布局，不含 Vue 更新、绘制、保存或模型等待。500 主题重排达到约 200 ms 的目标；首次冷布局超过该值，不能据此声称首次加载同样达到目标。实际编辑反馈由工作区集成验收另测。

初版每次重新测量所有标签，500 主题重排中位数 160.6 ms、最大 215.3 ms。现有实现复用精确的字体宽度，字体加载变化后清空；换行按已测量词或中文字符累加，最后仍使用完整行的真实测量决定节点宽度。前两轮记录原样保存在本目录；初版记录中的 `first_layout_ms` 已经经过一次预热，与最终记录中的首次冷布局不能直接比较。

## 导出与正确性

16 项针对性测试及 16 项既有 BoardCanvas 回归通过：500 节点无重叠、文字宽度包含、根锚点及左右归属稳定、折叠后隐藏数量与重新展开、自由卡片位置不变、中文及多行/Markdown 字面内容往返、导入的稳定 ID 与先父后子顺序、候选主题作用域、相邻主题的纵向关联标签不与节点重叠、宽度不同的上下主题仍从垂直方向连接。

Chromium 实际生成完整 32 主题图 `rust-full.png`（3378×2600）和原本折叠分支 `rust-branch.png`（1536×952），均直接查看，文字清楚，边缘没有裁切。图中使用的重复 Rust 概念仍是排布夹具，不能作为讲解质量样本。PNG 按完整语义内容展开，选定分支仅包含该分支；不依赖当前视口截图。尺寸超过浏览器可读输出范围时要求选择分支，避免静默缩小字号。

实际跨主题关联导出发现：旧画板将所有关系文字放在连线中点上方 9px，在自动布局的同级间隙中会被上方主题遮挡。共享 `connectionLabelOffsetY` 现在把纵向关系文字放在间隙中央，横向关系保持原位置；Reader 与 PNG 同用。真实 Reader 另测得关系字体边界高 19px，所以所有层级的同级间隔统一为 28px，容纳文字及描边；连线方向先检查两节点在哪个方向分离，避免宽度不同的上下主题误连成横线。修复前后图片保留为 `rust-relations-before-label-fix.png` 和 `rust-relations.png`，修复后直接查看标签、方向箭头均完整。1000 长标签导图超出输出尺寸时，实际返回选择分支的提示，没有生成模糊缩略图。

关联导出回归再次计时时，500 主题首次布局 175.8 ms、重排中位数 8.1 ms；前一轮首次布局 306.3 ms、重排中位数 13.8 ms。工作区同期存在其他构建，冷启动波动仍大，保留各轮原始数据，不用最快一次覆盖首次加载的限制。

Markdown 只保留主题文字和层级，不包含跨主题关系、讲解路线或演示。

候选预览通过真实 `boardPreviewAssets` 打包钩子构建，以 11 个有语义的 Rust 切片主题与一张旧自由图元加载 Chromium。11 个候选主题均有实际 DOM 几何，文字裁切、重叠和页面错误为零；根锚点使用 `(300,160)`，未采用服务端占位坐标，旧图元保留原位置。直接查看 `candidate-preview.png`，层次、左右分支与文字清楚。预览字体过滤现在保留 Noto Sans SC，确保 Linux 预览与 Reader 使用同一字体规则。

真实 `replay-01` 的第二个候选曾因水平树边的 SVG 控件高度为零而被通用预览校验拦住。UI 增加实际可点击矩形后，用同一冻结候选和重新构建的 `dist-multi` 复验：24 个主题全部出现，33 个通用交互控件均有非零边界；两条水平树边的命中范围约为 51.831×44px，文字/重叠问题及页面错误均为零。证据在 `frozen-candidate-2/`。复验没有降低通用预览门槛，已据此放行第二次真实模型回放。

## 重跑

```powershell
pnpm --filter @understand-book/web exec vitest run src/mind-map-layout.test.ts src/mind-map-outline.test.ts src/board-canvas-preview.test.ts
node evals/semantic/mind-map-layout-bench.mjs --source <本机书籍 source.txt> --out tmp/mind-map-layout-bench
node evals/semantic/mind-map-preview-smoke.mjs
node --import tsx evals/semantic/mind-map-preview-smoke.mjs --candidate <冻结候选.json> --renderer packages/web/dist-multi/board-preview --out tmp/mind-map-frozen-candidate
```

不提供 `--source` 时使用已提交的中文 Rust 概念夹具。该运行会生成真实 PNG 并记录环境、字体等待、每次样本及图片尺寸，私有材料正文不会写入报告。
