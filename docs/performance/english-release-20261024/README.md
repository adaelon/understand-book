# 英文适配上线与真实模型验收（2026-10-24）

2026-10-24 **19:07:10 HKT** 已上线 `english-20261024`，上一版为 `followup-understanding-20261024-r2`。服务 PID **34889**，schema **20**，维护窗口 **12.85 秒**。正式英文入口：[网站首页](https://understandbook.top/home.html?lang=en)、[阅读器](https://understandbook.top/index.html?lang=en)。

## 发布范围与数据

以实际线上源码为基线，合入 151 个英文适配生产文件及专用测试文件。网站、云端阅读器、账户页面、服务内置登录页支持中英切换；每轮请求固定 `response_language`，主模型采样、最终回答及失败补答继承该语言。选中文字翻译支持英文目标。原书标题、原文、引用和用户输入保留原语言。

管理后台保留原有中文版本。今天已上线的教学、历史恢复、输出预算等功能保留；本地其他 Core、Admin 修改没有进入此发布包。此次发布网站及 Linux 云端服务，Windows 下载包仍是 0.3.7；桌面本地界面及原生菜单的英文源码改动尚未随新的 Windows 安装包发布。

数据根继续使用 `/opt/understand-book/data/multi-reader-to-tb-20261012`。切换前确认没有活跃回答、未保存回合或进行中的后台任务，再进入维护、停服及备份。切换前后 reader 的 15 项、admin 的 16 项授权资料版本、正文读取与展示制作能力一致，无数据库迁移。

备份位于 `/opt/understand-book/backups/english-20261024`，包含原服务、nginx、环境与沙箱配置，以及 **15,911,387 字节**可变数据归档。原 release 和静态资源保留。上线后已发生测试写入，回退应保留当前数据，仅恢复相容的程序和配置，不能用上线前快照覆盖新写入。

[发布回执](deployment.json) · [源码文件清单](source-files.json) · [最终服务状态](service-final.json)

## 真实模型结果

通过正式 HTTPS 域名、CDN 和实际已发布的 `reader-j7WjdNuW.js`，在 Edge 中使用独立测试账号打开真实中文 AI Infra 资料，三轮均从页面输入框提交，均断言请求中的 `response_language` 为 `en`。未在问题中额外要求使用英文，未替换模型或模拟接口。实际模型为 **`deepseek-v4-flash`**。

| 场景 | 输入 | 实际输出 |
| --- | --- | --- |
| 英文提问 | Briefly explain what prefill and decode do in autoregressive generation. | 英文解释 prefill、decode 和 KV cache，完整保存 |
| 中文提问与教学 | 请用一个很短的例子说明这两个阶段的区别，再给我一道简短的理解检查题。 | 英文例子及英文理解检查题，完整保存 |
| 中文理解反馈 | 我的理解是 prefill 处理整个输入并生成第一个 token，decode 利用之前的 token 逐步生成后面的 token，直到生成结束，对吗？ | 英文确认与修正，并用英文说明原文没有明确给出的停止条件，完整保存 |

三轮均为 `completed`、`incomplete=false`。生成的续学建议也显示为英文。第三轮通过普通对话输入提交，没有覆盖独立教学作答控件的提交路径。

第二轮实际回答开头：

> A short example. Take one request whose prompt is 2,048 tokens and whose answer is 4 tokens.

第三轮实际回答开头：

> Yes — both halves of your description are right.

完整回答：[第一轮](live-turn-1-answer.md) · [第二轮](live-turn-2-answer.md) · [第三轮](live-turn-3-answer.md)。

## 页面与构建验证

- 发布包 Web 类型检查及多用户生产构建通过；精确发布源码的英文词条、语言请求和账户组件 5 项测试通过。之前本地的前端、Rust 及浏览器测试仍记录于 `tmp/english-adaptation`。
- Linux release 服务与辅助程序编译通过。编译期间观察到内存压力，调整独立构建进程限制后完成；临时 swap 已移除，原 swappiness 已恢复。旧 LibreOffice 下载压缩包已清理以腾出构建空间，安装后的运行时和解压 RPM 保留。
- 独立候选服务预检通过：登录、资料与正文、现场创建/重试、目标恢复、历史演示首次读取，无模型调用。
- 正式网站 1440px 桌面、390px 手机首页及阅读器截图已目视检查，手机无横向溢出，页面脚本错误 0。
- 手机端经“学习与会话 → 阅读设置”打开语言菜单，切回中文并刷新成功。初轮脚本没有先打开折叠菜单，导致选择控件超时；补充验收使用实际菜单路径通过，没有新增模型请求，也没有为此修改产品代码。
- 公网浏览器使用 SSH CONNECT 转发访问正式域名，启用正常 TLS 校验，资源来自已发布站点；这不是用户本机直接联网的延迟测量。

[浏览器结果](live-browser.json) · [手机首页](live-home-mobile-en.png) · [英文回答](live-turn-2-en.png) · [手机阅读器](live-reader-mobile-en.png) · [手机语言菜单](live-mobile-language-menu.png) · [候选预检](preflight.json) · [Web 构建](web-build.log) · [词条及请求测试](web-i18n-tests.log)

## 费用、收尾与已知问题

共记录 **35 次模型调用尝试**，其中 34 条已结算，已确认费用 **¥0.778047**；另 1 条为 `usage_missing`，保留 **¥0.571564** 预留额待核对，不能视为最终实际费用或零费用。没有伪造付款记录，测试账号已停用，没有仍在发送中的测试调用。

后台教学分析的四个任务中，两项 ready、两项 failed：起始分析的结构化输出在一次纠正后仍未满足学习目标／来源绑定要求；第二轮后的分析被后续对话取消，对应上述缺少用量的记录。这些发生在未改动的后台分析路径，三轮用户可见英文回答均已完整保存；本次不将后台分析宣称为全部成功。未伪造分析结果或手动把未知费用记为零。

[调用与费用记录](qa-accounting.json) · [后台分析与费用诊断](qa-cost-diagnostic.json) · [测试账号收尾](qa-cleanup.json)
