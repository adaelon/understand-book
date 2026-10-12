"""Compare fixed TB1/TB2 requests and delivery facts, without grading learners."""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent
BASE = ROOT.parent / "tutor-building-blocks-tb1"


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def compact_size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


def guidance(request):
    return "\n".join(m["content"] for m in request["input"]
                     if m["role"] == "System" and m["content"].startswith("sampling_guidance.v1"))


def main():
    after = pathlib.Path(sys.argv[1]).resolve()
    paired_before = pathlib.Path(sys.argv[2]).resolve() if len(sys.argv) > 2 else None
    current = read(after / "summary.json")
    rows = []
    for case in current["cases"]:
        name = case["case"]
        before = paired_before or BASE / ("direct-structured" if name == "direct-explanation" else "baseline")
        old = next(c for c in read(before / "summary.json")["cases"] if c["case"] == name)
        old_dir, new_dir = before / name, after / name
        assert read(old_dir / "input.json") == read(new_dir / "input.json")
        old_plan, new_plan = read(old_dir / "request-00.json"), read(new_dir / "request-00.json")
        assert "Tutor mission:" not in guidance(old_plan), "before must contain the old teaching policy"
        assert old_plan["tools"] == new_plan["tools"], name
        assert read(old_dir / "wire-request-00.json")["tools"] == read(new_dir / "wire-request-00.json")["tools"], name
        sampled = 0
        for path in sorted(new_dir.glob("request-*.json")):
            plan = read(path)
            selected = [a["asset_id"] for a in plan["instruction_assets"]]
            body = guidance(plan)
            assert "resident-agent.tutor.presentation" not in selected
            assert "resident-agent.tutor.assessment" not in selected
            if "resident-agent.tutor.select" in selected:
                assert body.count("Tutor mission:") == 1
                assert body.count("Select one teaching move:") == 1
                assert "deliver the whole requested content" in body
                assert "Stop at the user response boundary" in body
                assert "supply the concepts and rules needed to start" in body
                sampled += 1
        native_samples = 0
        for path in sorted(new_dir.glob("wire-request-*.json")):
            wire = read(path)
            body = "\n".join(m.get("content") or "" for m in wire["messages"] if m["role"] == "system")
            if "Select one teaching move:" in body:
                assert body.count("Tutor mission:") == 1
                assert body.count("Select one teaching move:") == 1
                native_samples += 1
        assert sampled and native_samples
        result = read(new_dir / "result.json")
        outcome = result["turn"].get("outcome") or {}
        rows.append({"case": name, "same_input_and_initial_tools": True,
                     "sampled_teaching_requests": sampled, "native_teaching_requests": native_samples,
                     "before": {k: old[k] for k in ["status", "incomplete", "delivery_count", "http_calls", "known_total_tokens", "elapsed_ms"]},
                     "after": {k: case[k] for k in ["status", "incomplete", "delivery_count", "http_calls", "known_total_tokens", "elapsed_ms"]},
                     "before_tool_errors": [e["result"] for e in old["tool_errors"]],
                     "after_tool_errors": [e["result"] for e in case["tool_errors"]],
                     "initial_guidance_utf8_bytes": [len(guidance(old_plan).encode()), len(guidance(new_plan).encode())],
                     "initial_tool_utf8_bytes": [compact_size(old_plan["tools"]), compact_size(new_plan["tools"])],
                     "initial_estimated_input_tokens": [old_plan["active_context"]["estimated_input_tokens"], new_plan["active_context"]["estimated_input_tokens"]],
                     "intent": [m["interaction_intent"] for m in case["moves"]],
                     "origin": [m["intent_origin"] for m in case["moves"]]})
        rows[-1]["delivered_source_count"] = len((outcome.get("answer_view") or {}).get("sources", []))
        rows[-1]["raw_source_marker_count"] = (outcome.get("answer") or "").count("[source_ref_")
    output = {"cases": rows, "human_readers": 0, "monetary_cost": None}
    (after / "comparison.json").write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(output, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
