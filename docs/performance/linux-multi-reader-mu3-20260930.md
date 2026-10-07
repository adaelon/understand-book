# Linux 多人阅读 MU3 只读发布书库

日期：2026-09-30。基线 HEAD `5e10516`，实施基线为 MU2 后实时工作树；Server 与 Book 源码副本在 `tmp/mu3-baseline`。

## 实施步骤

- [x] 读取 checkpoint 全部冷启动读序及 MU0 路径清点。
- [x] 离线暂存、材料依赖校验、不可变登记与存活 Book 容量。
- [x] 现场/Run/历史固定发布绑定、资源寻址、读时可写根。
- [x] 隔离测试、相关回归与证据记录。
- [x] 更新架构、代码链路、方案与 checkpoint。

## 材料范围

Book::load 消费 base.json、source.txt、formula_semantics.json、discourse_index.json、book_structure.json、paper_metadata.json、paper_lexicon.json，以及地图依赖 source_manifest.json、pdf_source_map.json、pass2_audit.json。Reader 另消费 asset_manifest.json 中声明可用的图片、PDF 原件、pdf_selection_map/manifest.json 及其 page_shards、alignment_report.json；Tutor 消费 teaching_readiness.json 及确切版本的 teaching/versions/<revision>/map.json。formal_objects/cognitive_materials、profile_sidecar、paper_reading_guide 保留为能力材料。

来源清单的绝对 PDF 路径是现有受支持输入；导入时复制附件并改写为包内引用。沿既有来源摘要验证，发布不新增文件指纹。构建运行目录、脚本、凭据和私人产物不进入发布清单。readiness 保存原判定，普通正文可读不意味着正式学习就绪。

## 写入接点

注册登记写 control.sqlite；地图纯内容缓存写 shared-cache/<publication_id>；位置/覆盖层沿私人 session 路径，Memory/History/Learning/演示/usage 沿 UserRuntime。构建与工作台维护入口不允许修改绑定的发布包。旧本地工作目录入口保持既有用途。


## 固定接口与容量

`PublishedLibrary` 使用已有 ServiceWriter，在服务中只创建一个共享实例。`publish` 是可信离线入口，按真实读取依赖组成暂存包，运行原有构建 readiness，再做包内来源/PDF/依赖检查并保存原教学 readiness。只接受已能进入 Reader 的构建成果；教学状态仍可为 preparing/stale。未声明可用的能力不升级。

封存后原子移动至 `library/published/<book_id>/<publication_id>`，同一事务登记 book_publications 和 book_defaults。服务库 schema 2 新增默认发布表，已有 schema 1 在短事务中升版，未知版本仍拒绝。正文、原始 PDF 或已声明图片内容不同必须换 book_id；比较已有文件的实际字节，不新增摘要文件。

`list/load/read/asset` 按显式 owner 检查有效用户与该 publication 的 grant。`list` 只返回授权条目及 readiness；不返回根目录。`open_workspace` 从用户 owner 取得同一容量入口的 Book，完成历史提交后绑定现场。Run 和新建 AgentChatTurn 固定该发布，历史视图返 published_book_ref；旧来源在新发布现场下拒绝读取，要求原绑定。旧无 publication 的历史保持 None，迁移映射归 MU9。

缺省限制为 **20 个存活 Book、2 GiB 估算计费**。计费覆盖 UTF-16 来源、索引和已加载结构，结构序列化体积采用 8 倍分配余量；这是入场估算，不是 RSS 实测。Weak<Book> 跟踪缓存、现场、Run、未保存结果及其他外部 Book Arc；同一对象计一次。缓存按最近使用释放强引用，仍被现场或 Run 持有的对象继续计量，无可释放空间时返回 BOOK_CAPACITY。缓存驱逐不删除目录。

`publish_book` 示例（服务须释放同一服务根写锁）：

```text
cargo run -p server --bin publish_book -- /absolute/service-root import /absolute/book-workspace
cargo run -p server --bin publish_book -- /absolute/service-root grant USER BOOK PUBLICATION
cargo run -p server --bin publish_book -- /absolute/service-root default BOOK OLD_PUBLICATION
```

最后一条只调整未来打开时的默认选择，不替换现场、Run 或已有历史，不删除旧包。

## 验证过程

