# SESSION_CHECKPOINT — 2026-10-16，最近改动已上线

## 新鲜度

- 工作树基线 commit：402d9be；本轮没有提交或推送。保留其他工作线未提交改动。
- 用户最新要求：把最近尚未上线的改动上线；中断后明确要求继续。

## 当前状态

2026-10-16 16:02 HKT 已发布 `current-20261016` 至 https://understandbook.top/ 。服务 PID64600、active，数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012`、schema14沿用。主服务、演示worker、Nginx静态目录均已切换。旧版本 `context-cache-20261015` 保留，完整备份与旧配置可恢复。无需再次部署本轮改动。

## 已发布

- 模板目录 revision4：discrete-update@3、原其余七项@2、spatial-linked-views@1（齿轮/弹簧）。完整源文件、Author元数据、按需读取已进入正式程序。
- 三维托管库、交互与图解联动、现场恢复与静态输出，演示版本/反馈/案例支持。
- 跨书补讲CB0–CB5、私人理解、引用与原任务接续。
- 当日对话压缩校验修复；原上下文缓存和命令行热发布保留。
- 模板库：https://understandbook.top/demos/methods/
- 完整齿轮：https://understandbook.top/demos/gears/
- 弹簧：https://understandbook.top/demos/methods/spatial-linked-views/v1/preview.html?topic=spring

## 验收

- 前端相关127项和正式构建通过；Linux Runtime611通过、8默认忽略。
- Server修正集成测试后并行789通过；6项逐项复验通过（5个本机并发桩超时、1个旧错误码断言），总795通过、77默认忽略。
- 测试修正仅涉及工具示例数量、独立临时数据库、MCP下一步调用投影、当前模板样例与既有输入/公开来源合同；不改变生产执行代码。
- Linux隔离数据副本、真实服务用户与独立sandbox，受控Provider完成模板发现→读取→写入→3种WebGL预览→交付→终答；外部模型调用0次。
- 上线前后与公网reader11项/admin12项授权版本完全相同；目录、正文、私有书隔离、四项制作能力正常；发布socket0600。
- 停服至本机恢复验收11.88秒，整个部署脚本48.86秒。完整数据备份333MiB。
- 公网浏览器验收及截图：docs/performance/current-release-20261016/public-browser.json。

## 恢复与记录

- 记录：docs/performance/current-release-20261016/README.md、deployment.json、validation.json、renderer-e2e.json、build.log、test.log。
- 数据备份：/opt/understand-book/backups/current-20261016。
- 旧配置：/opt/understand-book/acceptance/current-20261016/previous-configuration/。
- 恢复程序时沿用最新schema14数据，避免回滚上线后新增对话；流程见发布记录。
- 发布脚本：tmp/current-release-20261016；之前模板验收：docs/performance/presentation-method-library-v2 和 presentation-spatial-template-v1。

## 下一步与边界

本轮部署已完成，后续按用户新要求推进。SE8及FP7实验继续暂停，没有付费模型或自主迁移新主题质量评测，也没有提交或推送。发布后磁盘可用约1.9GiB，保留历史release与恢复副本。
