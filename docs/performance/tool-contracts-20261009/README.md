# 工具参数契约修复与发布

日期：2026-10-09。状态：已于 16:11:57（Asia/Hong_Kong）上线，16:12:23 公网与认证接口验证完成。

修复范围：presentation.author、goal.update、tutor.step、memory.save、两个 Reader 命令工具、artifact.read、book.query、三个搜索工具以及 MCP book_guide。保持工具名称与业务权限；Goal 的工作计划用独立 working 操作保存。执行入口拒绝不满足公布合同的参数。

## 验证

- 新增九项缺陷回归修复前全部失败，参数定义修复后全部通过；追加可选字段显式 null 的回归，十组均通过。
- 增加执行入口回归：无效参数不进入 Host，纠正后的调用可以保存。
- 首轮完整回归发现两个依赖固定工具体积的压缩夹具提前触发；夹具显式预留实际 author schema 的 Token 估算，保留压缩前后数据断言。
- 本地 HTTP 名称映射测试一次读超时，独立复查通过；最终回归串行执行以免短超时夹具受到同进程并发测试影响。
- 发布文件的修改前源码与现网发布目录逐一比对一致。
- 最终 Runtime 484 项、Book 合同 10 项、Artifact 8 项通过；Server MCP 14 项、Presentation 17 项、Tutor 17 项通过。
- 独立 JSON Schema 校验器验证全部 34 个工具定义和 35 个有效／无效输入案例。
- 现网模型接口接受全部 34 个最终工具定义，返回包含 operation、title、html、readable_content 的有效 presentation.author write 调用。此验收仅生成调用参数，不执行内容写入。

## 行为与决策

- 原会话在第 12 次循环停止，触发的是连续两批无进展保护；没有重新引入固定 12 轮上限，也未取消无进展停止规则。
- presentation.author 按操作公布必填和允许字段；goal.update 的 refine/revise 不接收工作计划，working 独立保存计划。无效字段不能静默丢弃。
- Tutor、Reader 命令和评分合同公布实际嵌套结构；memory.save 不再公布执行端拒绝的 note 类型，笔记使用 reader.note。
- 查询目标数量、搜索长度、导读条件必填，以及 artifact.read 的 JSON Pointer 和互斥规则与执行端对齐。
- 原 schema 校验仅影响活动展示；现在错误参数在进入实际处理器前被拒绝，并返回具体字段错误。
- 可选 Serde 字段接受显式 null；同一字段在另一个操作中必填时仍拒绝 null。此前教学夹具中的合法 intent_quote:null 已验证通过。

## 已知限制

- Runtime 3 项、Server Presentation 36 项、Tutor 4 项显式忽略测试需要各自的专用环境，本次没有启用。未将它们计为通过。
- 本次没有重放原用户整轮对话。模型仍可能生成错误参数；修复使合同完整、错误可纠正，不保证模型永不犯错。

## 发布

服务为 `understand-book-multi`，现网 release 为 `/opt/understand-book/releases/adm10-20261008`。仅更新本次 19 个后端源码／依赖文件，构建并替换 `server` 和 `presentation_worker`；前端资源、配置与业务数据保留。

- 最终 Linux release 构建成功，用时 4 分 48 秒；构建源码与最终测试源码逐文件一致。
- 切换前无执行中或未保存对话。旧 PID 382648，新 PID 392043；实际运行程序与验收候选按字节比较一致，worker 同样一致。
- 服务启动正常。公开首页、后台入口、登录、身份接口、书库、用量、退出共 7 项检查均为 200；9 本书和制作／预览／绘图／动画能力与发布前一致。
- 证据：[deployment.json](deployment.json)、[tests.json](tests.json)、[site-after.json](site-after.json)、[实际模型参数验收](provider-probe-final.json)、[独立 Schema 验证](jsonschema-validation-final.json)、[文件清单](manifest.json)。

### 回退

原二进制位于服务器 `/opt/understand-book/acceptance/tool-contract-20261009/backup/`，原 19 个文件的基线位于同目录 `before-source.tar.gz`。如需回退，将 backup/server 与 backup/presentation_worker 分别原子替换 release/target/release 下对应文件，再重启 `understand-book-multi`；无数据库迁移需要逆转。后续重新构建旧版本时也应恢复源文件基线。
