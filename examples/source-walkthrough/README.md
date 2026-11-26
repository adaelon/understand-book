# Markdown 怎样变成可引用的段落？

用浏览器打开 [交互导读](index.html)。右边是执行流程和当前步骤的详解，左边是该版本的真实源码。试着点击「识别标题」「识别正文」和「分配段落编号」，观察文件、行号和解释怎样变化。手机上点节点会进入对应源码，点「流程图」可以继续选步骤。无需启动服务。

例子取自 Understand Book 的 Markdown 导入路径。短文的一级标题是「光」，正文是「光会反射。」。导读追踪整章 `1`、标题 `1.1` 和正文 `1.2` 的原文范围。页面内嵌所用的三份文件。

## 在其他项目使用

将本仓库的 [source-walkthrough Skill](../../.agents/skills/source-walkthrough/SKILL.md) 交给 Agent，给出目标仓库与想学的操作。例如「把这个项目从收到请求到写入队列的代码做成可点击的源码导读」。Agent 阅读实际源码、编写节点与解释，生成器从磁盘提取原文件并检验行号。

## 重建与验证

在本仓库根目录用 Node.js 重建示例。修改讲解内容时编辑 [导读稿](tour.json)。

```powershell
node .agents/skills/source-walkthrough/scripts/build.mjs . examples/source-walkthrough/tour.json examples/source-walkthrough/index.html
```

装好项目依赖和本机 Edge 后，运行真实浏览器走查。可选的目录参数用于保存桌面与手机截图，省略时写入系统临时目录。

```powershell
node examples/source-walkthrough/verify.mjs
```

### 实测记录

2026-11-19 在 Edge 离线打开生成的 HTML。桌面 1440 × 900 下走完六个步骤，逐行比对三份真实源码，验证了分支切换、前后步和刷新后恢复当前节点。手机 390 × 844 下从流程图进入标题代码，再返回同一分支，长代码按屏宽换行。两种视口均未产生外部网络请求或脚本异常。该次截图保存在本机 `docs/evidence/source-walkthrough-20261119/`。
