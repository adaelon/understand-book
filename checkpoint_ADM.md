# checkpoint_ADM — 2026-10-08 22:46（Asia/Hong_Kong）

## 新鲜度自检
- 最新 commit：`4e0b68e feat: integrate book structure, tutoring and multi-user reader`；ADM 工作树未提交。
- **ADM0–ADM10 已实现并于2026-10-08正式上线**，release `/opt/understand-book/releases/adm10-20261008`，服务 active，schema8。
- 同次发布包括 EX14.0–EX14.3 / ex14.v4 / Manim 和近期 Reader 小修；EX14.4/5实验未因部署而完成。

## 当前结果
ADM10 发布演练、真实旧库迁入、schema8备份恢复、首批配置、有限真实模型验收与正式HTTPS验证完成。供应商账单差额已记录，未冒充完全对账。

## 运营配置与入口
- Reader `https://115.190.121.150`；后台 `https://115.190.121.150/admin/`，管理员admin。
- admin凭证已单独交付 `C:/Users/Lenovo/.understand-book/adm10-admin-credentials.json`，不在仓库。
- reader/puff/adaelon/admin各赠送10元模型成本额度；2026-10-08 22:26:37至2026-11-07 22:26:37香港。
- 原读者密码/授权/私人数据保持；9项默认材料、10个发布版本保留。
- 收款按官方模型成本加10%：11元收款授予10元额度；赠送无收款。联系方式19847595883已在Reader额度界面显示。
- Flash人民币价闲时缓存命中/未命中/输出每百万0.02/1/4元，高峰0.04/2/8元；高峰为非中国法定节假日的周一至周五09–12/14–18香港。
- model-rates.json适用区间至2026-11-08 00:00香港；区间到期前查价并更新、重启装载。历史调用保留快照。

## 发布与恢复证据
- 本地正式证据 `docs/performance/adm10-release-20261008/README.md`；包含回执、逐次费用、生产截图和6份测试日志。
- 远端证据 `/opt/understand-book/acceptance/adm10-20261008`；源码清单source-files.txt，同版构建收据multi-reader-build.txt。
- 最新切换前快照 `/opt/understand-book/backups/adm10-production-20261008/service-snapshot`；同级configuration保护环境和服务配置。
- 旧release `/opt/understand-book/releases/ex13-jl-20261002`；演练前快照 `/opt/understand-book/backups/adm10-before-rehearsal-20261008` 全保留。
- 真实旧库4→8保留3账号/6会话/10发布/30授权/9默认/39现场/8接单/169私人文件，新账目为空。
- 实际新库备份恢复：16张表、176私人文件、费率一致，包含21次真实调用与发送回执。
- 正式进程、静态根、worker同release；Nginx检查通过。候选进程和候选临时站点已关闭。
- 新schema不交给旧二进制；有新写入后先备份新库，前向修复或明确接回，不用旧余额覆盖。详见运行单。

## 验收
- ADM10+MU9恢复12、装配3、额度5、Reader92、目录/表格浏览器7、EX14 Runtime89全通过。
- Linux候选和正式HTTPS浏览器各4组通过：路由/资源/认证/CSRF/双端退出/普通账号拒绝/手机额度，模型请求0。
- 独立候选短答和长交互呈现完成，21次真实调用（其中3次含图片观察），无待核算记录；演练费用未进入正式读者账。
- 长任务交付presentation-1791469383816308218-2 revision1，证据presentation-verification.json。
- 用量与官方费率逐次整数折算0.721157元；Provider余额后续减少0.55元，用户22–23时图表0.54元；差额0.171157元未有逐请求账单解释。各口径均低于3元预算。
- live-verification.json保存首次余额变化0.20元；最新结果在billing-note.json，勿误用初次余额观测。

## 后续与已知限制
1. 需要供应商逐请求账单才能解释费用差额；不猜折扣、不依据小时图改价，已按方案如实记录。
2. 续期开通前更新费率适用区间；新账号需在后台显式赠送/登记收款与授额。
3. EX14.4自然A/B/C与EX14.5完整Reader连续使用仍待独立预算/实验；本次上线不代表自然收益已验证。
4. 既有MU4/MU10夹具失败及构建体积提示见方案第10节；未宣称全仓测试通过。
5. 当前成果未提交；工作树有其他任务改动，不整体提交或回退。主SESSION_CHECKPOINT.md仍属视频任务，未改。

## 冷启动读序
1. `docs/运营后台-ADM10发布运行单.md`、`docs/performance/adm10-release-20261008/README.md`。
2. `docs/切片方案-运营后台与账号额度.md`第8–10节；`docs/Linux多人阅读-现网运行说明.md`。
3. `docs/运营后台-ADM5实现.md`、`docs/运营后台-ADM9实现.md`、`docs/Linux多人阅读-MU9迁移恢复.md`。
4. `crates/server/src/tests/adm10_tests.rs`、`scripts/linux/smoke-admin-release.mjs`；实际执行脚本 `tmp/adm10/`。
5. `checkpoint_ex.md`；费用详情live-calls.json、最新核对billing-note.json、正式deployment.json。
