# 《西方经济学（宏观部分·第八版）》插图恢复

2026-10-27 18:26:17 HKT 已将全部 197 张图片补齐至[正式阅读器](https://understandbook.top/index.html)，18:29:25 HKT 公网全量验证通过，随后完成实际阅读页面抽查。缺失数为 0，原发布 ID、读者授权和正文阅读位置标识保持。

## 根因与处理

校订 Markdown 引用同目录的 `assets/`，但该目录没有随正文保存；构建和首次发布因此把全部 197 处图片登记为 `missing`。原始 OCR 稿保留 200 个可访问的图片地址，校订稿沿用其中 193 个编号，另有 4 张整图引用。

- 按原始 OCR 图片序号取回 193 张原图，写入 `E:/allwork/download/assets/`，文件名与既有正文引用一致。
- 从本地同版 371 页扫描 PDF 提取 4 张完整图，保留图框、坐标、分图标记及图注。
- 调用现有 `packages/core/src/asset-manifest.ts:buildAssetManifest`，使用原 `base.json` 中的阅读位置重建图片清单和打包图片。未重新分段或调用模型。
- 正文、base、阅读位置、图注和原始引用逐项保持一致；书籍结构、篇章分析和公式语义继续复用。

| 补图 | PDF 页码（从 1 开始） | 原书页码 |
| --- | ---: | ---: |
| 表 15-3 AD—AS 模型完整表 | 127 | 112 |
| 图 19-3 经济增长率分布 | 240 | 225 |
| 图 21-4 新凯恩斯主义短期总供给曲线推导 | 317 | 302 |
| 微观、宏观学习中心二维码 | 369 | 无 |

## 验证与交付

197 张文件均完整解码；原始 OCR 图片的尺寸与来源地址中的裁图范围一致；4 张 PDF 补图逐张目视确认。清单经项目现有 schema 验证，197 个图片位置、图注和来源引用与修复前一致，所有打包文件与源图片逐字节一致。

本地书籍目录为 `.understand-book/western-economics-macro-8e/`。完整图文包位于 `output/macro-image-recovery-20261027/西方经济学_宏观部分_第八版_完整图文包.zip`，包含 1 份原校订 Markdown 和 197 张图片。解压后保留正文与 `assets/` 的相对位置即可使用。

验证记录见 [verification.json](verification.json)。原图片清单备份、取图记录、PDF 裁图页码和恢复脚本保留在 `tmp/macro-image-recovery-20261027/`。

## 网站同步与验证

现网原生发布器会将新增图片判定为附件内容变更，不能向同一书籍直接重复导入。本次先以现网程序在独立目录成功导入完整补图包，再执行一次限定范围的数据恢复：停服并取得服务锁，向既有发布补入图片，替换图片清单和发布清单，同步该发布在控制库中的清单记录。这是对首次发布漏带资源的一次修复，不改变日常发布规则。

- 书籍：`western-economics-macro-8e`；发布：`01a17776-6474-7bc3-92b9-99da9ea5ee6c`，继续作为默认版本。已有书架和工作区引用可直接读取修复后的资源。
- 193 张原图和 4 张 PDF 补图全部为 `available`；正文、base、结构、语义成果和 197 组阅读位置／图注／原引用保持。没有应用程序发布或模型请求。
- 普通读者通过正式 HTTPS 入口逐张请求 197 张图片：197 个 HTTP 200，类型正确，返回内容逐字节等于本地已完整解码的图片，缺失 0。目录 6707 个节点，Tutor 为 `ready`。
- 实际阅读器抽查封面第一张图、消费曲线及全部 4 张 PDF 补图。封面缺图提示已消失；4 张补图均能展开，图框、表格三行、三分图及图注完整显示。浏览器检查为抽查；197 张全量检查覆盖线上文件可取回和内容一致性。
- 服务保持 `registration-gift-20261027`，验证时 active、PID 317538。成功修复的维护窗口 9.54 秒。

第一次修复尝试的启动探测早于监听端口就绪，已自动回退清单与控制库记录。为探测增加有限次数的启动就绪等待后，第二次成功；两次均未改动用户阅读数据和授权。第一次遗留的未被清单引用的图片在第二次逐字节核对后复用。

证据：[隔离导入检查](preflight.json)、[部署回执](deployment.json)、[197 张图片的公网逐项验证](online-verification.json)、[页面抽查记录](visual-verification.json)。

页面实景：[封面与扉页](online-frontmatter.png)、[消费曲线](online-consumption-curve.png)、[表 15-3](online-table-15-3.png)、[图 19-3](online-figure-19-3.png)、[图 21-4](online-figure-21-4.png)、[学习中心](online-learning-center.png)。验收读者会话已退出。

## 恢复位置

远端恢复目录为 `/opt/understand-book/acceptance/macro-image-recovery-20261027/`，其中 `before-repair/` 保留第一次操作前的发布目录、控制库和入口配置，`before-repair-retry/` 为成功修复前备份。原发布位于 `/opt/understand-book/data/multi-reader-to-tb-20261012/library/published/western-economics-macro-8e/01a17776-6474-7bc3-92b9-99da9ea5ee6c/`。若需回退，仅回退该发布的两份清单及 `book_publications` 对应行；不得用旧整库覆盖后续用户数据。同步与回退脚本保留在 `tmp/macro-image-recovery-20261027/remote_repair.py`。

## 已知限制

- 恢复的是原扫描/OCR 图片质量，图 19-3 的频数条本身较浅，未改造原图数据。
