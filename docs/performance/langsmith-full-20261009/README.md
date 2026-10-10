# LangSmith 完整输入输出

2026-10-09。用户要求开启网站追踪输入输出。已于 16:39:35（Asia/Hong_Kong）上线，正式网站对话与 LangSmith 云端读回通过。

## 实现

- 增加 full 模式；部署已把既有服务的 UB_OBSERVABILITY_MODE 从 metadata 改为 full，沿用 understand-book-dev 项目。
- 根：用户输入和对话上下文、最终回答／错误；模型：实际 HTTP 请求体、完整响应、解析后的结果／错误；工具：参数、返回结果及校验拒绝。
- 内容回调只转发给完整观测接收端，不进入前端活动事件。认证信息不在请求正文通路中。
- 完整内容使用原异步队列与 spool；适配长请求容量。终态导出后释放活动内容，后续证据元数据补写不会抹掉 inputs/outputs。

## 验证

- Runtime 484 项通过，3 项显式忽略。
- Server 观测 32 项通过，1 项专用云环境用例显式忽略；正式网站验收对话已完成云端读回。
- 正式多用户入口验证 metadata/full × 成功/模型失败/保存失败六种组合：完整模型请求与 HTTP 接收内容一致；根、模型、工具均保留内容；元数据模式不发送正文。
- 回归覆盖超过旧 64 KiB 限制的请求、工具拒绝以及后续元数据更新。

## 发布与回退

最终 Linux release 构建成功（4 分 52 秒）。服务从 PID392043 切换至 PID393524，实际运行程序与候选一致，切换前没有执行中或未保存对话。公开与认证 7 项检查通过，书库 9 本及制作能力保持。服务器验收目录 `/opt/understand-book/acceptance/langsmith-full-20261009`，其中 backup 保留本次切换前的两个二进制及配置。失败时恢复两个程序和配置，再重启 understand-book-multi。

## 已知限制

开启后新产生的追踪开始记录内容；此前只有元数据的历史追踪不会自动补齐。完整模式仍受异步队列容量限制，丢弃会计入观测状态。

## 云端证据

正式网站验收于 16:39:53 开始、16:40:01 完成：1 个根、2 个模型、1 个 book.structure 工具节点，云端全部有非空输入输出；模型保存实际请求、完整响应及解析结果，工具保存 arguments 和 result。根状态 success。

[LangSmith 实际追踪](https://smith.langchain.com/o/491b6259-365d-4169-b798-04d6e92643d7/projects/p/cbc2d6bb-40f1-4aa2-b7d1-4c7a98fd6d40/r/01a11fd1-9d73-7c33-b244-12bbbfed2afb?trace_id=01a11fd1-9d73-7c33-b244-12bbbfed2afb&start_time=2026-10-09T08:39:53.459525)；[云端验证](cloud-verification.json)、[发布回执](deployment.json)、[站点检查](site-after.json)、[测试结果](tests.json)。
