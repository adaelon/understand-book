# 精选音乐运营构建证据 · 2026-10-22

运营入口复用既有任务、平台额度、进度确认、暂停恢复和在线导入。运行中的音乐阶段为本地原声解码、Verovio 谱面渲染和精选标注接纳，未执行音乐理解模型，不制造语音转写或供应商费用回执。

## 已有证据

- `installed-engine-verification.json`：真实舒伯特录音与 Noctave MusicXML，编译 Engine 安装到源码目录外；替换 Verovio 模块后确实收到库失败，恢复模块后原任务继续并生成 3 页谱面、3 个音乐事件。要求尚未接纳的逐音符能力时任务保持失败，已生成的完整清单保留；只要求当前已就绪能力后继续成功。实际加载的是独立可替换的 Verovio 6.3.0-425dd7b，未使用源码树内模块回退。处理中的费用连接仅查询已确认的平台预算，模型调用为 0。
- `browser-verification.json`：真实 Admin 页面在 1440px 和 390px 下创建精选音乐并直接启动；API 状态为受控夹具。提交 1 次创建和 1 次启动，按实际 2/4 单元显示 50%，没有页面错误。截图已直接查看，窄屏创建弹窗内部可滚动，进度卡片和按钮无水平溢出。
- `http-acceptance-result.json`：既有真实 HTTP 服务、SQLite 账本、运营队列及源码目录外的 Engine，实际导入舒伯特录音和谱本；在谱面完成时暂停，销毁并重建宿主后以同一 invocation 继续，原音频字节不变。在线导入和共享发布就绪，两位用户各自创建私人工作区，共用同一资料发布；其中一人更新目标不改变另一人的目标或原发布引用。双方音频 Range 均返回 206 与正确 32 字节，谱面 SVG 可读。供应商支出、用户扣费和调用记录均为 0，重复进入没有触发公共预构建。
- 后台组件回归 18 项（运营构建 11、共享发布 7）通过；宿主准备路径 2 项通过；音乐合同 8 项与视频运营回归 9 项通过。后台与 DSH 生产构建、Core/DSH/Admin 类型检查通过。实际音频/谱面处理和发布范围以上述具体证据为准。
- HTTP 验收后，Engine 再次构建并纳入最终事件 occurrence 绑定、校准后事件／结构位置更新、声部与声线并集、theme_id 合同和本地校准命令；最终包为 608 个模块，仍使用相邻 Verovio 6.3.0。该次更新复用上述安装与运营证据，相关 Core 行为由对应切片的定向测试验证，未重复整条 HTTP 验收。
- `calibration-engine-verification.json`：开发入口与源码外安装 Engine 的 `music-calibrate` 命令，在真实舒伯特工作区的独立副本上保存候选、追加修订历史，JSON 输出与落盘一致、原音频不变，pending 保留。放置发布标记后两种入口都明确拒绝，manifest 与历史字节不变。此命令验证不代表实际听核；测试修订明确标注为自动化持久化检查。

## 复跑

```powershell
$env:UNDERSTAND_BOOK_SIDECAR_OUTPUT_DIR='E:/allwork/download/agent/understand-book/target/music-operator-20261022/engine'
node apps/desktop/scripts/build-sidecar.mjs
node --import tsx packages/dsh-plugin/test/operator-music-engine.acceptance.ts
node --import tsx packages/dsh-plugin/test/music-calibration-engine.acceptance.ts

# 在另一终端启动既有 HTTP 服务 4190 与 fixture 4191；编译完成后也可直接运行该测试二进制。
cargo test -p server --lib tests::s2_b1_browser_host::s2_b1_browser_host -- --exact --ignored --nocapture
node --import tsx packages/dsh-plugin/test/operator-music-http.acceptance.ts

# Admin 在 4186 运行；此脚本以受控 API 检查实际页面交互。
node apps/admin/scripts/music-operator-browser.mjs
```

## 能力边界

安装态输入包含真实录音和谱本，自动对齐候选仍是 pending，因此实际就绪能力是 `music`、`music_score`、`music_events`，不宣称已完成逐音符校准。页面测试证明界面行为，安装态测试证明实际本地处理与动态库替换，HTTP 测试证明任务、发布、授权、费用和私人目标归属；这些证据不证明模型音乐理解或初学者教学效果。恢复时保留原媒体与已接纳清单、校准，轻量本地解码与渲染会重新运行；没有将其描述为所有处理阶段均命中缓存。
