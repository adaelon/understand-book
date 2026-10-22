# 工作区资料展示、学习成果保存与 Windows 0.3.4 发布

最终服务于 **2026-10-21 20:32:05 HKT** 完成上线。Windows 已从 0.3.3 升级到 **0.3.4**，本机安装目录 `E:/allwork/Understand Book`。网站：[Understand Book](https://understandbook.top)；[Windows 0.3.4 安装包](https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.4)，4,897,161 字节，本地副本 `dist/UnderstandBookSetup.exe`。

## 已发布行为

- 工作区分为封面导航、当前学习目标和资料卡；继续学习为主要操作，添加资料、创建与管理操作收起呈现。
- 书籍使用已有封面或 PDF 首页，视频使用已发布画面。共享课程和学习路线显示可读标题及能力标签。
- 线上 CS336 视频原始导入文件实际为 `external.mp4`。正式标题、导入标题、普通文件名沿原顺序使用；临时名称 `external` 在可访问、单一资料的课程中回退到课程名。此课实际显示 **CS336 第 2 讲：PyTorch 与资源核算**。只引用用户可访问的课程版本，按完整资料发布引用匹配。
- 存储套餐只约束书籍、视频、课件等原件上传。演示页、交互状态与学习成果不再被无套餐、套餐到期或原件空间已满阻断。旧成果文件保留，旧 generated 记录退出上传空间统计。

## 验证

- 发布候选前端 **42 项**通过，类型检查与生产构建通过；1440、1024、390 像素宽度检查封面、卡片边界及工作区交互，无页面错误。
- 保存范围修复的 **13 项服务端回归**通过，覆盖无套餐、过期、空间满时保存与重开，以及原件上传和旧成果保留；见 [回归日志](../upload-storage-scope-20261021/learning-artifacts.log) 与 [上传回归](../upload-storage-scope-20261021/upload-regressions.log)。
- 两项视频目录测试通过，覆盖原文件名、封面字节、资料权限与临时导入名的课程标题回退。
- Linux 初次六个程序 Release 构建通过；标题补充修复仅重新构建 server。最终服务 PID **541359**、schema **20**。
- 切换前后普通读者 12 个、管理员 13 个授权资料版本保持一致，正文、工作区、演示运行环境及账号入口正常。
- 线上视频封面返回 PNG，396,570 字节；无套餐账号依然不能上传原件，空间明细仅含 upload。
- 真实公网登录后，资料卡只显示一个 CS336 视频，封面可解码，课程路线使用真实标题，无 UUID 或 external；没有创建工作区或调用模型。
- Windows 真实 WebView2 验证 TXT、Markdown、EPUB、PDF 阅读、删除原文件后的副本阅读、断网阅读、云端入口与返回本机书库通过。验证用隔离资料目录。
- Windows 加载新版 `/assets/reader-s2772UIH.js`，原生桥正常、页面错误为 0。本机安装程序退出码 0，安装后的 EXE ProductVersion 为 0.3.4。
- 公网 JS/CSS 与候选逐字节一致；HTML 仅多出原有 CDN 分析脚本。普通及带版本参数的安装包下载均 HTTP 200、逐字节一致。
- 验证未调用模型或付费搜索服务。

## 发布与恢复

当前目录 `/opt/understand-book/releases/workspace-library-title-20261021`；数据根沿用 `/opt/understand-book/data/multi-reader-to-tb-20261012`。本次没有数据库迁移。

以当时线上 `reader-latency-20261021` 源码为基线，仅合入本次展示与保存范围差异。初次发布后，真实课程验收发现原导入名称为 external，追加标题回退修复。原差异在 release.patch，追加差异在 title-fix/release.patch。当前工作树中其他尚未发布的阅读接续改动未混入。

两次切换均确认没有活动回答、未保存运行及在途费用，然后备份配置和可变数据。维护时长：首次 13.13 秒；第二次未留存（服务恢复后的最终目录验收触发登录限流，冷却后复核通过）。首次脚本因 Windows 换行在执行前退出，改为 LF 后完成切换。最终验收期间并行登录达到已有的每用户每分钟五次上限；等待窗口结束后顺序复核，没有修改限流策略或恢复旧数据。

最终配置与可变数据备份：`/opt/understand-book/backups/workspace-library-title-20261021`。上一个发布目录 `/opt/understand-book/releases/workspace-library-20261021` 保留；更早的 `reader-latency-20261021` 也保留。回退需先等待当前回答完成，恢复对应服务单元、环境、nginx 和 sandbox worker 路径，再启动旧服务。schema 相同，保留当前用户数据，不用历史数据库备份覆盖新增会话。

Windows 0.3.3 安装包仍在 `tmp/reader-latency-release-20261021/windows-source/dist/UnderstandBookSetup.exe`，原安装 EXE 备份在本次暂存目录。

## 验证边界

- 封面布局的多尺寸截图使用接口夹具；public-library.png 与 public-course-route.png 为真实公网账号验证。
- 本次针对发布范围运行定向测试，没有重新运行整个仓库测试集。无画面的视频仍使用标题封面。
- 已存在的会话与资料沿用；这次没有重新生成先前保存失败的演示内容。
- 未提交或推送 Git。
