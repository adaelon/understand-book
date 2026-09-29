# 局部动画运行环境

`presentation.author.render_animation` 使用独立 Python 环境。配置 `UNDERSTAND_BOOK_ANIMATION_PYTHON` 为该环境 Python 的绝对路径；请求期间不会安装依赖。当前固定 Manim Community 0.21.0、Cairo、30fps、无音频 H.264 MP4。

Windows 开发环境可复用 `tmp/ex9a-manim-venv/Scripts/python.exe`。新环境按 Manim 官方安装说明准备 Python 3.11+，安装 `scripts/presentation-animation-requirements.txt`。Manim 的 PyAV 编码/解码库和 Pillow 随 Python 依赖安装；确认 PyAV 所带 FFmpeg 有 H.264 编码器。使用 `Text` 时安装场景所需字体（中文使用 Microsoft YaHei 或 Noto Sans CJK SC）；使用 `Tex/MathTex` 时还需 LaTeX 与 dvisvgm，并加入进程 PATH。

场景定义 `class PresentationAnimation(Scene)`，Manim 符号和 JSON `data` 已提供。宿主固定执行入口及画幅，实际成品需能解码，定位点不能超过实际时长。默认画幅 1280×720，可选偶数尺寸 320–1600 × 240–1200。单次 180 秒超时，取消终止进程树。运行代码沿用本机受信任协作范围，临时目录用于隔离输出。

每段 MP4 上限 8 MiB；每版本全部动画及海报的解码字节合计上限 24 MiB；文本文件单独计入既有 1 MiB 限制。媒体和源代码随不可变版本保存，升级本机依赖不会重渲染旧版本。

验证入口：设置上述环境变量，运行 `cargo test -p server --lib ex11_animation -- --ignored --test-threads=1 --nocapture`，覆盖真实渲染、错误诊断、取消及版本复用。浏览器播放与异步恢复另外运行 EX11.4 Reader 验收。
