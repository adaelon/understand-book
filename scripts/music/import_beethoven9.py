"""Import the published Tennstedt score-sync archive and the user's chaptered video.

Keep source notes separate from performance occurrences. Native engraving is sliced
into readable pages; the archived recording clock is not refitted.
"""
from __future__ import annotations

import argparse
import bisect
import collections
import copy
import gzip
import json
import re
from pathlib import Path
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET

REPO_URL = "https://github.com/tofucurd/music-score-guide-archive"
SVG_NS = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG_NS)
ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")


def read_archive(root: Path, relative: str):
    path = root / relative
    raw = path.read_bytes() if path.exists() else gzip.decompress(Path(str(path) + ".gz").read_bytes())
    return json.loads(raw)


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf8")


def fraction(value):
    return value[0] / value[1] if isinstance(value, list) else float(value)


def recording_clock(alignment):
    # Repeated q=0 anchors describe the opening silence: the last is the musical start.
    by_q = {float(q): sample for sample, q in alignment["points"]}
    qs = sorted(by_q)
    rate = alignment["sample_rate"]

    def at(q):
        i = bisect.bisect_right(qs, q) - 1
        if i < 0:
            return by_q[qs[0]] / rate * 1000
        if i >= len(qs) - 1:
            return by_q[qs[-1]] / rate * 1000
        left, right = qs[i], qs[i + 1]
        sample = by_q[left] + (q - left) / (right - left) * (by_q[right] - by_q[left])
        return sample / rate * 1000

    return at


def instrument_group(name):
    for group, words in [
        ("弦乐", ("小提琴", "中提琴", "大提琴", "低音提琴")),
        ("木管", ("长笛", "短笛", "双簧", "单簧", "巴松")),
        ("铜管", ("圆号", "小号", "长号")),
        ("打击乐", ("定音鼓", "三角铁", "大鼓", "钹")),
        ("人声", ("独唱", "合唱")),
    ]:
        if any(word in name for word in words):
            return group
    return "其他"


