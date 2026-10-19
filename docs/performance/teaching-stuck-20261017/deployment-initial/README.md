# 教学闭环初次发布 — 2026-10-17

用户要求先将已完成的教学设计和主动邀请版本上线，再进行连续卡住的多轮模拟。发布对象为此前本地已经验证的版本，后续模拟中修改的源码不进入这个冻结候选。

## 发布方式

现网基线是 `/opt/understand-book/releases/reader-navigation-20261016`，并非更早的 `current-20261016`。候选 `/opt/understand-book/releases/teaching-initiative-20261017` 从现网完整目录复制，按内容差异覆盖教学设计、当前理解视图、参与选择、回答与帮助关联、演示绑定及邀请组件的必要源码和生成类型。归档包含 87 个文件，其中相对现网实际改变 64 个；见 [release-source.json](release-source.json) 和 [source-comparison.json](source-comparison.json)。未纳入无关品牌素材与方法构建脚本。

Web 多人版本使用 `VITE_MULTI_USER=1 pnpm -C packages/web exec vite build` 构建成功，上传 Vite 输出的入口和 assets。既有公开演示库、品牌资源和 admin 分发目录保留现网内容。Linux 在独立候选目录以单任务、1.8 GB 内存限制构建 server、manage_reader、publish_book、reader_maintenance 和 presentation_worker。

发布沿用现有停写流程：确认无活动或未保存运行，进入维护状态，再次确认，停止服务，完整备份数据，再切换主服务、演示 worker 和 Nginx。失败自动恢复旧程序配置。已有本地回归依据见 [validation.json](validation.json)；本次 Linux 构建及实际服务检查单独记录。

## 空间处理

发布前磁盘只余 1.4 GB。仅删除已确认非在用的旧 release 编译中间产物，见 [cache-cleanup.json](cache-cleanup.json)；没有删除旧程序、源码、Web、数据和验收证据。当前现网与紧邻回滚版本的缓存也未触及。准备完整候选后仍余约 2.46 GB。

## 验收目的

本机和公网检查比较 reader/admin 的授权版本列表、默认书目录和正文，以及演示制作能力；能发现程序启动失败、授权书库变化、正文读不到和演示能力不可用。最终逐字节比对源站首页和阅读器入口；公网比较实际引用的项目 JS/CSS 清单，并逐字节比对这些资源，能发现仍返回旧前端或漏传资源。若失败则恢复原程序和配置。发布检查不发送模型请求。

## 上线结果

2026-10-17 **08:36 HKT** 上线成功。`understand-book-multi` 的实际程序来自 `teaching-initiative-20261017`，PID **104288**，worker 和 Nginx 同步切换。数据根仍为 `/opt/understand-book/data/multi-reader-to-tb-20261012`，schema14 未变化。

Linux 五个正式程序构建成功，用时 **10 分 30 秒**；最终成功尝试的停服至本机验收 **10.05 秒**，整次脚本 **48.76 秒**。本机及正式域名的 reader 11 项、admin 12 项授权版本清单前后相同，默认书目录和正文可读，四项演示能力 ready。源站两个入口精确匹配候选；公网实际引用新版 `reader-BbGgozCW.js`、`reader-BRVIARO0.css`，两入口共 9 项引用资源比对通过。发布过程外部模型调用 **0 次**。

完整回执见 [deployment.json](deployment.json)，构建见 [build.log](build.log)，成功日志见 [deploy-success.log](deploy-success.log)。本地发布脚本在 `tmp/teaching-stuck-release-20261017/`，服务端脚本与日志在 `/opt/understand-book/acceptance/teaching-initiative-20261017/`；本目录亦保存了所用脚本。

最终切换前完整备份为 `/opt/understand-book/backups/teaching-initiative-20261017-final`；原配置为 `/opt/understand-book/acceptance/teaching-initiative-20261017/previous-configuration-final`。回退程序仍为完整保留的 `reader-navigation-20261016`。按同一维护流程恢复该 unit、sandbox 和 Nginx，保留 schema14 最新数据；完整备份只用于确有数据损坏时恢复。

## 两次检查失败及恢复

第一次候选程序、本机 API 与公网 API 检查通过，但新增静态入口检查的默认 Python urllib 客户端被网站返回 HTTP 403（1010），脚本自动恢复旧程序和配置。随后对旧版用相同请求重现 403，项目既有验证 User-Agent 则返回 200。见 [deploy-first-attempt.log](deploy-first-attempt.log) 和 [static-request-diagnosis.json](static-request-diagnosis.json)。

第二次以既有 User-Agent 获取页面后，整页字节比较又遇到当前 Cloudflare 自动注入的 analytics beacon，脚本再次恢复旧版。见 [deploy-second-attempt.log](deploy-second-attempt.log)。这是发布检查把 CDN 输出误当成源站原始字节，未发现程序启动或业务 API 回归。

最后使用与失败目标直接对应的检查：源站入口保持精确字节比较；公网检查实际引用的项目资源清单和资源字节，不引入 HTML 规范化。完整修订检查先在旧版通过，见 [static-check-previous.json](static-check-previous.json)，再执行第三次切换成功。前两次原始日志及各自数据、配置备份均保留，未改写成一次成功。

## 验证范围

本记录只确认版本部署、服务数据和入口资源生效；新的连续卡住多轮对话在确认上线之后由主任务另行进行。没有在部署检查中声称真人学习效果或新增对话质量验证。

