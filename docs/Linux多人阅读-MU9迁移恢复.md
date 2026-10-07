# MU9 离线迁移与恢复

依据：ADR-0147 §9、MU 方案 MU9 / T60–T64。生产源、目标账号、发布映射和备份目录须由操作者显式指定。

## 工具与输入

`cargo build --locked -p server --bin reader_maintenance` 构建离线命令；以下示例使用构建出的 `reader_maintenance`。命令与服务、账号管理、发布工具共用 ServiceWriter 独占锁。先停止目标服务，再用 `manage_reader` 创建一个未使用的目标账号，用 `publish_book import` 登记原材料的完整发布包。目标账号已有私人文件或阅读现场时拒绝迁入。

计划文件示例（路径和 ID 均须替换为已选定的真实输入）：

```json
{
  "operation_id": "legacy-reader-20260930",
  "memory_dir": "/srv/legacy-copy/memory",
  "private_dir": "/srv/legacy-copy/private",
  "library_root": "/srv/legacy-copy/books",
  "service_root": "/srv/multi-reader",
  "user_id": "reader-a",
  "backup_dir": "/srv/backups/legacy-reader-20260930",
  "books": [{
    "legacy_dir": "C:\\books\\中文 空格",
    "source_dir": "/srv/legacy-copy/books/chinese-book",
    "publication": {"book_id": "original-book-id", "publication_id": "registered-publication-id"}
  }],
  "reviewed_files": []
}
```

`legacy_dir` 是旧 session / library-registry 中的目录拼写，支持原 Windows 长路径前缀和大小写；`source_dir` 是当前机器可读的完整书籍副本。书库外的已登记材料也必须逐本映射。每个 book_id 唯一对应一个明确发布；已有历史发布绑定冲突、正文或来源身份不符、阅读位置不存在、书库索引未映射时停止。

## 预览与迁入

```bash
reader_maintenance preview /srv/migration.json > /srv/migration-preview.json
reader_maintenance migrate /srv/migration.json --stopped
```

预览读取当前 schema，不升级或写入业务数据；输出逐文件源 / 目标、字节数、账号、发布映射、Memory 条数 / revision / schema、聊天 / 回合 / 旧 pending 数、Learning 数据版本 / 各表行数、阅读检查点及待审核文件。演示文件的版本与状态路径也逐项列出。预览会取得锁；原本不存在的锁文件可被创建。

审核 `pending_review` 后，把对应的 `memory/<相对路径>` 或 `private/<相对路径>` 逐项写入 `reviewed_files`。私人成果树保留原布局，按清单审核；不会把文件内容送入日志。旧书库及其未知辅助文件整体保存在备份，发布服务只消费此前登记的发布包。

`--stopped` 表示操作者已停止旧单读者程序及所有不遵守新锁协议的写入者。工具不能检测旧二进制是否仍在写；此确认不能用新服务的锁替代。源、目标与备份根必须独立；备份目录专属于这一个 operation。

迁入流程：

```text
预览 / 审核 → 固定 operation 计划 → maintenance.json 拦截服务启动
  → 一致性 snapshot（旧 memory/private/library + 外部材料 + 原服务根）
  → 从 snapshot 生成目标文件 → 逐项核对 / 复制 / 记录完成
  → 同一事务登记该账号的材料授权与阅读检查点
  → 核对文件和映射 → operation complete → 移除启动拦截
```

Memory、Learning、演示和目标成果保持原数据版本、ID、事实和正文。History 仅为每个旧回合补充显式 `published_book_ref`；session 仅改结构化目录字段，并生成 MU5 阅读检查点。临时验证副本可沿原读取器升级格式，迁入文件本身保留原数据版本，正式首次加载沿既有升级路径处理。

`backup_dir/operation.json` 保存计划、逐文件完成状态及最终报告；`backup_dir/snapshot` 完成后封存为只读文件，包含文件清单和原始恢复数据。数据库使用 SQLite Backup API，完成后做数据库完整性检查；同一服务锁和旧实例停写约定确保跨文件恢复点一致。

## 中断与重跑

再次执行同一 `migrate` 命令：已存在的目标逐字节核对，相同项跳过，未完成项续接。计划的源、目标、账号、书籍映射或审核记录变更会拒绝；目标文件、授权或检查点不符也拒绝。数据库事务已提交而完成记录未写时，原工作区 ID 复用，不重复新增。

中断期间 `server`、`manage_reader`、`publish_book` 等正常入口返回 `SERVICE_MAINTENANCE_INCOMPLETE`；不要手工删除 marker。原入口配置与旧资料不变。完成后若新服务已产生业务写入，再重跑导入会报告目标已变化，此时使用服务备份 / 恢复流程。

旧 pending 由原 History 恢复规则标为中断；导入不会伪造网络接单。已有网络数据须按服务整体备份恢复，queued 的原冻结输入、权限和接单记录一起保留，启动时沿 MU6 对账。

## 备份与实际恢复

```bash
reader_maintenance backup /srv/multi-reader /srv/backups/service-20260930
reader_maintenance restore /srv/backups/service-20260930 /srv/multi-restored
```

先停服务，`backup` 必须成功获得独占锁。备份包含多用户私人根、control.sqlite、发布包、配置及其他辅助文件，SQLite 的已提交 WAL 内容由备份接口合入目标库。已有备份路径拒绝覆盖。

`restore` 只接受空的独立目录，按快照恢复全部数据，并只重定位 `book_publications.directory` 的根前缀，保留 PublishedBookRef 和历史正文。恢复中断仍有启动拦截，重跑同一恢复命令续接。恢复后的原包文件重新置为只读。也可用迁入时的 `backup_dir/snapshot` 恢复**迁入前的服务根**。

在隔离入口核对 History、Learning、演示、包绑定和 queued 恢复后，再按 MU11 停写切换。恢复某个备份会缺少该恢复点之后的数据；原服务根完整保留，工具不会替换它或自动切入口。

## 回到旧单读者程序

```bash
reader_maintenance export-user /srv/multi-reader reader-a /srv/legacy-export
```

停止多人服务后显式导出一个账号；该账号仍有 preparing / queued / claimed / 未保存回合时拒绝，先按运行接口处理这些回合。导出新根包含 `memory/`、`private/`、授权的不可变 `books/<book>/<publication>/` 及 `export.json`，没有其他用户目录和服务 control.sqlite。阅读位置取服务端各材料最后检查点，session 指向导出根。

旧二进制仅使用导出根的 memory/private/books；不得指向多人服务根。保留新根及备份，在隔离旧程序上核验其支持的数据版本后才切换。优先使用兼容当前 schema 的服务版本回滚；单用户导出不承诺所有历史二进制支持后来的私人格式。

## 验证与限制

验证记录见 [MU9 证据](performance/linux-multi-reader-mu9-20260930.md)。MU9 的工具和隔离恢复不等于生产迁移或入口发布。生产输入尚未指定，真实 Linux 掉电、Nginx/TLS 与兼容旧发布二进制的实际切换仍按 MU10/MU11 验收；本工具不修改 systemd/Nginx。恢复到其他机器后，Provider 环境和配置内的外部工具 / 沙箱路径由部署者按 MU11 核对。

没有完整来源副本或明确发布映射的历史先保留在原根 / 备份，待补齐材料再迁入。未知文件按清单审核并保留；不推断内容版本、不替换笔记或成果正文中的路径文字。快照的不可变性由只读文件与工具拒绝覆盖实现，管理员仍可改变文件权限。