def native_pages(root, movement, output, score, geometry, canonical, labels, bars_per_page=8):
    raw = gzip.decompress((root / f".score-sync/{movement}/score.svg.gz").read_bytes())
    tree = ET.fromstring(raw)
    system = next(e for e in tree.iter() if e.get("class") == "system")
    subtrees = {e.get("id"): e for e in system if e.get("class") == "measure"}
    first_measures = {}
    for measure in geometry["measures"]:
        first_measures.setdefault(measure["source_index"], measure)
    measures = sorted(first_measures.values(), key=lambda m: m["source_index"])
    events_by_measure = collections.defaultdict(list)
    for event in canonical.values():
        events_by_measure[event["source_measure"]].append(event)
    defs = [e for e in tree if e.tag.rsplit("}", 1)[-1] in ("defs", "style", "desc")]
    staff_by_svg = {svg_id: staff["id"] for staff in geometry["staves"] for svg_id in staff["svg_ids"]}
    signatures, page_signatures = {}, {}
    for index, measure in enumerate(measures):
        if index % bars_per_page == 0:
            page_signatures[index] = {sid: dict(state) for sid, state in signatures.items()}
        for staff in subtrees[measure["svg_id"]]:
            staff_id = staff_by_svg.get(staff.get("id"))
            if not staff_id:
                continue
            state = signatures.setdefault(staff_id, {})
            for symbol in staff.iter():
                if symbol.get("class") in ("clef", "keySig", "meterSig"):
                    state[symbol.get("class")] = symbol
            state["lines"] = [e for e in staff if e.tag.endswith("path")]
    height = geometry["height"]
    pages, mapping = [], {}
    for start in range(0, len(measures), bars_per_page):
        batch = measures[start:start + bars_per_page]
        first_x = batch[0]["x"]
        prefix_width = 3000 if start == 0 else 7000
        width = max(m["x"] + m["width"] for m in batch) - first_x + prefix_width + 200
        page_id = f"page-{len(pages) + 1}"
        svg_id = f"native-{movement}-{page_id}"
        page = ET.Element(f"{{{SVG_NS}}}svg", {"viewBox": f"0 0 {width} {height}",
            "id": svg_id, "color": "black", "font-family": "Times, serif", "role": "img"})
        title = ET.SubElement(page, f"{{{SVG_NS}}}title")
        movement_name = {"m1": "第一乐章", "m2": "第二乐章", "m3": "第三乐章", "m4": "第四乐章"}[movement]
        ranges = []
        range_start = previous = batch[0]['source_index']
        for measure in batch[1:]:
            current = measure['source_index']
            if current != previous + 1:
                ranges.append(str(range_start) if range_start == previous else f"{range_start}–{previous}")
                range_start = current
            previous = current
        ranges.append(str(range_start) if range_start == previous else f"{range_start}–{previous}")
        title.text = f"{movement_name} · 第 {'、'.join(ranges)} 小节"
        for definition in defs:
            definition = copy.deepcopy(definition)
            if definition.tag.endswith("style") and definition.text:
                definition.text = definition.text.replace("#" + tree.get("id"), "#" + svg_id)
            page.append(definition)
        header = ET.SubElement(page, f"{{{SVG_NS}}}text", {"x": "250", "y": "650", "font-size": "350", "fill": "#687364"})
        header.text = title.text
        opening = {}
        for staff in subtrees[batch[0]["svg_id"]]:
            staff_id = staff_by_svg.get(staff.get("id"))
            if staff_id:
                opening[staff_id] = opening_signatures(staff)
        for staff_id, state in page_signatures[start].items():
            for line in state.get("lines", []):
                y_match = re.search(r"M[\d.-]+[ ,]+([\d.-]+)", line.get("d", ""))
                if y_match:
                    y = float(y_match[1]) + 500
                    ET.SubElement(page, f"{{{SVG_NS}}}path", {"d": f"M2250 {y} L6500 {y}", "stroke-width": line.get("stroke-width", "13"), "stroke": "currentColor"})
            for kind, x in [("clef", 2450), ("keySig", 3300), ("meterSig", 5150)]:
                if kind in opening.get(staff_id, set()):
                    continue
                symbol = state.get(kind)
                if symbol is None:
                    continue
                xs = [float(match[1]) for node in symbol.iter() if (match := re.search(r"translate\(([\d.-]+)[ ,]", node.get("transform", "")))]
                if not xs:
                    continue
                group = ET.SubElement(page, f"{{{SVG_NS}}}g", {"transform": f"translate({x - min(xs)}, 500)"})
                clone = copy.deepcopy(symbol)
                for node in clone.iter():
                    if node.get("id"):
                        node.set("id", svg_id + "-signature-" + node.get("id"))
                group.append(clone)
        content = ET.SubElement(page, f"{{{SVG_NS}}}g", {"transform": f"translate({prefix_width - first_x}, 500)"})
        for measure in batch:
            subtree = copy.deepcopy(subtrees[measure["svg_id"]])
            # Only the first engraved occurrence is retained. Later performances point
            # back to these canonical source objects instead of creating new notes.
            id_map = {event["id"]: event["source_id"] for event in events_by_measure[measure["source_index"]]}
            present = set()
            for element in subtree.iter():
                old = element.get("id")
                if old in id_map:
                    element.set("id", id_map[old])
                    present.add(id_map[old])
            for event in events_by_measure[measure["source_index"]]:
                source_id = event["source_id"]
                mapping[source_id] = page_id
                if source_id not in present:
                    # Hidden editorial notes keep their source anchor, without drawing
                    # a replacement pitch or a spurious sounding marker.
                    ET.SubElement(subtree, f"{{{SVG_NS}}}g", {"id": source_id, "display": "none"})
            content.append(subtree)
        for staff in geometry["staves"]:
            text = ET.SubElement(page, f"{{{SVG_NS}}}text", {"x": "250", "y": str(staff["y"]),
                "font-size": "360", "font-family": "Noto Serif SC, SimSun, serif", "fill": "#34433a"})
            label = labels.get(staff["id"], staff["label"])
            base_name, separator, tuning = label.partition("（")
            text.text = base_name
            if separator:
                detail = ET.SubElement(text, f"{{{SVG_NS}}}tspan", {"x": "250", "dy": "350", "font-size": "280"})
                detail.text = "（" + tuning
        relative = f"music/score/{page_id}.svg"
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(ET.tostring(page, encoding="unicode"), encoding="utf8")
        pages.append({"page_id": page_id, "path": relative, "width": width, "height": height})
    return pages, mapping


def opening_signatures(staff):
    """Symbols before the first written note/rest already supply the page opening."""
    def first_x(element):
        positions = [float(match[1]) for node in element.iter()
            if (match := re.search(r"translate\(([\d.-]+)[ ,]", node.get("transform", "")))]
        return min(positions, default=float("inf"))

    notes = [node for node in staff.iter() if node.get("class") in ("note", "rest", "mRest", "multiRest")]
    onset = min((first_x(note) for note in notes), default=float("inf"))
    return {node.get("class") for node in staff.iter() if node.get("class") in ("clef", "keySig", "meterSig") and first_x(node) <= onset}


def instrument_labels(labels, signatures):
    result = dict(labels)
    for staff_id, staff in signatures["staves"].items():
        tunings = list(dict.fromkeys(event["pitch_label"] for event in staff["events"] if event.get("pitch_label")))
        if tunings and staff_id in result:
            tuning = "／".join(tunings).replace("♭", "降").replace("B降", "降B").replace("E降", "降E").replace("A降", "降A")
            result[staff_id] += f"（{tuning}调）"
    return result


