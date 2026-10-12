# TB5：教法与理解视图

2026-10-12，TB5 完成，未提交、未部署。教法说明覆盖尝试、示范与按需帮助，区分本次学习的默认教法和当前请求；理解详情直接展示适用目标、帮助条件、原话、暂定解释与纠正入口。

## 实施步骤

- [x] 按 checkpoint 完成冷启动读序；HEAD 为 `9f1ea18`，保留 TB1–TB4 与 TO3 在途修改。
- [x] 更新 TutorPanel 的默认教法说明；在 UnderstandingSpace 展示普通交流的适用目标、帮助条件和未评分性质。
- [x] 组件与类型检查；以 TB4 实际记录验证浏览器读取、切换教法、纠正及重开。
- [x] 更新切片方案、代码链路和 checkpoint。

## 实现与依据

`TutorPanel.vue` 将引导思考描述为“尝试、示范与按需帮助”，直接讲解描述为“完整说明当前问题”。默认教法仍由原 `set_mode` 保存；说明文字告诉读者可以在对话中要求示范或本轮直接讲解。

`UnderstandingSpace.vue` 将假设的 `prompt` 从折叠详情移到“适用目标与帮助条件”，有已展示活动帮助时显示次数。系统解释、原话和用户纠正分别呈现，普通交流明确为未评分；`teaching_implication` 的标题改为“接下来怎样帮助你”。正式活动的尝试、评估和反馈揭示继续采用原合同。

原 `/tutor/evidence` 已把假设的 `target.expected_performance` 投影为 `prompt`，同时返回 `assistance_count`。TB4 样例的次数为 0，但适用目标明确含此前讲解与教具已显示结果的支持，系统解释也保留代码、预设结果和错误原因；界面完整展示这些条件，没有据次数推断独立完成。生产 Server、Runtime、存储和工具合同无需修改。

## 实际记录与浏览器验收

测试从 [TB4 现场](../tutor-building-blocks-tb4/browser/followup-context.json) 的 `read_facts` 和 [接续结果](../tutor-building-blocks-tb4/browser/followup-result.json) 的 `evidence` 加载同一条已保存理解。原事实 ID、binding、payload、解释、原话、目标和引用保持原样；隔离夹具只补视图读取所需事件外壳，时间标为 `tb5-captured-fixture`，不重建历史因果边。

Browser 只使用测试挂载页，TutorPanel、useTutorControl、UnderstandingSpace 和所有 API 返回均沿生产实现。真实接口完成引导→直接讲解→引导的两次保存，读取理解，提交一条明确标识的脚本纠正，再重开 MemoryStore 和页面。当前列表仅留下纠正后的解释，原话、适用目标与帮助条件保持，状态仍为 `hypothesis / unassessed`，能力投影为空；普通交流没有发出反馈展示或评分请求。

| 验证 | 结果与证据 |
| --- | --- |
| TutorPanel／UnderstandingSpace | 20 项通过，[web-tests.log](web-tests.log)；含帮助次数 0／2、纠正保留条件、原有分页与正式反馈 |
| TB4 记录投影与纠正 | 1 项通过，[server.log](server.log)；原话／目标／引用保持，旧解释被替代，无能力行 |
| 浏览器真实接口与存储 | 1 条完整路径通过，[browser.log](browser.log)、[checks.json](browser/checks.json)；host 1 项通过，[browser-host.log](browser-host.log) |
| 类型检查 | 通过，[web-types.log](web-types.log) |
| 视图与截图 | 1280／320／390 像素无横向溢出；已查看[窄屏教法](browser/modes-narrow.png)、[桌面条件](browser/conditions-desktop.png)、[重开后的纠正](browser/corrected-reopened.png) |

接口证据：[原理解](browser/initial-evidence.json)、[纠正后的理解](browser/corrected-evidence.json)、[最终列表](browser/understanding.json)。

## 本地复现

Rust 环境沿 TB4：`+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195`，dev/test debug=0、incremental=0，`TEMP/TMP=D:/codex-build/tb1-temp`，`-j 2`。以下命令不调用外部模型，不重写 TB1–TB4 回执；浏览器复现会重写本片 browser 输出。

1. 仓库根目录运行 `pnpm --filter @understand-book/web test src/components/TutorPanel.test.ts src/components/UnderstandingSpace.test.ts` 和 `pnpm --filter @understand-book/web typecheck`。
2. 运行 `cargo +1.95.0 test -p server --lib -j 2 tutor_tb5_captured`。
3. 启动 `cargo +1.95.0 test -p server --lib -j 2 tutor_tb5_understanding_browser_host -- --ignored --nocapture --test-threads=1`，等待 `TB5_BROWSER_READY`。
4. 在 `packages/web` 运行 `pnpm exec playwright test --config playwright.tb5.config.ts`；配置管理 4186 的本地 Web，接口来自 4187 的隔离 host。
5. POST `http://127.0.0.1:4187/stop`，host 核对并保存最终理解后退出。

## 检查目的与失败处置

- 组件测试：检测教法切换丢失或乐观生效、假设的帮助条件不显示、普通交流出现作答评分、纠正／分页／正式反馈揭示回归；失败修对应组件。
- Web 类型检查：检测本片模板、测试和 API 调用不符合现有类型；失败修具体类型接点。
- 实际记录浏览器路径：在隔离存储加载 TB4 已保存的事实与理解，使用真实 Tutor 路由核对原话、目标、解释和纠正；检测窄屏溢出与保存／重开丢记录，失败修视图或原接口。
- 交接核对：检测本片链接缺失、差异空白错误与 checkpoint 状态不一致；失败修对应文档。

## 已知限制

本片验证既有真实记录的视图、接口与持久化，测试挂载页及纠正文案为脚本夹具；没有重新生成教具或重跑完整聊天链。新增模型调用 0，真人 0。TB6 的真实读者与跨材料验收待实施；生产 Provider 自主循环及 DeepSeek 历史 1,032,768 token 未结算预占沿原记录。
