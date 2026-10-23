# SESSION_CHECKPOINT_MUSIC — 2026-10-23 谱面事实与截图提问上线

## 本轮目标与状态
- 用户已授权谱面事实、截图功能、舒伯特材料和 Windows 一并上线。2026-10-23 13:00:51 HKT 已切换正式服务，维护 14.05 秒；真实模型图片联测、公网 API 和安装包比对通过。
- 产品源码 `ac003358814389927236be698639d7226c6180ec` 已提交并推送 main。其他自动构建任务的 dirty 文件未进入本次发布。
- 现网 release `/opt/understand-book/releases/music-score-images-20261023`，schema20，PID677907；Windows0.3.7 已公开下载并更新本机 `E:/allwork/Understand Book/UnderstandBook.exe`。
- 完整发布与已知教学表述问题以 `docs/performance/music-score-images-release-20261023/README.md` 为准。

## 当前成果
- 正式默认发布：`music-schubert-d780-3-vetter/01a16721-f123-7850-a8fd-b59c4beb1a1f`，共享课程 `schubert-moment-musical-3` 第 3 版；前版与账号授权保留。既有课程工作区可手动接纳新版。
- 已验收本地发布：`.understand-book/music-acceptance-service/library/published/music-schubert-d780-3-vetter/01a166d8-fabf-73d2-9502-4bf3d55da1b7`；正式导入使用完全相同的材料文件。
- 前版：`01a1646d-2408-7e21-acd7-9d3853666c29`；从此前版重建，保留全部 1066 谱对象、1059 对齐、78 出现、19 unobserved、覆盖、事件、示范与原素材字节。
- 构建目录：`.understand-book/music-schubert-score-facts-20261023`；初始输入 `input-curation.json`，可再建输入 `music/curation.json`。
- `evals/music/schubert-tonality.json` 是来源核对记录，确认作品 F 小调。原 XML `mode=major` 与上游 LilyPond `key as major` 同时保留为原编码。
- 5 条谱面声明：开头四降号、2/4、Allegro moderato 与两处 dim.；80 是 XML 文件播放参数。

## 实现入口
- `crates/base-schema/src/music.rs`：MusicScoreMark、MusicScoreMarkValue、MusicTonality，Rust 权威及 TS 生成类型；旧发布默认省略新字段。
- `packages/core/src/music-score.ts / music-import.ts / music-contract.ts`：提取、谱位锚点、标记 provenance、持久化及来源引用校验。
- `crates/read-tools/src/music.rs`：谱面分页附带适用的既有声明；搜索标记文字；返回 work_tonality，来源目标及摘要含实际事实与出处。
- `crates/runtime/src/music.rs`：工具说明区分原编码、作品调性和录音速度。保留他人的截图相关修改。
- 原 `source.present → server/music_source.rs → MusicSourcePreview` 保存并回看新事实，无需改引用协议或前端。

## 验证证据
- 真实舒伯特输入红测试先复现无 marks；修正后 Core 18 项通过，Core/Web typecheck 通过。
- Rust 音乐绑定导出 41、合同测试、read-tools 5、Runtime 3、Server 19 项通过；server / publish_book 构建通过。
- 现有贝九选段提取 58 marks / 1249 objects，并通过合同校验。
- `evals/music/results/score-facts-20261023` 保存 build/server/runtime/read 日志、重建比对、发布、重启回看证据。
- `live-01/report.json`：真实 deepseek-v4-flash，4 模型请求、130875 tokens、约 28.1 秒，回答 F 小调、2/4、Allegro moderato，解释编码差异与80的含义。
- `live-01/tool-calls.json`：实际 overview、score 范围读取、Allegro 搜索、source.present。`live-01/sources.json`：一个可打开且固定新发布的引用，含谱面事实和核对出处。
- 重启后 source.resolve 成功，原谱 SVG GET 200 / image/svg+xml；证据 `preview.json`。
- 本次真实图片联测：4 次请求均包含选区像素，294851 tokens，73.341秒，两条来源解析通过；发布证据目录保存 report、answer、sources。未宣称完整连续教学或真人效果通过。
- 公网 Edge 桌面1500×1000／手机390×844 的框选、预览、复制粘贴、上传、仅图片提问与失败草稿保留均通过，截图已实际查看。发布实现与验收无剩余工作；后续音乐教学质量问题见发布记录“已知问题”。

## 当前预览
- `http://127.0.0.1:6677/`；Node root session 20615，入口 `tmp/music-score-facts-preview.mjs`。
- 使用 `tmp/music-score-facts-preview-server.exe`（当前编译副本），不锁 target/debug/server.exe。
- 使用 live-01/memory 与 live-01/private，保留真实联测回答和引用；旧用户预览内存未覆盖。
- 停预览只处理本脚本/该 exe 且命令包含新发布 ID 的进程。

## 文档与复现
- `docs/代码链路.md`、`docs/验收-音乐伴听.md` 已记录本切片；ADR-0175 与 evals/music/README.md 已更新。
- `evals/music/rebuild-score-facts.mjs --source <确切发布目录> --curation <既有编选JSON> --out <新目录>` 经原构建器重建并对比保留字段和字节。
- `evals/music/score-facts-live.mjs --book <发布目录> --out <新证据目录>`：真实模型一次问答与来源回看，450000 tokens 启动上限。
- 工具契约文档已从本轮成功编译的 Server/Runtime 测试二进制实际导出并生成。直接执行的两项导出测试均通过，见 `tool-docs-prebuilt.log`；初次 `pnpm docs:tools` 的重复编译因 LLVM 内存不足退出，日志保留在 `tool-docs.log`。

## 其他音乐验收
- 本轮不改变既有录音校准结论。完整四轮连续教学、真人学习效果及贝九共享许可的历史验收见既有验收文档。
- 构建临时交换空间已关闭并删除；恢复点 `/opt/understand-book/backups/music-score-images-20261023`。恢复只回退程序与配置，不用旧数据库覆盖后续业务写入。
