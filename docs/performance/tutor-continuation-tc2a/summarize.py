"""Summarize recorded TC2A requests without calling a model."""
import json
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


groups = {}
tool_tables = {}
for group in ("baseline", "on-demand"):
    config = read(ROOT / group / "configuration.json")
    runs = []
    tables = []
    for directory in sorted((ROOT / group).glob("run-*")):
        result = read(directory / "result.json")
        reply = result["reply"]
        plans = [read(p) for p in sorted(directory.glob("request-*.json"))]
        # Compare cells/fields directly: neither requests nor tools need hashes.
        for plan in plans:
            if plan["tools"] not in tables:
                tables.append(plan["tools"])
        moves = [event["payload"]["move"] for event in result["events"]
                 if event["kind"] == "move_selected"]
        errors = []
        for step in reply.get("trace", []):
            if '"error_code"' in step.get("result_digest", ""):
                errors.append({"tool": step["tool"], "result": step["result_digest"]})
        (directory / "answer.md").write_text(reply.get("answer", ""), encoding="utf-8")
        save(directory / "persisted-moves.json", moves)
        runs.append({
            "run": directory.name, "http_status": result["http_status"],
            "incomplete": reply.get("incomplete"), "samples": len(plans),
            "estimated_input_tokens_per_sample": [p["active_context"]["estimated_input_tokens"] for p in plans],
            "cumulative_tokens_spent": reply.get("tokens_spent"),
            "tool_trace": [s["tool"] for s in reply.get("trace", [])],
            "tool_errors": errors, "persisted_moves": len(moves),
            "deliveries": len(result["deliveries"]),
            "initial_instruction_assets": plans[0]["instruction_assets"],
            "tool_tables": len({json.dumps(p["tools"], sort_keys=True) for p in plans}),
            "tool_free_samples": [i for i, p in enumerate(plans) if not p["tools"]],
        })
    groups[group] = {"configuration": config, "runs": runs}
    tool_tables[group] = tables

same_configuration = groups["baseline"]["configuration"] == groups["on-demand"]["configuration"]
same_tool_tables = all(t in tool_tables["on-demand"] for t in tool_tables["baseline"]) and all(
    t in tool_tables["baseline"] for t in tool_tables["on-demand"])
same_business_tables = all(t in tool_tables["on-demand"] for t in tool_tables["baseline"] if t) and all(
    t in tool_tables["baseline"] for t in tool_tables["on-demand"] if t)
assert same_configuration, "the comparison changed model, parameters, material or original answer"
assert same_business_tables, "the comparison changed business-sampling tool contracts"

counterexamples = []
for name, speeds, times in [
    ("equal_speed_unequal_time", (10, 10), (1, 2)),
    ("equal_time_unequal_speed", (10, 20), (1, 1)),
    ("original_unequal_time", (60, 30), (1, 2)),
    ("equal_speed_equal_distance", (10, 10), (1, 1)),
]:
    weighted = Fraction(sum(v*t for v, t in zip(speeds, times)), sum(times))
    arithmetic = Fraction(sum(speeds), 2)
    # For two segments with positive total time, equality is exactly this product.
    condition = (speeds[0]-speeds[1]) * (times[0]-times[1]) == 0
    assert (weighted == arithmetic) == condition
    counterexamples.append({"id": name, "speeds": speeds, "durations": times,
        "distances": [v*t for v, t in zip(speeds, times)],
        "time_weighted_mean": str(weighted), "arithmetic_mean": str(arithmetic), "equal": condition})

save(ROOT / "counterexamples.json", counterexamples)
save(ROOT / "summary.json", {"same_configuration": same_configuration, "same_sampled_tool_tables": same_tool_tables,
    "same_business_sampling_tool_tables": same_business_tables,
    "tool_table_note": "On-demand run 1 enters an extra tool-free finalization after failed selects; retain it as a failed-run outcome, with its request and cost. Normal pre-read/post-read tool tables are identical across groups.",
    "groups": groups, "counterexamples": counterexamples,
    "cost_note": "estimated_input_tokens_per_sample is a single request estimate; tokens_spent is cumulative provider usage, not context length or money."})
for group, result in groups.items():
    print(group, json.dumps(result["runs"], ensure_ascii=False))
