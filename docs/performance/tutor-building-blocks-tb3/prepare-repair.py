"""Freeze an authorized repair using the original output and real tool errors."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "sol"
phase = sys.argv[1]
assert phase in {"response", "reopen"}
base, variant, cases = ("after", "repair", ["support-mismatch", "independent-step"]) if phase == "response" else (
    "reopen", "reopen-repair", ["cannot-start", "independent-step"])
for case in cases:
    prior = ROOT / base / case
    packet = json.loads((prior / "input.json").read_text(encoding="utf-8-sig"))
    packet["previous_output"] = json.loads((prior / "output.json").read_text(encoding="utf-8-sig"))
    replay = json.loads((prior / "replay.json").read_text(encoding="utf-8-sig"))
    errors = []
    for request in replay["requests"]:
        for message in request["input"]:
            if message["role"].lower() == "tool":
                result = json.loads(message["content"])
                if result.get("status") == "error" and result["model_body"] not in errors:
                    errors.append(result["model_body"])
    assert errors, case
    packet["actual_tool_rejections"] = errors
    packet["task"] = "Repair the previous_output using the actual tool rejection and the supplied contract. Preserve the teaching content and judgment; correct only invalid tool argument fields and their required dependent fields. Return the complete JSON with evidence, select, answer. Do not simulate a learner reply. The original failed output is retained separately. This is one authorized repair attempt."
    dest = ROOT / variant / case
    dest.mkdir(parents=True, exist_ok=True)
    (dest / "input.json").write_text(json.dumps(packet, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
(ROOT / variant / "tutor.rs").write_bytes((ROOT / base / "tutor.rs").read_bytes())
