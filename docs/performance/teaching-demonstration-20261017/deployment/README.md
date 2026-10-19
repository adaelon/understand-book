# 演示教学闭环发布 — 已上线

最终 sampling 14文件候选已于 **2026-10-17 13:33:06 HKT** 发布至 [understandbook.top](https://understandbook.top/)。发布流程和实际公网验收通过，见 [deployment.json](deployment.json)、[部署日志](deployment-run.log) 与 [实际运行记录](post-deployment.json)。

## 当前运行版本

- Release：`/opt/understand-book/releases/teaching-demonstration-20261017`
- 主服务：`understand-book-multi`，PID `129213`，实际进程为该 release 的 `target/release/server`，WorkingDirectory 亦为该 release。
- 演示 worker：该 release 的 `target/release/presentation_worker`。
- Nginx root：该 release 的 `packages/web/dist-multi`。
- 数据：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 发布前后均为14。
- 停服至新本地服务就绪：**15.66秒**。发布和验收没有模型调用。

## 源码与构建

发布快照为 `tmp/teaching-demonstration-release-20261017/frozen-source-sampling/`，归档 `source-sampling.tar.gz`（276,007 bytes），精确14文件清单见 [release-source-sampling.json](release-source-sampling.json)。发布前工作区、冻结快照和实际候选逐字一致，见 [sampling-publish-source-check.json](sampling-publish-source-check.json)。

五个正式程序以 release 配置构建成功，**5分54秒、exit 0**，见 [build-sampling.log](build-sampling.log) 与 [build-sampling.exit](build-sampling.exit)。本次使用 `deploy-sampling.py` 发布。

Web 独立 vue-tsc 与 Vite 构建通过，`VITE_MULTI_USER=1`，279个入口/assets文件已上传，归档15,190,946 bytes。实际 Reader bundle 为 `reader-1dwsBhuV.js`。Web来源及保留既有public/admin分发的范围见 [web-source-provenance.json](web-source-provenance.json)、[web-upload.log](web-upload.log)。

## 验证结果

源站与公网均确认 reader 的11项、admin的12项授权出版物清单在发布前后保持一致，默认书 manifest 与正文可读取，reader受限书返回404。animation、authoring、plot、preview四项演示能力均ready。源站两个入口与候选逐字节一致，公网入口项目JS/CSS引用与资源字节均匹配（首页4项、Reader入口5项，含共享资源）；Cloudflare注入的analytics不属于项目资源。

完整验证清单见 [validation-sampling.json](validation-sampling.json)。runtime完整633 passed/8 ignored、server教学设计14/2、冻结场景2项、公开例子修复后5项、Web20项及构建/typecheck通过。inspection行为117项通过；恢复修复新增2项真实浏览器回归、3项既有preview回归通过；跨轮调用编号compaction41/5、tool_result34及组合构建通过。

sampling压力回归8 passed/1默认ignored，压力集成和显式真实录制回放各1 passed。真实request10逐消息与token估算先匹配原请求，再退役过期快照：含图input tokens由114,487降至50,870，runtime快照9份降至1份，20条原始/冻结消息及18个active模块保留，没有提高预算。

live01、03、08已完成，文字路径可根据学习者表达自然收束。live09第5、6、7轮均实际HTTP200：修订页交付后，学习者自主操作章节按钮、给出关系理由，教师针对解释反馈；真实server state.save/read保存后，使用服务器读回的chapter=true/theme=false重开页面成功。学习者看过重开截图并表达原困惑已消除，教师以144字conclude自然收束。harness正常结束，25次模型请求，进程exit0、1 passed。独立6.1-sol最终内容审阅确认页面观察、用户理由、针对反馈、自然收束成立，未见实质教学内容问题，见 [最终内容审阅](../content-review-final.md)。

## 备份与回滚

完整备份已在无活动或未保存运行时创建：

- 数据备份：`/opt/understand-book/backups/teaching-demonstration-20261017`，**351,425,805 bytes**。
- 上一版程序：`/opt/understand-book/releases/teaching-handoff-20261017`，五个正式程序保留。
- 上一版service、Nginx及sandbox配置：`/opt/understand-book/acceptance/teaching-demonstration-20261017/previous-configuration-final`。
- 远端发布证据：`/opt/understand-book/acceptance/teaching-demonstration-20261017`。

发布脚本包含切换失败时恢复上一版程序及配置的路径；本次所有验收通过，没有触发回滚。schema14和现有数据目录保持不变。

## 历史候选证据

早期候选暴露的实际问题和原始失败证据均保留：首次构建见 [candidate-initial](candidate-initial/)，inspection见 [candidate-inspection](candidate-inspection/)，页面恢复见 [candidate-restore](candidate-restore/)，跨轮调用编号修订见 [candidate-final](candidate-final/)。前版final构建5分55秒/exit0已收回日志；它随后遇到sampling状态堆积，最终由本次压力退役修订解决。实际源码范围见 [sampling-production-scope.diff](sampling-production-scope.diff)。这些旧候选没有单独上线。

## 限制

- 2026-10-17 13:34 HKT发布后可用空间为 **324,124,672 bytes（约309 MiB）**。没有再清理缓存、源码、程序、备份、数据或证据；本轮保留完整备份和所有发布证据。
- 教学内容验收使用模型模拟学习者，支持本次近距离即时应用和已观察页面场景，不能外推为真人长期学习或独立迁移效果。
- 历史TB2固定模型录制使用旧教学合同，不适用于本轮，未改录制，也未计作通过。
