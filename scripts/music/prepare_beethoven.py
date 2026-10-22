"""Prepare a local-only historic recording and an attributed score excerpt.

The intact publisher files are retained. The audition derivative concatenates
both finale sections, preserving the complete movement rather than cropping it.
"""
from pathlib import Path
import copy
import json
import subprocess
import urllib.request
import xml.etree.ElementTree as E
import zipfile

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "evals/music/assets"
SCORE_PAGE = "https://fourscoreandmore.org/openscore/orchestra/"
RECORDING_PAGE = "https://music.damians78s.co.uk/2024/05/07/albert-coates-conducts-beethovens-9th-1923/"


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    score_base = "https://raw.githubusercontent.com/MarkGotham/Hauptstimme/main/data/Beethoven,_Ludwig_van/Symphony_No.9,_Op.125/4/Beethoven_Op.125_4"
    for suffix in [".mxl", "_alignment.csv", "_annotations.csv", ".mm.json", "_part_relations.csv"]:
        target = ASSETS / ("beethoven-op125-4-openscore" + suffix)
        if not target.exists():
            urllib.request.urlretrieve(score_base + suffix, target)
    with zipfile.ZipFile(ASSETS / "beethoven-op125-4-openscore.mxl") as archive:
        container = E.fromstring(archive.read("META-INF/container.xml"))
        rootfile = container.find(".//{*}rootfile").get("full-path")
        (ASSETS / "beethoven-op125-4-openscore.musicxml").write_bytes(archive.read(rootfile))
    sources = []
    for suffix in ["a", "b"]:
        path = ASSETS / f"beethoven-op125-4-coates-damian-{suffix}.flac"
        url = f"https://www.damians78s.co.uk/D842-9%20-%20Beethoven%209%20ac%20-%20Coates04{suffix}.flac"
        if not path.exists():
            urllib.request.urlretrieve(url, path)
        sources.append({"file": path.name, "url": url, "bytes": path.stat().st_size})
    recording = ASSETS / "beethoven-op125-4-coates-damian-full.mp3"
    if not recording.exists() or recording.stat().st_size == 0:
        temporary = recording.with_suffix(".tmp.mp3")
        subprocess.run(["ffmpeg", "-y", "-v", "error", "-i", str(ASSETS/sources[0]["file"]), "-i", str(ASSETS/sources[1]["file"]), "-filter_complex", "[0:a][1:a]concat=n=2:v=0:a=1[out]", "-map", "[out]", "-codec:a", "libmp3lame", "-b:a", "128k", str(temporary)], check=True)
        temporary.replace(recording)
    root = E.parse(ASSETS / "beethoven-op125-4-openscore.musicxml").getroot()
    excerpt = copy.deepcopy(root)
    for part in excerpt.findall("part"):
        effective = {}
        for m in list(part.findall("measure")):
            label = int(m.get("number"))
            if label < 93:
                for attrs in m.findall("attributes"):
                    for item in attrs:
                        effective[(item.tag, item.get("number", "1"))] = copy.deepcopy(item)
                part.remove(m)
            elif label > 164:
                part.remove(m)
        first = part.find("measure")
        attrs = first.find("attributes")
        if attrs is None:
            attrs = E.Element("attributes")
            first.insert(0, attrs)
        existing = {(x.tag, x.get("number", "1")) for x in attrs}
        for key, value in effective.items():
            if key not in existing:
                attrs.append(value)
    active_parts = {p.get("id") for p in excerpt.findall("part") if p.find(".//note/pitch") is not None or p.find(".//note/unpitched") is not None}
    for part in list(excerpt.findall("part")):
        if part.get("id") not in active_parts:
            excerpt.remove(part)
    part_list = excerpt.find("part-list")
    for item in list(part_list):
        if item.tag == "part-group" or (item.tag == "score-part" and item.get("id") not in active_parts):
            part_list.remove(item)
    # Original IDs and bar labels are retained; importer assigns its stable
    # excerpt object IDs while the curation identifies the full source revision.
    target = ASSETS / "beethoven-op125-4-m93-164.musicxml"
    E.ElementTree(excerpt).write(target, encoding="utf-8", xml_declaration=True)
    probe = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_format", "-of", "json", str(recording)]))
    metadata = {
        "date": "2026-10-22", "status": "local_only_recording; score_excerpt_ready; alignment_pending",
        "score": {"source_page": SCORE_PAGE, "repository": "https://github.com/MarkGotham/Hauptstimme", "original_mxl": "beethoven-op125-4-openscore.mxl", "original_xml": "beethoven-op125-4-openscore.musicxml", "excerpt_xml": target.name, "original_parts": 36, "excerpt_parts": sorted(active_parts), "original_measures": 943, "selected_measures": [93,164], "score_license": "CC0 1.0 Universal", "annotation_license": "CC-By-SA (version unspecified by publisher)", "license_source": "https://raw.githubusercontent.com/MarkGotham/Hauptstimme/main/README.md", "changes": "Extracted measures 93–164; omitted parts entirely silent within this excerpt and removed their grouping brackets. Carried effective clefs/key/time/transpositions/divisions to each part's first measure. Original bar labels and full source files retained."},
        "recording": {"source_page": RECORDING_PAGE, "sources": sources, "derivative": recording.name, "derivative_bytes": recording.stat().st_size, "duration_seconds": float(probe['format']['duration']), "conductor": "Albert Coates", "ensemble": "Symphony Orchestra, chorus and soloists named on transfer publisher page", "recorded": "October–November 1923", "first_published": "May 1924, HMV D 842–849; finale D 847–849", "distribution": "local_only", "rights_basis": "Original 1924 recording term has elapsed under the cited US 95+5-year rule. The 2024 transfer contains editorial repairs and has no explicit open redistribution license; commercial release of this transfer remains unapproved.", "term_source": "https://www.copyright.gov/music-modernization/pre1972-soundrecordings/", "changes": "Concatenated publisher part 1 and part 2 in order, transcoded to MP3 for local browser playback. No excerpt crop, new repair, or denoising. Publisher's own repairs remain; see source page."},
        "alignment_reference": {"file": "beethoven-op125-4-openscore_alignment.csv", "recordings": ["IMSLP729211", "IMSLP484065", "IMSLP736780"], "usable_for_current_recording": False, "reason": "These are other performances; do not transfer their seconds to Coates."},
    }
    (ROOT / "evals/music/beethoven-sources.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print(json.dumps({"score_excerpt": str(target), "recording": str(recording), "duration": metadata['recording']['duration_seconds']}))


if __name__ == "__main__":
    main()
