# 音乐伴听界面验收 — 2026-10-22

本轮验证真实媒体播放、谱面呈现和播放器交互，不构成人工音乐听核或原演奏逐音符对齐验收。

## 真实舒伯特录音

- 输入：`.understand-book/music-schubert/music_source_manifest.json`，Harald Vetter 101 秒录音，Noctave MusicXML 经 Verovio 生成的 3 页 SVG。
- 页面：`http://127.0.0.1:4192/music-reader-visual.html?fixture=/@fs/E:/allwork/download/agent/understand-book/evals/music/results/music-review-context.json`。这是本地组件验收入口；正式入口为工作区的 MusicReaderPane，读取 `/book/music_context`。
- 已实际操作：播放、暂停、0.75 倍速、拖动播放位置、声部选择、作品导览、谱面合成示范入口、当前事件循环、问问这段。
- 播放器实际状态：点击提问后 `currentTime=6.850188`、`paused=true`、`playbackRate=.75`；问题冻结为原录音 6–7 秒，随后拖到 2 秒仍保留原问题区间。
- 循环实际状态：0.58–5.42 秒区间播放至 5.081688 秒，随后回到区间内，暂停读数 2.511507 秒。
- 当前原演奏对齐为 `pending`。页面不显示候选逐音符光点，不把候选小节作为当前真实小节或问题谱面锚；明确显示待校准状态。
- 视图：桌面 1280×720、手机 390×844。导览和示范使用浮层，底部控制始终可见；横向声部选择只占一行，多乐器默认按乐器显示，选中后才展开其内部声部。

## 动画的确定性验收

- 独立 fixture：`create-ui-animation-fixture.ts` 使用同一真实 MusicXML 的前 4 小节，由 `synthesizeScoreDemonstration` 生成 38 音、6150.0227 ms 的 WAV 和生成器实际时轴。
- `music-animation-context.json` 标明 `score_synthesis`，界面显示“谱面合成示范”。此 fixture 没有把原录音候选映射改成 accepted。
- 在 4700 ms，实际界面同时显示上谱表 2 音和弦的红点和下谱表 1 音绿点。125% 缩放后仍与谱音重合；手机宽度变化后播放器自动横向定位当前谱音（实测 scrollLeft=181.6 px）。
- 实际手动滚动后显示“回到播放位置”，点击恢复跟随；播放过程中实际光点 opacity 同时包含 .9 与 .460714，显示活跃音与短尾迹。暂停及任意跳转从当前时钟重新计算状态。
- 光点坐标缓存于各页 SVG 的 viewBox 坐标系，连续播放只查询当前时间的区间索引及已生成标记；不逐帧调用模型或重新解析谱面。

## 验证结果

- `pnpm --filter @understand-book/web typecheck`：通过。
- 音乐时间轴、主题身份/声部过滤、冻结问题、命令 claim/complete、播放器真实 promise、选段起点恢复、Agent run 状态相关测试：20 个通过。
- 测试能捕获：和弦/长低音漏高亮、反复定位到第一次演奏、pending 被当作可靠谱位、播放器拒绝却回报成功、seeked 前回报成功、用户已操作仍执行旧观察命令、已卸载或未获 claim 的播放器重复执行。
- 最终 `pnpm --filter @understand-book/web build`：通过，2224 modules，19.88 秒，包含最终音乐场景文案。统一 MusicSourceTarget 类型的检查及此次构建内 typecheck 均通过，dist 已冻结交给真实模型联调。有现有 PDF 混合导入、Lottie eval、较大 bundle 提示。

## 贝九真实选段与主题模式

- 最终输入：`.understand-book/music-beethoven/music_source_manifest.json`。完整第四乐章录音 24:08，精选原谱第 93–164 小节，保留真实录音 171.572–289.469 秒位置。6 个实际发声部件、1249 个谱面对象、17 页真实 Verovio SVG；没有为了演示增造 36 个发声部件。
- 第一次进入从深度选段起点开始；后续恢复听者真实播放位置。作品导览同时保留整个乐章 0:00 与选段/事件入口。谱面直接保留原第 93 小节起的印刷小节号。
- 支持“声部 / 音乐作用”两种明确着色方式。主题只消费显式 `MusicEventRole.theme_id`，不会从自由文字猜身份；伴随和低音分别保留角色名称。
- 真实浏览器核验：大提琴 + 倍大提琴 → 中提琴 → 第一小提琴三个呈现，主题谱音和图例始终为 `rgb(174,118,26)`，参与乐器名称随事件改变。其他乐器的伴随/低音角色不会取得主题色。主题聚焦在事件交接时更新声部，并定位该段证据谱面；无逐音符覆盖时按钮明确称“主题谱面”。
- 默认突出当前明确主题，完整声部视图可切回。390×844 时乐器/角色都为单排横向选择，未挤占底部播放/速度/循环/提问按钮。手动滚动后可返回主题谱面；尺寸变化后主题谱音仍回到视野。
- 贝九当前 `pending`；角色静态谱面来自原谱证据，事件时间仍是候选窗口。页面没有虚构逐音符光点，也没有将候选小节写入冻结问题。

## 性能记录

