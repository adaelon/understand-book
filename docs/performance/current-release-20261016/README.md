# 最近产品改动上线（2026-10-16）

2026-10-16 **16:02 HKT** 已发布到 https://understandbook.top/ 。正式 release 为 `current-20261016`，服务 `understand-book-multi` active、PID **64600**，数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012`，schema14。此前版本为 `context-cache-20261015`。主服务、演示 worker 和 Nginx 前端目录均已切换。

## 发布范围

- 解释模板目录 revision 4：`discrete-update@3`、其余原七类 `@2`、`spatial-linked-views@1`。正式 Agent 可按需读取完整源码、适配说明与元数据；三维模板包含齿轮和弹簧。
- Three.js 托管、三维与图解联动、现场保存/恢复与静态输出，以及演示版本、反馈和案例支持。
- 跨书补讲 CB0–CB5：账号授权书库的参考读取、私人理解接续、多书证据引用、原任务追问和卡片/笔记接续。
- 2026-10-16 的对话压缩校验修复：实际失败样本中的范围、条目身份与目标校验接续。保留已经上线的上下文缓存与命令行书籍热发布。
- 公开体验入口：[模板库](https://understandbook.top/demos/methods/)、[齿轮完整作品](https://understandbook.top/demos/gears/)、[弹簧三维模板](https://understandbook.top/demos/methods/spatial-linked-views/v1/preview.html?topic=spring)。Nginx `/demos/` 只服务对应静态目录，缺失文件返回404。

## 构建与验收

使用独立 release 目录和新 target 构建五个正式 Linux 程序：server、manage_reader、publish_book、reader_maintenance、presentation_worker。编译与 Runtime 测试限制为单任务、1.8GB 内存、100% 单核 CPU；原服务在构建期间继续运行。源码逐字节对照前版，共212项新增或修改；清单见 [source-comparison.json](source-comparison.json)。随后补入三个测试文件的集成修正，不改变生产执行代码。

- 前端相关六组测试 **127通过**，正式多人版本构建通过。
- Linux Runtime **611通过、0失败、8忽略**，见 [test.log](test.log)。构建见 [build.log](build.log)。
- 本地 Server **795项通过、77项默认忽略**：修正测试后并行运行789通过，剩余6项单独复验通过；其中5项为本机并发网络/邮件桩超时，1项验证更新后的输入错误合同。完整结果不是一次全绿的并行运行。
- Linux 真实服务账户、隔离数据副本和独立 sandbox 实例完成模板发现、读取、写入、三种视口 WebGL 预览、交付与正常终答。三维几何实际存在，四分之一周期位置符合弹簧模型。使用本机受控 Provider，**外部模型请求0次**。见 [renderer-e2e.json](renderer-e2e.json)。首次脚本未提供 DeepSeek continuation 字段，使记录投影成历史文本；补齐受控响应协议后通过，未改产品代码。
- 切换前、本机切换后、正式域名三次检查均通过：reader 的11项、admin的12项授权版本清单逐项相同，默认书目录和正文可读；reader 的私有书籍读取仍返回404；authoring/plot/animation/preview均 ready。
- 外部浏览器通过正式域名验证：首页和阅读入口与构建内容完全一致；八类二维模板均可定位并更新读数，三维模板的齿轮/弹簧均实际启用WebGL，完整齿轮作品可切换齿数。没有脚本异常或失败资源。该验证先完成模板，再以具体按钮选择器完成齿轮与总览截图；见 [public-browser.json](public-browser.json)、[桌面总览](methods-public-desktop.png)、[手机弹簧](spring-public-mobile.png)。
- schema仍为14，实际运行文件来自新 release，发布 socket存在且权限0600。部署记录见 [deployment.json](deployment.json)，验证汇总见 [validation.json](validation.json)。

测试修正的原因：旧临时文件父目录使多个测试共用 `learning.db`，现按测试线程隔离；工具示例数更新48→52；MCP候选读取使用已实现的公开工具名称投影；SE3样例按当前 author.json和新源码读取；公开历史允许合法的来源位置而仍排除内部诊断；缺失消息使用当前输入解析的 `INTERACTION_INVALID`。没有降低实际业务断言或跳过失败用例。

## 上线过程与恢复

用户授权发布最近未上线改动。部署前确认无活动或未保存运行，进入维护窗口后再次确认；停服、完整备份，再切换服务与 worker；本机验收后开放正式入口并检查公网。停服至本机恢复验收 **11.88秒**；整个脚本 **48.86秒**，包含前后账号登录与公网检查。

完整数据备份：`/opt/understand-book/backups/current-20261016`（333MiB）。

原配置：`/opt/understand-book/acceptance/current-20261016/previous-configuration/`，保存 systemd unit、Nginx和presentation-sandbox.json。候选准备与部署脚本在本地 `tmp/current-release-20261016/`，服务端验收和部署脚本位于同名 acceptance 目录。

恢复程序时，先确认无活动/未保存运行，再进入维护窗口；停止服务，恢复原 unit 与 sandbox 配置，daemon-reload并启动 `context-cache-20261015`，验收登录、正文、权限和制作能力后恢复原Nginx。schema未变化，保留恢复时最新数据；完整数据备份只在确有数据损坏时单独恢复。本次没有触发回退。

## 已知限制

本次没有进行付费模型问答或 Agent 自主迁移新主题的质量评测，SE8与FP7实验继续暂停。真实三维验收证明模板、正式工具路径和服务器渲染可用，不推导读者学习收益。发布后服务器可用空间约1.9GiB；旧release和恢复副本保留。
