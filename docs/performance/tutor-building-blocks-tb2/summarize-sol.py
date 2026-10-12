"""Check paired inputs, intent and citations; retain content observations separately."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "sol"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


rows = []
for folder in sorted(ROOT.glob("*/*/input.json")):
    variant, case = folder.parent.parent.name, folder.parent.name
    packet = read(folder)
    output = read(folder.with_name("output.json"))
    replay = read(folder.with_name("replay.json"))
    baseline = read(ROOT / "before" / case / "input.json")
    assert {k: v for k, v in packet.items() if k != "guidance"} == {
        k: v for k, v in baseline.items() if k != "guidance"
    }, f"unpaired input: {variant}/{case}"
    assert all(s["quote"] in packet["original_text"] for s in packet["presented_sources"])
    if variant == "prerequisite-repair":
        prior = read(ROOT / "after" / case / "input.json")
        assert [g for g in packet["guidance"] if g["name"] != "SELECT"] == [
            g for g in prior["guidance"] if g["name"] != "SELECT"
        ]
    move = output["select"]["move"]
    answer = output["answer"]
    expected_intent = "direct_explanation" if case == "direct-explanation" else "guided_inquiry"
    if variant != "before":
        assert move["interaction_intent"] == expected_intent
    assert move["intent_quote"] in packet["user_request"]
    ids = set(re.findall(r"\[\[source:([^\]]+)\]\]", answer))
    assert ids and ids <= {s["source_ref_id"] for s in packet["presented_sources"]}
    assert "source_ref_" not in re.sub(r"\[\[source:[^\]]+\]\]", "", answer)
    outcome = replay["outcome"]
    assert replay["http_status"] == 200 and not outcome["incomplete"]
    assert len(outcome["answer_view"]["sources"]) == len(ids)
    folder.with_name("answer.md").write_text(answer + "\n", encoding="utf-8")
    rows.append({"variant": variant, "case": case, "paired_input": True,
                 "intent": move["interaction_intent"], "intent_matches_request": move["interaction_intent"] == expected_intent,
                 "source_count": len(ids), "input_utf8_bytes": len(folder.read_bytes()),
                 "replay_delivered": True, "learner_response": False})
assert len(rows) == 9
summary = {"model": "gpt-6.1-sol", "reasoning": "model default (spawn override omitted for every agent)",
           "fork_turns": "none", "sampling": "four cases once per version; one prerequisite repair sample",
           "runs": 9, "deepseek_calls": 0, "codex_token_usage": None, "monetary_cost": None,
           "temperature": "not exposed by subagent tool", "human_readers": 0, "cases": rows}
(ROOT / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"{len(rows)} paired outputs and real delivery replays checked; no DeepSeek calls")
