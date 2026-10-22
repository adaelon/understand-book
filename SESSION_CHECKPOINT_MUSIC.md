# SESSION_CHECKPOINT_MUSIC — 2026-10-22 乐谱界面第二版

## 授权与交付状态
- 用户最新要求“这个页面给乐谱的空间太少了，而且设计感不够，再做一版”。界面第二版已完成。沿 software-engineering skill，Windows共享dirty工作区。没有提交/推送/部署；禁止重置或整树格式化。
- 工程已落地：资料合同、MusicXML/Verovio、真实录音导入、候选对齐与图形校准、来源化事件、播放器/动画/示范、Agent来源与教练、运营恢复和共享发布。
- **完整M0–M8成品验收未通过**：原声全曲逐音听核/精度、贝九共享许可、完整四轮教学/响应速度、真人学习效果仍未完成。不能把工程或浏览器播放当作听核/学习证据。
- 本轮工具/代码/文档已收口；没有运行中的付费模型、Cargo、QA浏览器或临时Vite。只保留最终6677预览。

## 最终版本与预览
- http://127.0.0.1:6677/，root session **56565**。入口 tmp/music-preview.mjs，复制target/debug/server.exe到tmp/music-preview-server.exe，不锁主binary。
- 最终server/publish_book编译 **37737通过**，包含全部最后修复；第二版Web **27137通过**，reader-DDJl87jm.js / reader-DbbIKkYm.css，2228modules。
- 最终发布 `.understand-book/music-acceptance-service/library/published/music-schubert-d780-3-vetter/01a1625b-808a-77d2-9cfb-1ecc62bd219f`。
- 独立 `.understand-book/music-preview-final-memory` / `music-preview-final-private`；旧预览空间保留。已有工作区固定旧publication，故不能仅换server入参就假定切了资料；API已核验新ID、3页、101041.633ms及新cue。
- 第二版实际App截图/检查见 evals/music/results/score-space-v2/published.json；readyState4、0pageerror、0alert，桌面/全屏/390手机与提问实际操作通过。
- 停预览时只处理6677且exe名music-preview-server.exe、命令含上述新UUID的进程；不要误停其他任务。

## 最新检查
- Runtime音乐定向11项通过；最后新增 `music_review_editorial_feedback_never_calls_revise_accepted` 单项通过96219。内部审阅13通过/1原ignored。最后两处仅指导文字，最终编译通过。
- Server音乐定向16项通过，包括双时钟/cue、本地校准、错源URL、未发表投影、发布/授权/原来源/私人理解。
- 前端播放器15+dispatch3+角色2=20通过，第二版typecheck/build通过；此前Core10+校准相关7、合同36导出+10Rust行为/10TS、Web文案85、Admin18、DSH2prepare+9video等证据沿用。
- docs:tools **2204完成**：Resident51/localMCP17/remoteMCP12/executor4/Tutor17。最后指导无schema新增，不需再生成。
- 第二版新cue真实Chromium桌面/390手机，9动作、0错误；示范play前提示已绘制，截图Root亲看。命令传输为测试适配器，不冒充模型联测。
- MusicReaderPane.vue 改为紧凑头部/单行工具栏、声部浮层、暖白谱纸/墨绿播放条、原生全屏；全屏提问先暂停并退出全屏，手机保留回到播放位置。乐谱高度占比58.2→81.0%、41.9→73.7%、45.0→76.2%。证据 score-space-ui.mjs、results/score-space-v2/、验收文档第二版章节。

## 真实模型结果（均已停）
- evals/music/live-listening.mjs，现有deepseek-v4-flash真实Provider+生产Reader+自动coach；脚本模拟学习者，不是真人。
- 01：0完整轮/19调用/1,450,564tokens。缺直接interact、来源数组顺序误拒、review混淆引用/播放。修复并回归，15份完整state改已有退休机制，后续1份。离线528382→284930B，不推断货币省费。
- 02：0轮/10调用/408,916tokens。实际播放未算进展导致提前fallback、音乐证据遗漏。已修已测。
- 03：0轮/17调用/977,332tokens/480s。review accept但额外字段误拒、accept后非阻塞润色继续重审。已修额外字段/优先交付，真实fixture通过。
- 04：1完整轮/17调用/930,660tokens，169.5s首轮，68.9s播放，解释晚100.6s；第二轮demo回执混时钟409取消。修独立demo状态/播放前cue。
- **05：1完整轮/29调用/1,806,215tokens；首轮208.434s，158.466s原声播放；二轮422.098s后incomplete材料摘录；第3/4未启动。** demo→原声Applied、无409、来源实际source.resolve正确。
- 05仅2coach ready，第二轮消费首轮exact guidance，无理解/掌握假报；不能声称5coach四轮通过。固定1.8M启动门槛，最后在途请求完成超6215后停止，未增上限/无追加模型run。含缓存1039616tokens，金额未核验。
- 二轮原声已播后6主请求+2review占657711tokens/56.1%；3次select字段错误273834，空参数重复读54396。两次review124.4/103.1s均真实抓到cue/顺序问题，不能跳过。末次请求307679B只有1份state；schema26.6%、system44%、tool21.2%。
- 05证据目录 `evals/semantic/quality-results/music-listening-20261022-schubert-05`：semantic-review.md、performance-review.json、request-cost-analysis.json、run.json、answer-1/2.md、turn-1/2.json。

