# MU9 旧资料迁入、备份与回滚验证

日期：2026-09-30。实现和命令：[MU9 说明](../Linux多人阅读-MU9迁移恢复.md)。基线为实时工作树，HEAD `5e10516`；MU0–MU9、RE 和其他既有工作尚未提交，不能仅按 HEAD 重建本轮版本。

## 结果

| 验证 | 结果 | 检测目标 / 证据 |
|---|---|---|
| 最终 MU9 定向 | 11 通过、0 失败 | T60–T64、未完成根、同操作冲突、恢复中断、未知材料 / 原 PDF 绑定；[日志](linux-multi-reader-mu9-20260930/mu9-final.log) |
| 多人相关回归 | 112 通过、0 失败、1 原有忽略 | MU2–MU8 的私人权威、授权、现场、接单、SSE、取消 / 恢复及此前 8 项 MU9；[日志](linux-multi-reader-mu9-20260930/multi-user-regression.log) |
| 旧 Node 目录重定位 | 3 通过、0 失败 | Windows 长路径、中文 / 空格、多书进度、冲突前零写入、正文不替换、重复执行不改字节 |
| 离线 CLI 构建 | 通过 | `reader_maintenance` 入口与库 API 链接；[日志](linux-multi-reader-mu9-20260930/cli-build.log) |

环境为 Windows PowerShell、本地 SQLite 3.53.2；服务 control schema 4 不变。单独指定 TEMP/TMP 目录隔离历史测试文件。已有 ts-rs 属性提示和原 NetworkRunPort 未使用方法警告保留。一个忽略项是需单独启动的 MU8 浏览器服务夹具；Linux 专属 MU7 隔离用例在 Windows 不参与编译。

多人回归之后仅补充 MU9 模块的独立恢复续接、备份目录占用和材料映射判定；最终 11 项定向测试与 CLI 构建覆盖这些收口改动。通过数包含重叠用例，不相加当作独立用例总数。

## T60–T64 证据

| ID | 实际注入与断言 | 范围 |
|---|---|---|
| T60 | 预览不创建账号数据 / 备份；迁入仅到 A，B 空；Memory 字节、note/chat/turn ID、Learning evidence 与 turn 引用、演示版本和私人成果保留；文件复制后与元数据提交后中断，正常启动被拦截，续跑不重复；同计划重跑核对，变更账号 / 目标内容拒绝 | 本地真实文件、SQLite 和 UserRegistry |
| T61 | 持有服务锁时，preview / migrate / backup / export-user 均返回 SERVICE_WRITER_BUSY；原 MU2 子进程锁、句柄保活和退出释放回归通过 | 维护路径 + 原进程锁回归 |
| T62 | 多用户服务备份后实际恢复至独立根；未 checkpoint 的已提交 WAL 账号仍存在；A/B Learning 可读；原 evidence/session/turn 关联与真实 PresentationStore 版本可读取；发布包在新根加载且正文相等；恢复提交后中断，重跑成功且备份字节不变 | SQLite Backup API、发布根重定位、真实读取 |
| T63 | 显式导出 A 到独立 memory/private/books；没有 B 或 control.sqlite；原单读者 MemoryStore、LearningStore、load_session、Book::load 可读取；存在网络未结接单时拒绝；迁入前服务备份可恢复成无私人数据的原账号状态 | 工具和当前单读者读取器；真实旧 release 切换归 MU11 |
| T64 | 原服务同时保存旧 pending 与完整网络 queued，整体备份 / 新根恢复；旧 pending 变 INTERRUPTED，新 queued 保留原 turn / 输入，不提前调用模型，原回合只领取一次，同请求键仍返回原 turn | 原 MU6 恢复与固定模型替身 |

附加验证覆盖：源 / 目标 / 备份重叠、未知私人文件未审核、session 未映射、正文不符、未来 control schema、备份目录已有数据、仅 Learning/Tutor 存在的未知材料、原 PDF 绑定不符。均有具体拒绝断言；计划验证失败时不创建维护 marker。

## 命令

在仓库根执行，TEMP/TMP 指向本次独立临时目录：

```powershell
$env:TEMP = Join-Path (Get-Location) 'tmp/mu9-tests'
$env:TMP = $env:TEMP
cargo test --offline --locked -p server --lib mu9_ --target-dir .tmp-rust -- --test-threads=2 --nocapture
```

多人回归使用单独的 `tmp/mu9-regression`：

```text
cargo test --offline --locked -p server --lib mu --target-dir .tmp-rust -- --test-threads=4 --quiet
node --test scripts/linux/relocate-reader-paths.test.mjs
cargo build --offline --locked -p server --bin reader_maintenance --target-dir .tmp-rust
```

## 失败与修正

1. [首次 7 项运行](linux-multi-reader-mu9-20260930/initial-unknown-files.log)：2 通过 / 5 失败。Memory 正常生成的 reader-profile.md / reading-handbook.md 未被识别，审核门槛在进入迁入前拒绝；按已存在的生产文件名修正分类。
2. [第二次 7 项运行](linux-multi-reader-mu9-20260930/windows-path-assertion.log)：6 通过 / 1 失败。导出位置属于正确的新根，但测试用非 canonical 路径比较 Windows 长路径前缀；修正为双方解析实际路径后比较，后续 Book::load 和位置断言通过。
3. 后续补充完整 evidence / PresentationStore 读取、恢复中断和材料映射反例，最终 11 项通过。没有降低业务断言或删除既有测试。

## 已知限制

本轮未选定或迁移生产个人资料，未连接生产入口、切换 systemd/Nginx 或调用付费模型。`--stopped` 依赖操作者停止旧二进制；新锁无法约束不识别此协议的旧程序。原始文件与备份保留，恢复不会覆盖旧根。

真实 Linux 掉电 / kill 时序、完整跨平台矩阵、真实旧 release 二进制与新格式兼容性、HTTPS/iPhone 和入口切换仍按 MU10/MU11 验收。T63 本轮证明独立导出根可由当前单读者读取器读取，不能将其写成某个未运行旧版本的实机回滚通过。快照不包含运行环境外部的 Provider 凭据和工具安装，部署者须按运行单配置。
