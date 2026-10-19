# SESSION_CHECKPOINT — 2026-10-15，FP7 联合验收进行中

## 新鲜度与授权
- 基准 commit：`6612932 ops: publish approved logo and latest reading updates`；大量既有未提交改动全部保留。
- 用户要求：阅读 checkpoint，实现 FP7，完成后刷新 checkpoint。未提交、未部署；§0冻结授权沿原切片。
- 证据入口：`docs/performance/focus-reading-fp7-20261015/README.md`。

## 当前状态（2026-10-15，已安排等待后续跑）
- 等待构建chat 01a120d7-f288-7442-94a3-add30572caa7（local）自然结束；wait_threads游标e17002ce-d104-493e-9a10-783074c15d6f:11，最新9/11、第10项仍在处理中。不要向该chat发消息或停止它。
- 已创建当前chat的heartbeat自动续跑，id=fp7，ACTIVE，每10分钟检查；未变化保持安静。构建自然结束后自动完成下列步骤，FP7真正交付后关闭该自动化。
- 当前无性能浏览器运行。79349、18965、53123已结束失败；59105独立基准、46393类型检查及观察器回归已结束。
- WebKit P5先后约25/90/4次等待后台补页超过5秒；90次状态ready/idle/misses0、preparing=true；第三次超时后继续推进并完成，排除死锁。原件long-wait-first/、long-wait-state/、long-wait-timeline/。
- 当时系统空闲内存降至286,640KiB；其他书籍构建并行。独立WebKit同一数值算法33/106/64ms→1948/5825/5847ms，见alignment-webkit-current-load.json。不要停止其他任务；用户13:05明确选择“等当前构建自然结束后继续”；等待，不停止其他任务。
- 移鼠误触发测量：正文/PDF观察器只接受读区内buttons非零的pointermove，记录inputType；新正式WebKit回归通过observer-input.log。既有P1较宽区间是保守上界、P6零仍有效；严格P3为程序按钮点击不受影响。
- 最终正文短测Chromium9项/WebView2三项均通过；严格P3 Chromium四组800次、WebView2两组400次均通过，零未就绪/长任务/P6，实际中位约1006ms。
- Chromium P5旧589次/20分2.304秒零未就绪/长任务/P6，但返回22px而初始19px；原件return-typography-first/。现用例返回前恢复初始字号并断言同源同几何，须再完成一次。
- 尚缺：WebKit严格本地200、弱网200和P5；Chromium同字号返回P5。WebKit旧1156ms本地组automation-overhead/仅诊断。
- 正式Web产品未再改，build-final.log通过；typecheck-final.log在13:04通过。临时产品probe均移除；afterEach保留失败状态JSON，10秒诊断flag已移除。

## 本轮修复
- `PdfFocusReader.run` 正常吸收已销毁时的准备取消，销毁后不通知宿主；新增回归先红后绿。
- `App.doGoto` 使用Reader上下文，使首次聊天恢复不再取消同材料MCP来源导航。
- WebKit正文原本地第26次输入至收尾alignments/geometryProbes各+1。`FocusReader.schedule`后台补页/暂停恢复经过RAF后任务，复查暂停/选择/可见/销毁；dispose取消排队帧。
- 首轮调度扩大到前台，手机旋转两次Layout为78.262/213.541ms；现缩小到后台/恢复，前台直接排任务。
- 四项新调度回归先红后绿；最新受影响46项单元通过。跨日志按文件计数，累计11文件164个不同相关单元分批通过。

- session81677在WebKit本地53次未就绪后自动停止；原件cancelled-neighbor/。准备连续三秒未完成，后两次ready翻页又取消并重做。现ready同几何且无前台请求时保留进行中的邻页准备，目标/几何变化仍取消；新回归先红后绿。

- session39333在102次停止：脚本追赶固定时间表产生556/589ms输入间隔。原件catch-up-inputs/。正常节奏及正文长读改按实际上一输入计时；联合汇总保留间隔并拒绝<900ms的正常组。既有PDF和其他正常组间隔均≥926ms，证据继续有效。

