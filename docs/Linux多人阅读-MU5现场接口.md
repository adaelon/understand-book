# Linux 多人阅读 MU5 现场接口

MU5 在已登录的多人 Host 中提供独立阅读现场。账号、Cookie、CSRF 与材料授权沿 [MU4 入口](Linux多人阅读-MU4候选入口.md)；完整 Vue Reader 接入属于 MU8。

## 创建和挂接

每个新页面实例生成自己的 `attachment_id`。创建成功后保存 `workspace_id`、`generation`、`revision` 作为恢复句柄；这些字段不授予跨用户访问权限。

```json
POST /api/workspaces
{
  "attachment_id": "page-instance-1",
  "published_book_ref": {"book_id": "BOOK", "publication_id": "PUBLICATION"}
}
```

也可只传 `book_id`，由服务选择当前默认发布并验证授权。响应固定确切 `published_book_ref`；以后默认发布改变不替换此现场。

`GET /api/workspaces/{id}` 返回现场、服务端版本、当前聊天、Reader 和已保存演示现场引用。冷现场在重新加载时换代。除创建和 GET 外，现场请求均使用 POST，并带以下字段：

```json
{
  "attachment_id": "page-instance-1",
  "generation": 1,
  "expected_revision": 1
}
```

后续请求使用最近成功响应的版本；409 时先读取当前状态，依据用户动作处理，不自动重放旧导航。

| `/api/workspaces/{id}/` 下的操作 | 行为 |
| --- | --- |
| `attach` | 同一页面实例幂等挂接；已有其他活动挂接时复制检查点与聊天选择，返回新现场和 `forked_from` |
| `fork` | 显式建立独立现场，保留原发布、位置及聊天选择 |
| `takeover` | 显式取得原现场，generation 增加，原页面、附属页及旧 Run 的实时读写失效 |
| `detach` | 主页面释放挂接并换代；无运行引用或未结接单时立即卸载。附属页只解除自己的挂接 |
| `book/open` | 提交新 `published_book_ref` 或 `book_id`；只替换本现场，清除聊天选择与演示现场，并换代 |
| `chat/new` | 在当前用户历史创建聊天并选中，换代 |
| `chat/select` | 提交本人当前材料的 `session_id`；选择改变时换代 |
| `checkpoint` | 可带 `top_lid` 保存当前位置；使用服务端 CAS 和提交序列 |

刷新收到原页面 detach 回执时，可用其最新版本重新挂接。无法证明旧实例已离开时，使用新的 attachment 挂接会分叉；需要保留原现场身份时由用户显式接管。新建、恢复、分叉均不自动创建聊天。旧 `/agent/history/select` 复用同一选择实现，须增加 `workspace_id` 和上述版本字段。

## Reader、演示与 Tutor

现场内支持 `reader/state`、`reader/goto`、`reader/scroll`、`reader/note`、`reader/highlight`、`reader/layout.apply`、`reader/paper_minimap.state`、`reader/paper_minimap.apply`、`reader/pdf_selection.resolve`、`reader/pdf_ranges.project`。命令参数沿原 Reader 合同；布局与地图的 proposal/revision 检查继续生效。地图动作从此 HTTP 入口按用户操作执行。

`linked/attach` 由原页面提交当前 `session_id`、已交付的 `turn_id` 和演示 `reference`，返回服务生成的附属 `attachment_id`。附属视图共享原现场和聊天；每现场最多 4 个附属视图。它可以操作 Reader 和演示，不能替换父页面的书、聊天或再派生附属页。

`presentation/save` 提交 `session_id / turn_id / reference / state`，复用私人演示存储，返回不可变状态回执，并将当前回执保存到本现场。`presentation/restore` 提交明确的 `saved_state` 回执，恢复该确切版本/状态；旧 Run 不自动覆盖用户后来选择的版本或滑块。原聊天的演示只在该回合的原发布现场挂接和恢复，换版后须显式打开原发布。

`GET /api/me/tutor/state` 与 `POST /api/me/tutor/mutate`（兼容 `/tutor/state`、`/tutor/mutate`）使用用户唯一 LearningStore。修改沿原 `operation_id / expected_revision / action` 合同；同版本竞争只有一次成功。另一窗口关闭或重新开启 Tutor 后，旧 Run 的原控制版本不能继续正式教学推进。

## 容量与持久化

常驻现场上限为每用户 4、全局 20；分配前回收闲置现场，Host 每 60 秒也检查一次。最后访问超过 15 分钟、无外部 Run 句柄及 preparing/queued/claimed 接单时可卸载；未保存运行须继续持有句柄或占位。不能回收时返回 `WORKSPACE_CAPACITY`。全部 Book 引用仍经 MU3 的 20 本/2 GiB 预算计量，不能以驱逐缓存掩盖活引用。

`control.sqlite` schema 3 增加 `checkpoint_seq`；已知 schema 1/2 原位短事务升版，保留原数据，未知版本拒绝。检查点格式为版本 1，保存位置、选择、布局、地图和演示回执；不恢复未接受提案或执行状态。每次成功变更同步提交检查点，再替换内存现场；普通滚动不改变 generation。按材料继续阅读使用最近服务端提交序列，仅影响新建现场。

## 已知限制

MU5 提供 HTTP 与受限 Run 状态端口。MU6a–MU6b 已接通持久接单、执行、取消、聊天删除占位和既有 SSE，见 [MU6 运行接口](Linux多人阅读-MU6运行接口.md)；服务库当前为 schema 4。MU6c/MU6d 已接通公平调度与跨重启观察，容量可从服务根统一配置。完整浏览器页面实例生命周期及 iPhone 验收属于 MU8/MU11。本轮证据为 Windows 隔离服务根，未部署。
