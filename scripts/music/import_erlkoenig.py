"""Prepare the user's full Erlkoenig recording and PDF as a curated music source.

The original notation stays authoritative. Page navigation is a broad visual
reading of this exact score-sync video, not an accepted note/measure alignment.
"""
from pathlib import Path
import argparse
import json
import shutil
import subprocess
import xml.etree.ElementTree as ET

import fitz

SCORE_URL = "https://scorefl.com/score/bc5cf594-14d4-420f-9711-2f290814d05b"
PAGES = [
    (0, 28, 1, "开场与低音区的叙述", "第 1–18 小节；Presto agitato、dramatico、recitando。", "先跟持续的八度三连音，再注意低音区开始出现的、像说话一样分句的线条。"),
    (28, 54, 2, "音区与力度的对答", "第 19–34 小节；poco ff、cresc.、molto energico。", "留意短促的上行与较长的回答怎样轮流出现；重听时只选一条线，不必同时追所有音。"),
    (54, 79, 3, "伴奏之中的歌唱线", "第 35–49 小节；sotto voce ma marcato、sempre marcato il canto、ansioso。", "把连续运动当成背景，寻找音值更长、句尾更清楚的那条线；看看它是否一直位于最高处。"),
    (79, 110, 4, "tranquillo 与 misterioso", "第 50–69 小节；tranquillo、estinto、ppp misterioso、espress.。", "对比这一段与开头：快速音符仍在时，哪些力度、音区和织体变化会改变你的感受？"),
    (110, 136, 5, "催促与回落", "第 70–85 小节；precipitato、tremante、dim.，随后再见 tranquillo。", "先找向前冲的上行，再留意句尾怎样回落；比较前后两种语气，不必把每个强音都当作高潮。"),
    (136, 161, 6, "轻柔的邀请与再次催促", "第 86–100 小节；legg. amorosamente、un peu plus animé，随后 precipitato、tremante。", "先听下方流动的伴奏，再回到上方较疏的线条。第二遍只问：两层是否始终有相同的重音和句尾？"),
    (161, 187, 7, "熟悉的运动与更强的诉说", "第 101–117 小节；dim.、cresc.、riten.、molto appassionato。", "关注渐强和 riten. 前后的方向：声音的密度、音区与推进感，是否一起发生变化？"),
    (187, 210, 8, "加速标记与密集和弦", "第 118–131 小节；cresc. subito、precipitato、il più presto possibile、sempre tumultuoso。", "这一遍先抓住低音落点，再看高处的八度移动；把大段运动分成几个可辨认的方向。"),
    (210, None, 9, "冲刺、宣叙与终止", "第 132–148 小节；末段出现 Recit. 与 Andante。", "留意持续运动何时让位于停顿和分句。最后不要立即回拖，听完终止和余音，再与开头比较。"),
]
EXERCISES = [
    ("pulse", 1, 0, 13, "先抓住三连音脉动", "第 1 页开头是反复的八度三连音。先用它建立脉动，再分辨下方运动的方向。", "先轻点稳定的脉动，再只追下方的上行与落点；观察两者是否承担相同的任务。", ["三连音", "伴奏", "音区"]),
    ("recitando", 1, 22, 28, "从连续运动中找出说话的线条", "第 1 页第 15 小节附近有 recitando 指示；它为这一段的分句提供谱面依据。", "保持对背景运动的感觉，只注意较长音与停顿组成的句子。你会在哪里断句？", ["宣叙", "分句", "声部"]),
    ("contrast", 4, 79, 99, "同样在运动，语气为何改变", "第 4 页可见 tranquillo 与 ppp misterioso 等指示，和开场 dramatico 构成可比较的谱面线索。", "先听这一段，再回听开头 13 秒。用音区、音量、音符疏密三个具体词描述差别。", ["力度", "织体", "对比"]),
    ("layers", 6, 136, 149, "把两层线条分开追踪", "第 6 页 legg. amorosamente 附近，上下谱表同时保持不同的运动形状。", "第一遍只跟下方起伏；第二遍只找上方长一些的音。两遍结束时，你能各哼出一点轮廓吗？", ["声部", "分层", "旋律"]),
    ("pressure", 8, 190, 209, "把密集段落听成方向", "第 8 页有 cresc. subito、precipitato 和 il più presto possibile；这些是谱面指示，并非本录音的实测速度数值。", "用手画出你听到的上行、回落和落点。不要数所有音，只比较两次上行的长度和收束方式。", ["方向", "渐强", "速度"]),
    ("ending", 9, 236, None, "最后的停顿改变了什么", "第 9 页的 Recit. 和 Andante 可与开场持续的三连音并看；宽片段包括终止与余音。", "听到停顿时先不要操作。听完余音后，再回听开头：结尾改变的是速度、织体、分句，还是几项一起？", ["终止", "停顿", "整体联系"]),
]


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def prepare(pdf, video, output):
    output.mkdir(parents=True, exist_ok=True)
    score_dir, audio_dir = output / "music/score", output / "music/audio"
    score_dir.mkdir(parents=True, exist_ok=True); audio_dir.mkdir(parents=True, exist_ok=True)
    target_pdf = score_dir / "original.pdf"
    if target_pdf.exists() and target_pdf.read_bytes() != pdf.read_bytes():
        raise ValueError("The PDF source changed; use a new workspace/revision")
    shutil.copyfile(pdf, target_pdf)
    audio = audio_dir / "recording.m4a"
    if not audio.exists():
        subprocess.run(["ffmpeg", "-v", "error", "-i", str(video), "-map", "0:a:0", "-c:a", "copy", "-movflags", "+faststart", "-y", str(audio)], check=True)
    probe = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", str(audio)]))
    duration = float(probe["format"]["duration"]) * 1000
    subprocess.run(["ffmpeg", "-v", "error", "-xerror", "-i", str(audio), "-f", "null", "-"], check=True)
    write_json(audio_dir / "probe.json", probe)
    doc = fitz.open(pdf)
    if len(doc) != 9:
        raise ValueError("This curated edition must have 9 pages; re-review the page navigation for another edition")
    pages, page_facts = [], []
    for i, page in enumerate(doc, 1):
        relative = f"music/score/page-{i}.svg"
        svg = page.get_svg_image(text_as_path=True)
        # Each imported page carries its own defs, preventing PDF glyph/clip IDs
        # from colliding when two pages are mounted in the same reader.
        root = ET.fromstring(svg)
        identifiers = {element.get("id") for element in root.iter() if element.get("id")}
        for element in root.iter():
            for key, value in list(element.attrib.items()):
                if key == "id": element.set(key, f"pdf{i}-{value}")
                elif value.startswith("#") and value[1:] in identifiers: element.set(key, f"#pdf{i}-{value[1:]}")
                elif "url(#" in value:
                    for identifier in identifiers: value = value.replace(f"url(#{identifier})", f"url(#pdf{i}-{identifier})")
                    element.set(key, value)
        ET.register_namespace("", "http://www.w3.org/2000/svg")
        ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
        (output / relative).write_bytes(ET.tostring(root, encoding="utf-8", xml_declaration=True))
        pages.append(dict(page_id=f"page-{i}", path=relative, width=page.rect.width, height=page.rect.height))
        page_facts.append(dict(page=i, text=page.get_text(), drawing_count=len(page.get_drawings())))
    recording_ref = dict(recording_id="erlkoenig-scorefl-user-video", revision="user-mp4-20261103")
    def evidence(page, start, end):
        return [dict(provenance_id="user-pdf", score_object_ids=[], score_page_ids=[f"page-{page}"], audio=None),
                dict(provenance_id="video-page-observation", score_object_ids=[], audio=dict(recording=recording_ref, start_ms=start, end_ms=end))]
    events, structure = [], []
    for start, end, page, title, description, cue in PAGES:
        start_ms, end_ms = start * 1000, end * 1000 if end is not None else duration
        event_id = f"page-guide-{page}"
        events.append(dict(event_id=event_id, title=title, explanation=description, listen_for=cue, start_ms=start_ms, end_ms=end_ms,
                           roles=[dict(part_id="piano", role="钢琴独奏")], evidence=evidence(page, start_ms, end_ms), relations=[], concept_tags=["谱页导听"]))
        structure.append(dict(section_id=f"section-{page}", parent_id="whole", title=f"{page:02} · {title}", description=description,
                              start_ms=start_ms, end_ms=end_ms, event_ids=[event_id], coverage="navigation_only"))
    for name, page, start, end, title, explanation, cue, tags in EXERCISES:
        start_ms, end_ms = start * 1000, end * 1000 if end is not None else duration
        events.append(dict(event_id=f"exercise-{name}", title=title, explanation=explanation, listen_for=cue, start_ms=start_ms, end_ms=end_ms,
                           roles=[dict(part_id="piano", role="钢琴独奏")], evidence=evidence(page, start_ms, end_ms), relations=[], concept_tags=["导听练习", *tags]))
    by_id = {e["event_id"]: e for e in events}
    by_id["exercise-contrast"]["relations"] = [dict(target_event_id="exercise-pulse", kind="contrast", description="比较第 1 页开头 dramatico 的持续运动与第 4 页 tranquillo、misterioso 附近的谱面与原声宽片段。")]
    by_id["exercise-ending"]["relations"] = [dict(target_event_id="exercise-pulse", kind="contrast", description="将第 9 页 Recit.、Andante 附近的分句与开头三连音连续运动并看、回听。")]
    structure.insert(0, dict(section_id="whole", parent_id=None, title="全曲 · 从疾奔到终止", description="9 页原谱与全长原声；沿宽片段导航完整聆听，再选一个问题重听。",
                             start_ms=0, end_ms=duration, event_ids=[f"page-guide-{i}" for i in range(1,10)], coverage="navigation_only"))
    manifest = dict(version="music_source_manifest.v1", book_id="music-schubert-liszt-erlkoenig-user-video", title="舒伯特—李斯特 · 魔王",
        work=dict(work_id="schubert-liszt-s558-4", title="魔王 · Erlkönig, S.558/4", composer="Franz Schubert；钢琴改编：Franz Liszt", movement=None),
        score=dict(source=dict(score_id="erlkoenig-scorefl-user-pdf", revision="user-pdf-20261103"), title="用户提供的 ScoreFL PDF 原谱 · 9 页", pdf_path="music/score/original.pdf"),
        recording=dict(source=recording_ref, title="用户视频音轨 · 王羽佳（据文件说明）", performers="王羽佳，钢琴（据用户视频内说明）", media_path="music/audio/recording.m4a", content_type="audio/mp4", duration_ms=duration, kind="performance"),
        rights=[dict(asset_kind="recording", title="用户提供的视频音轨", url=video.as_uri(), license="用户提供；公开分发许可未确认", attribution="音轨从用户 MP4 原样提取 AAC，未重新编码、裁剪或改速。演奏者归属取自文件内说明。", distribution="local_only"),
                dict(asset_kind="score", title="用户提供的 PDF 原谱", url=SCORE_URL, license="用户提供；本谱本公开分发许可未确认", attribution="Schubert_Liszt - Erlkönig.pdf；原件保留，9 页 SVG 直接来自 PDF 矢量内容；Leipzig 字体许可见 music/NOTICE.txt。", distribution="local_only")],
        parts=[dict(part_id="piano", name="钢琴", instrument="钢琴", group="键盘", voices=["whole-score"])], pages=pages,
        score_objects=[], score_marks=[], occurrences=[], note_alignments=[], coverage=[],
        provenance=[dict(provenance_id="user-pdf", kind="score", description="直接查看用户 9 页 PDF 原谱，保留原排版及页码。术语与小节范围来自实际谱页；未将失败的自动识谱当作已确认音符数据。", source_url=SCORE_URL, processor="PyMuPDF 1.26.7 PDF vector extraction; curator visual reading"),
                    dict(provenance_id="video-page-observation", kind="reference", description="依据这份用户视频的同步谱画面建立整页宽片段导航，并保留观察帧。页区间为粗导航，分界未逐小节校准；不是人工起音听核，也不是逐音符对齐。", source_url=video.as_uri(), processor="FFmpeg frame extraction; AI visual review of this exact local video"),
                    dict(provenance_id="user-video-metadata", kind="reference", description="曲目、钢琴改编归属及演奏者标注来自用户 MP4 的 title、artist、description；未独立核验演奏发行版本。", source_url=video.as_uri(), processor="ffprobe metadata"),
                    dict(provenance_id="curator-guide", kind="interpretation", description="编选导听与回听问题，依据原谱指示和具体视频画面组织；它们是聆听任务，不冒充已完成的人类听核或角色逐句判定。", source_url=None, processor="curated listening guide")],
        events=events, structure=structure, demonstrations=[])
    guide = f"""# 舒伯特—李斯特《魔王》：全曲伴听

作品为 Erlkönig, S.558/4。原声来自你提供的视频；文件说明将钢琴演奏者标为王羽佳。原谱为你提供的 9 页 PDF，原排版完整保留。

## 先完整听一遍

第一遍只选一个观察点：持续运动怎样在最后让位于停顿和分句。中途漏掉旋律也可以继续向前。听完余音，再选一个短片段重听。

## 沿原谱走过全曲

""" + "\n\n".join(f"### 第 {page} 页 · {title}\n\n{description}\n\n{cue}" for _,_,page,title,description,cue in PAGES) + """

## 六次回听

每个练习都保留自己的原声区间和原谱页。先读“这一遍留意”，再播放；第二遍可以降至 0.75×，或循环当前片段。回听时用“音区更高”“音值更长”“停顿更明显”等具体词描述，不必急着猜角色。

""" + "\n\n".join(f"### {title}\n\n{explanation}\n\n{cue}" for _,_,_,_,title,explanation,cue,_ in EXERCISES) + """

## 把故事当作问题

视频说明介绍了叙述者、父亲、孩子与魔王四个角色。可以把它们当作理解戏剧的入口，再回到具体谱面与声音寻找依据。先比较音区、分句、力度和织体，再表达你怎样理解这一段。

## 已知限制

- 当前提供全曲播放、原谱浏览、页级宽片段导航和回听练习；页分界附近可手动前后查看。
- 自动识谱首轮漏出 12 小节，且开头谱号、音高有误，结果未进入正式谱面或音符索引。逐小节、逐音符对齐尚未接纳，播放器不显示逐音高亮。
- 原声为视频中 AAC 音轨的原样提取，不能据此恢复发行母带音质。演奏者标注依据文件说明。
- 本材料保存在本机，录音与这一谱本的公开分发许可尚未确认。
"""
    (output / "source.txt").write_text(guide, encoding="utf-8")
    shutil.copyfile(Path(__file__).resolve().parents[2] / "packages/core/assets/music/NOTICE.txt", output / "music/NOTICE.txt")
    write_json(output / ".build/music/import-candidate.json", manifest)
    write_json(output / "music/pdf-page-facts.json", page_facts)
    write_json(output / "music/build-report.json", dict(version="music_pdf_build_report.v1", source_pages=len(doc), pages=len(pages),
        events=len(events), exercises=len(EXERCISES), original_pdf_bytes=pdf.stat().st_size,
        duration_ms=duration, audio_operation="AAC stream copy from user's MP4; ffprobe and complete decode", model_calls=0,
        note_index_accepted=False, omr_accepted=False, navigation="coarse page intervals visually reviewed against supplied video"))
    print(json.dumps(dict(pages=len(pages), events=len(events), duration_ms=duration, output=str(output))))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--pdf", type=Path, required=True)
    parser.add_argument("--video", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path(".understand-book/erlkoenig/workspace"))
    args = parser.parse_args()
    prepare(args.pdf.resolve(), args.video.resolve(), args.output.resolve())
