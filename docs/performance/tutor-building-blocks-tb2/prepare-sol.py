"""Freeze teaching-only packets for isolated Codex subagents; no Provider calls."""
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
TB1 = HERE.parent / "tutor-building-blocks-tb1"
OUT = HERE / "sol"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def prepare(variant, source_path):
    source = source_path.read_text(encoding="utf-8")
    guidance = []
    for name in ["COMMON", "DISCOVERY", "SELECT", "CONTINUATION", "UNDERSTANDING"]:
        match = re.search(rf'const {name}: &str = "(.*?)";', source, re.S)
        assert match, name
        guidance.append({"name": name, "text": match[1]})
    recorded = read_json(HERE / "to3-after/starting-ownership/request-06.json")
    calls = {c["id"]: c for m in recorded["input"] for c in m.get("tool_calls", [])}
    references = []
    for message in recorded["input"]:
        call = calls.get(message.get("tool_call_id"))
        if call and call["name"] == "source.present":
            body = json.loads(message["content"])["model_body"]
            quote = json.loads(call["arguments"])["quote"]
            references.append({"quote": quote, "source_ref_id": body["source_ref_id"]})
    assert references
    scenarios = read_json(TB1 / "scenarios.json")
    for case in scenarios["cases"]:
        if variant == "prerequisite-repair" and case["id"] != "starting-assignment":
            continue
        packet = {
            "protocol": "tb2.sol.teaching-only.v1",
            "task": "Act as the Tutor for this single turn. Original text and source.present receipts have already been supplied below. Produce one tutor.step select argument object and the actual final teaching answer. Return JSON with keys select and answer. Do not simulate a learner reply. Do not call tools or providers for teaching; the surrounding executor only saves your output.",
            "guidance": guidance,
            "tool_contract": next(t for t in recorded["tools"] if t["name"] == "tutor.step"),
            "citation_contract": "Use only returned [[source:<source_ref_id>]] markers after the supported sentence, with the exact ID from presented_sources.",
            "context": {"status": "active", "default_teaching_intent": "guided_inquiry", "formal_objects": [], "interpretations": [], "prior_learner_evidence": [], "source_lids": {"start_lid": "1.1", "end_lid": "1.1"}},
            "original_text": (TB1 / "source.txt").read_text(encoding="utf-8"),
            "presented_sources": references,
            "user_request": case["background"] + "\n" + case["request"],
        }
        save(OUT / variant / case["id"] / "input.json", packet)
    (OUT / variant / "tutor.rs").write_text(source, encoding="utf-8")


if __name__ == "__main__":
    import sys
    variant = sys.argv[1]
    assert variant in {"before", "after", "prerequisite-repair"}
    prepare(variant, HERE / "to3-baseline-tutor.rs" if variant == "before" else ROOT / "crates/runtime/src/tutor.rs")