- 初次 all-targets 编译暴露内部 split_url 引用位置错误，改为既有 parse_query 后通过。见 `check.log`、`check-2.log`。
- 首次测试编译修正夹具对 Book::text 返回 String 的误用；第一轮实际行为测试 5 项均在 Book::load 失败。定位为 Windows 规范化 `\\?\` 根与字符串拼接 `/file` 不兼容，Book 加载及地图 sidecar 改用 Path::join。保留 `mu3-first.log`、`mu3-second.log`、`import-debug.log`、`mu3-third.log`。
- 路径修复后 4 通过/1 失败，失败是带 `/api` 前缀的非当前发布资源未走明确拒绝分支；修复后 6 项通过，见 `mu3-fourth.log`、`mu3-fifth.log`。
- 增加失败登记注入、字节预算和未就绪/非法依赖后，定向 8 项通过，见 `mu3-final.log`。随后补充重开服务根保留封存权限及 Host 资源/JSON 分流回归，最终结果以下节为准。
- 收口发现 ServiceWriter 原先递归整理整个根，会在 Linux 上恢复发布目录写权限。新增只整理目录自身的 secure_directory，用户私人树仍由原路径单独整理；有独立重开回归。

## 最终验证

1. `cargo test -p read-tools -p server -- --test-threads=1`，使用 `TEMP/TMP=tmp/mu3-regression-temp`：Read-tools **165** 通过；Server 库 **366** 通过、33 原有忽略（428.54 秒）；Book MCP 5、CLI 1、预览取消集成 1 通过、10 原有浏览器忽略；doc-tests 无用例。合计 **538 通过、0 失败、43 忽略**。见 [regression.log](linux-multi-reader-mu3-20260930/regression.log)。子进程 writer_child 的 1 项已在 Server 用例内，不重复计数。
2. 后续收口只改服务根权限入口、JSON/二进制分流及 URL 字段绑定；新增的原文保护用例先失败，证明 `/book/text is the command I typed` 被错误改写，见 [url-binding-red.log](linux-multi-reader-mu3-20260930/url-binding-red.log)。修复为仅处理 asset_manifest.v1 的 images[].url_path，不改写问题、正文、答案或 alt。
3. `cargo test -p server --lib -- mu3_ control_store::tests:: host::tests:: --test-threads=1`：最终版本 **39 项通过**，包含 **10 项 MU3**、ControlStore 5、Host 24；覆盖最终权限、来源、资源与缓存实现。见 [final-targeted.log](linux-multi-reader-mu3-20260930/final-targeted.log)。只读 PDF 包测试同时调用真实 query 核心与固定 Provider，正常生成完整问答，文件字节不变。
4. `cargo test -p memory --lib -- --test-threads=1`：**125 项通过**，9.54 秒；`cargo check -p server --all-targets`：**通过**，18.97 秒。见 [memory-final.log](linux-multi-reader-mu3-20260930/memory-final.log)、[check-final.log](linux-multi-reader-mu3-20260930/check-final.log)。

按不同用例合并本轮验证：Read-tools 165、Server 库 368、二进制/集成 7、Memory 125，合计 **665 项通过，43 项原有忽略**。这不是一次全量命令验证了最终源码的全部用例。

最终源码补充后没有重跑整个 Server 全量；以上全量与收口定向验证分别记录。未调用付费 Provider；保留原有 ts-rs serde 属性解析警告。

## 已知限制

当前交付是 MU3 服务内核与离线命令。HTTP Host 仍为显式本地模式；网络身份、每次请求授权、现场挂接及撤销进行中的任务由 MU4/MU5/MU6 接入，不能把内部 grant 测试视为网络鉴权验收。

本轮在 Windows 隔离临时根验证；Linux 目录只读模式和 symlink 拒绝的测试已写入，但 Linux 实际只读挂载、进程 kill/掉电、跨存储恢复、4 GiB RSS/容量及真实 HTTPS/iPhone 未执行。Linux 发布运行时应挂只读发布目录；普通文件封存模式不等于对管理员的隔离。Windows 验证了只读文件、发布绑定写入闸及读后字节保持，不声称完成 Linux 挂载验收。

目录移动后、数据库登记前失败可能留下未登记的封存目录；目录不可通过目录接口或资源接口取得，保留供管理员核对。首版不自动清理旧包或孤立目录。没有迁移真实用户、安装、部署或开放新读者入口。
