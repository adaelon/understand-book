# RN1 笔记存储合同

RN1 实现私人笔记的内容身份、主要关联、v4 存储及原子编辑。决策沿 [ADR-0160 §7](adr/0160-reading-share-images-and-calm-surfaces.md#7-笔记内容与关联)，演示留存和交互已由 [RN2](切片方案-阅读分享与安静呈现.md#rn2-演示笔记保存与返回闭环) 实现。

## 持久字段

`Record.note?: NoteData` 是新入口的笔记专属结构；存在时 `Record.content` 表示用户明确填写的文字。`note` 缺省的旧记录和现有 Agent 提议保留原正文与中性内容身份，升级不拆解 blockquote、不补写作者。

| 字段 | 内容 |
| --- | --- |
| `note.material` | 服务端确定的 `book_id` 与 `publication_id`（本地为 null） |
| `note.association` | `selection`、`body_placement { placement }`、`answer { session_id, turn_id }`、`presentation { receipt, title }` |
| `note.retained_excerpt` | null，或 `original { text }`／`assistant { text }`；与用户文字分开保存 |
| `note.source_bindings` | 从原回答／演示复制其实际来源绑定，包含原 evidence range、既有 digest 和标签快照 |
| `generated_at` | 首次保存时间；Note replace、reanchor、promote 均保留 |

原文精确范围继续保存在 `selection_context`。`note_placement` 是当前正文显示位置，重锚不改变主要关联；`body_placement` 关联保留首次明确放置的对象。演示 `receipt` 与 `PresentationFollowUp` 同形，记录原 session、turn、presentation/revision、state_revision 和 saved_state_ref；这里不复制演示文件。

旧字段缺省时内容寻址保持不变；新笔记将 `note` 结构纳入既有内容身份。同文不同回答、发布、演示版本或现场分别保存；相同请求继续返回 `CREATED | EXISTING`。至少有用户文字或保留摘录，只有演示关联不形成空笔记。

## 创建与读取

`POST /memory/save` 及网络工作区 `memory/save` 共用服务端解析入口。增加可选 `note: NoteCreateRequest`，关联类型由调用方明确选择，来源、发布、演示标题和摘录身份由服务端解析。旧请求继续沿原选区／放置合同。

```json
{
  "type": "note",
  "content": "我的想法",
  "note": {
    "association": { "kind": "answer", "session_id": "chat-id", "turn_id": "turn-id" },
    "retained_excerpt": "选定的已交付回答片段"
  }
}
```

- 原文：`association.kind=selection`，另传 `selection_context`；选用的 `retained_excerpt` 必须等于 `raw_quote`。
- 回答：验证当前用户保存的已完成回合和原材料／发布。摘录使用已交付 Markdown 文字中的连续片段；无摘录时仍须有实际回答及用户文字。来源沿原回合绑定，旧回答沿既有投影解析。
- 演示：`association={kind:"presentation",receipt:PresentationFollowUp}`，复验实际交付版本及持久现场回执，绑定该回合的原材料／发布；用户文字必填。
- 正文放置：`association.kind=body_placement`，另传 `note_placement`，沿原来源与精确位置校验。

回答／演示创建不接受同时指定选区或正文放置；保存后可另行重锚。`source_session_id` 从回答／演示关联派生，切换当前聊天不改变已选关联。`memory/recall` 返回笔记结构和真实时间。Rust 类型通过现有 ts-rs 测试生成 Web 类型，`api.ts` 引用生成的请求、笔记与放置合同。

## 编辑、重锚与保留

`memory/replace` 只接收用户要修改的 `content`，默认继承摘录、关联、来源、选区、正文放置及 layer，保留首次保存时间。已有摘录时允许清空用户文字；没有摘录时拒绝空正文。原文的显式重新选择继续沿 `selection_context`，核对原材料／发布和 LID，并同步原文摘录；回答／演示不得通过该参数换关联。

`memory/reanchor` 沿原真实位置校验，原子改变当前 placement、anchor、citations 和 mem_id，保留原关联。`memory/promote`、Agent 提议保留与撤销继续使用原命令和原所有权。碰撞或持久化失败不切换内存及磁盘中的原记录。

RN4 在原 v4 文档增加可选 `note_replacements`：replace／reanchor 同次原子保存旧 ID 到当前 ID 的只读导航，删除清除对应关系；回顾可解析编辑后的记录，命令仍使用当前 ID。旧文档缺省为空，已发生的旧替换不反推。见 [RN4 验收](performance/reading-notes-rn4-20261010/README.md)。

## 版本升级与验证

MemoryDocument 当前为 v4；v2、v3 及既有裸数组入口原子升级。版本升级只改变 schema 门禁，保留原 records、document/projection revisions、profile、review、exclusions 和 governance；未知版本拒绝读取。

固定旧结构样本位于 `crates/memory/tests/fixtures/memory-v3.json`，沿仓库既有脱敏 legacy 样本扩展 v3 精确选区与 PDF 放置结构。memory 测试逐字段比对升级前后，另覆盖完整私人记忆状态、旧裸数组、升级写入失败、新关联身份、内容编辑、重锚与保存失败。server 路由测试覆盖四类入口、当前聊天变化、未交付回答、错误版本／现场及原发布准入。

验证结果（2026-10-10）：memory 141 项、server RN1／memory／note／effect 定向测试共 31 项、reader 笔记测试 1 项及 Web 类型检查通过。命令见代码链路 RN1 条目。

## 已知限制

- RN1 提供存储和服务接口；新编辑入口、内容分区呈现、搜索和列表时间排序由 RN2–RN4 接入。
- RN2 已实现从已存笔记读取原演示，不再要求原聊天存在，见 [RN2 实现与验收](performance/reading-notes-rn2-20261010/README.md)。
- 回答摘录接口接收已交付 Markdown 中的连续文字；RN3 接入浏览器选区时需沿实际呈现格式提取相应原片段。
