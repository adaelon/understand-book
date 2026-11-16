# SESSION_CHECKPOINT — 2026-11-12 完成

## 本轮结果
已完成自动课程构建与通用探究活动的本地实现，并更新本机 Codex 维护插件及 Engine。用户澄清本次更新范围为本机插件，线上候选编译已停止，正式线上未切换。工作区原有其他未提交变更保留，本轮没有提交 Git。

## 主要链路
- 管理端“构建课程”与 course_goal → DSH → BuildPlan → BookStructure → scope / plan / lesson / solve / review。
- 冻结任务、预算计量、执行器恢复、独立解题、逐课来源核验及定点修复复用原 Engine。
- inquiry_course.json / course_build_report.json → 导入核验来源和报告 → 正式 publication 绑定 → 共享发布自动带课程。
- CourseExperiment::Inquiry 支持阅读取证、论证、练习；开放回答、逐轮历史、暂停恢复与 Coach 反馈。
- Admin 可在发布前查看已构建课数、问题、目标、预计用时；旧资源实验及冻结教学计划可用。

## 完成证据
- docs/evidence/course-build-20261112/README.md 为权威验收记录，含桌面／390 px 手机截图和验证限制。
- Memory课程42；Server运营含新课程导入3，原课程API3；Web播放器18+资源实验4+批次尺度1；Admin13通过。
- Core课程4、能力6、原编排23、Driver86、自定义goal真实executor1、来源路由3通过。
- 打包Engine + DSH接口测试夹具 + 真实stdio + 受控候选完整课程DONE，13个执行器会话，约75.7秒；不能视为当前DSH Desktop 0.2.1的安装态可用。
- 旧DSH启用Pass2场景source CLI达到原240秒限时；同一场景打包Engine约76.7秒完成通过，其余运营宿主／工具合同回归通过。
- Core/Web/DSH类型检查，Web/Admin生产构建，DSH插件和Engine打包通过。
- 没有新付费模型调用；新教材真实内容及学习效果须另做验收，不能将受控候选视为真实试学。

## 文件与运行
- 关键实现：packages/core/src/course-build.ts、agents/course-builder.md、crates/memory/src/course.rs、crates/server/src/operator_builds.rs、packages/web/src/components/CourseInquiryActivity.vue。
- 记录已更新：切片方案、ADR-0182、代码链路、课程方法README、DSH README、Build技能CLI说明。
- 本轮日志移入 docs/performance/course-build-20261112/（原有忽略规则覆盖）。
- 临时组件预览源码已删除；4276／4277两个本轮Vite进程已停止。
- E盘空间不足时使用 D:/codex-course-build-validation，CARGO_INCREMENTAL=0，profile.dev.debug=0 完成Rust验证。
- 已编译验证Engine：D:/codex-course-build-validation/course-build-engine.exe。
- pnpm.cmd测试不要传多余 --：精确Vitest命令用 node ../../node_modules/vitest/vitest.mjs run test/file。

## 后续
- Codex 插件 `0.1.0+codex.20261112051710` 已 installed/enabled；安装启动器和 12 文件直比通过。项目配置及插件 MCP 指向 `E:/allwork/Understand Book/operator-builds/20261028-prebuild-reuse/understand-book-build-course-20261112.exe`，原进程保留，重启 Codex 后加载。
- 当前 DSH Desktop 为 0.2.1-rc.2，spawn 缺少旧适配器要求的 toolFilter/persona，准备返回 build_harness_unsupported。DSH profile 已恢复原 rc.2 与原启动配置，rc.3 候选包保留；尚未适配当前宿主。
- 线上仍为 delivery-repair-20261111；course-build-20261112 候选编译已停止，未部署、未迁移。不得继续上线，除非用户另行要求。
- 更新操作与回执：tmp/course-deployment-20261112/；权威记录：docs/evidence/course-build-20261112/README.md。新教材内容核对与试学仍按 IC6。
