# SESSION_CHECKPOINT — 知识树与理解果实 — 2026-11-05

## 当前状态
- 用户明确授权上线，已于 2026-11-05 15:55:12 HKT 完成；维护 9.174 秒。
- 正式版本 `/opt/understand-book/releases/knowledge-tree-socratic-20261105`，服务 PID 159704，服务名 `understand-book-multi.service`。
- 数据仍为 `/opt/understand-book/data/production-20261031`，control schema 22，无数据迁移。账号、书库、历史、额度、工作区、画板与制作能力读取均核对通过。
- 本地基于 `6c18a75f`，共享工作树有大量其他未提交修改。禁止整体回退或将全部工作树当作本次发布；本轮未提交。

## 已交付
- 来源知识树由已有章节、Pass1、discourse、BookStructure 与媒体成果确定性投影，首次需要时生成并缓存。生成树不调用模型。
- 普通段落、图片、公式 LID 保留来源地址，不各占可见枝条；同节同知识合并入口并保留全部来源。范围超过六项时按关系、概念及解释分枝浏览。
- 初次打开跟随阅读／播放范围，后台刷新保持手动选择。具体主题、已有说明或标明来源的开篇线索提供起点。
- 私人果实复用现有证据与后台任务；准确知识引用、用户原话、帮助条件、历史及原讨论保持连续，无笔记理解可接续。
- 同一关系与能力的新表达更新当前判断。跨知识引用的旧表达仍留在原枝条，later_evidence_ref 标明已有后续，显示“当时卡点”。
- Agent 在讲解后可提出苏格拉底问题，承接当前表达；问题单独呈现，支持“先到这里”并保留未发送草稿。

## 真实验收与修复
- 用 D2L 锚框章节及 deepseek-v4-flash 实际运行两组四轮，学习者输入为预设验收表达，隔离数据未污染真实用户历史。
- 修复连续对话丢前问来源、面板内跳树失败、前后台抢写理解、重复问题、后台时限过短、旧误解仍列为当前六处实际问题。
- 连续问答沿表达之前已交付帮助恢复同会话同版本的真实来源；前台显式更新同样保留原先帮助。
- 普通观察不主动加载显式理解修改工作流；重复问题交既有回答修复；后台任务有独立 180 秒上限。
- 本地当前判断只剩一条，旧、新原话与帮助同时保留；桌面和手机及服务重开核对通过。
- 本地原始记录 `.tmp/knowledge-live-s3q5Lf` 与 `.tmp/knowledge-live-tfQd2Q`；所有自有本地验收服务已停止。

## 上线证据
- 正式账号 reader 可读的 25 份材料、28 个发布版本，文字／音乐／视频树接口全部成功，来源和发布身份逐项匹配。
- 正式 AI Infra“量化分析”上两个实际模型回合验证提问与退出；1440/390 界面、树／我的理解入口、草稿保留通过，浏览器错误和横向溢出均为 0。
- 正式理解读取接口通过；所选 AI Infra 发布当前理解与果实均为 0，没有向真实账号编造理解。完整果实闭环已在独立数据中验证。
- 测试聊天已删除，现场已释放；原钢琴来源资源可读。
- Linux Rust 25 项、Web 163 项、类型检查、多人网页及六个正式程序构建通过。三个入口、13 个直接资源及正式后台路由核对通过。

## 发布范围与恢复
- 以实际现网 answer-piano-lossless-20261104 为基线，只合入 68 个功能及必要依赖文件；本地音乐后台锁、音乐启动优化与 ProductTour 焦点修复等未混入。
- 初次候选缺 MusicReaderPane.position，补入后前端全套检查与构建通过。原失败与后续成功分别保留。
- 本地操作 `.tmp/knowledge-tree-release-20261105`，远端 `/opt/understand-book/acceptance/knowledge-tree-20261105`；source-files.json 与 updates.patch 为实际范围。
- 恢复点 `/opt/understand-book/backups/knowledge-tree-20261105`。新回答含 socratic_question，公开后恢复程序须保持新数据可读，不能覆盖后续用户数据。
- SSH 复用 `.tmp/reading-combined-release-20261104/remote_stage.py`。后续发布新建独立阶段，勿重跑已完成阶段的构建／切换脚本。

## 已知边界与后续
- 首答长度、个别问题前提完整性、理解长标题及未渲染数学标记、偏保守的待确认描述仍是内容质量项，功能上线不代表全部教学内容通过。
- 完整音乐／视频播放器的真实模型理解闭环尚未新增；已验证真实来源投影、合同及线上树读取。
- 后续按实际用户反馈优先处理上述内容与文案问题，不新增独立归类模型调用，不重新扫描整库生成树。

## 冷启动读序
1. `docs/performance/knowledge-tree-20261105/deployment/README.md`（线上事实）
2. `docs/performance/socratic-understanding-live-20261105/README.md`（真实对话与质量边界）
3. `docs/adr/0181-source-knowledge-tree-and-understanding-fruits.md`
4. `docs/切片方案-知识树与理解果实.md`
5. `crates/read-tools/src/knowledge.rs`、`crates/server/src/knowledge_tree.rs`、`packages/web/src/components/KnowledgeTree.vue`
