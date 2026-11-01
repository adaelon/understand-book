# Understand Book 服务迁移 — 2026-10-31

状态（2026-10-31，DNS 修改后复验）：网站和现用数据服务已迁到 `184.195.172.171`。用户已修改 Cloudflare DNS；动态请求实测确认 Cloudflare 直接访问新源站。旧服务器继续保留临时转发和原始数据。

## 运行位置

| 项目 | 当前值 |
| --- | --- |
| 正式网址 | https://understandbook.top |
| 新服务器 | `184.195.172.171`，Ubuntu 24.04 |
| 正式服务 | `understand-book-multi.service`，已启动并启用开机启动 |
| 程序版本 | `/opt/understand-book/releases/note-thinking-20261101`（[2026-11-01 后续发布](../note-thinking-20261101/README.md)） |
| 正式数据根 | `/opt/understand-book/data/production-20261031` |
| 环境文件 | `/opt/understand-book/multi-reader.env` |
| 切换时完整备份 | `/opt/understand-book/backups/server-move-final-20261031` |
| 媒体入口目录 | `/opt/understand-book/media` |
| 原材料书库 | `/opt/understand-book/books` |
| 迁移日志与证据 | `/root/understand-book-migration-20261031` |

源服务器 `115.190.121.150` 上的应用已停止并禁用开机启动，Nginx 暂时经验证过的 HTTPS 链路转发到新站。旧机器的原始资料和历史备份保留；用户明确要求历史备份暂不传输，本次没有继续传历史备份。

新站已经接收业务请求，后续数据以新站为准。**不得直接重启旧数据根提供业务，也不得用旧快照覆盖新站。** 如需回退，必须先保存并同步新站此后产生的数据。

## 数据与验证结果

- 最终停写后完成内容比较和增量同步，没有删除原有数据；保存完整备份并通过项目原生 `reader_maintenance restore` 恢复正式副本。
- 27 个账号保留。27 个 SQLite 数据库完成完整性检查、结构核对及逐表内容比较；仅允许原生恢复程序重写发布目录的根路径。
- 925 个私人文件、147,769,832 字节逐字节一致，涵盖数据库以外的个人记录和附件。
- 21 个书库发布版本的 8,779 个登记资源均存在，共 4,110,019,113 字节；保留线上发布 ID。
- reader/admin 实际登录、书目与正文、学习工作区、聊天历史、额度及管理读取通过。授权版本数分别为 18 和 19，与源站基线一致。
- 正式域名经 Cloudflare 和旧站临时转发的登录/阅读验证通过；Windows 外部访问首页 HTTP 200，匿名账号接口 HTTP 401，TLS 验证成功。
- 目标运行环境实际通过中文向量生成、Whisper 模型加载和转写执行、绘图、动画、LibreOffice 三页含隐藏页转换，以及制作任务的资源限制、超时和停止清理验证。
- 恢复演练验证了服务启动时清理遗留制作进程与暂存输入，服务可正常退出。

主要证据位于目标迁移目录中的 `rehearsal-complete.json`、`final-data-validation.json`、`target-ready.json`、`target-public.json`、`public-health-complete.json`、`migration-summary.json`。本机操作资料位于 `tmp/server-migration-20261031/`，凭据不写入公开报告。

## 本地资料复用与传输

用户指出整个书库都有 Windows 副本后，核对了全部线上发布版本。CS336 的 2,731 个非重复文件，以及其他书籍的 2,261 个文件，与源站内容一致；只上传尚未到齐的 4,368 个文件，共 1,401,081,475 字节。其他书籍的 338 MB 内容压缩为约 106 MB 上传。

上传后按已比对的内容校验导入，并恢复源站的时间、模式和账号归属。3,931 个重复图片及三个播放缓存文件在新机本地重建，少通过网络传输约 2.59 GB，最终保留完整原路径。

Windows 单连接 SFTP 实测约 0.266 MB/s；四连接实测约 0.9–1.0 MB/s；八连接剩余阶段实测 2.01 MB/s。单连接慢不能直接推断为带宽套餐不足。摘要比较用于跳过慢线路上的完整文件重传，结果实际决定了传输来源和需要补传的文件。

## 运行依赖

- Node：`/opt/understand-book/tools/node-v24.20.0-linux-x64/bin/node`
- 语义检索：`/opt/understand-book/models-ubuntu/local-embedding.json`，模型 revision 与源机一致，实测 384 维向量。
- 制作运行时：`/opt/understand-book/tools/render-runtime-ubuntu`，浏览器位于其 `browsers/google-chrome/chrome`。
- Whisper：`/opt/understand-book/tools/whisper-venv-ubuntu/bin/python`；模型 `/opt/understand-book/tools/whisper-models-ubuntu/small.en`，模型文件与源机一致。
- LibreOffice：`/opt/understand-book/tools/libreoffice26.8-ubuntu/program/soffice`，版本和 build id 与源机一致。
- 活跃观测队列仍按既有配置位于 `/opt/understand-book/data/multi-reader-inv9-20261009/observability-spool`，已完成最终同步。
- 既有制作服务账号、Polkit 与资源限制已迁移。Ubuntu 仅为 bwrap 配置 userns AppArmor profile，保留全局限制。