def double_fugue_guides(regions, source_events, objects, parts, recording, at):
    """Score-grounded listening activities for the archive's selected double fugue."""
    selections = [("seid", "P49:1", "拥抱主题", "先找第二小提琴的两个较长同音，随后听音高下行，再向下跳开。和另一条旋律的长短交替作比较。"),
        ("freude", "P4:1", "欢乐主题", "先跟双簧管1开头的两个同音，再找接着向上走的两个音。留意长、短交替的节奏，试着哼出这几个音的轮廓。")]
    guides = []
    pitch_names = ("C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B")
    for theme, staff, title, prompt in selections:
        region = min((r for r in regions["regions"] if r["theme"] == theme and r["staff"] == staff), key=lambda r: r["start_q"])
        notes = [source_events[eid] for eid in region["event_ids"] if source_events[eid]["kind"] == "pitched"
            and source_events[eid].get("is_player", True) and not source_events[eid]["tie_stop"]][:4]
        ids = [e["source_id"] for e in notes]
        start_q = fraction(notes[0]["q"])
        end_q = fraction(notes[-1]["q"]) + fraction(notes[-1]["duration"])
        a, b = at(start_q), at(end_q)
        part_id = objects[ids[0]]["part_id"]
        pitches = "、".join(f"{pitch_names[int(n['pitch']) % 12]}{int(n['pitch']) // 12 - 1}" for n in notes)
        guides.append({"event_id": f"m4-listening-{theme}", "title": f"双赋格·辨认{title}",
            "explanation": f"谱面第{notes[0]['source_measure']}–{notes[-1]['source_measure']}源小节，{parts[part_id]['name']}承载归档标注的{title}。前四个音为{pitches}，时值依次为{'、'.join(str(fraction(n['duration'])) for n in notes)}个四分音符单位。",
            "listen_for": prompt, "start_ms": a, "end_ms": b,
            "roles": [{"part_id": part_id, "role": title, "voice_ids": list(dict.fromkeys(objects[sid]["voice"] for sid in ids)), "theme_id": f"m4:{theme}"}],
            "evidence": [{"provenance_id": "curated-score-study", "score_object_ids": ids, "audio": {"recording": recording, "start_ms": a, "end_ms": b}}],
            "relations": [], "concept_tags": ["导听练习", "谱面依据", "听核待完成"]})
    a, b = min(e["start_ms"] for e in guides), max(e["end_ms"] for e in guides)
    guides.append({"event_id": "m4-listening-combined", "title": "双赋格·同时追踪两个主题",
        "explanation": "第二小提琴的拥抱材料先开始，双簧管1的欢乐材料随后进入。两组源谱音的时间范围重叠，可作为比较长音线条与长短交替线条的练习。",
        "listen_for": "先跟第二小提琴，等双簧管1进入后继续保留前一条线。重听时改跟双簧管1，再试着指出第二小提琴仍在进行的位置。可分别选择两个声部聚焦，并循环这段原声。",
        "start_ms": a, "end_ms": b, "roles": [r for g in guides for r in g["roles"]],
        "evidence": [{"provenance_id": "curated-score-study", "score_object_ids": list(dict.fromkeys(sid for g in guides for e in g["evidence"] for sid in e["score_object_ids"])), "audio": {"recording": recording, "start_ms": a, "end_ms": b}}],
        "relations": [], "concept_tags": ["导听练习", "谱面依据", "听核待完成"]})
    return guides


