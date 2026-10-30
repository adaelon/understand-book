# 《失败的逻辑》书架名称修正

2026-10-29 10:26:13 HKT 上线，10:27:10 公网验证通过。书架显示名称为“失败的逻辑：事情因何出错，世间有无妙策”。原 `book_id=logic-of-failure`、`publication_id=01a18553-cdfa-7b00-b8bf-172505c00301`、发布清单和账号访问权限保持。

## 实现与范围

原发布没有可识别的标题，书架回退显示书籍 ID。现有 `paper_metadata.json` 同时参与论文类型判断，不能作为普通图书的改名渠道；隔离副本验证拒绝该方案，线上发布包未写入论文元数据。

`crates/server/src/book_title.rs` 增加发布目录旁的显示名称读取：对目录 `…/<publication_id>/`，读取相邻的 `…/<publication_id>.display.json`，格式为 `{"title":"显示名称"}`。非空标题优先显示，缺失或空值沿用原回退规则。此文件位于只读出版包之外，不进入正文、图谱、资料类型或质量判断。

现网文件为 `/opt/understand-book/data/multi-reader-to-tb-20261012/library/published/logic-of-failure/01a18553-cdfa-7b00-b8bf-172505c00301.display.json`。本次没有知识构建、模型调用、数据库迁移或发布版本替换。原正式知识对象构建继续保持用户取消状态。

## 验证与部署

- 基于当前线上 `board-source-window-20261028` 复制独立 release，只改上述一个源文件；三个标题测试通过，Linux 优化构建成功。
- 新 release 为 `catalog-title-20261029`；停启维护 7.06 秒。原静态资源和其他辅助程序继续使用原有配置。
- 管理员和普通读者通过公网登录，书架标题逐字匹配目标、默认版本身份保持、正文样本匹配原文、Tutor 为 `ready`。
- 两类账号的既有书籍授权集合、阅读入口、历史和额度接口在服务切换前后一致；本书注册表内的完整发布清单逐字保持。

证据：[部署回执](deployment.json)、[公网验证](verification.json)。远端操作记录位于 `/opt/understand-book/acceptance/logic-of-failure-title-20261029/`。单次编译最初受过低内存限额影响，按服务器可用资源调整后完成，未降低书籍质量门槛。

回退只需恢复该目录 `before-display-update/service.unit` 的服务配置，并撤去上述显示名称文件，然后重启服务；不回退控制库或读者数据。后续程序发布应保留 `book_title.rs` 的这项增量。
