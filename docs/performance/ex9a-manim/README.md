# EX9a 学习率连续讲解

模型辅助的定向 Manim 样片，使用 EX0 的一维模型与 EX1 的三种学习率。打开 `index.html` 的本地 HTTP 地址可播放视频、跳到关键章节，并在下方使用 EX1 交互版对照。

## 文件

- `learning-rate-final.mp4`：正式可播放视频（生成后交付）。
- `index.html`：带章节定位和 EX1 对照的播放器。
- `model.py`：真实递推、数字格式和唯一线性坐标变换。
- `scene.py:LearningRate`：连续运动、真实端点、完整更新箭头、跨点停顿、公式同步强调。
- `test_scene.py`：独立有理数闭式解与实际 Manim 对象坐标核对。
- `scene-evidence.json`：最近一次成功渲染的时刻、真实端点、箭头和距离线坐标。
- `runs/`：每次渲染的日志、成本和独立场景记录，失败也保留。
- `media/`：可重新生成的 Manim 中间媒体。

## 最小运行方式（Windows / PowerShell）

从仓库根目录执行，Python 3.11、FFmpeg、LaTeX/dvisvgm 已在本机可用，中文字体为 Microsoft YaHei。Manim 安装依据[官方本地安装说明](https://docs.manim.community/en/stable/installation/uv.html)。本项目本轮使用 venv，避免改变系统 Python。

```powershell
py -3.11 -m venv tmp/ex9a-manim-venv
New-Item -ItemType Directory -Force tmp/ex9a-os-temp | Out-Null
$env:TEMP = (Resolve-Path tmp/ex9a-os-temp).Path
$env:TMP = $env:TEMP
& tmp/ex9a-manim-venv/Scripts/python.exe -m pip install --no-cache-dir manim==0.21.0
& docs/performance/ex9a-manim/render.ps1 -Quality draft
& docs/performance/ex9a-manim/render.ps1 -Quality final
Push-Location docs/performance/ex9a-manim
& ../../../tmp/ex9a-manim-venv/Scripts/python.exe -m unittest test_scene -v
& ../../../tmp/ex9a-manim-venv/Scripts/python.exe verify_media.py learning-rate-final.mp4
Pop-Location
py -3.11 -m http.server 8769 --bind 127.0.0.1 --directory docs/performance
```

浏览器打开 <http://127.0.0.1:8769/ex9a-manim/index.html>。`render.ps1 -Python <其他虚拟环境的绝对路径>` 可使用另一已安装环境；完整实际依赖版本见 `requirements-lock.txt`。

## 数学与画面约定

`L(w)=(w−2)²`、`w₀=0`、`e=w−2`、`Δw=−2ηe`、步长 `2η|e|`。轨迹只计算 k=0…5；显示域固定为 w∈[−2.5,7.5]，三条轨迹共用线性尺度，覆盖全部真实端点。

实心点在两个真实端点间做线性插值；跨点暂停是讲解时刻，算法不在暂停位置重新计算梯度。箭头完整显示当次真实更新。过渡期间数值行以 `k→k+1` 显示起点、目标和位移；到达后切换为实际 w/e/L。轴下线段在运动时保留起点距离，到达后切换为新的端点距离；点的半径不编码距离。五次更新均在红、橙跨点处停顿，首步停顿更长。

## 已知限制

视频是固定参数、字幕讲解的媒体，没有音轨，不能在片内改 η 或恢复 Reader 语义现场。任意参数探索仍由 EX1 浏览器版承担。该样片不改产品渲染路径；自然请求成功率、真实读者学习效果和用户偏好另行评价。