def score_marks(xml_path, objects):
    """Keep original note notation and declarations under the frozen source IDs."""
    document = ET.parse(xml_path).getroot()
    indexed = collections.defaultdict(list)
    for obj in objects:
        indexed[(obj["part_id"], obj["measure"])].append(obj)
    marks = []
    for part in document.findall("part"):
        part_id, divisions = part.get("id"), 1
        for mi, measure in enumerate(part.findall("measure"), 1):
            anchors = indexed[(part_id, str(mi))]
            if not anchors:
                continue
            cursor = 0.0
            note_index = 0
            note_anchors = {int(a['source_id'].rsplit('-n', 1)[1]): a for a in anchors
                            if '-n' in a['source_id'] and a['source_id'].rsplit('-n', 1)[1].isdigit()}
            for child in measure:
                declarations = []
                staff = None
                if child.tag == "attributes":
                    divisions = float(child.findtext("divisions", str(divisions)))
                    for key in child.findall("key"):
                        if key.find("fifths") is not None:
                            declarations.append(({"kind": "key_signature", "fifths": int(key.findtext("fifths")), "encoded_mode": key.findtext("mode")}, key.get("number")))
                    for meter in child.findall("time"):
                        components = [{"beats": b.text, "beat_type": int(t.text)} for b, t in zip(meter.findall("beats"), meter.findall("beat-type"))]
                        if components:
                            declarations.append(({"kind": "time_signature", "components": components, "symbol": meter.get("symbol")}, meter.get("number")))
                elif child.tag == "backup":
                    cursor -= float(child.findtext("duration", "0")) / divisions
                elif child.tag == "forward":
                    cursor += float(child.findtext("duration", "0")) / divisions
                elif child.tag == "note":
                    note_index += 1
                    if note_index in note_anchors:
                        pitch = child.find('pitch')
                        modification = child.find('time-modification')
                        time_modification = None if modification is None else {
                            'actual_notes': int(modification.findtext('actual-notes')),
                            'normal_notes': int(modification.findtext('normal-notes')),
                            'normal_type': modification.findtext('normal-type'),
                            'normal_dots': len(modification.findall('normal-dot')),
                        }
                        if time_modification is not None and (time_modification['actual_notes'] <= 0 or time_modification['normal_notes'] <= 0):
                            raise ValueError('MusicXML time-modification requires positive actual-notes and normal-notes')
                        note_anchors[note_index]['notation'] = {
                            'written_pitch': None if pitch is None else {
                                'step': pitch.findtext('step'), 'alter': float(pitch.findtext('alter', '0')),
                                'octave': int(pitch.findtext('octave'))},
                            'note_type': child.findtext('type'), 'dots': len(child.findall('dot')),
                            'visible': child.get('print-object') != 'no',
                            'playback_dynamics': float(child.get('dynamics')) if child.get('dynamics') is not None else None,
                            'chord': child.find('chord') is not None, 'grace': child.find('grace') is not None,
                            'rhythm': {'time_modification': time_modification,
                                       'tremolos': [{'kind': t.get('type', 'single'), 'marks': int(t.text or '0')}
                                                    for t in child.findall('notations/ornaments/tremolo')]},
                        }
                    if child.find("chord") is None and child.find("grace") is None:
                        cursor += float(child.findtext("duration", "0")) / divisions
                elif child.tag in ("direction", "sound"):
                    words = [e.text.strip() for e in child.findall("direction-type/words") if e.text and e.text.strip()]
                    sound = child if child.tag == "sound" else child.find("sound")
                    tempo = float(sound.get("tempo", "0")) if sound is not None else 0
                    metro = child.find("direction-type/metronome")
                    metronome = None
                    if metro is not None and metro.findtext("per-minute"):
                        beat_unit = (metro.findtext("beat-unit") or "").strip()
                        if beat_unit:
                            metronome = {"beat_unit": beat_unit, "dots": len(metro.findall("beat-unit-dot")), "per_minute": metro.findtext("per-minute")}
                        else:
                            words.append(f"原谱节拍器：{metro.findtext('per-minute')}（未注明拍值）")
                    staff = child.findtext("staff")
                    if words or tempo or metronome:
                        declarations.append(({"kind": "direction", "words": words, "metronome": metronome, "sound_tempo_quarter_bpm": tempo or None}, staff))
                for value, staff in declarations:
                    mark_beat = cursor + (float(child.findtext("offset", "0")) / divisions if child.tag == "direction" else 0)
                    candidates = [a for a in anchors if not staff or a["voice"].split(":")[0] == staff] or anchors
                    anchor = min(candidates, key=lambda a: abs(a["beat"] - mark_beat))
                    marks.append({"mark_id": f"{part_id}-m{mi}-mark{len(marks) + 1}", "part_id": part_id,
                        "measure": str(mi), "measure_index": mi, "beat": mark_beat, "staff": staff,
                        "value": value, "score_object_ids": [anchor["source_id"]], "provenance_id": "archive-score"})
    return marks


