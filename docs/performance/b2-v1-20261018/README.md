# B2 / V1 切片验收证据

日期：2026-10-18。实现说明见[个人资料准备与跨资料媒体](../../个人资料准备与跨资料媒体-20261018.md)，状态见[统一切片§17](../../切片方案-目标工作区与统一Agent额度.md)。当前工作树未提交、未部署。

## 已通过场景

| 范围 | 结果 | 证据 |
|---|---|---|
| 定向测试 | Server19、真实原声1、Runtime11、DSH6、Core15，共52项，0失败 | [验证汇总](verification.json) |
| 真实后台书籍准备 | 新编译Engine、原生DSH、Rust HTTP与账本贯通；预算拒绝前无调用，暂停及宿主重建后复用已接纳单元 | [HTTP结果](http-result.json) |
| 逐次计量 | 49次受控模型调用，49份费用记录；测试费率合计637 micro-CNY，待核算0，均归原账号/工作区；改目标和复用不增加调用 | [HTTP结果](http-result.json) |
| 个人视频 | 实际8秒MP4上传与本地Whisper生成2段原文，导入原工作区；没有付费CLI调用和模型扣减 | [HTTP结果](http-result.json) |
| 跨资料媒体 | 当前资料仍为书籍，读取另一份视频真实画面及原始音轨；词级时间与发布绑定正确，保存的转写不变 | [真实原声结果](real-cross-audio.json) |
| 页面闭环 | 7.6秒通过，81,016字节分3块上传，确认1.25%上限、暂停/继续、切换后归回原工作区；页面错误0、390px无横向溢出 | [浏览器结果](browser.json)、[桌面](personal-desktop.png)、[手机](personal-mobile.png) |
| 类型与构建 | Core/DSH/Web类型检查、Engine/DSH/Web生产构建通过 | [验证汇总](verification.json) |

所有52项均去重计数，重复复验不累加；HTTP和浏览器另外列为完整场景。桌面及手机截图已逐张查看。浏览器使用实际HTTP服务，由受控宿主提交中间执行状态；书籍实际Engine和视频本地转写由HTTP场景独立覆盖。

## 执行范围

语义模型内容和用量由本机受控供应商提供，测试费率只用于验证记账；没有真实付费模型请求或供应商账单对照。FFmpeg解码、Whisper推理、Engine计划/候选接纳、DSH原生协议、数据库和浏览器为真实执行。原声结果保留机器识别不确定性，不证明主模型听过原音频。

个人视频准备当前为本地转写及原文学习，完整语义预构建由既有平台路径提供。原始PDF仍需工作台对齐。E1/E2联合验收、真实运行与发布准备独立待做。

## 本机复跑与原始日志

定向命令和环境条件见实施说明的“复跑入口”。本机日志保留在`target/`：

- `b2-v1-server-final.log`、`v1-real-audio2.log`、`v1-runtime-tests2.log`、`b2-metering-tests2.log`、`b2-core-tests.log`。
- `b2-http-acceptance.log`、`b2-browser-test3.log`；后者为清理浏览器轮询后的最终通过记录。
- `b2-core-check.log`、`b2-dsh-check2.log`、`b2-web-check3.log`、`b2-engine-build.log`、`b2-dsh-build.log`、`b2-web-build.log`。

当前Engine位于`target/b2-v1-20261018/bin/`。公开证据只保存结果、费用事实与截图，不包含宿主登录连接。历史视频接入和后台构建证据仍分别位于相邻的`video-workspace-integration-20261018/`及`video-operator-20261018/`目录。
