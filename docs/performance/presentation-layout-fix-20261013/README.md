# 演示工作区高度与全屏切换修复

已于 2026-10-13 21:46:44（Asia/Hong_Kong）发布到 [正式站点](https://understandbook.top/)。

## 页面行为

展开演示时，演示正文使用左侧完整高度。关联教学说明、作答区、后续建议与“这一步没跟上”入口移到右侧对应回合，手机在“展开讨论”中访问。同一组教学组件随布局移动，展开和收起保留作答草稿与演示参数，演示 iframe 不重新加载。

从会话全屏进入演示时，全屏布局样式暂停，右侧聊天按演示比例使用剩余宽度。收起演示后恢复原全屏会话与输入草稿。

## 根因与验证

原演示容器将 iframe 与不限高的教学说明纵向排列，长说明使 iframe 只能得到 160px。另一个问题是会话全屏的 `width: 100%` 覆盖了演示分栏宽度，使聊天伸到演示背后。

三个新增浏览器场景在修前分别暴露桌面高度、全屏宽度和手机高度失败，修后全部通过。桌面 1536 × 700 与手机 390 × 844 的正文均超过演示容器高度的 90%；长教学说明和展示请求失败提示不再占用主画布。验证包含真实控件输入、作答草稿、收起后再展开、主输入草稿、演示宽度滑块、手机讨论和 iframe 加载次数。

相关组件测试 69 项通过；`VITE_MULTI_USER=1 pnpm build --outDir dist-multi` 的类型检查与生产构建通过。数据见 [验证记录](validation.json)。

21:52:58 HKT 完成正式 HTTPS 首页、阅读入口及其直接引用 JS／CSS 的逐字节核对，均与本次构建产物一致，后端进程保持运行。见 [公网资源复核](public-verification.json)。

正式浏览器完成真实账号登录、打开 AI Infra 默认材料与问答全屏。该验收账号在此材料版本下没有保存的历史对话，未能继续执行已有演示的线上交互检查；三个演示布局场景的验收依据为上述本地真实组件回归。见 [线上浏览器范围记录](production-browser.json)。

- [从全屏进入演示](expanded-from-fullscreen.png)
- [长教学说明与作答](expanded-long-teaching.png)
- [手机讨论与作答](mobile-discussion.png)

## 发布与回退

前端基线为 `66129325f04d4ffcfa6a2e630f13f10b658099c6`，发布包含本次工作树中两个组件和一个回归文件的修改。发布前与现网逐文件比较了 487 个前端基线文件。新静态资源为 `reader-CfJ2PVJy.js` 与 `reader-BpRNSPKW.css`，其余 284 个资源复用；同版源文件同步到现有 release。

- Release：`/opt/understand-book/releases/tutor-response-fit-20261013`。
- Web 根：该 release 下的 `packages/web/dist-multi`。
- 恢复点：`/opt/understand-book/backups/presentation-layout-20261013T134637Z`，保留旧入口及前端源码。
- 后端服务持续 active，PID 8915；数据根、schema 与后端程序沿用当前版本。

旧静态资源保留。回退前端时恢复备份中的 `index.html`、`home.html` 与被修改的两个组件即可，不需要停止后端或回退用户数据。[部署回执](deployment.json)。

## 已知问题

本次处理布局问题。教学展示记录的独立网络保存失败未在本次修改范围内；回归用例主动模拟该请求失败，验证错误提示仍可见且不会挤压演示正文。
