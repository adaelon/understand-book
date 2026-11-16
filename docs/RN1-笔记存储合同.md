# RN1 笔记存储合同

本合同现由 PN1 接续：私人笔记使用稳定身份、追加修订、schema 6 存储与原子编辑；RN1 的原话/摘录和主要关联规则继续生效。决策沿 [ADR-0160 §7](adr/0160-reading-share-images-and-calm-surfaces.md#7-笔记内容与关联)，演示留存和交互已由 [RN2](切片方案-阅读分享与安静呈现.md#rn2-演示笔记保存与返回闭环) 实现。

2026-10-30 已实现 [ADR-0177](adr/0177-personal-notes-evolution-and-teaching-recall.md) 与 [PN1–PN6](切片方案-个人想法演进与教学召回.md)。新用户级笔记允许没有材料，使用 `/me/notes/*`；旧 RN1 材料采集入口仍按下述来源规则核验。验证见 [PN 报告](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/personal-notes-20261030/README.md)。

## 稳定笔记与版本

`MemoryDocument.notes` 是正文和历史的唯一权威，note_id 由宿主创建，不随正文、位置或材料变化。每个已提交 `NoteRevision` 保存原话、可选标题、authorship、来源素材、NoteContext、source_refs、based_on_turn、saved_at、change_note 与 restored_from。current_revision 指向当前版，历史按确切版本只读。

创建/修订/恢复携带 operation_id；重试返回原结果。同一内容不添加空版本；修订必须匹配 base_revision，冲突保留调用方草稿供明确整合。恢复复制被选版内容并追加新修订。删除清除正文与所有版本，保留必要身份/重试状态，防止晚到任务复活正文。

`NoteRef` 是 note_id 和可选 revision。当前查看和当前画板解析最新版本；讨论接纳、历史反馈与历史画板冻结 revision。source_refs、NoteLink 的建立版本和说明依据版本分别保存；已删除来源不妨碍已有综合稿继续写。

`NoteContext.origin` 保存产生时的消息原句/出现序号或演示状态，画布来源保存 board_id / board_item_id。展示位置独立于修订：`MemoryDocument.note_attachments` 保存消息与演示引用，画布沿用 `BoardItem::PersonalNote` 和 `BoardLink`。添加/移除位置不追加正文版本、不改写产生现场、不复制正文；同一目标的重复放置返回已有引用。画布的卡片与连线经 `BoardStore::place_personal_note` 一次提交。正式正文先保存成功但放置失败时，设备草稿保留已确认 NoteID；重试只补放置。`notes.read` 返回产生现场与消息/演示展示引用，画布的反向位置由用户私有画布查询读取。验证见[放置验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/personal-notes-20261030/placements/README.md)。

内容来源分别为 user、assistant_draft、adopted_assistant、material_excerpt、unknown。用户修改 AI 稿后仍保留采用 AI 修改身份和形成它的讨论。旧内容不会因为迁移而被推测为用户原话或新掌握事实。

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

`memory/reanchor` 沿原真实位置校验，原子改变当前显示 placement、anchor、citations，保持 note_id 和正文修订不变，保留原关联。正文的旧情境仍属于其保存版本。`memory/promote`、Agent 提议保留与撤销继续使用原命令和原所有权。碰撞或持久化失败不切换内存及磁盘中的原记录。

RN4 原 v4 文档留下的 `note_replacements` 在升级后继续解析旧链接到稳定身份；PN1 后正文 replace 追加版本，重锚改变显示位置，两者都不再产生另一条笔记身份。旧系统没有留下的正文与替换过程不反推。见 [RN4 验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/reading-notes-rn4-20261010/README.md)。

## 版本升级与验证

MemoryDocument 当前为 schema 6；旧 v5、v4、v3、v2 与裸数组由原入口有限升级。v5 原有笔记和历史不变，新位置集合为空。现存 note Record 迁为稳定笔记首版，来源与批注位置保留，作者无法确定则为 unknown；普通记忆、document/projection revisions、profile、review、exclusions 和 governance 保留。升级持久化失败不替换内存与旧文件；未知更高版本拒绝读取。旧 v4 的当前正文是可迁移的全部原话，不补写已丢失历史。

固定旧结构样本位于 `crates/memory/tests/fixtures/memory-v3.json`，沿仓库既有脱敏 legacy 样本扩展 v3 精确选区与 PDF 放置结构。memory 测试逐字段比对升级前后，另覆盖完整私人记忆状态、旧裸数组、升级写入失败、新关联身份、内容编辑、重锚与保存失败。server 路由测试覆盖四类入口、当前聊天变化、未交付回答、错误版本／现场及原发布准入。

验证结果（2026-10-10）：memory 141 项、server RN1／memory／note／effect 定向测试共 31 项、reader 笔记测试 1 项及 Web 类型检查通过。命令见代码链路 RN1 条目。

## 已知限制

- 全局编辑、内容分区、历史与召回已由 PN1–PN6 接入；未发送草稿仅保存在当前设备，历史旧正文无法追补。
- RN2 已实现从已存笔记读取原演示，不再要求原聊天存在，见 [RN2 实现与验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/reading-notes-rn2-20261010/README.md)。
- 回答摘录接口接收已交付 Markdown 中的连续文字；个人讨论的 AI 稿保留 based_on_turn 与实际想法引用，保存或采用本身不证明掌握。
