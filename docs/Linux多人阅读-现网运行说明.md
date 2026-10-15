# Linux 多人阅读现网运行说明

2026-10-15 18:24:16 HKT 已发布目录长标题重叠修复，沿用 `current-20261015`。目录行按内容高度展开；线上《深入理解ai agent》292项在1440×900、1100×440、390×844均无文字越界，搜索及滚动通过。仅更新前端，服务PID 101669保持。刷新页面加载新版；见 [验证与恢复记录](performance/outline-wrap-20261015/README.md)。

2026-10-15 15:15:29 HKT 已发布当前工作区的产品改动，release 为 `current-20261015`，Tutor 指导 `tb4.v5`，服务 PID 101669、active。包含正文/PDF专注分页、精确读位与来源接续、相关阅读器修复及新出题规则。15:30:19 HKT 同版追加首页内测群二维码，支持查看原图与保存，保留10月22日前有效提示；15:53:32 HKT 加入版权页脚、服务条款、隐私说明、联系我们与关于我们入口。两次首页更新均未重启服务。数据根与 schema 13 沿用，账号、材料、聊天及笔记保留。FP7 剩余性能验收按用户明确指令暂停，不宣称完整通过。见 [发布与回退记录](performance/current-release-20261015/README.md)。下列旧日期段落保留历史。

2026-10-14 17:55:48 HKT 已发布理解判断问题的区分力与按需追问规则。现网 release 为 `tutor-question-discrimination-20261014`，Tutor 指导 `tb4.v3`，服务 PID 53917、active。新回合中的理解判断问题须要求未被代答的关系运用；证据不足保持尚未确认，按下一步教学需要追问。数据根与 schema 13 沿用，公网首页、登录、11 本授权材料及四项制作能力验证通过。见 [发布与恢复记录](performance/tutor-question-discrimination-20261014/README.md)。下列旧日期段落保留历史。

2026-10-13 22:41 HKT 已发布《这一刀怎么补》（`last-hit-positioning`）与演示长图导出。现网 release 为 `last-hit-default-grants-20261013`，schema 13，4 个现有账号均已授权，以后创建的账号自动获得该书。演示工具栏点击“导出长图”可预览并下载保留当前 HTML 布局及参数的完整 PNG。服务 PID 3526、active；数据根沿用。实现、验证及恢复点见 [发布记录](默认材料授权与补刀站位发布.md)。

2026-10-13 21:46:44 HKT 已发布演示工作区布局修复：演示正文独占左侧高度，关联教学与作答移到右侧对应回合；从问答全屏打开演示时使用正确分栏，收起恢复原全屏状态。沿用 `tutor-response-fit-20261013` release，静态资源更新为 `reader-CfJ2PVJy.js` / `reader-BpRNSPKW.css`，服务 PID 8915 持续运行。正式首页、阅读入口与直接引用资源逐字节一致。见 [修复、验证与回退记录](performance/presentation-layout-fix-20261013/README.md)。

2026-10-13 20:58 HKT 已发布 Tutor 表达判读与记忆适配修正，现网 release 为 `tutor-response-fit-20261013`，PID 8915。Tutor 指导采用 `tb4.v2`，历史教案可按原文与当前表达修订；指定错误理解已追加更正，原话、帮助条件和历史对话保留。数据根与 schema 12 沿用，公网首页、登录和四项制作能力 ready。原演示修复包含于此版本；详见 [实施与验证记录](performance/tutor-response-fit-20261013/README.md)。

2026-10-13 19:51 HKT 已发布演示环境与失败终答修复，现网 release 为 `presentation-recovery-20261013`，数据根仍为 `multi-reader-to-tb-20261012`，schema 12；既有品牌与阅读前端保持。已修正生产执行器目录归属、隔离历史恢复演练实例，并让工具发现和终答反映真实环境状态。实际服务账户下的绘图、写页、三视口预览及交付通过，公网制作四项能力均 ready。见 [故障与部署记录](performance/presentation-recovery-20261013/README.md)。下列旧日期段落保留历史。