- 阶段/yield诊断定位复杂表格对齐，排除暂停未唤醒、读取、字体与图片；正式WebKit任务等待最高228ms。原件diagnostic-pagination/与diagnostic-yields/。
- `markdown-cooperative-alignment.ts`预取UTF-16数值用于Hirschberg内层，保留算法/edit顺序/4ms片段/线性空间。三张真实表独立WebKit110/238/174ms→33/106/64ms，完整edit数组逐项相同。alignment-comparison.cjs、alignment-original.ts、alignment-inputs.json可从仓库根重跑；正式页面仍须完整验证。

## 其他已完成证据
- 正式Web、Tauri宿主、本地服务构建；最新build-final.log、typecheck-final.log通过。
- Chromium正文14/PDF6，WebKit正文12/PDF5，实际WebView2正文3/PDF3功能通过。
- 两项真实本地服务正式入口understandbook/kimik3通过。普通阅读20个不同场景通过；RE3–RE5夹具修正合并读取和现有公开控件。
- PDF最终短测有效：Chromium P1最高38.102ms；WebView2 P1最高20.131ms，P2最高0.15625%，P4最高53.989ms。
- PDF正常：Chromium4组、WebView2两组、WebKit两组各200次通过；最终P6均0。
- WebView2 PDF长读20分13.599秒/601次，零未就绪/长任务/P6；峰值6页/5,170,176像素/3077DOM，原位返回4页/3,446,784像素/1560DOM与初始相同。
- Chromium PDF首轮本地第63次附近52/50/60ms三次长任务未归因；保留pdf-first-normal/。保留原RAF观察方式的200次诊断复测无长任务、最大轨迹事件34.172ms，P6全0；最终根PDF正常本地采用完整诊断复测。
- 原正文P5 Chromium/WebKit均≥20分钟/600次通过；本次因调度修正重新验证，原数据在prose-before-resume/。
- 165Hz WebView2逐帧getComputedStyle观察器造成测量干扰；实际帧采集移除该诊断。旧失败和独立帧事件归因均保留。
- Tracing 450张上限通过已收尾操作间分段采集；P4另有实际screencast保守上界，不以DOM-ready替代。

## 完成步骤
1. 用户已选择等构建自然结束；等上述chat完成，确认独立WebKit基准恢复正常后，运行tmp/fp7-finish.ps1：typecheck → WebKit严格两组200 → WebKit P5 → Chromium同字号返回P5 → Chromium真实触摸帧验证。每阶段首错停止，全部串行。
2. tmp/fp7-finish.ps1通过且无性能浏览器运行时执行 `python packages/web/playwright/fixtures/summarize-focus-release.py`；要求16组正常、14组实际帧、6组P4、3组长读。根results/summary.json是联合判据。
3. 用捆绑Python3.12和PYTHONPATH=tmp/fp7-plot-python运行plot-focus-release.py，检查resource-curves.png；依赖已经隔离安装。
4. tmp/fp7-final-docs.py只有summary通过才能运行；先核对最终日志/计数。它会把现README原样存RUN_NOTES.md并生成简洁最终README，更新ADR/切片状态。当前两者仍FP7待实施。
5. 架构/代码链路已写三项修复及测试接线；最终整体重写checkpoint≤80行。用户未授权提交或部署。

## 冷启动读序与入口
- 切片方案FP7、§2/§5/§8 → ADR0166 → FP7 README → FP6/FP5/FP1 README。
- `playwright.focus-release.config.ts`：正式dist4195，FP7_ENGINE=chromium/webkit/webview2，FP7_SUITE=prose/pdf/live。
- `focus-production-entry.ts`只读注入；`focus-test.ts`实际Tauri/WebView2；`focus-release.spec.ts`后台问答及DPR2/3触摸；`focus-live.spec.ts`真实服务。
- `focus-turn.spec.ts / focus-operations.spec.ts / focus-pdf.spec.ts`复用功能及P1–P6过程；fixtures中两份旧帧分析和联合summary/plot脚本。
- 捆绑Python：`C:/Users/Lenovo/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe`。

## 已知限制
- 实体手机、系统选择手柄、物理显示延迟未测；WebKit无同等时间戳画面/合成帧及Long Tasks API，相应指标为null。
- WebView2为实际Tauri调试宿主加载正式Web构建，非安装包验收；165Hz为浏览器帧测量。
- 资源是实例持有量，非浏览器总堆/GPU；PDF固定栅格清晰度限制沿FP6。
- 可控API回执与两项真实本地服务分别记录；后台问答未调用外部模型。PDF首轮三次长任务仍未归因。
