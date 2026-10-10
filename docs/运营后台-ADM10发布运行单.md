# ADM10 发布演练与首批开通运行单

日期：2026-10-08，Asia/Hong_Kong。状态：已完成发布演练及正式上线；费用差额按实际证据保留，详见验收记录。

2026-10-09 入口更新：阅读器 `https://understandbook.top/`、后台 `https://understandbook.top/admin/`，旧 IP HTTPS 地址自动跳转。发布版本仍为 ADM10，域名、证书和后端 Origin 的配置及回退见 [域名接入记录](performance/domain-20261009/README.md)。下文 IP 地址保留为当次发布历史。

## 已确认的首批配置

用户在本次会话确认：管理员 `admin`；沿用现有材料；每个首批读者赠送 **10 元模型成本额度**；有效期 **30 天**；付费售价为官方模型成本加 **10%**；联系方式 **19847595883**；真实 Provider 验收合计不超过 **3 元**。

赠送独立记录原因，不生成收款。付费开通示例：实际收款 11 元，授予 10 元使用额度；收款以分记录、额度以 micro_cny 记录，追加时沿用当前有效期。新一期从实际开通时刻起 30 天，旧期不自动结转。现有 reader、puff、adaelon 的账号、密码、材料授权和私人数据保留。

## 发布输入

| 项目 | 本次目标 |
| --- | --- |
| 主机 / 公网入口 | `115.190.121.150` / `https://115.190.121.150` |
| 服务 / 数据根 | `understand-book-multi.service` / `/opt/understand-book/data/multi-reader` |
| 原 release | `/opt/understand-book/releases/ex13-jl-20261002`，现场读取 schema 4 |
| 候选 release | `/opt/understand-book/releases/adm10-20261008` |
| 演练证据 | `/opt/understand-book/acceptance/adm10-20261008` |
| 升级前演练快照 | `/opt/understand-book/backups/adm10-before-rehearsal-20261008` |
| 读者联系入口 | Reader 使用额度对话框；构建时 `VITE_READER_OPERATOR_CONTACT=19847595883` |

源码为 `4e0b68e` 基线的当前工作树，包含 ADM1–ADM9 及同树既有改动；源码文件清单保存在 `tmp/adm10/source-files.txt`。Linux 在最终 release 目录构建 Rust、Vue Reader 和 React Admin，成功后由现有脚本生成同目录构建收据。

用户随后明确要求同时发布 EX14.0–EX14.3（含 Manim、当前指导 `ex14.v4`）及近期 Reader 修改：目录居中跟随、手机问答/键盘、正文表格与引用定位、Agent 表格来源按钮、代码/表格文字划选、运行活动折叠。候选包含这些源码；本次发布不把 EX14.4 自然 A/B/C 对照和 EX14.5 全部连续使用验收标记为完成。

## 执行顺序

1. 核对活动和未保存运行；为零后停服，用**原 release 的** `reader_maintenance backup` 保存 schema 4 快照，然后恢复原服务。
2. 用候选 `reader_maintenance restore` 把旧快照恢复到独立候选根，自动顺序升级到 schema 8。比较原账号、授权、阅读现场及私人文件；新费用和活动表为空。为候选配置管理员、费率与额度，再做 schema 8 备份和第二次恢复比较。
3. 候选 Nginx 指向同版 `dist-multi`；验证后台深链接、资源 MIME/404、Reader 与 Admin 的真实登录、普通账号拒绝、CSRF 与跨端身份变化。真实 Provider 验收使用独立候选额度，记录短答、含压缩或呈现的长任务与支持的媒体观察，累计成本预算 3 元。
4. 演练通过后重新确认现网在途任务为零，停服生成**最新**升级前快照。由同版程序升级正式根，授予管理员，给三个既有读者建立 30 天额度期并赠送各 10 元；保留原材料授权。
5. 统一切换服务、Nginx 静态根和呈现 worker 路径，验证两套页面与 API；保存部署回执、管理员凭证交付路径、逐次费用核对结果和恢复证据，刷新 checkpoint_ADM。

