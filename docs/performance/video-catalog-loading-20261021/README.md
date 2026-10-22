# 课程目录与视频图谱读取修复 — 2026-10-21

> 发布更新：2026-10-21 已上线，Windows 0.3.3 已发布并完成本机升级。以下保留修复阶段记录；最终线上状态见 [发布记录](../reader-latency-release-20261021/README.md)。

状态：修复与定向验证完成，尚未部署。Windows 与网站共用的课程目录不再为显示卡片加载完整视频图谱、讲解及图片。

## 原因与改动

`GET /api/workspace-releases → AdminStore::workspace_releases → material_readiness` 在课程目录查询中调用 `Book::load`。CS336 第 2 讲的发布包含 871 个增补批次、5,599 张 PNG；图谱与讲解加载为了检查 PNG 签名和宽高完整读取图片，并逐条重复校验相同视频清单、转换相同原文。

- `crates/server/src/workspace_publication.rs::material_readiness`：视频目录只加载原文基座、转写和来源元数据；继续检查原视频、转写依赖、各能力的完整覆盖和实际文件存在性。非视频的 Tutor 准入保持原路径。
- `crates/read-tools/src/{video_graph,video_discourse}.rs`：图片校验只读取所需的 24 字节文件头；短文件、错误签名、错误尺寸及缺失图片仍被拒绝。
- `crates/base-schema/src/video.rs::EvidenceTarget::validate_batch`：同一贡献中的证据共用清单校验和按需生成的 UTF-16 原文；逐条证据、联合成员、来源身份和范围检查保留。
- `packages/web/src/components/SharedWorkspaceCatalog.vue` 与 `network-client.ts`：目录和详情请求支持取消；15 秒超时后展示错误并恢复刷新，切换账号或关闭目录取消旧请求。创建个人工作区的操作不设置该读取超时。

## 同机、同一发布的性能结果

服务器输入为线上已存在的完整 CS336 发布 `19ac880d-a788-43f4-9725-6c56f4980189 / 01a15c89-da55-7d71-8329-a0ba9f25c86b`，原服务为 `search-cache-20261021`。在独立目录编译 release 测量程序；原服务、原数据及原发布程序保持原状。

| 路径 | 修改前，第 1 / 2 次 | 修改后，第 1 / 2 次 | 每次逻辑读取量，前 → 后 |
| --- | --- | --- | --- |
| 课程材料就绪检查 | 14,625 / 10,861 ms | **25.37 / 23.00 ms** | 2,357 MB → **1.46 MB** |
| 完整视频 Book 构造 | 16,566 / 10,266 ms | **1,736.69 / 225.52 ms** | 2,356 MB → **17.97 MB** |

每次均启动新进程，不复用应用内 Book 缓存；没有清空操作系统文件缓存，因此分别列出首次和再次读取。读取量来自该测量进程 `/proc/self/io` 的 `rchar`，MB 使用十进制。原始结果见 [linux-comparison.json](linux-comparison.json)；完整基线和候选输出保存在 `linux-before-1.json`、`linux-after-1.json`。

四次运行的课程就绪状态、完整图谱节点与边、720 条转写及 `einsum` 检索结果直接比较完全一致。没有用摘要替代内容比较，没有调用模型。

此前线上真实 HTTP 诊断：普通账号和书架请求约 1.5–1.8 秒；公网课程目录在 21.48 秒触发探针超时；源站内部 HTTP 返回 1 门课程用了 18.172 秒，该时间段服务进程的逻辑读取量增加 2.358 GB、磁盘读取增加 2.198 GB。见 [baseline.json](baseline.json)。隔离测量解释了该线上延迟的主要来源。

Windows 的同课程完整 Book 对照也保留在 [book-comparison.json](book-comparison.json)，优化前约 5.3–6.4 秒、候选约 3.9–4.0 秒。Windows debug 构建的就绪检查与完整能力结果见 [catalog-local.json](catalog-local.json)，不将它与 Linux release 耗时混用。

## 回归与复跑

- 视频契约 10 项、Read Tools 视频相关 8 项、Server 课程发布/个人实例/视频相关 23 项、Reader 9 项：共 **50 项通过**；Reader 类型检查通过。
- 另行执行现有完整课程的目录基准测试，7 种能力全部就绪。常规 Server 测试中忽略的浏览器宿主和真实 Whisper 场景未运行。
- 前端新增用例在修改前因没有取消信号、没有超时而失败，修改后通过。后端新增用例在修改前将缺失的图谱图片错误扩散成原视频和翻译不可用，修改后保留其他可用能力。对应 `web-before.log`、`server-before.log` 与通过日志。
- 缺失原视频、缺失已接纳依赖、无效 Tutor 产品、撤权、归档、历史版本、个人实例和现有证据约束的定向回归通过。

```powershell
cargo test -p base-schema --test video_contract
cargo test -p read-tools --lib video_
cargo test -p server --lib -- video_workspace_ tests::video_publication_tests:: tests::s1_workspace_publication_tests:: tests::s2_workspace_instance_tests::
pnpm -C packages/web test src/components/SharedWorkspaces.test.ts
pnpm -C packages/web typecheck
```

`run-linux.py` 复用已有 SSH 连接方法，在 `/opt/understand-book/acceptance/video-catalog-loading-20261021` 建立隔离源码。测量程序使用原版或候选的实际 `material_readiness` 函数和实际 Read Tools；内存数据库仅承载该课程的发布记录，非视频 Tutor 分支不在此测量中。首次套补丁因源文件 CRLF 失败，已在隔离源码中统一换行后完成；失败与恢复日志分别保留，未改线上文件。

改动基线为工作树 HEAD `33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本轮开始时已有改动；相关原文件保存在 `before/`，本轮源码差异为 `fix.patch`。原工作树其他改动保留，没有提交或推送 Git。

## 已知限制

- 尚未切换线上服务或发布网页，当前 Windows 客户端仍连接旧服务。这里的候选毫秒数是服务器同机的核心读取路径测量，不是已上线的整页响应时间。
- 完整视频首次构造仍需加载图谱和讲解元数据及检查图片文件头；首次与操作系统缓存命中后的耗时不同。
