# 上下文缓存稳定性上线记录

2026-10-16 00:14:01 HKT 完成正式上线。服务 `understand-book-multi.service` active，PID14712，正式入口为 https://understandbook.top/ 。用户明确授权本次上线。

## 发布内容与状态

- 当前 release：`/opt/understand-book/releases/context-cache-20261015`。
- 前版：`/opt/understand-book/releases/online-publication-20261015`。
- 数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema14 → schema14。
- 从线上前版复制源码与 Cargo 锁定文件，逐文件直接比较本轮 Runtime 修改前文本，全部吻合后覆盖本轮11个文件。工作区其他未发布改动未加入。清单见 [source.json](source.json)。
- CC1 稳定基础 system 和合法工具暴露，指导沿 ledger 差量发布；CC2 将48 KiB滚动淘汰改为整体请求压力下整批退休；CC3 完整验证后投影精简 checkpoint。权限、来源验证和完整持久检查点沿用原合同。
- 服务和展示 worker 均指向新 release。前端、技能等既有资源仍链接旧 release；须保留 `online-publication-20261015` 及其引用的 `current-20261015`。

## 验证

Linux 正式构建使用 `cargo build --locked --release -j 1 -p server --bin server --bin manage_reader --bin publish_book --bin reader_maintenance --bin presentation_worker`，五个程序构建成功，用时10分04秒。构建期间旧服务保持运行，构建进程限制内存1800MiB、单编译任务，并限制CPU。见 [build.log](build.log) 和 [build.exit](build.exit)。

候选 release 的 Runtime 回归：**590通过、0失败、4忽略**，执行8.28秒。命令：

```bash
cargo test --locked --release -j 1 -p runtime --lib \
  --config 'profile.release.package.runtime.opt-level=0' \
  --config 'profile.release.package.runtime.codegen-units=64' \
  -- --test-threads=1
```

见 [test.log](test.log)、[test.exit](test.exit)。首次测试缺4个编译期样本；补齐后第二次测试缺实际回放JSON和量化书籍fixture，5项因此失败。补齐这些固定样本后全量通过，未跳过用例或改动生产代码。两次中间结果保存在 `test-incomplete-fixtures.*` 与 `test-missing-replay-files.*`。本地真实检查点手动测试已另行执行，结果见[实现验收报告](../README.md)。

部署前确认没有活动或未保存运行，临时维护入口后再次确认，停服并用前版 `reader_maintenance backup` 完整备份，切换 systemd 和 presentation worker 路径，完成本机验收后恢复公网入口。停服到本机恢复验收为 **16.00秒**；该数值包含备份、启动及本机验证，不是完整发布流程时长。

本机切换前、切换后以及正式域名均验证：

- reader 登录、11项授权书籍版本、默认书 manifest 与正文；admin 登录、12项授权书籍版本、默认书 manifest 与正文。三次完整授权版本清单相同。
- reader 看不到私有 `understandbook`，直接读取私有版本返回404。
- authoring、plot、animation、preview 四项能力均 ready；退出成功。
- 数据库仍为schema14，新进程实际可执行文件来自新release，发布socket存在且权限0600。

原始记录见 [deployment.json](deployment.json)。另外从本地 Windows 客户端访问公网，首页200、未登录 `/api/auth/me` 401，见 [external-http.json](external-http.json)。本次部署验收模型调用为0。

## 备份与恢复

完整数据备份：`/opt/understand-book/backups/context-cache-20261015`。

切换前配置：`/opt/understand-book/acceptance/context-cache-20261015/previous-configuration/`，包括 `understand-book-multi.service`、`understand-book-multi.conf` 和 `presentation-sandbox.json`。线上发布证据及部署脚本在同级 acceptance 目录。

如需恢复程序版本，先确认没有活动或未保存运行，再进入维护窗口；停服、恢复旧 systemd unit 和 sandbox 配置、daemon-reload、启动前版并验证登录/正文/权限/四项展示能力，最后恢复 Nginx 并检查公网。schema未变化，应保留当时最新数据，避免将上线后新增对话回滚。完整备份用于确有数据损坏时的单独恢复。本次没有触发恢复流程。

## 已知限制

本次上线验证覆盖构建、Runtime回归、服务读取与权限合同，没有发起正式问答或制作交付。四项展示能力检查为就绪检查。线上长会话的缓存率尚未测量；此前8次固定请求探针为55.90% → 54.91%，未证明命中率提升。真实检查点消费字节减少83.29%的结果已验证，不能据此推导线上缓存率达到80%。
