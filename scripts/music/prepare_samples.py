"""Download the two explicitly selected, openly licensed Schubert source files.

Run from the repository root. Existing files are reused; this is sample preparation,
not a production importer or evidence of score/audio alignment.
"""
from pathlib import Path
import json
import subprocess
import urllib.request
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "evals/music"
ASSETS = OUT / "assets"
SOURCES = [
    {
        "id": "schubert-d780-3-vetter",
        "kind": "human_recording",
        "file": "schubert-d780-3-vetter.mp3",
        "url": "https://ks15.imslp.org/files/imglnks/usimg/4/41/IMSLP456646-PMLP02066-Schubert_Moment_musical_Nr3_f-Moll_Op94_D780.mp3",
        "source_page": "https://imslp.org/wiki/Moments_musicaux,_D.780_(Schubert,_Franz)",
        "source_record": "IMSLP456646; uploaded 2017-01-22 by Harryplocker",
        "creator": "Harald Vetter (piano, recording publisher)",
        "license": "CC-BY-4.0",
        "license_url": "https://creativecommons.org/licenses/by/4.0/",
        "license_basis": "The official IMSLP work page lists No.3 in the Harald Vetter recording group with Creative Commons Attribution 4.0.",
        "attribution": "Franz Schubert, Moments musicaux D.780 No.3; piano: Harald Vetter; source: IMSLP #456646; CC BY 4.0. Original recording unchanged.",
    },
    {
        "id": "schubert-d780-3-noctave",
        "kind": "structured_score",
        "file": "schubert-d780-3-noctave.mxl",
        "url": "https://noctavepiano.com/partitions/schubert-moment-musical-d780-3/schubert-moment-musical-d780-3.mxl",
        "source_page": "https://noctavepiano.com/partitions/schubert-moment-musical-d780-3/",
        "source_record": "Noctave edition; derived from Mutopia-2007/08/11-1023",
        "creator": "Franz Schubert; original typesetting Claude Chaudron; MusicXML conversion Noctave",
        "license": "CC0-1.0",
        "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
        "license_basis": "The publisher explicitly dedicates its downloadable MusicXML and PDF to CC0 and identifies the public-domain Mutopia typesetting source.",
    },
]


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    for source in SOURCES:
        target = ASSETS / source["file"]
        if not target.exists():
            with urllib.request.urlopen(source["url"], timeout=60) as response:
                target.write_bytes(response.read())
        source["local_path"] = target.relative_to(ROOT).as_posix()
        source["bytes"] = target.stat().st_size
        print(source["local_path"], source["bytes"])
    compressed = ASSETS / SOURCES[1]["file"]
    with zipfile.ZipFile(compressed) as archive:
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = next(x for x in container.iter() if x.tag.endswith("rootfile"))
        score_bytes = archive.read(rootfile.attrib["full-path"])
    score_path = ASSETS / "schubert-d780-3-noctave.musicxml"
    score_path.write_bytes(score_bytes)
    score = ET.fromstring(score_bytes)
    stats = {
        "root": score.tag,
        "musicxml_version": score.get("version"),
        "rights": [x.text for x in score.findall("./identification/rights")],
        "parts": [
            {
                "id": part.get("id"),
                "measures": len(part.findall("measure")),
                "notes": len(part.findall(".//note")),
                "pitched_notes": len(part.findall(".//note/pitch")),
                "grace_notes": len(part.findall(".//note/grace")),
                "voices": sorted({x.text for x in part.findall(".//voice")}),
                "staves": sorted({x.text for x in part.findall(".//staff")}),
                "repeat_barlines": [
                    {"measure": m.get("number"), **repeat.attrib}
                    for m in part.findall("measure") for repeat in m.findall("./barline/repeat")
                ],
            } for part in score.findall("part")
        ],
    }
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", str(ASSETS / SOURCES[0]["file"])],
        check=True, capture_output=True, text=True, encoding="utf-8",
    )
    manifest = {
        "date": "2026-10-22",
        "work": "Franz Schubert, Moments musicaux D.780 / Op.94, No.3 in F minor",
        "status": "downloaded; score/recording compatibility and alignment not yet accepted",
        "sources": SOURCES,
        "extracted_musicxml": score_path.relative_to(ROOT).as_posix(),
        "score_stats": stats,
        "recording_probe": json.loads(probe.stdout),
    }
    (OUT / "samples.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(stats, ensure_ascii=True))


if __name__ == "__main__":
    main()
