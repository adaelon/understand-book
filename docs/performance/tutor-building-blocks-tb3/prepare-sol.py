"""Prepare isolated TB3 content experiments from real local context exports."""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def main(variant):
    source = (HERE / "before-tutor.rs" if variant == "before" else ROOT / "crates/runtime/src/tutor.rs").read_text(encoding="utf-8")
    guidance = [{"name": name, "text": re.search(rf'const {name}: &str = "(.*?)";', source, re.S)[1]}
                for name in ["COMMON", "DISCOVERY", "SELECT", "CONTINUATION", "UNDERSTANDING"]]
    tool = read(HERE.parent / "tutor-building-blocks-tb2/sol/after/starting-ownership/input.json")["tool_contract"]
    for case in read(HERE / "scenarios.json")["cases"]:
        context = read(HERE / "contexts" / case["id"] / ("reopen.json" if variant == "reopen" else "response.json"))
        packet = {
            "protocol": "tb3.sol.v1",
            "task": "Act as Tutor for this one turn. The original text, source receipts, current context and reread facts below have already been obtained. Return JSON with evidence (one tutor.step evidence argument object, or null if no warranted update), select (one tutor.step select argument object), and answer (actual final teaching text in Chinese). An evidence call returns evidence_ref=interpretation:<operation_id>, which can be adopted in select. Use only actual fact IDs from this packet. Do not simulate any later learner answer. Do not call external providers or change files other than the designated output.",
            "guidance": guidance,
            "tool_contract": tool,
            "citation_contract": "Use returned [[source:<source_ref_id>]] markers only for supported claims, using presented_sources.",
            "original_text": (HERE.parent / "tutor-building-blocks-tb1/source.txt").read_text(encoding="utf-8"),
            "user_request": case["reopen_request"] if variant == "reopen" else case["response"],
            **context,
        }
        path = HERE / "sol" / variant / case["id"] / "input.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(packet, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (HERE / "sol" / variant / "tutor.rs").write_text(source, encoding="utf-8")


if __name__ == "__main__":
    assert sys.argv[1] in {"before", "after", "reopen"}
    main(sys.argv[1])
