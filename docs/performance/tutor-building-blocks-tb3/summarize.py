"""Check paired inputs and report actual frozen-output delivery, without grading prose."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def changed_fields(before, after, prefix=""):
    if isinstance(before, dict) and isinstance(after, dict):
        return [path for key in sorted(before.keys() | after.keys())
                for path in changed_fields(before.get(key), after.get(key), f"{prefix}.{key}".lstrip("."))]
    return [prefix] if before != after else []


def main():
    rows = []
    for case in read(HERE / "scenarios.json")["cases"]:
        cid = case["id"]
        before = read(HERE / "sol/before" / cid / "input.json")
        after = read(HERE / "sol/after" / cid / "input.json")
        before.pop("guidance")
        after.pop("guidance")
        assert before == after, f"unpaired input: {cid}"
    for path in sorted((HERE / "sol").glob("*/*/output.json")):
        variant, cid = path.parent.parent.name, path.parent.name
        output = read(path)
        packet = read(path.with_name("input.json"))
        path.with_name("answer.md").write_text(output["answer"] + "\n", encoding="utf-8")
        if variant.endswith("repair"):
            assert output["answer"] == packet["previous_output"]["answer"], f"repair changed answer: {cid}"
            assert output["select"]["move"]["prompt"] == packet["previous_output"]["select"]["move"]["prompt"]
            if output.get("evidence"):
                for field in ["interpretation", "teaching_implication", "target"]:
                    assert output["evidence"][field] == packet["previous_output"]["evidence"][field], (cid, field)
                prior_refs = packet["previous_output"]["evidence"]["fact_refs"]
                if variant == "repair" and cid == "support-mismatch":
                    # This observed repair also dropped an optional delivery reference.
                    # Preserve that deviation in the report, without calling it a required fix.
                    assert len(prior_refs) == 2 and prior_refs[1].startswith("delivery:")
                    assert output["evidence"]["fact_refs"] == prior_refs[:1]
                else:
                    assert output["evidence"]["fact_refs"] == prior_refs
        replay_path = path.with_name("replay.json")
        replay = read(replay_path) if replay_path.exists() else {}
        errors = []
        for request in replay.get("requests", []):
            for message in request["input"]:
                if message["role"].lower() == "tool":
                    result = json.loads(message["content"])
                    if result.get("status") == "error" and result["model_body"] not in errors:
                        errors.append(result["model_body"])
        deliveries = [f for f in replay.get("facts", []) if f["kind"] == "message_delivered"]
        row = {"variant": variant, "case": cid, "input_utf8_bytes": path.with_name("input.json").stat().st_size,
               "intent_origin": output["select"]["move"].get("intent_origin"),
               "supersedes": (output.get("evidence") or {}).get("supersedes"),
               "delivered": len(deliveries) == 1 and not replay.get("outcome", {}).get("incomplete", True),
               "tool_errors": errors, "replay_present": bool(replay),
               "new_hypothesis": output.get("evidence") is not None}
        if variant.endswith("repair"):
            row["repair_changed_fields"] = changed_fields(packet["previous_output"], output)
        if replay:
            row["source_count"] = len(replay["outcome"].get("answer_view", {}).get("sources", [])) if replay.get("outcome") else 0
        rows.append(row)
    if "--final" in sys.argv:
        assert len(rows) == 16
        for stage in ["response", "reopen"]:
            for case in [c["id"] for c in read(HERE / "scenarios.json")["cases"]]:
                variant = ("repair" if case in {"support-mismatch","independent-step"} else "after") if stage == "response" else (
                    "reopen-repair" if case in {"cannot-start","independent-step"} else "reopen")
                row = next(r for r in rows if r["variant"] == variant and r["case"] == case)
                assert row["delivered"] and not row["tool_errors"], row
                if stage == "reopen":
                    assert not row["new_hypothesis"], row
    (HERE / "summary.json").write_text(json.dumps({"model":"gpt-6.1-sol","fork_turns":"none",
        "reasoning":"model default; override omitted", "temperature":"not exposed",
        "runs":len(rows), "deepseek_calls":0,"human_readers":0,"token_usage":None,"monetary_cost":None,"model_elapsed_seconds":None,
        "paired_inputs":True,"cases":rows},ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(rows,ensure_ascii=False,indent=2))


if __name__ == "__main__":
    main()