def import_movement(archive, output_root, video, chapter, revision):
    movement = chapter["id"]
    number = int(movement[1:])
    started = time.monotonic()
    root = archive / "projects/beethoven9"
    output = output_root / movement
    output.mkdir(parents=True, exist_ok=True)
    score = read_archive(root, f".score-sync/{movement}/score.json")
    geometry = read_archive(root, f".score-sync/{movement}/geometry.json")
    alignment = read_archive(root, f".score-sync/{movement}/alignment.json")
    labels = read_archive(root, f".score-sync/{movement}/labels.json")
    movement_name = {"m1": "第一乐章", "m2": "第二乐章", "m3": "第三乐章", "m4": "第四乐章"}[movement]
    signatures = read_archive(root, "research/material-regions-full/staff_signatures.json")[movement_name]
    labels = instrument_labels(labels, signatures)
    structure = read_archive(root, f".score-sync/{movement}/structure.json")
    regions = read_archive(root, f"research/material-regions-full/{movement}-regions.json")
    rate = alignment["sample_rate"]
    duration_ms = (chapter["end_sample"] - chapter["start_sample"]) / rate * 1000
    at = recording_clock(alignment)
    canonical = {}
    for event in score["events"]:
        canonical.setdefault(event["source_id"], event)
    pages, mapping = native_pages(root, movement, output, score, geometry, canonical, labels)
    first_measure = {}
    for measure in score["measures"]:
        first_measure.setdefault(measure["source_index"], measure)
    parts_by_id, objects = {}, []
    for source_id, event in canonical.items():
        staff = event.get("storage_staff", event["staff"])
        part_id, staff_number = staff.split(":")
        voice = f"{staff_number}:{event['voice']}"
        name = labels.get(staff, staff)
        part = parts_by_id.setdefault(part_id, {"part_id": part_id, "name": name, "instrument": name, "group": instrument_group(name), "voices": []})
        if voice not in part["voices"]:
            part["voices"].append(voice)
        rest = event["kind"] == "rest"
        tie = "none" if rest else "continue" if event["tie_start"] and event["tie_stop"] else "start" if event["tie_start"] else "stop" if event["tie_stop"] else "none"
        obj = {"source_id": source_id, "part_id": part_id, "measure": str(event["source_measure"]),
            "beat": max(0, fraction(event["q"]) - fraction(first_measure[event["source_measure"]]["q"])),
            "duration_beats": fraction(event["duration"]), "voice": voice,
            "midi_pitch": int(event["pitch"]) if event["pitch"] is not None else None,
            "rest": rest, "tie": tie, "page_id": mapping[source_id], "svg_element_id": source_id}
        if event["kind"] == "unpitched":
            obj["unpitched"] = True
        objects.append(obj)
    by_id = {obj["source_id"]: obj for obj in objects}
    ids_by_measure = collections.defaultdict(list)
    for obj in objects:
        ids_by_measure[obj["measure"]].append(obj["source_id"])
    alignment_by_id = {note["id"]: note for note in alignment["notes"]}
    events_by_occurrence = collections.defaultdict(list)
    for event in score["events"]:
        events_by_occurrence[event["measure"]].append(event)
    boundaries = []
    for measure in score["measures"]:
        onset = at(fraction(measure["q"]))
        pitched_starts = [alignment_by_id[e["id"]]["range"][0] / rate * 1000 for e in events_by_occurrence[measure["index"]]
            if e["kind"] != "rest" and e["id"] in alignment_by_id]
        boundaries.append(min([onset, *pitched_starts]))
    boundaries.append(min(duration_ms, at(fraction(score["measures"][-1]["end"]))))
    occurrences, note_alignments = [], []
    zero_candidates, adjusted_candidates = 0, 0
    for i, measure in enumerate(score["measures"]):
        occurrence_id = f"{movement}-occurrence-{measure['index']}"
        start, end = boundaries[i:i + 2]
        if end <= start:
            raise ValueError(f"Non-increasing source measure clock: {movement} {measure['index']}")
        occurrences.append({"occurrence_id": occurrence_id, "order": i, "measure": str(measure["source_index"]),
            "start_ms": start, "end_ms": end, "score_object_ids": ids_by_measure[str(measure["source_index"])]})
        for event in events_by_occurrence[measure["index"]]:
            if event["kind"] == "rest":
                continue
            item = alignment_by_id[event["id"]]
            a, b = [sample / rate * 1000 for sample in item["range"]]
            if b <= a:
                zero_candidates += 1
                continue
            # Leave anomalous archived candidates in the raw snapshot, outside the
            # usable projection. Do not move them into a different measure silently.
            if a < start or a >= end or b > duration_ms:
                adjusted_candidates += 1
                continue
            obj = by_id[event["source_id"]]
            hidden = not event.get("is_player", True) or not event.get("visible", True)
            note_alignments.append({"occurrence_id": occurrence_id, "score_object_id": event["source_id"],
                "start_ms": a, "end_ms": b, "attack": not hidden and obj["tie"] not in ("stop", "continue"),
                "provenance_id": "archive-clock", **({"unobserved": True} if hidden else {})})
    recording_ref = {"recording_id": f"beethoven9-tennstedt-19921008-{movement}-video-aac", "revision": "chapter-pcm-v1"}
    provenance = [
        {"provenance_id": "archive-score", "kind": "score", "description": "OpenScore CC0 修订 MusicXML 与冻结 Verovio 谱面；保持原谱对象身份，反复演奏另建出现。", "source_url": REPO_URL + "/tree/main/projects/beethoven9/derived/scores", "processor": "score-sync archive import v1"},
        {"provenance_id": "archive-clock", "kind": "audio_analysis", "description": "继承源视频工程的录音时钟与 DTW/人工锚点修订。视频四章时间与交付样本边界相同；逐音符为投影候选，未宣称独立声学起音验证。", "source_url": REPO_URL + f"/blob/main/projects/beethoven9/.score-sync/{movement}/alignment.json.gz", "processor": alignment.get("method", "archived recording clock")},
        {"provenance_id": "archive-analysis", "kind": "reference", "description": "归档作者编选的两层曲式与精选材料区域；保留所选范围，不自动推演未标注的主题出现。", "source_url": REPO_URL + "/tree/main/projects/beethoven9/research/material-regions-full", "processor": "source annotation projection"},
    ]
    music_events, sections = [], []
    def add_section(section, parent=None):
        sid = f"{movement}-section-{len(sections) + 1}"
        sections.append({"section_id": sid, "parent_id": parent, "title": section["label"],
            "description": f"归档曲式导航：{section['label']}。", "start_ms": at(section["start_q"]), "end_ms": at(section["end_q"]), "event_ids": [], "coverage": "navigation_only"})
        for child in section.get("children", []):
            add_section(child, sid)
    for section in structure["sections"]:
        add_section(section)
    source_event = {e["id"]: e for e in score["events"]}
    for region in regions.get("regions", []):
        selected = [source_event[eid] for eid in region["event_ids"] if eid in source_event]
        ids = list(dict.fromkeys(e["source_id"] for e in selected))
        if not ids:
            continue
        roles = []
        for pid in dict.fromkeys(by_id[eid]["part_id"] for eid in ids):
            voices = list(dict.fromkeys(by_id[eid]["voice"] for eid in ids if by_id[eid]["part_id"] == pid))
            roles.append({"part_id": pid, "role": regions["short_names"].get(region["theme"], region["theme"]), "voice_ids": voices, "theme_id": f"{movement}:{region['theme']}"})
        theme = regions["short_names"].get(region["theme"], region["theme"])
        players = "、".join(parts_by_id[role["part_id"]]["name"] for role in roles)
        a, b = at(region["start_q"]), at(region["end_q"])
        music_events.append({"event_id": region["id"], "title": f"{theme} · {players}",
            "explanation": f"归档作者将这组谱音编选为“{theme}”的材料区域。范围与声部来自冻结标注。",
            "listen_for": f"先跟随{players}，寻找“{theme}”的节奏与旋律轮廓；再关注它与同时进行的其他声部如何结合。",
            "start_ms": a, "end_ms": b, "roles": roles,
            "evidence": [{"provenance_id": "archive-analysis", "score_object_ids": ids, "audio": {"recording": recording_ref, "start_ms": a, "end_ms": b}}],
            "relations": [], "concept_tags": ["材料区域", theme]})
    vocal_count = 0
    if movement == "m4":
        provenance.append({"provenance_id": "curated-score-study", "kind": "score", "description": "根据冻结谱音、时值与归档主题标识编写的双赋格听辨活动；实际录音的独立听核和初学者验证待完成。", "source_url": REPO_URL + "/tree/main/projects/beethoven9", "processor": "double fugue score study v1"})
        music_events.extend(double_fugue_guides(regions, source_event, by_id, parts_by_id, recording_ref, at))
        vocals = read_archive(root, "content/vocals/timeline.json")
        translations = read_archive(root, "content/vocals/translations.json")
        provenance.append({"provenance_id": "archive-lyrics", "kind": "reference", "description": "归档唱词及其中文译文，按源工程分句时间呈现；音节时间沿用谱面/录音时钟，不是声学音素测量。", "source_url": REPO_URL + "/tree/main/projects/beethoven9/content/vocals", "processor": "archived vocal phrases v4"})
        for lane in vocals["lanes"]:
            for phrase in lane["phrases"]:
                ids = list(dict.fromkeys(source_event[s["source_lyric_event_id"]]["source_id"] for s in phrase["segments"] if s.get("source_lyric_event_id") in source_event))
                translation = translations.get(phrase["translation_key"], "")
                music_events.append({"event_id": phrase["id"], "title": phrase["de"], "explanation": translation or phrase["de"],
                    "listen_for": f"{lane['label']}：{translation or phrase['de']}", "start_ms": phrase["start_sample"] / rate * 1000,
                    "end_ms": phrase["end_sample"] / rate * 1000, "roles": [],
                    "evidence": [{"provenance_id": "archive-lyrics", "score_object_ids": ids, "audio": {"recording": recording_ref, "start_ms": phrase["start_sample"] / rate * 1000, "end_ms": phrase["end_sample"] / rate * 1000}}],
                    "relations": [], "concept_tags": ["lyrics", lane["label"]]})
                vocal_count += 1
        for filename in ["timeline.json", "translations.json", "phrases.json"]:
            write_json(output / f"music/vocals/{filename}", read_archive(root, "content/vocals/" + filename))
    for section in sections:
        section["event_ids"] = [e["event_id"] for e in music_events if "lyrics" not in e["concept_tags"] and e["start_ms"] >= section["start_ms"] and e["end_ms"] <= section["end_ms"]]
    xml_path = output / "music/score/original.musicxml"
    source_xml = root / f"derived/scores/movement-{number}.musicxml"
    xml_path.write_bytes(source_xml.read_bytes() if source_xml.exists() else gzip.decompress(Path(str(source_xml) + ".gz").read_bytes()))
    unperformed = []
    if movement == 'm2':
        from beethoven9_supplement import supplement_unperformed_score
        supplement = supplement_unperformed_score(xml_path, output, pages, objects, parts_by_id)
        unperformed = {str(index) for index in supplement['measures']}
        write_json(output / 'music/supplement-report.json', supplement)
        provenance.append({'provenance_id': 'score-supplement', 'kind': 'score',
            'description': '从原始 MusicXML 补刻本次录音未演奏的小节，仅供查谱。',
            'source_url': REPO_URL + '/tree/main/projects/beethoven9/derived/scores', 'processor': supplement['engraver_version']})
    audio_path = output / "music/audio/recording.flac"
    audio_path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-y", "-i", str(video), "-map", "0:a:0", "-af",
        f"atrim=start_sample={chapter['start_sample']}:end_sample={chapter['end_sample']},asetpts=PTS-STARTPTS",
        "-c:a", "flac", "-sample_fmt", "s16", str(audio_path)], check=True)
    rights = [
        {"asset_kind": "recording", "title": "滕施泰特 / 伦敦爱乐 · 1992-10-08 现场（用户视频音轨）", "url": REPO_URL,
            "license": "用户提供视频音轨；用户已选择启用分发，未附独立授权文件", "attribution": "Klaus Tennstedt; London Philharmonic Orchestra & Choir, 8 October 1992. User-supplied score-sync MP4 by tofucurd; AAC decoded to chapter FLAC without a second opening trim.", "distribution": "allowed"},
        {"asset_kind": "score", "title": "OpenScore CC0 · 归档修订 MusicXML", "url": REPO_URL + "/blob/main/THIRD_PARTY_NOTICES.md", "license": "OpenScore CC0；保留原 XML 权利声明", "attribution": "Ludwig van Beethoven; OpenScore transcribers; frozen revisions and engraving archived by tofucurd.", "distribution": "allowed"},
        {"asset_kind": "reference", "title": "tofucurd 贝九曲式、材料与唱词存档", "url": REPO_URL + "/tree/main/projects/beethoven9", "license": "归档代码与原创说明 MIT；原有引用及第三方资料保留原权利", "attribution": f"tofucurd/music-score-guide-archive, revision {revision}; selected annotations retained with source pointers.", "distribution": "allowed"},
        {"asset_kind": "font", "title": "Verovio 冻结刻谱字形", "url": "https://github.com/rism-digital/verovio/tree/develop/fonts", "license": "SIL Open Font License 1.1", "attribution": "Original Verovio SVG glyph definitions retained; see music/NOTICE.txt.", "distribution": "allowed"},
    ]
    manifest = {"version": "music_source_manifest.v1", "book_id": f"music-beethoven9-tennstedt-{movement}-release1",
        "title": f"贝多芬第九交响曲 · {chapter['title']} · 滕施泰特 1992",
        "work": {"work_id": "beethoven-op125", "title": "贝多芬第九交响曲", "composer": "路德维希·凡·贝多芬", "movement": chapter["title"]},
        "score": {"source": {"score_id": f"beethoven9-openscore-archive-{movement}", "revision": revision}, "title": "OpenScore 修订总谱 · 归档冻结刻谱", "musicxml_path": "music/score/original.musicxml"},
        "recording": {"source": recording_ref, "title": "Tennstedt 1992 Live · 视频音轨", "performers": "滕施泰特 / 伦敦爱乐乐团与合唱团", "media_path": "music/audio/recording.flac", "content_type": "audio/flac", "duration_ms": duration_ms, "kind": "performance"},
        "rights": rights, "parts": list(parts_by_id.values()), "pages": pages, "score_objects": objects,
        "score_marks": score_marks(xml_path, [obj for obj in objects if obj['measure'] not in unperformed]) + score_marks(xml_path, [obj for obj in objects if obj['measure'] in unperformed]), "occurrences": occurrences, "note_alignments": note_alignments,
        "coverage": [
            {"start_ms": boundaries[0], "end_ms": boundaries[-1], "granularity": "measure", "method": "audio_alignment", "status": "accepted", "provenance_id": "archive-clock", "detail": "继承同一源视频制作时钟的小节定位；保留开头静默与结尾余音/掌声。"},
            {"start_ms": boundaries[0], "end_ms": boundaries[-1], "granularity": "note", "method": "audio_alignment", "status": "pending", "provenance_id": "archive-clock", "detail": "逐音符时刻继承归档投影候选；尚未完成独立起音与连续听核，暂不显示已校准逐音光点。"}],
        "provenance": provenance, "events": music_events, "structure": sections, "demonstrations": []}
    guide = f"# {manifest['title']}\n\n编选导读依据 tofucurd 的贝九工程归档。录音来自用户提供的完整曲谱同步视频，按原有章节提取。\n\n## 怎样聆听\n\n先从作品导览选择一个段落，完整听一遍，再关注一个声部或一组材料。谱页保留完整总谱；重复演奏返回同一谱源位置。\n\n"
    for section in structure["sections"]:
        guide += f"## {section['label']}\n\n" + "、".join(c["label"] for c in section.get("children", [])) + "。\n\n"
    guide += "## 来源与同步\n\n小节定位沿用源视频的制作时钟。逐音符数据保留为待核候选；谱面时值投影不代表已测得每件乐器的物理起音或止音。材料染色只覆盖归档作者精选的区域。\n\n" + REPO_URL + "/tree/main/projects/beethoven9\n"
    (output / "source.txt").write_text(guide, encoding="utf8")
    write_json(output / ".build/music/import-candidate.json", manifest)
    for relative in [f".score-sync/{movement}/alignment.json", f".score-sync/{movement}/score.json", f".score-sync/{movement}/structure.json", f"research/material-regions-full/{movement}-regions.json"]:
        write_json(output / "music/archive" / Path(relative).name.replace(".json", "-" + movement + ".json"), read_archive(root, relative))
    shutil.copyfile(archive / "THIRD_PARTY_NOTICES.md", output / "music/THIRD_PARTY_NOTICES.md")
    shutil.copyfile(archive / "LICENSE", output / "music/ARCHIVE-LICENSE.txt")
    notice = Path(__file__).resolve().parents[2] / "packages/core/assets/music/NOTICE.txt"
    (output / "music/NOTICE.txt").write_text(notice.read_text(encoding="utf8") + "\n\n" + (archive / "LICENSE").read_text(encoding="utf8") + "\n\n" + (archive / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf8"), encoding="utf8")
    report = {"movement": movement, "workspace": str(output.resolve()), "archive_revision": revision,
        "audio_source": str(video), "source_chapter": chapter, "audio_operation": "AAC decode, sample-exact chapter trim, PCM16 FLAC; no resampling or second opening trim",
        "pages": len(pages), "canonical_score_objects": len(objects), "performed_occurrences": len(occurrences),
        "note_candidates": len(note_alignments), "omitted_zero_duration_candidates": zero_candidates,
        "omitted_outside_occurrence_candidates": adjusted_candidates, "source_flags": alignment.get("flags", []),
        "material_regions": len(regions.get("regions", [])), "vocal_phrases": vocal_count, "sections": len(sections),
        "unpitched_objects": sum(bool(o.get("unpitched")) for o in objects), "elapsed_seconds": round(time.monotonic() - started, 3)}
    write_json(output / "music/import-report.json", report)
    print(json.dumps({**{k: v for k, v in report.items() if k != "source_flags"}, "source_flag_count": len(report["source_flags"])}, ensure_ascii=False), flush=True)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--video", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--movement", choices=["m1", "m2", "m3", "m4"])
    args = parser.parse_args()
    delivery = read_archive(args.archive / "projects/beethoven9", "delivery-record.json")
    probe = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_chapters", "-show_streams", "-of", "json", str(args.video)], encoding="utf8"))
    audio = next(s for s in probe["streams"] if s["codec_type"] == "audio")
    if int(audio["sample_rate"]) != delivery["audio"]["sample_rate"]:
        raise ValueError("Video sample rate differs from the frozen source clock")
    chapters = delivery["audio"]["chapters"]
    if len(probe["chapters"]) != len(chapters):
        raise ValueError("Video must retain all four source chapters")
    for video_chapter, chapter in zip(probe["chapters"], chapters):
        for name, sample in [("start_time", chapter["start_sample"]), ("end_time", chapter["end_sample"])]:
            if abs(float(video_chapter[name]) * int(audio["sample_rate"]) - sample) > 1:
                raise ValueError("Video chapter boundary differs from archived production")
    revision = subprocess.check_output(["git", "-C", str(args.archive), "rev-parse", "HEAD"], text=True).strip()
    reports = [import_movement(args.archive, args.output, args.video, chapter, revision) for chapter in chapters if not args.movement or chapter["id"] == args.movement]
    write_json(args.output / "import-summary.json", {"archive_revision": revision, "movements": reports})


if __name__ == "__main__":
    main()
