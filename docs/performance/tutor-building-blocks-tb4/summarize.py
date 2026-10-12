"""Check the handoff chain and summarize captured evidence; no model calls."""
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parent
repo = root.parents[2]


def read(name):
    return json.loads((root / name).read_text(encoding="utf-8-sig"))


original = read("sol/author/output.json")
artifact = read("artifact.json")
repair = read("page-repair.json")
expected = json.loads(json.dumps(original))
expected["write"]["html"] = expected["write"]["html"].replace(repair["old_text"], repair["new_text"])
style_repair = repair["style_repair"]
expected["write"]["html"] = expected["write"]["html"].replace(style_repair["old_text"], style_repair["new_text"])
assert expected == artifact, "Unexpected changes to model teaching content"
assert read("content-original.json")["state_restore"]["passed"] is False
content = read("content-final.json")
assert content["state_restore"]["passed"] is True
assert len(content["cases"]) == 4
receipts = read("replay/receipts.json")
version = read("replay/delivered-version.json")
assert receipts["deliver"]["reference"] == version["reference"]
assert read("replay/accepted-preview.json")["candidate_id"] == version["candidate_id"]
previews = [v for k, v in receipts.items() if k.startswith("preview-")]
assert len(previews) == 5 and all(not v["errors"] for v in previews)
assert {v["environment_name"] for v in previews} == {"narrow-content", "short-content", "desktop-content"}
author_result = read("replay/author-result.json")
assert author_result["status"] == 200
author_outcome = json.loads(author_result["body"])
assert not author_outcome["incomplete"]
browser = read("browser/checks.json")
current = read("browser/followup-context.json")
scene = current["scene"]
assert browser["reference"] == scene["receipt"]["reference"] == version["reference"]
assert browser["session_id"] == scene["receipt"]["session_id"] == current["context"]["binding"]["chat_session_id"]
assert current["prior_delivery"]["payload"]["presentation"]["reference"] == version["reference"]
assert scene["state"]["values"]["page"] == {
    "leftCopy": True, "leftMutable": False, "leftRevealed": True,
    "rightCopy": False, "rightMutable": False, "rightRevealed": True,
}
assert scene["state"]["observed_result"].count("E0594") == 2
assert scene["receipt"]["saved_state_ref"] in current["agent_message"]
assert not browser["errors"] and browser["reloaded_conditions_and_result"] and browser["original_chat_continued"]
assert browser["source_labels_readable"] and browser["source_opened"]
follow = read("browser/followup-result.json")
assert not follow["outcome"]["incomplete"]
evidence = follow["evidence"]
assert evidence["nature"] == "hypothesis" and evidence["status"] == "unassessed"
assert evidence["assessment_ref"] is None
scene_label = f'revision {scene["receipt"]["reference"]["revision"]}、state_revision {scene["receipt"]["state_revision"]}'
assert scene_label in evidence["interpretation"]
assert evidence["learner_quote"] in current["context"]["current_request"]
delivery = [f for f in follow["facts"] if f["kind"] == "message_delivered"]
assert len(delivery) == 1
assert evidence["evidence_id"] in delivery[0]["payload"]["move"]["interpretation_refs"]
assert not [f for f in follow["facts"] if f["kind"] == "response_assessed"]
follow_raw = read("sol/followup/output.json")
follow_corrected = read("followup-delivery.json")
ref_repair = read("followup-repair.json")
follow_expected = json.loads(json.dumps(follow_raw))
follow_expected["select"]["move"]["interpretation_refs"].append(ref_repair["added"])
assert follow_corrected == follow_expected
assert ref_repair["added"] == evidence["evidence_id"]

before = (root.parent / "tutor-building-blocks-tb3/sol/after/tutor.rs").read_text(encoding="utf-8")
after = (repo / "crates/runtime/src/tutor.rs").read_text(encoding="utf-8")
delta = {}
for phase in ("Global", "Local", "Review"):
    pattern = rf'PresentationPhase::{phase} => "(.*?)",'
    old = re.search(pattern, before, re.S)[1]
    new = re.search(pattern, after, re.S)[1]
    delta[phase] = len(new.encode()) - len(old.encode())
pattern = r'const UNDERSTANDING: &str = "(.*?)";'
delta["Understanding"] = len(re.search(pattern, after, re.S)[1].encode()) - len(re.search(pattern, before, re.S)[1].encode())

summary = {
    "guidance_revision": "tb4.v1", "model": "gpt-6.1-sol", "authorized_runs": 2,
    "completed_runs": 2, "fork_turns": "none", "reasoning": "model default; override omitted",
    "token_usage": None, "monetary_cost": None, "model_elapsed_seconds": None,
    "deepseek_calls": 0, "human_readers": 0, "production_provider_loop_verified": False,
    "input_utf8_bytes": {p: (root / f"sol/{p}/input.json").stat().st_size for p in ("author", "followup")},
    "guidance_utf8_byte_delta_from_tb3": delta,
    "runtime_tests": 29, "server_related_tests": 4, "browser_scenarios": 1,
    "preview_calls_final_candidate": 5, "rust_code_cases": 4,
    "delivered_reference": version["reference"], "followup_receipt": scene["receipt"],
    "scene_receipt_rebinding": read("browser/scene-rebinding.json"),
    "author_output_repaired": repair, "followup_output_repaired": ref_repair,
    "new_interpretation": {k: evidence[k] for k in ("evidence_id", "nature", "status", "supersedes", "interpretation", "teaching_implication")},
    "observations_are_performance": False,
    "limitations": ["Fixed source reads and tool replay; no autonomous Provider loop", "Authored learner expression; no human learning outcome", "Two cases have no control group and do not establish improvement or stability"],
}
(root / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("TB4 captured code, delivery, browser scene and interpretation chain agree")
