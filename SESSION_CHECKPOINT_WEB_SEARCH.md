# SESSION_CHECKPOINT — 联网检索与独立费用

## 新鲜度
2026-10-21 Asia/Hong_Kong。共享工作区存在大量先前未提交改动，保留原样。本任务实现、必要验证和付费原题对照均已完成；已于 2026-10-21 14:01 HKT 部署 `search-cache-20261021`（schema 20、PID 520445），最终真实验收费用 ¥0.118462。见 `docs/performance/web-search-jsonl/release-20261021/README.md`。

## 完成状态
用户已授权接入 DeepSeek Search/Fetch、复用预算和账本，并付费重放 JSONL 原始问题验证质量与成本。

- Runtime 工具发现、正文预算与证据传递，Server 缓存／重试／限流，独立请求账与共享账号额度，Admin／Reader 分类费用界面均已实现。
- 首轮候选未自主联网，已据此修正通用知识缺口规则，没有写入本题或答案关键词。
- 实际请求发现历史工具名随工具显露变化，已固定 Resident 历史名称映射；当前可执行工具仍受现有可见范围约束。
- 取消后已付费正文丢失的问题已修复：先保存成功正文，再返回取消。网页额度不足或过期沿现有 SpendStop 停止。
- 最终候选自主完成 2 次 Search 和 2 次 Fetch，引用官方正文；仍未解释 JSONL 与模型请求缓存的关系，且本地选型依据不如基线完整。质量结论为部分改善，不是全部通过。

## 实际费用与证据
原始问句：好的，jsonl我知道是什么了，那为什么用jsonl呢，还有什么其他选择吗

重放 `tmp/jsonl-search-analysis-20261019/original-session.jsonl` 第 1–547 行，原问句不追加提示。两组使用相同本地书、历史、固定程序和真实 DeepSeek 直连模型；隔离账号各有 2 元共享额度。

- 更早的隔离登录失败发生于外部请求前，费用 0。
- 首轮 baseline-r2：￥0.483463；首轮 candidate：￥0.487177，均无搜索费用。
- 最终 baseline-final：模型 ￥0.576242，搜索与读取为 0。
- 最终 candidate-final：模型 ￥0.806916，Search ￥0.010000，Fetch ￥0.010000，总计 ￥0.826916。
- 全部实际费用 **￥2.373798**，无活跃预占或待确认金额。
- candidate-final 的一笔后台 memory_review 回执未在旧脚本停机前落账；保留原快照后，仅通过既有管理员接口按真实 usage 和原费率补记 ￥0.032146，没有新发模型请求。runner 已修复后台等待。
- 独立 diagnostic 已取消，未产生调用与费用。不要再为此次报告追加付费采样。

## 已完成验证
- Server 定向 18 项通过。
- Runtime 首轮 21 项通过；名称映射与提示修正后另 10 项相关测试通过。
- Admin 6 项、Reader 4 项费用测试与两端类型检查通过。
- 回放脚本 6 项测试通过，覆盖后台等待和超时保留未知费用；原始前缀与原题校验通过。
- server/manage_reader/publish_book 构建成功，最终固定在 `tmp/web-search-eval-20261021/bin-r2`。
- 工具契约已从真实注册表生成：Resident 46 项，Tutor 状态 17 项。

Windows 默认并行编译曾因页面文件不足失败；改为单 job、Server 测试关闭调试信息、Runtime 16 codegen units 后完成。没有修改系统设置。

## 接续时先读
1. [最终实验报告](docs/performance/web-search-jsonl/README.md)
2. [配置与实现说明](docs/实施计划-联网检索与独立费用.md)
3. `tmp/web-search-eval-20261021/comparison-final.json` 与 `candidate-final/accounting-reconciled.json`
4. 核心实现：`crates/runtime/src/web.rs`、`crates/server/src/web_search.rs`、`crates/server/src/web_usage.rs`；评测入口 `evals/semantic/web-search-case.mjs`。

## 已知边界
- 接入需要独立配置项 DEEPSEEK_SEARCH_API_KEY 和服务根目录 web-rates.json；不要求新建一把 Key。实验确认官方直连后，在隔离子进程中显式复用 OPENCODE_API_KEY 的值；生产不自动跟随模型渠道的密钥或费率。模型仍可走现有 ahriapi 路径，本次实际费用来自 DeepSeek 直连。
- 复用的是账号共享货币额度；普通聊天没有既存每 run 人民币上限。工具正文与上下文预算继续复用。
- 仅受管理的网络 Reader 有付费联网归属；普通本地 Reader 与离线桌面不开放该工具。日常网站现已部署本版；普通离线使用范围不变。
- 两组采用同一份已更新的本地书，没有完整恢复原线上教学数据库、跨聊天记忆和旧演示资产，不能称旧线上状态完整复原。
- ADR0152 的 JSONL 动机是增量持久化；ADR0155 的稳定 prompt 前缀是另一层机制。JSONL 不等于 API 请求只追加，不将“JSONL 本身提高缓存”作硬编码正确答案。
- 单题、单次采样及供应商缓存状态不支持整体质量比例或因果节费结论。细节见实验报告限制。