2026-10-12 08:53 HKT 已整体发布 main 合并提交 `8497f29`（Tutor TC0–TC6、Reader 与 MCP0–MCP9），MCP10 正式 HTTPS 和恢复验收完成。现网 release 为 `mcp10-20261012`，数据根 `multi-reader-mcp10-20261012`，schema 12；服务 active。MCP 入口 [https://understandbook.top/mcp](https://understandbook.top/mcp)，本人从 [MCP 接入](https://understandbook.top/?account=mcp) 创建凭证。旧账号、材料、私人文件和模型费用保留，完整版本／恢复边界见 [MCP10 运行单](内测MCP-发布运行单.md)。下列旧日期段落保留历史。

2026-10-09 20:00 HKT，INV0–INV9 与 T15–T18 已同版发布。schema 9，正式域名注册/找回/后台和原账号阅读复核通过，真实收信在隔离同版候选通过。现网此版因缺少 Tutor 必要接纳回执阻塞旧书；随后已按用户要求修改源码，直接使用已有 Pass1、discourse 与 BookStructure，修复已于 20:32:02 HKT 部署并通过公网与实际 Tutor 界面复核，书籍无需重建或重新导入。见 [Tutor 上线记录](performance/tutor-artifacts-deployment-20261009/README.md)。版本、数据根、备份和证据见 [INV9 发布运行单](邮箱账号-INV9发布运行单.md)。

2026-10-09 15:26 HKT 已发布无文字翻页加载动画：56px 书页、1.85 秒一轮，入口文件下载、登录恢复、初始书架与工作区加载连续显示，内容就绪立即退场。15:32 HKT 公网登录、打开书籍与刷新恢复验收通过，脚本错误 0、问答模型请求 0；原服务持续 active。刷新页面加载新版。见[发布与回退记录](performance/reader-loading-20261009/README.md)。

2026-10-09 14:55 HKT 已发布阅读助手连续对话布局：单行输入按需展开、任务收为一行并按需打开详情、顶部一行导航、发送与停止共用一个位置。刷新已打开页面加载新版。15:00 公网登录、打开书籍、历史、草稿与桌面/手机布局验收通过，模型请求 0；仅更新前端，原服务进程和私人数据保持。见 [发布与回退记录](performance/reading-assistant-space-20261009/README.md)。

2026-10-09 正式入口已切换为 [https://understandbook.top/](https://understandbook.top/)，后台为 [https://understandbook.top/admin/](https://understandbook.top/admin/)。Cloudflare A 记录代理到 `115.190.121.150`，用户已确认 Full (strict) 与 Always Use HTTPS；源站安装 Origin CA 证书，Nginx Host 与后端 Origin 同步使用域名。旧 IP HTTPS 入口以 308 保留路径跳转到域名。公网证书、管理员登录/后台 API/退出及源站路由验收通过，模型请求 0。详见 [域名接入记录](performance/domain-20261009/README.md)。

2026-10-08 已切换到 `/opt/understand-book/releases/adm10-20261008`，schema8。ADM运营后台、账号额度、EX14.0–3（ex14.v4 / Manim）与近期阅读器修补已上线。后台入口 `https://115.190.121.150/admin/`，用户名admin，密码单独交付。reader/puff/adaelon/admin各10元成本额度、30天；付费开通收款11元授予10元；联系19847595883。旧标签页刷新加载新版。备份、验收和费用差额见[ADM10发布运行单](运营后台-ADM10发布运行单.md)。以下旧日期段落保留历史。

2026-10-02 已发布对话排序与闲置恢复修补，随后EX13/JL上线，当时 release 为 `/opt/understand-book/releases/ex13-jl-20261002`。历史列表与问答正文最新在上；返回网页和重试会重新绑定失效现场并保留同一对话草稿，运行恢复回执补齐展示字段。发布与实机复验见[修复记录](修复记录-Linux对话排序与闲置恢复.md)及[EX13/JL上线记录](Linux上线-EX12-EX13-JL.md)。旧标签页刷新一次加载新版。

2026-10-03 16:51，AI Infra全书BSR7结构已发布为新的默认书籍版本`01a100f5-e3c3-72f3-8bfb-eb8a08774ab5`：14单元、956重点、345主干点、11跨章主线。沿用原正文/base与图片，补入结构、篇章索引和修正后的公式释义；reader、puff、adaelon继承原权限，旧版与私人数据保留。HTTPS实际读取全量通过，服务active，模型请求0。[发布与验收记录](performance/book-structure-bsr7.md#linux书籍同步)。已打开的旧现场仍绑定旧发布，从材料选择中打开新版（默认）即可使用。

已部署：2026-10-01 MU12 布局与首屏修补，以及 MU13 刷新传输修复。刷新请求时间线及最终实测见 [MU13 记录](performance/linux-reader-refresh-root-cause-20261001.md)，布局见 [MU12 记录](performance/linux-reader-layout-startup-20261001.md)。迁入与制作环境配置沿用 [MU11 记录](performance/linux-multi-reader-mu11-20261001.md)。

同日 15:29 已追加选书封面：原书图片、PDF 首页与书名回退，15:30 公网登录、手机布局、选书及正文验收通过。见 [选书封面发布与验收](选书封面.md)。

同日 16:31 删除正文“高亮整段 / 记笔记”浮动条，保留选中文字后的操作与已有批注。仅更新前端，无服务重启、无数据迁移；旧源码与网页 index 位于 `/opt/understand-book/backups/reader-toolbar-removal-20261001-162958`。见 [发布回执](performance/reader-toolbar-removal/deployment.json)。

16:32 公网验收：桌面与 390px 手机点击正文均无段落浮动条；选中文字仍能打开笔记编辑器，取消后正常返回阅读，页面脚本错误为 0。已有批注查看/编辑/删除由组件测试覆盖；本次线上所选段落无已有批注。见 [验收结果](performance/reader-toolbar-removal/verification.json)。

## 入口与版本

- 正式入口：`https://understandbook.top/`；后台 `https://understandbook.top/admin/`；旧 `https://115.190.121.150` 保留路径以 308 跳转到域名，历史 8080 入口经旧 IP 跳转。
- release：`/opt/understand-book/releases/current-20261015`，Tutor 指导 `tb4.v5`；包括专注阅读、首页内测群二维码与网站信息页脚。Rust `--release`，schema 13。此前 release 保留，恢复位置见[本次发布记录](performance/current-release-20261015/README.md)。
- 服务：`understand-book-multi.service`；账户 `understand-book`；后端 `127.0.0.1:8788`。
- 数据：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 13；旧资料归属账号 `reader`。
- 环境：`/opt/understand-book/multi-reader.env`，0600；沿用旧 Provider 路由和 Key。
- 初始登录资料：远端 `/opt/understand-book/multi-reader-credentials.json`，0600；本地交付另存用户私人目录。凭证不进入仓库或运行证据。

查看状态与日志：

```bash
systemctl status understand-book-multi
journalctl -u understand-book-multi -n 100 --no-pager
```

## HTTPS 续期

正式域名的源站证书为 `/etc/nginx/ssl/understandbook.top.pem`，私钥为 `/etc/nginx/ssl/understandbook.top.key`，覆盖根域名与一级通配子域名，有效期至 2041-10-05 04:37 UTC。Cloudflare 管理浏览器侧证书续期。Origin CA 证书用于 Cloudflare 到源站的连接，正式域名保持橙色云朵代理；若将来需要域名直连源站，改用公开可信的域名证书。

后续发布继续使用 `server_name understandbook.top`、`proxy_set_header Host understandbook.top` 与 `UNDERSTAND_BOOK_ORIGIN=https://understandbook.top`。旧 IP 证书继续服务旧书签的 HTTPS 跳转，其续期机制如下。

证书 `/etc/letsencrypt/live/understand-book-ip/{fullchain.pem,privkey.pem}` 覆盖公网 IP，使用 Let's Encrypt shortlived profile。申请方式见 [官方说明](https://letsencrypt.org/2026/03/11/shorter-certs-certbot)。现有 80 端口站点只增加 ACME challenge 路径，其他路由保留。

`understand-book-cert-renew.timer` 每六小时检查续期，带最多 30 分钟随机延迟和 Persistent；成功后 deploy hook 先验证 Nginx 配置，再 reload。首次模拟续期已成功。检查入口：

```bash
systemctl list-timers understand-book-cert-renew.timer
journalctl -u understand-book-cert-renew.service --no-pager
/opt/understand-book/tools/certbot/bin/certbot certificates --cert-name understand-book-ip
```

续期依赖公网 80 端口的 `/.well-known/acme-challenge/` 可访问，目录 `/var/lib/understand-book-acme`。不要只替换证书文件而移除续期入口。

## 迁入与旧数据

本次材料目录共 9 项。四个旧来源清单在迁移副本中将 `truth_file=source.txt` 对应的 canonical 路径归一化为 `source.txt`，补齐发布合同要求的来源身份；正文与 base 未改。旧测试 `missing-paper` 路径和正式论文目录内容一致，使用正式目录的阅读位置；原位置、原 session / registry 和来源清单存入该账号的 `private/mu11-legacy-metadata.json`。

停写后的原始私人数据与配置：`/opt/understand-book/backups/mu11-legacy-raw-20261001`。迁移 operation 与完整快照：`/opt/understand-book/backups/mu11-production-20261001`。旧 `/opt/understand-book/data/lx6` 与 `/opt/understand-book/books` 均保留。

迁移计划与副本：`/opt/understand-book/acceptance/mu11/production-migration.json`、`production-input`。快照包含完整材料副本；用户记忆和原私人文件逐字节核验，聊天原字段保持，补确切发布绑定。迁入完毕后不重跑该导入计划来覆盖新业务写入。

## 对话历史与演示环境

对话历史按当前材料显示。新打开材料时建立独立阅读现场，默认尚未选择聊天；空白对话不表示历史丢失。点击“本书历史”或空白区的历史入口可以打开旧记录。迁入的 17 段对话分布为 `ai-agent-engineering` 6、`quantification-essence` 7、`mastering-rust` / Transformer 论文 / `quickstart-demo` / `sample-book` 各 1；其他三份材料在旧数据里没有对话。

演示配置为服务根 `presentation-sandbox.json`，worker 使用现网 release，Python 运行时 `/opt/understand-book/tools/render-runtime`，浏览器 `/usr/lib64/chromium-browser/chromium-browser`。运行时沿用已验收的 Matplotlib 3.11.0 / Manim 0.21.0 / PyAV 17.1.0。`/etc/polkit-1/rules.d/49-understand-book-render.rules` 授予专用宿主账号 systemd 的 `manage-units` 权限；生产模板及权限边界见 [发布运行单](Linux多人阅读-MU11发布运行单.md)。生成任务仍为 nobody，使用独立文件系统、网络命名空间和原有资源限额。制作状态从登录后的 `/api/auth/me` 查看，改配置后需重启宿主；浏览器刷新后显示新能力。

## 账号、备份与回退

新增 / 改密 / 禁用账号前先停服务，操作后启动；密码从 stdin 输入：

```bash
systemctl stop understand-book-multi
/opt/understand-book/releases/mu12-20261001/target/release/manage_reader /opt/understand-book/data/multi-reader password reader
systemctl start understand-book-multi
```

材料授权使用同 release 的 `publish_book`；参数见 [MU11 运行单](Linux多人阅读-MU11发布运行单.md)。日常恢复点使用 `reader_maintenance backup`，目标必须是新的独立目录；停服、备份、启动按 [MU9](Linux多人阅读-MU9迁移恢复.md) 执行。

旧服务为 `understand-book.service`，旧 release `/opt/understand-book/releases/working-tree-c12cbb9-huawei-viewport-20260919-1140`。切换后停用旧服务自启；原 Nginx 8080 配置存于原始恢复点的 `configuration/understand-book-reader.conf`。

回退前先停止新版并保留最新恢复点。若新版已产生写入，先按 MU9 导出 reader 并验证旧程序读取，再决定回退目标；不能直接切回旧 lx6 根而丢弃新写入。若尚无新写入，可以恢复原 8080 配置、停用新版、重新启用旧服务。每次 Nginx 改动先 `nginx -t`，成功后 reload。

## MU12 页面与程序回退

MU12 仅更新程序与 Nginx 配置，沿用原私人数据根；配置回退点为 `/opt/understand-book/backups/mu12-config-20261001`。版本静态资源一年 immutable 缓存并启用 gzip，HTML 与 API 不缓存。页面点击“收起”后用右上角“菜单”恢复，账号/退出位于工具栏“账号”内。当前一次公网选书至正文约 2.55 秒，详见 MU12 实测。

## MU13 刷新修复与回退

MU13 在原 MU12 release 内追加 HTTP/2、所有 JSON API gzip、PDF 已知长度及首个请求即限定字节范围的按需读取。API 仍不缓存、逐次授权，SSE 不受 JSON 压缩影响。当前一次公网稳定刷新：Rust 文字书约 0.315 秒，Transformer PDF 约 4.079 秒，不能套用为所有材料/网络的固定时间。Nginx 原配置在 `/opt/understand-book/backups/mu13-nginx-20261001.conf`，原 Server / 网页 index / 相关源文件在 `/opt/understand-book/backups/mu13-pdf-20261001`；回退二进制前同样先检查活动/未保存问答，并在停服后恢复。旧网页资源保留，恢复 index 即可回退网页版本。

## 选书封面回退

封面变更沿用 MU12 release 与原数据根，仅叠加封面相关源码；未迁移书籍、聊天或笔记数据。发布前恢复点 `/opt/understand-book/backups/book-covers-20261001-151852` 保留原源码、五个程序和网页 index。回退程序前检查活动/未保存问答并停服；网页旧静态资源保留，可恢复旧 index。发布回执见 [deployment.json](performance/book-covers/deployment.json)。

## 已知限制

用户明确决定 MU10 预构建相关失败不阻挡此次部署；原始失败与单独复跑结果仍保留。当前机器没有完成冻结的 8 GiB / 40 GiB 空闲容量和完整渲染混合负载验收；实体 iPhone、Tauri 完整来源 / 富呈现、RE7 正式验收仍单列。网络制作在补齐运行时与宿主权限后已开启，四项 capability 均为 true / ready；模型问答沿用已配置 Provider。


## 2026-10-13 23:44 演示长图高清与顶部品牌栏

在现有 `last-hit-default-grants-20261013` release 更新前端：`reader-CI6pWOjO.js` 提供 4 倍像素长图及顶部 logo/understand-book 品牌栏。后端 PID 3526 保持运行。23:45 核对用户入口 `index.html?account=login` 返回新程序，线上文件与已验证多用户生产构建字节一致。旧入口和源文件备份位于 `/opt/understand-book/backups/presentation-export-quality-20261013T154355Z`，旧静态资源保留。详见 [发布与验证记录](performance/presentation-export-quality-20261013/README.md)。
