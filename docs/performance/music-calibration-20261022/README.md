# 本地音乐校准工作台验收

本轮给已有音乐构建目录增加可操作的校准入口。未发布音乐在本地 Reader 启动后进入“时间校准”，使用原录音、实际 SVG 谱面、演奏出现顺序和候选音符时间。修订继续调用 `calibrateMusicWorkspace`，保存到 `.build/music/calibrations.json` 并在重建时重放。

## 实际材料与保存边界

- 舒伯特：101 秒原录音、78 个演奏出现、900 条音符候选、3 页谱面。
- 贝九：完整第四乐章原录音，选编原第 93–164 小节，6 个实际声部、72 个演奏出现、17 页谱面。录音与谱面仍为 `local_only`，可本机检查，共享发布继续受原许可检查约束。
- HTTP 与浏览器验收使用 `.understand-book/music-calibration-qa/` 下的副本，原样本不被修改。流程测试没有接纳实际演奏音符，保存后的覆盖仍为 `pending`。
- 未发布上下文没有 `publication`，媒体从当前本地构建目录读取。已发布副本和纯阅读模式不能校准；读入口返回 `null`，写入被拒绝。

## 接线

- `packages/web/src/LocalApp.vue`：本地入口；`MusicCalibrationPane.vue`：选窗、原候选／修改后复听、循环、当前时刻填入边界和音符、理由与依据、保存及历史。
- `packages/web/src/music-calibration.ts`：完整出现窗口、缺音符、延音线及接纳检查。新增或修改音符时点需要明确复听依据，可继续保存为待核；缺失音符不会按等分补齐。
- `crates/server/src/music_calibration.rs`：`GET /api/build_workbench/music/context`、`POST /api/build_workbench/music/calibrate`，只操作当前工作区；媒体沿既有字节范围服务返回。资源 URL 使用现有录音与谱本 ID、版本区分材料，服务拒绝与当前工作区不一致的旧请求，保留 `no-store`。相对工作区路径先解析为绝对路径。
- `skills/build/music-calibration-cli.ts` 与 Engine 的 `music-calibrate` 命令：调用 Core，失败返回非零退出码及实际错误。
- `packages/core/src/music-import.ts`、`music-alignment.ts`：核对录音与谱本身份，校验完整范围后事务保存修订、清单和可重建策展输入；相关事件与详细结构跟随已绑定的演奏出现更新，原音频证据保留原区间。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| Core 导入、修订、事件定向测试 | 7 项通过；类型检查通过 |
| Server 校准路由测试 | 2 项通过，含实际 Node/Core 保存、相对路径、刷新、错源、缺音符接纳失败和已发布／纯阅读拒写 |
| 开发态与安装态 Engine 命令 | 两路径均能追加历史，保留候选和原音频；已发布拒写且清单／历史不变 |
| 两份真实材料 HTTP | 两者均通过，下列结果文件保留详情 |
| 两份真实材料浏览器 | 实际音频 0.75 倍速播放、选中谱音、共享小节边界修改后丢弃、保存待核修订、刷新历史、LocalApp 返回与重开均通过；移动端无横向溢出 |
| 实际切换材料 | 舒伯特→贝九→真实非音乐论文→舒伯特通过；播放器实际时长随材料变化，旧音频／谱页地址返回 404；非音乐隐藏校准入口，返回未发布作品导读无发布错误 |
| 浏览器保存后的实际重建 | 两者均通过：舒伯特 7.268 秒、贝九 8.907 秒；每份 2 条修订原样保留，演奏顺序、音符时间、覆盖、事件、结构及原音频完全一致 |

[舒伯特 HTTP 结果](schubert-http.json)与[贝九 HTTP 结果](beethoven-http.json)验证：音频范围请求返回 206，32 字节与原文件一致；实际谱面返回 SVG；修订保存并在刷新后读回；缺音符却请求接纳、录音版本不匹配都返回 400 且不覆盖已存清单；临时标记为已发布后，写入拒绝、媒体返回 404。测试后已移除测试用发布标记。

浏览器操作记录：[舒伯特](../../../evals/music/results/music-calibration-schubert-ui.json)、[贝九](../../../evals/music/results/music-calibration-beethoven-ui.json)。界面实图：[舒伯特桌面](../../../evals/music/results/music-calibration-schubert-desktop.png)、[贝九桌面](../../../evals/music/results/music-calibration-beethoven-desktop.png)、[贝九移动端保存](../../../evals/music/results/music-calibration-beethoven-mobile-save.png)。浏览器保存使用“界面流程验证，候选保留待检查”，保留原候选的音频分析依据；流程检查本身标为解释记录，不标为人工听核。

[跨材料切换结果](../../../evals/music/results/music-calibration-workspace-switch.json)固定了真实浏览器复现的旧音频复用问题：新页面只读取当前上下文，媒体 URL 绑定实际来源版本，服务拒绝旧来源请求。贝九播放器实际解码时长为 1,448.817596 秒，清单经 FFprobe 得到 1,448.85551 秒，二者相差约 38 毫秒；不会再复用舒伯特约 101 秒的音频。此次末轮检查没有新增修订。

安装态证据见[校准 Engine 验证](../music-operator-20261022/calibration-engine-verification.json)。验收脚本 [verify-http.mjs](verify-http.mjs) 会追加一条明确标记为流程验证的修订，只应对 QA 副本运行：

```text
node docs/performance/music-calibration-20261022/verify-http.mjs <QA工作区绝对路径> http://127.0.0.1:<端口> <结果JSON路径>
```

[重建重放结果](rebuild-replay.json)使用两份已经通过浏览器保存的副本，从原始策展输入完整重新构建。对应脚本为 [verify-rebuild.mjs](verify-rebuild.mjs)，在仓库根目录用 `node --import tsx` 执行；它检查浏览器修订确实存在，再比较历史及所有校准相关结果。

启动多个本地验收服务时，每个服务分别设置 `UNDERSTAND_BOOK_MEMORY_DIR` 和空闲端口；用 `UNDERSTAND_BOOK_BUILD_SIDECAR` 指向包含 `music-calibrate` 的 Engine。校准工作区仍通过 server 原有目录参数指定，没有新增任务队列或数据库。

## 尚未接纳的内容

这些结果证明校准流程与持久化可用，不代表舒伯特、贝九的候选时间已经完成实际听核。实际录音仍需要独立锚点和逐段检查；贝九共享许可状态不变。尚未导入演奏出现顺序的音乐显示候选未就绪状态，不自动生成等分时间。
