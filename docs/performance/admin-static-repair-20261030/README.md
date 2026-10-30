# 后台 404 修复 · 2026-10-30

16:38 HKT 已恢复 `https://understandbook.top/admin/invites`。正式 release 仍为 `all-pending-20261030`，服务 PID 524591 未变。后台 12 个静态文件恢复，无服务重启或业务数据改写。

## 原因与修复

15:16 的合并发布先复制旧 release，再用 `install_web_prepare.py` 清空候选 `packages/web/dist-multi` 并解压本地阅读器构建包。该包的 300 个文件没有 `admin/`，因此原有后台随目录清空一起丢失。nginx 的后台回退规则正确，但目标 `admin/index.html` 缺失。此前“admin 健康通过”是管理员账号登录和接口检查，没有覆盖后台网页。

比较当前版和 `board-source-publication-20261030` 的后台源码、构建配置后，确认没有差异。将前版后台 12 个文件复制到暂存目录，确认入口引用存在，再更名为当前版的 `admin/`；源站 8 个后台路径立即恢复 200。[恢复回执](restoration.json)

进一步修正了三处发布问题：

- **替换前未检查包是否完整**：新包先解压到临时目录，通过首页、阅读器、后台及其直接静态引用检查后才替换候选目录。部署切换前重复使用同一检查函数，防止准备阶段后的误操作。
- **公网验收遗漏后台页面**：现检查 10 个入口和后台深链接，确认返回对应应用；13 个直接引用资源与发布目录逐字节比较，并检查 JS/CSS 类型。HTTP 200 却返回阅读器回退页也会失败。
- **运行说明的版本段落过期**：更新仍指向 10 月 28 日版本、schema 21 的条目，统一为实际 `all-pending-20261030`、control schema 22 与按需加载的 Memory schema 6。

远端和本地暂存的 `web-dist.tar.gz` 已补齐为 312 个文件，其中后台 12 个；缺陷包另存 `web-dist.reader-only.tar.gz`。远端三个原发布脚本保留 `*.before-admin-static-repair.py`，原发布验收记录保留以说明当时覆盖范围。[发布包修正回执](release-repair.json)

## 代码与后续发布

- `scripts/linux/verify-web-release.py:verify_dist`：检查三个应用入口及直接资源；`verify_published`：检查公网路由、实际资源内容和类型。
- `scripts/linux/build-multi-reader.sh`：Reader 和 Admin 组装后调用检查，成功后才继续后端构建及生成完成回执。
- 本次临时发布脚本已接入相同检查；正式源码中的构建脚本只追加这次验证调用，保留既有构建修改。

后续完整构建使用 `bash scripts/linux/build-multi-reader.sh`。单独准备前端包时，需要同时包含阅读器和后台，并在替换候选目录前运行：

```bash
python3 scripts/linux/verify-web-release.py /path/to/prepared/dist-multi
```

上线后按实际 release 目录运行：

```bash
python3 scripts/linux/verify-web-release.py packages/web/dist-multi --origin https://understandbook.top
```

## 验证结果

- 6 项回归测试通过；覆盖缺后台入口、缺后台脚本、后台路径误回退阅读器、旧静态文件与正确发布。
- Linux 原有构建流程 4 项测试通过；缺失依赖或构建失败不留成功回执。
- 本次真实缺陷包被新检查拒绝，错误为 `Missing web entry: admin/index.html`；修正后的实际包通过。[实际发布包验证](package-verification.json)
- 10 个公网页面及 13 个入口引用资源通过。[公网回执](public-web.json)
- 管理员真实登录后，账号、内测码、用量、监控、空间发布、制作任务、材料、额度商品和云端方案共 9 个读取接口全部返回 200 与 JSON；验收会话已退出，模型请求 0、业务写入 0。[接口回执](admin-api.json)
- 对当前构建内模块导入、Vite 懒加载映射及样式资源检查了 268 项引用，无缺失。旧版被替换的 6 个版本化文件不被当前构建引用。[资源审查](static-audit.json)

## 验证边界

浏览器实际加载出了后台登录表单，提交已有账号后进入登录确认状态，但浏览器控制工具在后续 DOM 和截图请求中连续超时。因此没有获得登录后页面截图，也没有宣称完整视觉及交互验收通过。公网路由、资源文件及已认证后台接口已独立完成验证。

本次审查覆盖后台恢复及发布资源完整性，未对全产品进行重新回归。账号授权、额度、内测码生成/禁用及材料发布等业务写入未操作。
