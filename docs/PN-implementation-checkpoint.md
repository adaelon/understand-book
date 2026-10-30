# PN 实施与发布进度

## 2026-10-30 本地交互续作

浮窗及跨场景放置已在根工作树实现，尚未发布。默认只保存并记录产生现场；“放到…”支持当前画布的独立卡片/选中元素连线、对话原句所在段落下、演示可识别页或已保存状态。位置引用与正文修订独立，返回/移除不改变产生现场；保存成功后放置失败的设备草稿只重试放置。MemoryDocument 本地版本为 6，v5 有限升级已验证；控制库版本不变。入口及验证见 `docs/performance/personal-notes-20261030/placements/README.md`。本地没有创建提交、推送或部署。下面的 schema 5 与正式发布信息仍是线上状态；不得将根工作树直接覆盖线上。

## 已上线的 PN1–PN6

2026-10-30：PN1–PN6、服务端 MiniLM 与隐藏首页 Windows 下载入口已同版发布，用户本轮授权工作完成。正式入口 https://understandbook.top，11:50:34 HKT 切换；release personal-notes-20261030，PID 506143，control schema 22 / MemoryDocument 5，维护 10.57 秒。

以实际线上 main-agent-planned-20261029 为基线合并，保留根工作树尚未收齐的空间笔记、可编辑思维导图和来源阅读窗口。新个人引用使用 personal_note；旧空间笔记 note 保持。正式源码在 /opt/understand-book/releases/personal-notes-20261030；覆盖包在 /opt/understand-book/acceptance/personal-notes-20261030/source.tar.gz，本地合并副本 .tmp/pn-service/merged。不要用未收齐线上功能的根工作树直接覆盖线上。

模型已由服务器统一供应：/opt/understand-book/models/local-embedding.json。固定 MiniLM revision、Transformers 3.8.1，Linux 跳过无用 CUDA 安装。服务启动预热；真实服务用户 embedding 检查、实际语义召回和复用检查通过。普通用户无须模型配置。

六个 Linux 发布二进制、合并版前端构建与类型检查、54 项相关 Web 测试、82 项合并后的 memory/boards 测试、个人画板工具合同、7+4 部署脚本测试通过。实际数据副本升级、笔记版本/恢复/隔离/图板/删除后搜索与无材料聊天通过；保存 58 份聊天、100 份图板、1 个空间笔记本、18 条额度记录。桌面/手机个人笔记界面和公网中英文首页通过，公网资源与读者/管理员能力验收通过。

已停止临时浏览器服务、SSH 转发和本地前端预览；已删除隔离用户数据，移除 2 GiB 构建交换文件，swappiness 恢复 0。恢复备份 /opt/understand-book/backups/personal-notes-20261030 保留；上线后不可直接覆盖新增数据或将 schema 22 交给旧程序。

详细报告与证据：docs/performance/personal-notes-20261030/deployment/README.md。原真实教学与 100/1000 条笔记检索实验：docs/performance/personal-notes-20261030/README.md。ADR 0177、切片、架构与代码链路已更新。根工作树包含其他任务原有未提交改动，保持不动；本轮没有创建 Git 提交或推送。
