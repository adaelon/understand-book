# 命令行书籍热发布上线记录

2026-10-15 22:44 HKT 上线服务能力，22:48 HKT 完成真实热发布验收。

正式 release：`/opt/understand-book/releases/online-publication-20261015`。数据根沿用 `/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 13 升至 14。服务 PID 4476。

## 实现与验证

`publish_book online-import` 通过本机 Unix socket 交给运行中的服务处理。发布、默认版本、指定授权和操作回执同事务提交；同编号重试返回原结果。命令见 [书籍不停服发布](../../书籍不停服发布.md)。

- Linux 隔离测试 4 项全部通过，覆盖精确授权、失败回退、schema13 升级、实际 socket 导入时 HTTP 可读、重启后重试及 socket 清理。见 [测试日志](test-bounded.log)。
- server、manage_reader、publish_book、reader_maintenance、presentation_worker 正式 release 构建通过，见 [构建日志](build.log)。
- 升级前确认无在途或未保存回答，执行原生完整备份。首次启用停启窗口 15.06 秒；内外网登录、reader 原有 11 本授权材料及四项制作能力均通过。见 [部署回执](deployment.json)。
- 《深入 Understand Book》在线发布成功，发布前后 PID 均为 4476；原发布正文仍可读取，新发布成为默认。同编号重试返回相同发布，只有一条操作回执。
- 新发布的授权表精确为 admin、adaelon，没有未来账号自动授权。admin 经正式 HTTPS 验证书架、正文、图片和 Tutor ready；reader 书架不可见，认证客户端直接访问返回 404（材料不可用）。adaelon 使用精确授权表验证，没有变更其密码。
- 验证未发起模型请求。见 [真实热发布验收](hot-verification.json)及[发布清单](publication.json)。

新发布：`understandbook / 01a14006-1291-7dd1-8641-9c4a18148419`。

旧发布：`understandbook / 01a13fbf-8230-7173-856d-1f388850f13f`，仍保留，已有阅读现场继续使用原引用。

操作编号：`understandbook-online-20261015-v1`。验收记录中的 `command_seconds` 来自同编号复验，不代表首次导入耗时。

## 恢复点

- 旧程序：`/opt/understand-book/releases/current-20261015`。
- 升级前完整快照：`/opt/understand-book/backups/online-publication-20261015`。
- 原服务、Nginx 与制作沙箱配置：`/opt/understand-book/acceptance/online-publication-20261015/previous-configuration`。
- 部署过程中在恢复公网流量前设有失败恢复路径；本次没有触发。公网开放后如需回退，必须保留并核对新增数据，不能直接用旧快照覆盖现网。
- 旧 schema13 程序不能直接打开 schema14 数据根。前端继续使用前版资源；新 release 的 packages、apps 等资源链接到保留的旧 release，不能单独删除旧目录。

## 已知问题

服务器约 4 GiB 内存、无 swap；首次测试构建在服务器重启前未完成。恢复后限制构建内存、CPU 与并行度，完成 Linux 测试和正式构建。SSH 握手仍偶有断开，重试后可用。

验收脚本最初错误地要求书架只返回一个发布，随后按现有 `is_default` 合同修正。默认 Python HTTP 请求曾返回403；改用项目认证客户端、统一 User-Agent 复核，确认 reader 的应用层拒绝为404（材料不可用）。这些修正只涉及验收脚本，未修改或重启已部署程序。

上线验收时，清理本次已完成的 81 MiB 测试可执行文件后，磁盘可用约 235 MiB。随后按用户要求完成下述磁盘清理。

## 磁盘清理

2026-10-15 23:01 HKT，清理8个发布版本下的32个 Rust 编译缓存目录（`target/release` 内的 `deps`、`build`、`incremental`、`.fingerprint`）。删除前确认没有运行进程从这些目录加载程序或动态库。

释放4.07GiB，可用空间由233MiB增至4.30GiB，文件系统使用率由100%降至89%。服务PID4476不变；新旧版可执行文件、运行资源、书籍、用户数据和所有备份均保留。正式HTTPS首页200，未登录身份接口按合同返回401。见[清理回执](disk-cleanup.json)。下次编译将重新生成已清理的缓存，不影响现有程序运行。
