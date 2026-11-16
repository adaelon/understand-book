# 最优化期末练习试用

第 16 讲「对偶理论」的参考书样章、课件例题、课堂习题、作业题及独立解答见 [对偶理论样章](duality-chapter/README.md)。

三道原创题使用《最优化：建模、算法与理论》已构建的本地正文。两道选择题取材于 §5.4.1「拉格朗日函数与对偶问题」，计算题取材于 §1.5.6「全局和局部最优解」。每题的公开题面、公开选项、私有评分标准和教材位置放在 [questions.json](questions.json)。这个文件供出题者使用，包含答案。

## 在阅读器里试

从「练一练」选单选、多选或计算题，也可以填写具体想练的考点。下面的样例会话已准备好三道题，每题都可以独立作答。

1. 单选题先选 A，提交后改为 B。比较两次提交，看看选中选项与正式交卷的区别。
2. 多选题先在「我的思路」写下“两个 8 分别能说明什么？”，点「问这一步」。再先交 A，之后补选 B，点「提交改答」。
3. 计算题把“我令导数为零得到 x=1，所以值是 0 吗？”写进过程，选中这行求助。也可以试“我算出 3 和 4，哪个是最小值？”。刷新后接着写。先只交「4」，再用不等式或导数补全理由。

## 实际走查

桌面浏览器中，单选从答错改到正确；多选从漏选改到正确。计算题仅报数值得到部分正确，提交不等式证明、再换成导数证明均获正确。对选中一行的求助只发送那一行，未发送的草稿在刷新后恢复。

在 390 × 844 的手机视口也完成了题内求助、刷新、首次提交和改答。以下记录来自实际阅读器、题目服务和已配置的模型。

- [桌面作答、助教原话和分项反馈](verification/walkthrough.json) · [桌面题内讨论](verification/desktop.png)
- [手机作答和分项反馈](verification/mobile.json) · [手机题内讨论](verification/mobile.png)

## 重新准备一份试用会话

语音配置见本文末尾；未配置时其他答题功能照常使用。

在仓库根目录运行下面的 PowerShell。先按项目原有方式配置模型。脚本读取已构建的最优化教材，并在临时目录下创建一份独立的学习记录。每次运行请给试用目录取一个新名字。

```powershell
$env:UB_QUESTION_BOOK = (Resolve-Path '.understand-book/optimization-modeling-algorithms-theory').Path
$env:UB_QUESTION_DEMO_DIR = Join-Path ([System.IO.Path]::GetTempPath()) ('ub-optimization-' + [guid]::NewGuid())
cargo test -p server --lib seed_optimization_question_walkthrough -- --ignored
pnpm --filter @understand-book/web build
$env:UNDERSTAND_BOOK_MEMORY_DIR = $env:UB_QUESTION_DEMO_DIR
$env:UNDERSTAND_BOOK_ADDR = '127.0.0.1:45941'
cargo run -p server --bin server -- "$env:UB_QUESTION_BOOK"
```

浏览器打开命令行显示的地址即可作答。要复跑自动走查，可在另一个终端对一份全新的试用会话运行下面的命令。手机走查同样使用一份新的试用会话，并在输出目录之后加上 `mobile`。

```powershell
node examples/optimization-practice/walkthrough.cjs http://127.0.0.1:45941 "$env:TEMP/ub-optimization-results"
```

## 语音输入

题目的思路框、计算题过程和聊天框下方均有「语音输入」。开始录音后点「停止并转文字」，检查同音字和数学表述，再点「加入输入框」。文字补到现有草稿末尾；学生继续选择「问这一步」、发送或正式交卷。每段最多 90 秒，取消会关闭麦克风；转写失败可以重试同一段录音。

服务使用 **faster-whisper 的多语言 small 模型**，支持中文和英文。先在自己的 Python 环境安装 `faster-whisper==1.2.1`，通过 `huggingface_hub.snapshot_download('Systran/faster-whisper-small', local_dir=模型目录)` 下载模型，再设置服务器环境变量：

```dotenv
UNDERSTAND_BOOK_VOICE_PYTHON=安装了faster-whisper的Python完整路径
UNDERSTAND_BOOK_VOICE_MODEL_DIR=多语言模型目录的完整路径
```

配置样例也在仓库 `.env.example`。修改后重启阅读服务。运行期间不再下载模型，音频经阅读服务在本机或部署服务器上转写，请求结束删除临时音频。使用浏览器的麦克风需要 HTTPS 或本机地址，并由用户授予权限。

自动走查用 Windows「Huihui」合成句“我认为最小值是四，但是不知道在哪个点取得。”作为浏览器的音频输入；真实执行 `MediaRecorder → HTTP → 本地识别 → 编辑确认 → 原草稿 → 题内辅导`。可用自己的普通话 WAV 复跑：

```powershell
node examples/optimization-practice/voice-walkthrough.cjs http://127.0.0.1:45941 examples/optimization-practice/verification C:\path\student.wav
```

识别结果是普通文字，变量、符号和同音字仍需要学生检查；这轮验证使用合成音频，未验证用户的物理麦克风及房间噪声。

本机实测三次约 5.9 秒的录音，停止后 5.4～5.8 秒返回文字。识别中的“区得”由学生在预览改成“取得”；加入后，录音前和录音时手打的文字均保留，刷新也能恢复草稿。点击「问这一步」后，助教回复“在约束 x≥3 下，请先解方程 (x−1)²=4，并检查得到的哪个点满足 x≥3。”，未产生正式作答。完整教材首次打开或刷新时，题卡需等教材加载完成；本机走查约 27 秒恢复全部题卡。

- [识别用时与助教实际回复](verification/voice.json)
- [桌面识别预览](verification/voice-desktop.png) · [手机识别预览](verification/voice-mobile.png) · [语音后继续求助](verification/voice-guidance.png) · [聊天框追加文字](verification/voice-chat.png)
- `VoiceInput.test.ts` 覆盖取消权限等待、切题取消请求、隐藏输入区关闭录音、迟到结果、重试、静音、90 秒停止、多个输入框共用麦克风、原稿追加与恢复。相关前端共 173 项检查通过，服务端 2 项语音检查通过；服务端额外验证多人现场权限和超过普通文字请求大小的录音请求。
- 最终试用页还验证了录音时从「问答」切到「画像」：浏览器音频轨道立即结束，没有发起转写请求；切回后可重新录音，三道题仍然保留。