## 清理与保留

临时迁移 SSH 授权已从源机移除，新机上的对应临时私钥已删除。恢复演练的两份大目录、第一次未公开切换的副本，以及本地上传的临时归档已清理；最终正式数据、最终完整备份和原始源机资料保留。清理后新机约有 17 GiB 可用空间。

源机的旧 IP 专属证书续期 timer 继续为旧 IP 到域名的 HTTPS 跳转服务。新站正式域名使用已验证的 Cloudflare Origin CA 证书。

## DNS 切换确认与功能复验

用户已在 Cloudflare 将根域名 A 记录改为 `184.195.172.171`，保留橙云代理。2026-10-31 15:24 香港时间，以旧站专用临时响应头作阳性对照，旧站直连请求带标记，两次经 Cloudflare 的动态、不缓存账号请求均不带标记，确认已直接进入新站。验证后恢复旧站原转发配置，未停机。

- 普通账号与管理员登录、注销通过，可见版本分别为 18、19，合计覆盖 20 个不同发布版本。全部 20 版目录及首、中、末段正文抽样可读；另一份未向这两个账号授权的发布版本沿用迁移时完整文件验证，不额外授予权限。
- 19 版全文搜索正常；测试资料 `sample-book` 的搜索存在迁移前已有的异常，见已知问题。
- 三份 PDF 原件的 4 KiB 范围请求、站内图片抽样、三版音乐的音频范围请求、SVG 乐谱和 MusicXML、CS336 视频原件与 HLS 清单/媒体范围请求、Windows 安装包下载均通过。
- 个人笔记、笔记详情、既有聊天详情、学习记录、工作区详情、阅读接续、额度、账单用量、MCP 配置及共享工作区目录读取通过。读取了 reader 的三条笔记和一个已有工作区，没有新增业务内容。
- 实际浏览器检查了首页、普通账号登录、18 个书目卡片、已有工作区与阅读接续、刷新保持登录、退出；桌面和 390 px 手机视口均正常，手机文档宽度为 390 px，无横向溢出。管理后台登录、用量页刷新和已有账号详情可用。
- 正式 AI 配置在新机完成一次最小实际响应：`deepseek-v4-flash` 返回 `OK`，HTTP 200，约 0.7 秒，用量 39 输入 + 34 输出 token。此调用没有写入用户聊天记录。
- 邮件配置存在，新机可与邮件服务建立 TLS 1.3 连接。四本含外链图片的资料分别抽查一张，范围请求均返回 206。
- 当前制作能力接口返回 `authoring/plot/animation/preview = true`；绘图、动画、Whisper、Office 转换及任务清理沿用本次迁移过程中已经实际执行并通过的运行时验证。

本机证据：`tmp/server-migration-20261031/dns-route-confirmed.json`、`post-dns-api-checks.json`、`post-dns-followup-result.json`、`post-dns-browser.json`、`post-dns-admin-browser.json`、`post-dns-provider-result.json`、`post-dns-dependencies-result.json`。服务器相应证据位于 `/root/understand-book-migration-20261031/`。原始接口检查中六处 `source_manifest` 404 属于视频、音乐和演示资料未提供 PDF 来源清单，不是媒体内容缺失。

旧服务器和切换时完整备份继续保留，新站仍是唯一业务写入端。当前没有待执行的 DNS 操作。

## 已知问题与验证边界

- 《西方经济学》既有 `publication.json` 将 `asset_manifest.json` 长度记为 103094，源站实际文件为 103883 字节。已确认源站、迁移副本和恢复副本内容完全一致，保留原件并记录这处既有清单差异；没有修改线上书籍内容。
- `sample-book` 是内容为 100 个 `X` 加“尾巴”的既有测试资料，原文 102 字符，目录叶节点只覆盖前 100 字符。全文搜索因未归属的尾部正文返回 HTTP 500。已逐字节比较旧、新服务器的 `source.txt` 和 `base.json`，完全相同；正文读取正常。保留原件，未在迁移验证中改写已发布版本。
- 首次私有启动后，检查脚本直接比较 Python tuple 与落盘 JSON array，产生健康比对误报。系统在开放新站前自动恢复旧站。修正为统一 JSON 表示后，用实际新站登录验证完全一致，再重新停写、同步、备份和恢复；第二次切换成功。第一次未公开新站，没有用户请求写入目标。
- 已执行一次最小 AI 响应验证；没有发送测试邮件，也未在正式账号中创建长对话、长视频生成或更改笔记。注册/重置密码的邮件实际投递、完整长任务不在本轮实操结果中。Whisper 验证证明实际加载和转写执行，不是识别准确率评测。

## 后续操作入口

新机器上的 Claude 可从本目录说明和 `/root/understand-book-migration-20261031/migration-summary.json` 了解实际运行位置。现有业务服务是 `/opt/understand-book` 下的正式部署；`/root/projects/understand-book` 是开发项目目录，两者用途不同。

迁移脚本已经完成其一次性任务，临时传输授权已撤销，**不要重新运行 `cutover.py prepare/finish` 或早期预复制脚本**。后续发布应基于当前正式服务和新数据根执行项目既有发布流程。
