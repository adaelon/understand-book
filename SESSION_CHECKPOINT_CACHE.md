# SESSION_CHECKPOINT — Resident 上下文工作集，2026-11-05

后续后台分析修复已于 23:21:59 HKT 上线 `coach-cost-20261105`；当前状态以 `SESSION_CHECKPOINT_COACH.md` 为准。下文保留上下文缓存发布当时的证据。

## 已上线，2026-11-05 21:36:31 HKT
- 用户明确授权“上线吧”。当前生产版本 context-cache-20261105，服务 understand-book-multi.service，PID 165198；正式数据 /opt/understand-book/data/production-20261031，schema 22，没有数据迁移。
- 正式目录 /opt/understand-book/releases/context-cache-20261105，前一版 knowledge-tree-socratic-20261105 保留，配置和 control 恢复点 /opt/understand-book/backups/context-cache-20261105。
- 18 个本任务文件相对实际现网合入；保留既有知识树、音乐和网页。Linux 1053 通过/18 ignored，六个程序优化构建成功。首轮五项缺夹具失败与补齐后通过记录均保留。
- 空闲检查后切换，维护 17.518 秒；普通/管理员的材料清单、历史、额度、工作区、画板及制作能力健康读取通过。
- 公网 index.html、Reader JS 200，匿名 auth/me 401。生产机同一二进制在独立 memory/private 的两问验证通过：首次模型2/工具2，追问模型1/工具0，两问有效引用均为1，测试进程已停止。
- 报告 docs/performance/context-locality-20261105/deployment/README.md；本机资料 tmp/context-cache-deploy-20261105/，远端脚本/日志 /opt/understand-book/acceptance/context-cache-20261105。
- 服务器可用空间约1.46 GiB，未删除旧发布或备份；后续编译应复用缓存并留意实际空间。

## 新鲜度
- 工作树基于 6c18a75f8155e479bee176a4bee38bc766ac9adb，有大量用户原有未提交修改，均保留。
- 本轮目标是项目自身 Agent 的 Inline/Spill、减少模型往返与成本；实现、真实模型重放与生产部署均已获授权并完成。
- 2026-10-21 的旧交付见 docs/performance/web-search-jsonl/cache-optimization.md。

## 已实现
- context_cache.rs：搜索/关联内容有界预取、同版本历史定位恢复及原文回读、完整与分页读取的现成引用、续读游标。
- orchestrator.rs：接入工作集；业务历史保留原结果并追加实际交付范围；历史回执不扩大已读范围。
- deepseek_messages.rs / active_projection.rs：运行数据增量消息，下一轮替换旧上下文，保持工具续接。
- tutor.rs / tool_schema.rs / tool_registry.rs：唯一设计关系由宿主绑定，真实草稿恢复，邀请入口约束。
- goal.rs / agent_progress.rs / agent_prompt.rs：普通解释不为记账单开工具回合，直接复用已登记引用。
- 实测发现的带空格引用标记已接入既有引用归一化；代码和引用文本里的字面标记仍保留。

## 验证与实测
- 基线：今天五问主模型 33 次、全部已对齐模型 35 次、工具 41 次、错误 10 次；已对齐输入 8,070,017 token。
- 全天 58 条账单共 6.727019 元，23 条没有与五问主链对齐。
- 保存原请求离线投影：2,235,522 → 688,270 字节；其他消息、调用/结果及工具声明相同。这不是账单降幅。
- 固定五问读取路径回放全部通过，无定位拒绝；最后一问需要的 859 字可随首次搜索交付。
- 第一轮真实五问完成：32 次全部模型请求，共 1.0960338 元，其中主对话 0.60910152 元，后台 0.48693228 元。
- 第二轮真实五问完成：33 次全部模型请求，共 0.96956816 元。两轮全部取得 usage。
- 完整短文引用、相邻带空格引用和受影响的旧测试假设均已修正。最终全量 1053 通过、18 ignored，见 runtime-tests-isolated-export.log；工具契约导出与 Server 构建通过。
- 最终 live-candidate-3 五问主模型 33→26、工具 41→26、错误 10→8。主输入 7,690,486→2,684,276 token，统一高峰费率主费用 3.8476616→1.1773568 元。
- 最终真实非高峰费用 1.37085968 元，主对话 0.5886784、后台及辅助 0.78218128；36 次请求全部取得 usage，已排空后台队列并停止服务。
- 当时六个后台教练任务三个 ready、三个 failed（格式、超时、空正文），费用均计入；后续同六份输入修复为全部通过，详见后台分析检查点和报告。
- 连续重放第 2 问受到新生成的凭据主题追问影响，不能宣称等质量严格 A/B。
- 另固定原第五问全部上下文/设置作单次生成，输入 547,362→168,094 token；生成稿保留 Cache 的关键机制和两个原引用，末句有“汰汰”笔误。返回的是未执行的教学选择，不等同整轮验收，费用 0.16180808 元。
- 本次三轮五问和一次固定请求共核算 3.59826972 元，无运行中的试验。主报告已写入 docs/代码链路.md，数字在 docs/performance/context-locality-20261105/。

## 当前交付与后续位置
- 本次实现、真实模型验证、上线与发布记录已完成，无运行中的试验。
- 后续已完成后台材料去重、无效输出纠正与超时修复，五问后台费用从¥0.781633降至¥0.308233；实际旧失败正文仍保存在最终试验目录。
- 做严格教学质量 A/B 时，应固定原学习状态和上一轮问题，沿用本次已记录的分支差异，不能把不同追问当作同一质量条件。
- 后续已确认并修复后台来源恢复：recover_turn_text_read_ranges接入内联回执，当前引用支持无新读工具回合；Server实际保存回归和两轮真实流程通过。

## 证据与运行入口
- 私有实验目录 tmp/context-paths-20261105/ 含 turn-summary.json / roots.json / spans.json / details.json，不要整份打印。
- 修改前源码：before-cache-change/crates/runtime/src/。每轮请求/响应/回答：live-candidate-1/、live-candidate-2/。
- 重放脚本 live-replay.mjs、汇总 summarize-replays.mjs、固定请求 frozen-live.mjs 均在私有实验目录。已有输出目录会拒绝重复收费运行。
- 模型为官方 deepseek-flash，与当天旧别名同属 Flash-1017。新试验为非高峰；比较应统一高峰费率。
- 本地新会话未导入原账号目标和画像；后续原问题依据原回答写成，不能当作严格控制变量的 A/B。
- Server：cargo rustc -p server --bin server -- -C debuginfo=0 -C link-arg=/DEBUG:NONE。Windows 普通调试链接遇到 PDB 大小上限，不改变运行行为。
- 测试：cargo test -p runtime --lib -- --test-threads=1。TS_RS_EXPORT_DIR 指向 tmp/context-paths-20261105/type-export/a/b/c，将类型导出隔离，避免现有前端文件的 Windows 占用错误。
- 不要并行运行 Cargo。读取 .env 时不能打印密钥；实验只记录正文和 usage。
