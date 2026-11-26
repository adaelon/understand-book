# 学习材料与预构建数据

下载原文和已生成的阅读数据，解压后即可在 Understand Book 中打开。资料包发布在本仓库的 [Learning materials · 2026-11-26](https://github.com/adaelon/understand-book/releases/tag/learning-materials-2026-11-26) 与 [Learning materials · 2026-11-16](https://github.com/adaelon/understand-book/releases/tag/learning-materials-2026-11-16)，每份 ZIP 都带有使用说明。

## 下载

| 材料 | 下载 | 内容 |
| --- | --- | --- |
| 复旦统计（校准版） | [fudan-statistics.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-26/fudan-statistics.zip) | 18 节医学统计学整理原稿、153 张配图；Markdown 原稿及图谱、篇章、公式语义、数学结构与书籍结构预构建 |
| 最优化方法 I：复习讲义与历年考试题 | [optimization-review.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/optimization-review.zip) | 19 章讲义、精选课后习题、5 套历年试题与参考解答；Markdown、完整课后习题版 PDF，以及图谱、篇章、公式和书籍结构 |
| 贝多芬第九交响曲：全部四个乐章 | [beethoven-symphony-9.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/beethoven-symphony-9.zip) | Tennstedt 版本音频、MusicXML、分页 SVG 乐谱、小节定位、音乐事件、讲解与示范音频；四个乐章各有独立预构建目录 |
| 深入理解 AI Infra：量化分析与系统设计 | [ai-infra-book.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/ai-infra-book.zip) | 李博杰著，前言与第 1—12 章、482 张插图、Apache-2.0 许可；离线 Markdown 原书及图谱、篇章、公式、书籍结构 |
| A Survey of Agent Memory in the Second Half: Towards Self-Evolving and Long-Horizon Agents | [agent-memory-paper.zip](https://github.com/adaelon/understand-book/releases/download/learning-materials-2026-11-16/agent-memory-paper.zip) | 原论文 PDF、正文、图谱、篇章、公式、书籍结构、论文元数据、术语表、图片和 PDF 定位数据 |

## 打开方法

只阅读原书时，最优化与 AI Infra 的正文位于各包的 `original/`；论文原 PDF 位于 `agent-memory-paper/prebuilt/original.pdf`。音乐音频与乐谱位于各乐章的 `prebuilt/m1/` 至 `prebuilt/m4/` 中。

使用[本地阅读器](../apps/local-desktop/README.md)时，选择已有书库，打开对应的预构建目录：

| 材料 | 解压后的预构建目录 |
| --- | --- |
| 复旦统计（校准版） | `fudan-statistics/prebuilt` |
| 最优化讲义 | `optimization-review/prebuilt` |
| AI Infra | `ai-infra-book/prebuilt` |
| Agent Memory 论文 | `agent-memory-paper/prebuilt` |
| 贝九第一至第四乐章 | `beethoven-symphony-9/prebuilt/m1`、`m2`、`m3`、`m4` |

也可以按仓库根目录的 [Quick Start](../README.md#quick-start) 准备开发环境，然后将目录传给服务。例如，解压到仓库的 `downloads/` 下时：

```powershell
cargo run -p reader-service --bin reader -- --reader-only downloads/optimization-review/prebuilt
```

阅读现成材料无需重新运行预构建。AI 问答和教学使用阅读器中配置的模型接口。

## 材料来源

- **复旦统计（校准版）**：整理校准的医学统计学课程讲义原稿与 153 张配图，随包保留原稿与预构建产物。
- **最优化讲义**：项目内整理的复习材料。可编辑章节、题目来源和制作说明见[讲义目录](../examples/optimization-practice/course-book/README.md)。参考解答为项目整理的推导，非官方评分答案。
- **AI Infra**：李博杰的 [ai-infra-book](https://github.com/bojieli/ai-infra-book/tree/115c8638d54bdc2e1afa00f44d438e5fdb04826e)，离线合并版保持原书内容与插图，原许可证随包提供。
- **贝多芬第九交响曲**：用户提供的 Tennstedt 版本音轨与既有谱面归档。乐谱源为 OpenScore CC0 版本，修订归档及生成谱面的第三方说明随包保留；录音权利独立于乐谱及仓库代码许可证。
- **Agent Memory 论文**：论文 PDF、作者信息、正文来源及元数据随包保留，原文中的引用继续有效。项目的代码许可证不替代原论文的权利声明。

## 预构建范围

这些包使用已发布的阅读或音乐产物，保留原文、资源文件和发布清单中的引用关系。

论文包已包含原 PDF 与各项阅读预构建数据；新版自动教学所需的 `formal_objects`、`cognitive_materials` 和 `teaching_publish` 尚未生成。贝九提供小节级定位，逐音符对齐仍未完成独立验收。具体能力与原发布状态保存在各预构建目录的 `publication.json` 中。
