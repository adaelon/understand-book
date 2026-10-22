# M3 本地音乐校准界面验收

2026-10-22，最终产品构建 `reader-C3t1OHpj.js` / `reader-aiMBTXEW.css`。类型检查与 15 项相关行为测试通过，生产构建 2228 modules、21.10 秒。同拍和弦按科学音名（实音）区分，出现序号不展示冗长机器 ID。伴听恢复播放保留原循环，显式指定 start 或 end 的播放仍清除循环并单次播放；两种行为均有组件时钟事件测试。

## 可用路径

本地未发布音乐工作区从 `LocalApp` 自动进入校准页；可关闭返回工作区并从“本地音乐校准”重新打开。已发布材料与非音乐材料返回空 context，继续原阅读入口。尚无 occurrence 的材料显示候选预构建提示，不制造等分时间。

切换本地材料后重新读取可用入口，不自动抢占当前工作区。点击校准会先卸下旧预览并重新读取 context，读取失败清空旧入口且显示错误；该状态路径已有组件测试。真实切换验收发现同一个无版本 audio URL 会复用已加载录音，服务端已将本地资源 URL 绑定真实录音与谱本版本；最终真实浏览器复验确认舒伯特 101 秒切换为贝九实际解码 1448.817596 秒，切回后为 101 秒，旧音频和谱面 URL 均返回 404。普通论文切回音乐后入口出现且不会自动抢占，未发布工作区返回 App 的 publication 错误已消失。

校准页使用真实本地录音、当前谱页和源对象 ID，支持原时间 / 草稿时间复听、暂停、速度、循环、连续 occurrence 范围、共享小节边界、谱面点选音符、捕获播放位置、音符起止与重新起音标记、修改原因和依据、保存修订及历史。空心符头中心的点击由音符边界接收，谱线不再截获点选。

保存默认为 pending。仅接纳小节时，音符 coverage 仍为 pending；逐音符接纳要求所有非休止音符有时间。新增或改动音符时间需要明确复听依据；未改动的候选音符保留原 provenance。写入失败保留草稿，只有收到服务器返回的新 context 后显示成功并更新历史。

## 实际浏览器与数据

使用仓库 Playwright 的独立 Chromium 浏览器，没有连接用户浏览器。先以 Vite 4193 / 4194 代理真实本地服务 4195 / 4196，随后完成正式生产构建。无伪 publication、无 mock API。

- 舒伯特：`.understand-book/music-calibration-qa/schubert`，78 次小节出现、3 页谱面，真实 Vetter 录音。0.75× 播放时 `readyState=4`、`paused=false`、实际 `currentTime` 前进。校准预听采用 rAF 读取实际媒体时钟，暂停和卸载取消；同一 1.820 秒边界复测在 1.822831 秒触发回跳至 0.580 秒，实测越界 2.831 毫秒，修复了此前原生 timeupdate 约 200 毫秒的余量。
- 贝九：`.understand-book/music-calibration-qa/beethoven`，原谱 93–164 小节、72 次出现、6 个声部、17 页谱面。实际播放从全乐章原时间轴 171.572 秒附近开始，0.75×、`readyState=4`；无逐音符候选的输入留空。
- 两份材料均完成：点击真实谱音 → 调速和复听 → 选入相邻小节并移动共用边界 → 丢弃修改 → 保存待核修订 → 刷新后见历史 → 通过真正的 LocalApp 根入口打开、关闭和重新打开。
- 两次浏览器保存都保持原候选时间，理由为“界面流程验证，候选保留待检查”，依据明确写为未进行人工复听核准，新增 provenance 为 `interpretation`，coverage 均为 `pending`；各自历史从 1 条增至 2 条。
- 桌面 1440×1000 与窄屏 390×844 截图均已亲看；未出现页面横向溢出或脚本错误。返回工作区按钮在长标题窄屏下完整可见。保存区滚入视口时不会被粘性播放器遮住标题。

## 证据

- `music-calibration-schubert-desktop.png`、`music-calibration-beethoven-desktop.png`：源谱点选和时间编辑。
- `music-calibration-{schubert,beethoven}-mobile.png`、`music-calibration-{schubert,beethoven}-mobile-save.png`：窄屏及保存区。
- `music-calibration-{schubert,beethoven}-local-app.png`：真正 LocalApp 入口关闭后重开。
- `music-calibration-{schubert,beethoven}-history.png`：刷新后保存历史。
- `music-calibration-{schubert,beethoven}-ui.json`：真实请求后的 revision、历史数量、媒体状态和浏览器结果。
- `music-calibration-schubert-final-ui.json`：最终窄屏按钮边界、实际循环回跳。
- `music-calibration-workspace-switch.json`、`music-calibration-switched-{beethoven,schubert}.png`：最终真实切材料、音频版本、旧资源拒读与实音标签。
- `music-reader-published-app.png` / `music-reader-published-app-check.json`：正式发布 Reader 的“这首作品 / 带我听懂 / 专注聆听”文案、真实 publication 音频及空错误列表。

复跑入口：`node evals/music/calibration-ui.mjs --url http://127.0.0.1:4193 --name schubert`。该命令会在指向的本地副本新增一条明确标识为界面验证的 pending 修订。`--screens-only` 只验证播放循环与窄屏，不保存修订。`--published` 只检查正式 Reader，不调用模型。

## 验收边界

这次完成的是实际可用的人工校准入口与保存闭环，不代表原演奏已获得人工音乐听核。缺音符、跨源和发布目录拒写由 core 的真实 HTTP 验证覆盖；前端组件覆盖缺音符拒接纳、保存失败保草稿与上下文成功后复位。历史持久化和重建重放由 core 的独立验收记录负责。原样本仍保持 pending。
