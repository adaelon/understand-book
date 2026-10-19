# 演示执行环境恢复与终答修复

## 故障

2026-10-13 18:14:59–18:19:53（Asia/Hong_Kong），用户请求第二章整体架构心智模型及演示页。LangSmith root 为 `01a134c2-1e8f-77e2-b87c-77c97ad9ef16`。两次 `presentation.author/write` 均直接返回 `PRESENTATION_SANDBOX_UNAVAILABLE`，未生成或交付页面。最终采样已解释环境不可用并提供文字说明，Runtime 因页面未交付将全文替换为固定未完成提示，warning 变成 `AGENT_NO_PROGRESS`。

根因链：

1. 10 月 12 日的 MCP 恢复演练以 root 启动副本，沿用生产 sandbox `instance=reader`。数据根、端口虽不同，执行器临时目录与 systemd 名称仍共享。
2. `/var/tmp/ub-render-reader` 被创建为 root:root、0755。生产进程使用 understand-book 账户，不能在 sticky `/var/tmp` 下删除该目录，启动回收失败，执行器保持 `probe_failed`。
3. 网站 capability 正确报告不可用，但 Runtime 工具暴露未接入该状态，仍让模型发现和使用制作工具。
4. 通用交付缺口保护覆盖了有用的失败说明，最终只剩 `AGENT_NO_PROGRESS`。发布验收未检查制作 capability，未发现这个状态。

## 修复

- 确认无在途／未保存运行、无生产执行器子任务且临时目录为空后，修正目录归属并重启。实际 capability 恢复为四项 true、`reason=ready`。
- NetworkRunPort 向 Runtime 提供实际不可用原因。工具发现及每次采样同步状态，不可用时隐藏制作工具并说明宿主故障。
- 页面仍未交付时保持 incomplete；已知环境故障下保留经过原有答案编译／来源校验的模型终答，warning 使用 `PRESENTATION_SANDBOX_UNAVAILABLE`。不更改一般的“文字不能代替页面交付”规则。
- 历史 MCP 演练启动脚本分别写入独立实例名。两份运行单明确恢复副本和集成测试也必须隔离执行器；生产发布冒烟新增制作能力门槛。

## 验证

| 验证 | 结果 |
|---|---|
| 三条新增 Runtime 回归 | 修复前分别复现错误激活、终答丢失和无效重试；修复后全部通过 |
| 相关 Runtime 回归 | 59 项通过，覆盖发现、制作、交付与未交付保护，包含上述三条新增回归 |
| Server 回归 | 4 项通过，覆盖缺失执行器、无效配置、输出和私人配额 |
| 发布制作能力门槛 | ready、probe_failed／缺失、部分不可用三项通过 |
| Linux 实际渲染 | 同一发布程序、understand-book 账户、独立恢复数据根及 `recovery-e2e-20261013` 实例；真实绘图、写页、320×420／640×240／960×720 预览、交付和终答完成，`incomplete=false` |
| 公网部署验收 | 首页 200，登录、10 本授权书籍、额度读取正常；制作四项 true、`reason=ready` |

实际渲染测试通过本地确定性 Provider 驱动完整 HTTP 运行路径，9 次脚本化响应，不请求外部模型、不写生产聊天。报告见 [renderer-e2e.json](renderer-e2e.json)。本机相关回归共 66 项通过。

## 部署

2026-10-13 19:51:17 HKT 公网复核通过。现网 release 为 `/opt/understand-book/releases/presentation-recovery-20261013`，服务 PID 6519。数据根仍为 `/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 12，不迁移用户数据；当前品牌与阅读前端静态资源保持。

切换前确认没有在途或未保存运行；短暂维护期间更新服务路径和 worker 路径，启动后校验实际可执行文件与制作能力，再开放公网。旧 release `to-tb-20261012` 保留，三个配置原件在服务器验收目录的 `previous-configuration`。公网原始 Python 请求首页得到 403，既有验收客户端请求成功；不是服务或资源读取失败。最终回执见 [deployment.json](deployment.json)。

## 验证期间的主机中断

发布二进制已成功编译；随后在 3.7GB、无 swap 的线上主机上无内存限额编译大型 Rust 测试，主机失去响应，用户重启后恢复。本次没有取得重启前的内核日志，不能确认是否 OOM；在生产主机上这样执行重型测试编译是不当操作。

重启后，1.5GB 限额的测试编译持续内存回收，已停止，未将该 Linux 单体测试记为通过。改用已编好的候选程序执行上述真实 HTTP 渲染验收，完成后停止隔离服务。后续大型测试在构建环境完成，不在低内存生产主机上无约束编译。

本机原始日志在忽略目录 `tmp/demo-no-progress-20261013`；服务器构建与部署记录在 `/opt/understand-book/acceptance/presentation-recovery-20261013`。原始 LangSmith 输入含用户正文，仅保留在私人本机证据目录，不纳入公开文档。

## 已知边界

修复不会追溯替换历史聊天终答，也不会把失败轮次改成已完成。原任务需要继续执行才能补交演示页。此次未重新调用真实模型生成原用户的第二章内容；动画仅验证 capability，真实渲染验收执行了绘图与浏览器预览。
