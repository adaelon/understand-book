# 最终示范提示可读性验证

2026-10-22。资料使用实际发布版本 `01a1625b-808a-77d2-9cfb-1ecc62bd219f`；没有修改产品源码、生产 dist 或旧验收证据。

实际显示的 `two-staves` 提示为：

> 谱面示范按谱另行合成，并非原录音分轨；谱上上下两层原本同时进行。这一遍只跟上方旋律的走向。

通过独立 Vite 页面与 Chromium 播放该发布版本的真实 `upper-line-m3-4.wav`。命令传输使用浏览器测试适配器，不调用模型或服务端。

- 5 个实际命令完成，浏览器错误 0 个；示范回执为播放中，原录音保持暂停在 4,200.125 ms。
- 实际调用音频 `play()` 时完整提示与来源标签已挂载可见，提示经过动画帧比播放调用早 38.7 ms。
- 1440×1000 桌面：提示区域 994×82.77 px，播放控件 994×34 px；全部位于视口内。
- 390×844 手机：提示区域 340×102.56 px，播放控件 340×34 px；完整文字换行显示，未裁切，面板与控件全部位于视口内。
- 两张实际截图均已查看：提示说明合成与原录音的关系，来源为“作品导览 · 换一条线听”，没有被控件或其他文字遮挡。

证据：[操作与几何记录](final-cue-music-playback-cue-ui.json)、[手机截图](final-cue-music-demo-cue-mobile.png)、[桌面截图](final-cue-music-demo-cue-desktop.png)。

复跑命令：

```text
node evals/music/playback-cue-ui.mjs --cue-only --publication-dir .understand-book/music-acceptance-service/library/published/music-schubert-d780-3-vetter/01a1625b-808a-77d2-9cfb-1ecc62bd219f --output-prefix final-cue --demo-cue two-staves
```
