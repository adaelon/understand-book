# EX11.8：按需源码读取与局部补丁

日期：2026-09-29。基于 HEAD `5e10516` 与现有未提交工作树。用户明确接受按搜索、按需读取、局部补丁改善修订流程。本切片工程实现及验证完成，制作方法为 `ex11.v11`；EX11.7 自然生成验收仍未完成。

模型服务恢复后的实际修订验证见 [真实模型记录](explorable-explanation-ex11-8-live-20260929.md)：学习率页已使用搜索/局部读取/补丁交付并通过315组数值图形与Reader三视口；圆盘页的端点修复及恢复回归单独记录。

## 现象与改动

此前 Author read 固定返回最多 4,000 个字符，局部修订只能通过 write 重交整页；batch12 学习率诊断出现反复读取源码直至上限。工程回归进一步确认：工具结果账本在取得很小的新结果前，先按单次最大额度腾空间，可能提前逐出已有的 40 KB 源码。该预算行为可确定性复现；它对真实 Agent 重读次数的影响尚待新模型运行验证。

现在原 `presentation.author` 提供以下操作：

| 操作 | 行为 |
| --- | --- |
| `search` | 在指定逻辑文件中做区分大小写的字面搜索，返回字符位置、行号及附近源码。默认最多 20 个命中，可用 `next_offset` 继续。 |
| `read` | 默认从 offset（缺省 0）读取到文件末尾，也可指定 length；预算内完整返回，超预算时返回连续前缀和准确的后续 offset/length。 |
| `patch` | 对入口页顺序执行精确原文替换，每段 old_text 必须唯一；全部成功才保存新候选，失败不落半成品。 |
| `write` | 继续用于新建页面、整体改版以及库/渲染资源选择变化。 |

read/search/patch 均明确选择一个已交付 `reference` 或本轮生成的 `candidate_id`。offset/length 按 Unicode 字符计数，中文和 emoji 不按 UTF-8 字节或 UTF-16 代码单元计数。

patch 自动保留其余入口源码、库、动画、静态资源、来源绑定和元数据，可显式更新 title/readable_content/state_contract/initial_state。从已交付版本修订时沿用原追问冻结现场的兼容参数继承；继续修订本轮候选时保留该候选已选定的状态。每次补丁生成新的候选身份，需要自己的预览结果，再按原合同交付。

Runtime 保持 48 KiB 活跃工具结果总预算。先投影实际返回，再为其实际大小腾出空间，源码读取可使用这个总额度；其他调用沿用各自上限。尚未被模型采样的同批结果不会被后来的结果逐出。搜索结果超限时只移除末尾完整命中，避免截坏源码片段。全历史压缩仍沿已有机制运行。

## 工具使用示例

局部修订可以先找函数，再读取附近代码：

```json
{"operation":"search","candidate_id":"本轮候选ID","query":"window.presentationScene="}
```

```json
{"operation":"read","candidate_id":"本轮候选ID","file":"index.html","offset":5000,"length":1200}
```

定位后只提交需要替换的源码：

```json
{
  "operation": "patch",
  "candidate_id": "本轮候选ID",
  "edits": [{
    "old_text": "window.presentationScene={seek:seek, snapshot:snapshot};",
    "new_text": "window.presentationScene={seek:function(o){return seek(o.semantic_state,o.transition_progress);}, snapshot:snapshot};"
  }]
}
```

后续 preview/deliver 使用补丁返回的新 candidate_id。`skills/presentation/SKILL.md` v11 与实际工具 schema 已同步此流程。

## 验证与证据

每项检查对应可影响实现的具体失败：源码缺失或范围跳过则修读取/投影；补丁越界改写或部分落盘则修替换/存储；旧预览允许交付新候选则修候选隔离；原问题页定位仍错误则检查实际接口包装及预览反馈。

| 检查 | 结果与证据 |
| --- | --- |
| 新接口红绿回归 | 旧路径因 search/patch 不存在及 4,000 字符截断失败；实现后 3 项通过。见 `engineering-checks/ex11-edit-red.log`、`ex11-edit-green.log`。 |
| 预算红绿回归 | 原行为使小返回逐出 40 KB 源码、35 KB 页面被单次 16 KiB 限制截短、范围续读超过请求终点；修复后通过。见 `ex11-edit-budget-red.log`、`ex11-edit-budget-green.log`。 |
| Runtime 全量 | `cargo test -p runtime --lib`：411 passed，3 ignored；覆盖工具 schema、方法加载、历史、压缩、目标与进展判断。见 `ex11-edit-runtime.log`。 |
| 最终工具结果回归 | 14 passed；包括完整搜索片段及续读、实际 ledger 调用路径的同批总预算和未采样结果保留。见 `ex11-edit-budget-final.log`。该计数与 Runtime 全量重叠。 |
| Server Presentation | `cargo test -p server --lib presentation_ -- --test-threads=1`：29 passed，29 ignored；包括原子失败、旧版本保持、预览隔离及冻结参数继承。见 `ex11-edit-server.log`。 |
| 原问题页真实浏览器 | 独立运行 `presentation_edit_original_learning_page_preview_delivery`：1 passed，三种既有 Author 视口全部完成非零、回退、重复 seek 后正式交付。见 `ex11-edit-browser.log`。 |

证据目录：[ex11.8](ex11-local-demonstrations/ex11.8/)。日志均在该目录的 `engineering-checks/`。

实际浏览器回放读取 batch8/runs/learning-2/content.json 原稿，通过正式 search/patch 修复双参数 seek 与单对象公开接口不一致的问题。完整内容比较确认只改变示例中的接口包装，Konva 与其余资源相同，旧版本内容不变；新版本 revision 增加 1。三视口均依次定位 2+0.35、1+0.2、重复 1+0.2，并通过现有场景位置与稳定性检查。

- [精确替换与前后版本引用](ex11-local-demonstrations/ex11.8/patch.json)
- [工程补丁生成的新内容](ex11-local-demonstrations/ex11.8/patched-content.json)
- [三视口预览结果](ex11-local-demonstrations/ex11.8/preview-results.json)

## 已知限制

- 这是原模型页面上的确定性工程回放，未计作 Agent 自然修订或 EX11.7 独立生成通过；batch8 原稿与判定保持原样。
- 本文工程验证阶段尚未重新调用模型；服务恢复后的实际调用、用量及修订范围见独立的真实模型记录。
- 大页面仍受结果总预算约束；省略 length 不保证任何大小的文件一次返回。预算不足时使用准确续读或搜索局部内容。
- patch 只修改入口页和列明的可选元数据。动画过程变化需重新渲染并通过 write 选择新资源。
- EX11.7 的短屏媒体播放、圆盘端点提示、机制生成及剩余独立样本问题保持待处理，详见 EX11.6–7 报告。未提交、未部署、未更新安装包。
