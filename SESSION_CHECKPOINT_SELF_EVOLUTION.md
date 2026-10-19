# SESSION_CHECKPOINT_SELF_EVOLUTION — 2026-10-16，三维联动模板已登记

## 新鲜度自检

- 写入时最新 commit：402d9be feat: preserve current reader, focus mode and runtime work。
- 本线未提交，其他工作线改动保留；线上发布另看专线 checkpoint。

## 当前状态

用户先要求建立首批模板，随后认为表现力不足，指定参考 Ciechanowski 制作完整齿轮演示。用户已认可作品并明确要求提取三维交互与图解联动。spatial-linked-views@1 已完成并接入正式目录；SE8 继续暂缓。

## 已完成

- catalog revision 4，共九单元：discrete-update@3、其余原七类@2、spatial-linked-views@1；原版目录保留。八新版已完成104组数学/SVG、24次重开/导出、24预览/8交付。
- 同一运行核心生成齿轮和弹簧两个自包含作品；附主例/改写例、卡片、适配说明、Author 元数据、检查与按需可读核心。
- 宿主固定 Three.js 0.180.0，prepare 可选 three，write 可选 libraries:["three"]；版本冻结、读取元数据、修订淘汰依赖遵循既有机制。
- 三维与图解使用单一计算结果；相机/拆解/时间通过原有桥保存，重开暂停，WebGL 像素可静态输出。
- 42 组独立数学及几何、6 次桥重开、6 次实际导出、6 组真实 Author 预览和 2 次交付通过。
- Runtime presentation 110 项、库版本 2 项、SE2 2 项、前端装配 5 项通过。新方法 23 次源文件分片读取进入 25 次受控 Provider 请求。
- 总览九单元在三宽度可用；原八单元首批证据保留在 docs/performance/presentation-method-library-v1。

## 下一步

1. 用户体验 http://127.0.0.1:4197/spatial-linked-views/v1/preview.html?topic=spring ，根据新主题需求改写模型与视图。
2. 后续 Agent 制作遵循 read_method → prepare → write(method_refs/libraries 明确) → 当前候选 preview → deliver。
3. 2026-10-16 16:02 HKT 已随 current-20261016 上线；Linux真实三视口WebGL/Author交付通过，见 docs/performance/current-release-20261016/README.md。

## 边界

- 当前证明共用核心能迁移两个主题；没有 Agent 自然选型、独立迁移收益或读者学习效果的结论。
- SE8 收益实验继续暂缓；本轮无付费模型调用。
- SE1–SE7 原证据、discrete-update@1 和候选清单保持历史身份。
- 原工作树其他专线改动不属于本次；FP7 暂停。

## 冷启动读序

1. skills/presentation/methods/spatial-linked-views/v1/card.md、example/README.md。
2. docs/performance/presentation-spatial-template-v1/README.md、acceptance.json。
3. skills/presentation/methods/README.md、catalog.json、SOURCES.md。
4. crates/runtime/src/presentation_methods.rs、agent_prompt/presentation.rs；crates/server/src/presentation_libraries.rs。
5. docs/adr/0168-executable-explanation-methods-and-evidence-based-evolution.md §8；原首批模板记录。
