# 音乐 M0 样本与对齐实验

日期：2026-10-22。状态：舒伯特开放素材、贝九完整乐章的本机试听素材与选段总谱已取得；两份真实录音均已生成依赖音高内容的对齐候选。尚未完成人工听核、逐音符精度验收或内容商业发布。本目录不能被视为完整首批内容发布包。

## 已取得的素材

| 素材 | 实际文件与大小 | 许可及来源 |
| --- | --- | --- |
| 舒伯特 D.780/3，Harald Vetter 钢琴演奏 | [原 MP3](assets/schubert-d780-3-vetter.mp3)，2,425,753 字节，媒体封装时长 101.041633 秒 | [IMSLP 官方作品页](https://imslp.org/wiki/Moments_musicaux,_D.780_(Schubert,_Franz))中 #456646，Harald Vetter 录音组标为 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) |
| Noctave 结构化谱本 | [原 MXL](assets/schubert-d780-3-noctave.mxl)，5,921 字节；[解压 MusicXML](assets/schubert-d780-3-noctave.musicxml) | [出版者页面](https://noctavepiano.com/partitions/schubert-moment-musical-d780-3/)明确 MusicXML 和 PDF 为 [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)；源为 Claude Chaudron 的 Mutopia 公版排谱 |

录音归属建议显示：**Franz Schubert, Moments musicaux D.780 No.3；钢琴：Harald Vetter；来源：IMSLP #456646；CC BY 4.0。** 本目录原 MP3 未修改；未来裁剪、转码或做教学编辑时，应注明相应变化。素材许可与程序许可分别管理。

用户给出的参考 MP4 没有被提取或复制为发布录音。Noctave 网页的声音是合成预览，本实验也没有将其当作真人录音。

具体来源 URL、文件大小、原谱版权字段和完整 FFprobe 输出见 [samples.json](samples.json)。

## 贝九本机样本

[beethoven-sources.json](beethoven-sources.json)记录完整素材、时间轴、许可依据和改动；[beethoven-curation.json](beethoven-curation.json)供现有音乐构建器读取。

- 总谱来自 [OpenScore Orchestra 官方目录](https://fourscoreandmore.org/openscore/orchestra/)链接的 [Hauptstimme 项目](https://github.com/MarkGotham/Hauptstimme)。第四乐章原 MXL 1,200,879 字节，解压 XML 26,523,258 字节，36 个 part、943 个 measure。谱面 CC0；原文件中的分析标注单列为 CC-By-SA，来源未指定版本。
- [本次选段](assets/beethoven-op125-4-m93-164.musicxml)保留原第 93–164 小节和 6 个在选段内发声的声部：第一巴松、第一／第二小提琴、中提琴、大提琴、倍大提琴。其余全程休止的声部从选段视图省略；完整原谱保留。选段 515,952 字节、1,249 个谱对象（含休止），其中 1,084 个有音高；没有无音高打击乐音符。
- 原声是 Albert Coates 指挥的 1923 年演奏、1924 年首次发行的 HMV 录音，由 [Damian J Rogan 于 2024 年转录和修复](https://music.damians78s.co.uk/2024/05/07/albert-coates-conducts-beethovens-9th-1923/)。出版者的两段原 FLAC 分别为 27,176,474／40,249,403 字节，590.213333／858.604263 秒。保留两原件，并按顺序拼成 [完整乐章 MP3](assets/beethoven-op125-4-coates-damian-full.mp3)，23,182,550 字节、封装时长 1,448.85551 秒；没有截去选段之外的音乐。
- [真实音频子序列 DTW](results/beethoven-chroma-dtw/candidates.json)把 288 个四分拍的选段匹配到原录音约 171.572–289.469 秒。第 117／141 小节候选为 209.355／249.787 秒；分析步长约 49.98 ms、运行 2.597 秒。这些只是定位候选，全部 `pending`，不是音符精度验收。
- 三个事件的事实依据是实际总谱及已署名的 Hauptstimme 分析：先跟低弦，再关注中提琴，最后关注第一小提琴。完整乐章其余部分标为 `navigation_only`；未建完整贝九的深度内容。

本样本的录音和带分析标注谱本均标为 **`local_only`**。原 1924 录音按[美国版权局所列 95+5 年规则](https://www.copyright.gov/music-modernization/pre1972-soundrecordings/)已超过该保护期，但这份现代转录明确包含补接音符、修改长号入口等修复，没有明确的开放再发布许可；它未被当作全球可商用的发布录音。谱面本身的 CC0 与保留分析标注的许可分别记录。先完成本机工程验证，发行素材许可仍是未完成内容工作。

Hauptstimme 附带的 `*_alignment.csv` 对应 IMSLP 729211／484065／736780，**不是本次 Coates 演奏**；没有把这些秒数套用给当前录音。该三份历史演奏在 IMSLP 当前均标明 Non-PD US。BSED/BSD 的[官方发布记录](https://zenodo.org/records/20344500)为 CC-BY-NC-SA-4.0，未作为商用数据下载。Commons 上 22:30 的贝九录音经追源实际是 Papalin 竖笛与人声改编，也未冒充交响乐。

## 实际谱面数据形状

该文件为 MusicXML 4.0 `score-partwise`，含一个 part `P1`、78 小节、两个 staff、voice `1/2/3/5`、1,066 个 note 元素，其中 1,059 个有音高、47 个为 grace。全谱共 156 个四分音符单位，没有 repeat barline；重复内容已经在谱面中分别书写，不能假设该文件需要按反复标记再复制一遍。

前两小节上谱表为整小节休止，下谱表有伴奏；第 3 小节上谱表进入。逐项比较原 XML 的音高、声部、谱表、相对位置、时值与装饰标记后，第 3、11、51 小节的音符结构完全相同。这是谱面事实，尚未据此接受录音中的对应时刻或听觉角色判断。

原谱存在需校对的元数据：调号写四个降号，但 `mode=major`，而作品和出版者标题均为 F minor。保留原文件，不把这一字段当作已经确认的音乐调性。原谱无 note ID；实验使用 `partId-m{从1起的小节序号}-n{该小节从1起的note序号}`，note 序号计入休止符，另存原小节标签。

## 已运行的真实音频实验

[alignment_probe.py](../../scripts/music/alignment_probe.py)用 FFmpeg 解码真人 MP3，计算实际录音的 STFT 色度与低音特征，与 MusicXML 的音高／时值特征进行带窗口 DTW，再在预测点附近寻找声谱起音峰。它没有根据曲长匀分音符时间。

运行环境为 Python 3.14.4、当前安装的 NumPy／SciPy 和本机 FFmpeg。实验不使用音乐理解大模型、不调用付费服务。全曲本次耗时 10.862 秒，音频帧步长 20 ms，谱面步长为 1/32 四分音符；实际音频有效分析区间为约 0.58–98.08 秒。

产物：

- [候选对齐](results/schubert-chroma-dtw/candidates.json)：包含全部有音高谱对象、源 ID、谱位、候选音频时刻与方法；整体状态为 `experimental_candidates_not_accepted`。
- [小节候选表](results/schubert-chroma-dtw/measure-candidates.tsv)：可供校准界面先定位；每行仍是未复核候选。
- [负对照结果](results/schubert-chroma-dtw/negative-control.json)：错谱色度整体移高六半音，平均匹配代价由原谱的 0.4576 增至 0.5677；支持算法确实使用音高内容，不能据此推算毫秒精度。

非装饰音产生 380 个不同谱面起音位置；此次局部起音细化未产生先后倒置。47 个装饰音保持 `onset_seconds=null`；声谱局部峰也可能来自同时出现的另一声部。没有独立人工起音真值，因此没有“误差小于某个阈值”的结论，所有候选在接纳前需要复核。

随后运行 [pitch_onset_probe.py](../../scripts/music/pitch_onset_probe.py)，用约 92.9 ms 的短窗和约 4.99 ms 帧步长拟合实际波形中的谐波音高能量。v1 误把同音和弦当成可独立区分的主音；v2 改用和弦中独有的音高作为见证，并限制多装饰音的先后顺序。47 个装饰音得到 44 个非零间隔候选、3 个零间隔缺口，均未接纳。

四张实际频谱曲线图与 [逐项视觉观察](results/schubert-pitch-onsets-v2/visual-review.json)保留在 `results/schubert-pitch-onsets-v2/`。其中 m3／11／51 的对应短音显示先后能量起伏；m4／12／52 的多装饰音可借和弦另一音高改善定位；m38／46 的粗锚点偏移导致选中了更晚的波动，m65／69／73 仍不能分开两次起音。m6／14／54 等还存在前音衰减与装饰音归因歧义。观察是对图中真实录音能量的检查，不能代替独立人工听核。

误差口径也写在视觉观察 JSON：按录音版本、出现实例和谱对象 ID 对齐独立标签，分别报告有标签数、缺口、median／p95／max 绝对误差及阈值内比例；装饰音、主音、和弦分开统计。**当前没有独立标签，准确率结果为未测量**，不能拿算法输出作自己的真值。

## 三个首轮校准／伴听案例

| 案例 | 可核对的谱面对象 | 原声候选位置 | 下一步要确认的内容 |
| --- | --- | --- | --- |
| 节奏与旋律进入 | m1–m4，上／下谱表 | m1 约 0.58 秒，m3 约 3.04 秒，m4 约 4.18 秒 | 录音是否保留两小节伴奏引入；旋律进入的实际起音及装饰音位置 |
| 重复材料 | m3 与 m11 的同形谱面，必要时扩展前后乐句 | 约 3.04 秒与 12.52 秒 | 两次原声能否支撑“相同材料回来”的听辨提示；起音和片段边界分别校准 |
| 声部关系 | m3，staff1/voice1 与 staff2/voice5 | 约 3.04–4.18 秒 | 上方线条和下方伴奏在该录音中的可闻性；是否需要谱面合成示范，不能用谱上参与代替听核 |

以上秒数仅用于打开候选附近，未作为最终教学或动画证据。完成听核后应记录实际校准值、所用谱对象、处理人／方式和依据，再经现有候选接纳流程进入构建成果。

## 重跑

从项目根目录执行：

```powershell
python scripts/music/prepare_samples.py
python scripts/music/alignment_probe.py
python scripts/music/probe_controls.py
python scripts/music/pitch_onset_probe.py
python scripts/music/prepare_curation.py
node --import tsx scripts/music/prepare-alignment.mjs evals/music/schubert-curation.json evals/music/results/schubert-chroma-dtw/candidates.json evals/music/results/schubert-alignment.pending.json
python scripts/music/prepare_beethoven.py
python scripts/music/beethoven_alignment_probe.py
python scripts/music/prepare_beethoven_curation.py
```

素材准备复用已存在原文件并重建元数据；对齐项重算实际波形候选；负对照测试错误音高输入是否仍得到同样匹配，并检测细化后先后次序。若错谱与原谱无法区分，应调整特征或停止采用这条方法；若候选与实际起音不符，进入人工锚点及局部校准。`prepare_beethoven_curation.py` 使用本次 Coates 音频候选，不读取其他演奏的时间表。

## 未完成与实际限制

本 M0 实验没有独立人工音乐听核与起音标签，不把 DTW 代价当成准确率。声谱窗口产生的低频时间模糊、装饰音、同时声部及实际止音均未解决，当前只证明可生成依赖原声的候选。产品界面另有[运行验证记录](results/ui-verification.md)；使用可控合成音验证连续动画，不代表这两份真人录音已达到同样的逐音精度。

当前实验按同一时序的谱面路线匹配；对不同反复、删节或插入，必须先确认演奏路径，不能靠扩大 DTW 窗口掩盖。谱面全曲序列与这份录音是否完全一致仍需逐段核对。贝九不得直接沿用钢琴效果作能力结论。