## 费率与核算

2026-10-08 查阅 [DeepSeek 官方人民币价](https://api-docs.deepseek.com/zh-cn/quick_start/pricing/)：Flash 每百万 Token 的缓存命中、未命中输入、输出分别为闲时 **0.02 / 1 / 4 元**、高峰 **0.04 / 2 / 8 元**；北京时间周一至周五 09:00–12:00、14:00–18:00 为高峰。现用 `deepseek-v4-flash` 别名由 V4.1 Flash 提供服务，保留原模型配置并按 Flash 费率计价。

`model-rates.json` 使用当前代码支持的绝对 UTC 区间与独立版本号；provider 必须精确等于现网 `https://api.deepseek.com`。费率保留官方成本，10% 加价通过收款与授予量体现。费率在启动时装载，下一期或官方调价时重新查价、生成新适用区间并按维护流程重启；历史调用保留原快照。

逐次核对保留 call_id、run_ref/task_ref、模型、用量、费率版本、预占、供应商成本、账号扣减和核算依据。没有供应商逐请求账单时标记为“用量折算”；账单余额差额单独记录，不把它伪装成可逐次关联的账单。

## 回退

schema 8 不交给旧二进制写入。若尚无新业务写入，可用旧版工具把升级前快照恢复到新的独立根，再将旧 release 指向恢复根。**不要用新版 restore 生成供旧版直接读取的根**，因为新版恢复会升级数据库。

若新版已产生付款、用量或私人写入，先停服并用新版工具保存完整最新快照；保持维护状态进行前向修复或明确的数据接回，不用旧余额覆盖新账。原根、快照及旧 release 保留。恢复点包括 control.sqlite、所有私人状态、发布包和 model-rates.json；服务环境文件和 Nginx/systemd 配置另行备份。

## 验收记录

- 本地 ADM10 两项 Rust 演练通过：旧快照恢复升级且源快照不变；schema 8 管理事实、收款更正、费用、免扣未知、在途恢复、私人文件与材料重新定位。
- 装配脚本 3 项通过；额度对话框 5 项既有测试通过。私人 SQLite 按表比较内容，普通文件按字节比较。
- 2026-10-08 正式切换到 `adm10-20261008`，schema 8；Rust、Reader、Admin 与呈现 worker 同版。候选和正式 HTTPS 各4组通过，正式服务 active。
- 旧库真实迁入保留169私人文件；schema 8 实际备份恢复的16张表、176私人文件及费率一致。
- 短答、长交互呈现及3次含图片观察的发送完成，共21次调用，均结算。用量折算0.721157元；Provider 最新余额减少0.55元，用户图表0.54元；差额0.171157元待逐请求账单解释，未据此改价。演练留在独立候选根，预算3元内。
- reader/puff/adaelon/admin 各赠送10元，有效期2026-10-08 22:26:37至2026-11-07 22:26:37香港；9默认材料保留。最新旧库快照为 `/opt/understand-book/backups/adm10-production-20261008/service-snapshot`。
- 正式后台 `https://115.190.121.150/admin/`，用户名admin；凭证交付至用户私人目录 `C:/Users/Lenovo/.understand-book/adm10-admin-credentials.json`，仓库不保存密码。
- 完整回执、费用和截图见 [发布证据](performance/adm10-release-20261008/README.md)。

## 已知限制

- 现有费率区间到2026-11-08 00:00香港；续期前更新区间并重启装载，过期后模型发送停止，阅读继续可用。
- Provider 账单与用量折算的差额尚未逐请求对齐；EX14.4/5自然对照和完整连续使用不因本次部署而完成。
- 预占不构成供应商绝对费用上界；验收采用小请求、独立额度并逐次核对预算。
- 历史 MU4/MU10 夹具问题及前端体积提示沿切片方案第 10 节保留。
