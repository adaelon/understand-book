# main 两分支合并版本上线记录（2026-10-10）

已上线并完成线上验证。正式站点：https://understandbook.top/ 。管理后台：https://understandbook.top/admin/ 。

部署提交：`12b765bb33fe42fcd30fcda743187241ed56d1f3`，与本次拉取的 `origin/main` 一致。部署源为该提交的独立归档，未包含工作目录中其他未提交修改。合并内容包括 PDF 阅读、演示交互、对话记忆，以及 RN1–RN4 阅读笔记和 RS1–RS8 分享／阅读接续。

## 发布与数据

2026-10-10 19:49:42（香港时间）停止旧服务，19:49:56 启动新服务。运行中的可执行文件已与新发布目录的程序逐字节比较一致。

| 项目 | 路径 |
| --- | --- |
| 新程序 | `/opt/understand-book/releases/main-12b765b-20261010` |
| 新数据 | `/opt/understand-book/data/multi-reader-main-12b765b-20261010` |
| 旧程序 | `/opt/understand-book/releases/inv9-20261009` |
| 旧数据 | `/opt/understand-book/data/multi-reader-inv9-20261009` |
| 停服快照与配置备份 | `/opt/understand-book/backups/main-12b765b-20261010` |
| 服务器构建和部署记录 | `/opt/understand-book/acceptance/main-12b765b-20261010` |

切换前确认没有正在处理或尚未保存的对话。使用项目的 `reader_maintenance` 在停服后备份、恢复到独立新数据目录。恢复后比较原有数据库各表全部业务行，并比较非数据库文件内容：4 个账号、10 个发布版本、39 条阅读授权、56 个阅读工作区、19 条运行记录及其余管理和计费记录均保留；1,698 个文件中包含 225 个用户私有文件，另有 4 个文件数据库完成逐表比较。证据见 [data-preservation.json](data-preservation.json)。

旧前端的不可变资源也保留在新发布目录，供升级前打开的页面继续请求。新数据目录的演示 worker 路径已切换到新发布目录。编译期间临时创建的 2 GiB 交换文件已关闭并删除。

## 验证

- Linux Release 构建成功：Reader、Admin、server、manage_reader、publish_book、reader_maintenance、presentation_worker。
- Web 单元测试：19 个文件，194 项通过。
- 浏览器回归：PDF、笔记、分享、理解空间、会话回顾等 53 项通过；阅读接续四种宽度另 4 项通过，合计 57 项。
- memory：143 项通过；会话回顾：5 项；PDF 服务：1 项；阅读接续服务：2 项；网络笔记持久化：1 项。
- 演示存储首次运行：20 项通过、3 项失败、5 项忽略。追问相关失败经独立临时目录重跑通过，详见下方已知问题和 [isolated-tests.json](isolated-tests.json)。没有将原失败记录改写为全绿。
- 上线前真实站点验证已通过，详见 [live-before.json](live-before.json)。上线后完成账号登录、材料与 Tutor 就绪状态、原有书籍与笔记读取、刷新后继续阅读、390px 书架、PDF 分段读取与正文文字层渲染、后台用量与内测码深链接访问。记录见 [live-after.json](live-after.json) 和 [pdf-admin-live.json](pdf-admin-live.json)，画面见 [手机书架](live-bookshelf-mobile.png) 与 [PDF 正文](live-pdf-desktop.png)。验证于 19:57（香港时间）完成。

## 已知问题与验证边界

1. 演示测试夹具把不同 memory 文件放在同一临时目录，配套 `learning.db` 因而共用；测试固定的会话时间和序号又生成相同事件身份，导致 `TEACHING_INVALID`。首次运行的两项追问失败，以及再次串行运行时受污染的另一项追问，在各自独立 `TMPDIR` 下全部通过。没有为此修改产品代码。
2. `presentation_state_save_failure_and_invalid_semantics_return_no_receipt` 仍失败：把状态目录替换成普通文件模拟存储故障时，期望 `PRESENTATION_STORAGE_FAILED`，实际收到 `PRESENTATION_PUBLIC_CONTENT_INVALID`。原因是合并后的 `route_with_restore` 在观察校验前读取最新状态，而 `save_state` 将该读取错误映射为内容无效。此时保存被拒绝，没有成功回执；问题在错误分类。正常状态保存和追问用例通过，因此将其记录为本次发布的非阻断问题，保留 main 原始代码。
3. 这次没有运行整个仓库的全部测试。5 个忽略的演示用例是独立浏览器测试宿主或依赖浏览器的场景；不能把它们计为通过。上线验证没有调用付费模型，不代表重新验证了真实模型的生成质量。
4. 新版刷新页面先显示带有“阅读接续”的书架，点击“继续阅读”才重新附着到原工作区。最初上线验收沿用了旧版自动返回工作区的断言，因此超时；页面无脚本异常或 API 5xx。验收步骤已按新版契约调整。
5. 多人服务中的论文缩略导航自动定位仍返回 `501 CAPABILITY_NOT_READY`；旧版本同样未实现此工作区操作。原版 PDF 阅读正常。此次约 16.5 MB 的 Agent Memory 论文在当前网络中约 50 秒完成首屏及文字层渲染，超过最初 30 秒的验收等待时间，延长观察后确认渲染成功，读到 677 个文字片段。该材料的可选 `formula_semantics` 资源返回 404，界面显示“PDF 对齐：部分可用”；上线没有重建已有发布材料。上述响应保留在原始线上记录中，不计为验证完全无告警。

## 回退

旧程序、旧数据和切换前配置快照均保留。若必须回退，先停止服务并备份当前新数据，再将服务、Nginx、环境配置恢复为备份中的版本，检查配置后启动旧程序和旧数据。新版本的记忆数据会从 schema 3 升级至 4，不能让旧程序直接打开新数据目录。直接切回旧数据会回到发布时刻，因此需要单独处理上线后新增数据，不能假定无损即时回退。

数据库比较使用服务器已有 Python 3.12 与 `adm10-20261008/python-deps` 下的 pysqlite3；系统自带 SQLite 不支持 STRICT 表。失败发生在停服前，未修改线上数据。