- `music-beethoven-desktop-performance.json` 与 `music-beethoven-mobile-performance.json` 是页面中的开发测量输出，含真实页数、对象数、媒体状态、SVG请求耗时、坐标建立、绘制函数与播放帧间隔。绘制耗时指 JS 查询/样式更新，不等同于浏览器合成耗时。
- 最终冻结后桌面单次载入：17 页 + 坐标建立 439.5 ms，其中坐标建立 239.8 ms；连续从约 200.3 秒播放到 245.3 秒，跨过低弦到中提琴交接。末尾 600 次采样绘制 JS p95 0.1 ms，最大 0.2 ms；帧间隔 p95 11.6 ms，最长 83.7 ms。
- 窄屏单次载入：17 页 + 坐标建立 390 ms，其中坐标建立 149.5 ms。连续播放末尾 600 次采样中绘制 JS p95 0.1 ms，帧间隔中位数约 6.1 ms，p95 6.7 ms，最长 50.8 ms。
- 建立坐标时先测量所有页，再附加覆盖标记，避免逐页交替写 DOM 与读布局。连续播放只更新时间索引命中的谱音；开发测量最多保留 600 个样本。
- 本机浏览器开发页面的首次载入仍有主线程长任务，长任务记录保留在 JSON，不把单次环境观察当作所有设备的帧率承诺。已接受原演奏逐音符数据尚缺，贝九数据本身无法测试大量同时活跃光点；多声部光点由独立合成 fixture 验证。

## 连续画面

- `music-synthesis-animation-screenrecord.mp4`：10.08 秒，1280×720，H.264；真实浏览器中已有前四小节合成录音以 0.75× 播放，126 张连续截图按实际采集间隔编码为 25 fps 视频。
- 这是无声 UI 录屏；屏幕标题和横幅明确标识“合成动画验收 / 谱面合成示范”。未给原演奏添加伪同步声轨。原始采集时间保存在 `synthesis-recording-frames/capture.json`。
- 已检查编码长度、252 帧与中段实际画面，上谱表红色和下谱表绿色光点同时出现并有短淡出。

## 截图

- `music-reader-desktop.png`：真实录音，钢琴下谱表聚焦、待校准状态。
- `music-reader-mobile.png`：真实录音，手机视图、循环播放。
- `music-reader-synthesis-animation.png`：真实乐谱合成 fixture，和弦与低音同时标记。
- `music-reader-synthesis-mobile.png`：合成 fixture，手机横向定位当前谱音。
- `music-beethoven-desktop.png`：贝九低弦主题聚焦。
- `music-beethoven-theme-viola.png`：主题交到中提琴，保持同一颜色。
- `music-beethoven-mobile.png`：390×844，第一小提琴接过主题。
- `music-beethoven-mobile-parts.png`：390×844，六个真实部件横向选择。
- `music-reader-published-app.png`：实际发布的完整 Reader，包含真实谱面、伴听控制、问答栏。

## 正式发布阅读器检查

- 入口 `http://127.0.0.1:6677/`，实际 publication `01a16203-d21b-7f12-b3c7-fe12ede993de`。
- 完整 App 已显示音乐伴听、3 页谱面和问答侧栏；媒体加载到 readyState 4，实际 duration 101 秒。音频 URL 带真实 book_id / publication_id，未使用组件 QA 的伪 publication。
- 浏览器错误日志为空、页面没有 alert 错误。本次仅打开检查，没有启动模型对话；真实模型多轮联调由另一项验收记录覆盖。
- 音乐场景文案已适配“这首作品 / 带我听懂 / 专注聆听”，学习目的的枚举、scope 和就绪条件不变；LearningPurposeEntry、TutorPanel、TopBar、RightRail 合计 85 项组件测试通过，类型检查通过。

## 双时钟与播放前提示补充验收

- 最终前端生产产物为 `reader-ufZ0xoxF.js` / `reader-Cgwx7AQ3.css`。类型检查与生产构建通过，2,228 模块、构建 22.33 秒；播放器 15 项、命令分发 3 项相关行为测试通过。
- [playback-cue-ui.mjs](../playback-cue-ui.mjs)使用真实已发布舒伯特资料及浏览器实际音频元素，命令传输使用浏览器测试适配器；没有调用模型。该检查验证前端媒体和显示顺序，服务端与模型验收由 `live-listening.mjs` 单独覆盖。
- [music-playback-cue-ui.json](music-playback-cue-ui.json)记录 9 个实际操作回执。原声停在 4,200.125 ms 时，示范播放、观察、暂停均保持原声位置和暂停状态，独立回报示范时钟；返回原声后移除示范标识与状态。
- 两次实际 `audio.play()` 调用前，精选事件的原文提示已经显示并经过动画帧。此次示范提示领先 63.7 ms，原声提示领先 26.7 ms；两者的文字与对应 `listen_for` 完全一致。用户手动调速后提示清空。
- 示范播放时点击“问原声这段”，两路音频都暂停，问题仍使用原录音的 4,200.125 ms 位置；窄屏的取消问题按钮可实际点击，不受示范浮层遮挡。
- [music-demo-cue-mobile.png](music-demo-cue-mobile.png)为 390×844 的示范与来源提示；[music-original-cue-desktop.png](music-original-cue-desktop.png)为 1440×1000 的原声提示。两张实际截图均已查看，控件可见，浏览器错误日志为空。

## 验证边界

原录音的完整逐音符准确性、贝九选段的音乐解释及教师/用户对话质量不由本次界面验收证明。独立组件预览没有替代正式服务端发布、权限、会话和来源回查的端到端验收。
