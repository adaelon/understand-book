# 视频 main 集成与 Windows Debug 构建

## 目标与基线

将主工作区最新代码合入视频 worktree，保留视频功能并生成可在本机运行的 Windows Debug 桌面程序。

- 主工作区保存提交：`402d9be`，已推送 `origin/main`。
- 视频功能保存提交：`35e199f`，已推送 `origin/codex/video-understanding`。
- 集成位置：`E:/allwork/download/agent/understand-book-video-understanding`。
- main 的历史缓存、私有执行记录和导出文件保留本地，不作为源码快照提交。

## 执行步骤

1. [x] 保存并推送两侧源码基线。
2. [x] 合并 `origin/main`，解决 Reader、Runtime、Server 接入冲突；保留双方能力。
3. [x] 执行类型检查、视频与冲突接入点相关测试，修正合并造成的问题。
4. [x] 构建 Windows Debug 桌面程序，验证启动与视频材料加载，记录产物位置。

## 验证目的

- 类型检查和编译：发现合并后的字段、函数签名、资源引用不匹配；失败时修正对应接入。
- 相关单元测试：发现视频上下文、来源引用、Reader 状态或 Tutor 交互回退；失败时按案例恢复双方行为。
- 桌面启动与视频材料读取：发现缺少运行资源、启动错误或视频入口不可用；失败时修复构建或打包入口。

## 当前状态

21 个冲突文件已整合；Windows Debug 构建与真实 WebView2 桌面验证通过。集成提交已在本地保存；随后推送 GitHub 遇到 443 超时/SSL reset，重试原地址、规范仓库地址和 HTTP/1.1 均未成功，待网络恢复后推送视频分支。main 基线与视频原成果的先前推送已完成。

## 集成修正

- 保留 main 的 Focus Reader、结构化笔记、Tutor continuation 与视频分支的播放、草稿、来源和发布切换能力。
- App 的聊天作用域按 Reader 身份和 workspace generation 区分，视频 publication 信息补全不会清空聊天；API 的出版物鉴权身份仍按原逻辑维护。
- 结构化笔记同时保留可选文本范围和视频媒体来源快照，支持纯画面回答保存后恢复原始来源。
- main 的文本 Tutor 交互接入可选文本范围；纯媒体来源不能伪装成文本位置，现有的文本快捷操作返回不可用，视频提问仍使用普通输入框。
- 视频草稿测试将 MemoryStore 放入各自临时目录，避免公共临时目录下的 learning.db 互相污染。
- 按 main 的 `.gitattributes` 将 `agents/*.md` 实际工作区换行统一为 LF，使桌面 Build Engine 的来源契约冒烟检查通过。

## 已完成验证

- `pnpm install --frozen-lockfile`；Web TypeScript 检查与 Vite 构建通过。
- 前端选择集 166 项通过；Core 视频相关 93 项通过。
- Rust Runtime 接入选择集 52 项通过；Server 视频选择集 23 项通过，1 个独立浏览器驱动入口按原定义忽略。
- main 的 Tutor continuation、笔记路由选择集 65 项通过，8 项需要专用浏览器或真实模型的案例按原定义忽略。
- NoteSourceBinding TypeScript 类型由 Rust 导出测试重新生成。
- 桌面 Build Engine 与 Book MCP 辅助程序冒烟检查通过，视频资源已复制。
- `pnpm -C apps/desktop build --debug --no-bundle` 完整入口通过，产物为 unoptimized + debuginfo。
- 真实桌面 WebView2 验证通过：加载 1920×1080 视频、实际播放、跳转到 555.123 秒、拖动框选、HTTP Range 206 返回指定字节；页面脚本错误为 0。本次没有发送模型请求。
- `desktop-video-smoke.json`、`desktop-video-debug.png` 与各阶段日志保存在产物目录。
- FFmpeg 资源由构建脚本生成，已加入 `.gitignore`，无需提交复制的二进制。

## 产物位置

- 构建和验证记录：`D:/codex-build/understand-book-video-debug-20261015`。
- Debug 程序：`E:/allwork/download/agent/understand-book-video-understanding/target/debug/UnderstandBook.exe`。
- 本机试用入口：`D:/codex-build/understand-book-video-debug-20261015/Start-Video-Debug.cmd`。
- 试用脚本默认打开 VI18 已完成的 CS336 视频，并将设置、WebView 缓存、记忆和私有工作数据放到本次试用目录。


## 试用

双击 `Start-Video-Debug.cmd`。初次启动会沿用已有桌面程序的模型配置，之后设置保存在试用目录，独立修改即可。

可跳到 9:15，点击“框选画面”，圈住代码，再在右侧输入“这段代码为什么会出现 underflow？请结合当前画面解释”。也可使用“选择时间范围”询问一段视频。

## 已知限制

- 此次是本机 Debug 构建，依赖 worktree 的 Web 资源、辅助程序及已有视频产物，不是可单独搬走的安装包。
- 自动验证覆盖播放与提问上下文操作；本轮未重新调用真实模型，也未重复执行 VI18 的整片语义生产。
- main 的部分文本 Tutor 快捷操作需要文本来源；纯画面来源仍通过视频输入框提问。
- 当前视频样例的顶部材料名显示转写首句，目录栏没有章节项；视频、720 段中英转写及全片视觉索引可用。