## 最后具体修正
- 原声receipt.current_ms/paused始终原声，demonstration_state独立真实示范时轴；pause停双方，原声play/seek/loop退出demo。用户示范中提问暂停双方并明确问原声该段。
- Play有start/end=指定单次清loop，无参数=resume保loop；cue_event_id需本轮已读事件。前端显示原文、绘制后play；displayed_cue由server验证event/text，完整解释仍design/review，progress只进展。
- 05发现两次review仍不通过：旧two-staves安排下一遍下方与用户上方目标冲突，合成/原声关系只在播放后草稿说。新curation cue精确为“谱面示范按谱另行合成，并非原录音分轨；谱上上下两层原本同时进行。这一遍只跟上方旋律的走向。”
- 同源新发布链：旧05版01a16203…→只收窄cue01a16258…→最终补关系01a1625b…。旧版本/05证据保留；manifest仅cue变，原录音/谱本/demo原字节、78occurrences/900notes/allpending不变；真实m3–4上下voice从beat0同时，支持谱面事实。
- Runtime最终guidance：demo/原声同一听觉目标/合适关系cue；修实际播放顺序后再review；select修稿保留完整participation/delivery_text/当前invitation_ref，direct_explanation用explain；revise反馈不再自称已accept。未再付费模型验证，完整四轮依旧失败验收。

## 已完成工程证据与素材限制
- M3 LocalApp→MusicCalibrationPane→localcontext/media/calibrate→Engine music-calibrate→Core事务，支持待核/显式接纳、理由/依据/历史、重建重放；published只读，local_only可校准不放宽共享。
- 两样本真实HTTP+桌面/390操作、保存/刷新/重建7.268s/8.907s通过；切材料101→1448.817596→普通→101，旧媒体URL404。rAF循环同1.820边界越界实测2.831ms，非跨设备保证。
- M3证据 docs/performance/music-calibration-20261022/README.md、rebuild-replay.json、schubert-demo-relation-cue-{verification,publication}.json；evals/music/results/calibration-ui-verification.md、music-calibration-workspace-switch.json、final-cue-verification.md。
- Engine608modules，源码外Verovio6.3.0替换失败→恢复/校准通过；运营create→pause→destroy/rehost→resume→publish→2用户private隔离+audioRange206/SVG，0模型/费用，第二用户不重复建。证据docs/performance/music-operator-20261022/README.md。
- Schubert VetterCCBY4.0录音101.041633s，78小节1066对象3页；900候选+47grace，m38/46漂移、m65/69/73未分开、3grace重合，无独立真值/人耳听核，全部pending。
- 贝九IV完整录音1448.85551s，选m93–164共72小节6实际声部1249对象17页，候选171.572–289.469；现代转录/标注许可未明local_only。无固定音高打击乐合同未支持，选段其休止。
- 动画真实浏览器无声录屏 evals/music/results/music-synthesis-animation-screenrecord.mp4，10.08s、合成38音前4小节；不证明原声同步。ui-verification.md有和弦/角色/跟随/性能证据。
- 未接读时音乐音频模型，music.read heard_audio=false；用户MP4仅视觉参照，无听核/再分发。

## 文档与后续入口
- 必读 docs/验收-音乐伴听.md（最新结论/所有证据）、docs/adr/0175-curated-music-prebuild-and-guided-listening.md、docs/切片方案-音乐预构建与交互伴听.md。架构/代码链路/CONTEXT已更新。
- 后续实质工作：独立起音真值与实际连续听核→校准接纳；贝九素材权利；修订后四轮教学/合理时延验证；真人初学者学习反馈。不能仅加预算或放宽review来宣布通过。
- 早期音乐子代理已完成；第二版UI由主代理完成，未派发新子代理。未完成音乐验收项已明确落文档；不存在正在后台完成的模型任务。
