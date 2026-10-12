"""Summarize the fixed, three-run recovery replay without model calls."""
import json
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")
ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


previous_config = read(ROOT.parent / "on-demand/configuration.json")
current_config = read(ROOT / "runs/configuration.json")
configuration_changes = {
    key: {"previous": previous_config.get(key), "current": current_config.get(key)}
    for key in previous_config.keys() | current_config.keys()
    if previous_config.get(key) != current_config.get(key)
}
runs = []
for number in range(1, 4):
    directory = ROOT / "runs" / f"run-{number}"
    result = read(directory / "result.json")
    reply = result["reply"]
    plans = [read(path) for path in sorted(directory.glob("request-*.json"))]
    movements = [event["payload"]["move"] for event in result["events"]
                 if event["kind"] == "move_selected"]
    trace = []
    for step in reply.get("trace", []):
        try:
            output = json.loads(step["result_digest"])
        except (ValueError, KeyError):
            output = {"text": step.get("result_digest")}
        trace.append({"sample": step.get("model_tool_loop"), "tool": step["tool"], "result": output})
    errors = [step for step in trace if isinstance(step["result"], dict) and step["result"].get("error_code")]
    (directory / "answer.md").write_text(reply.get("answer", ""), encoding="utf-8")
    save(directory / "persisted-moves.json", movements)
    runs.append({
        "run": number,
        "http_status": result["http_status"],
        "turn_status": result["turn_status"],
        "incomplete": reply.get("incomplete"),
        "warning": reply.get("warning"),
        "persisted_moves": len(movements),
        "deliveries": len(result["deliveries"]),
        "model_samples": len(plans),
        "estimated_input_tokens_per_sample": [plan["active_context"]["estimated_input_tokens"] for plan in plans],
        "cumulative_tokens_spent": reply.get("tokens_spent"),
        "tool_free_samples": [index for index, plan in enumerate(plans) if not plan["tools"]],
        "tool_errors": errors,
        "trace": trace,
        "initial_instruction_assets": plans[0]["instruction_assets"],
    })

old_request = read(ROOT.parent / "on-demand/run-1/request-00.json")
new_request = read(ROOT / "runs/run-1/request-00.json")
old_tools = {tool["name"]: tool for tool in old_request["tools"]}
new_tools = {tool["name"]: tool for tool in new_request["tools"]}
changed_tools = [name for name in sorted(old_tools.keys() | new_tools.keys())
                 if old_tools.get(name) != new_tools.get(name)]
summary = {
    "same_model_and_fixed_input_configuration": not configuration_changes,
    "configuration_changes": configuration_changes,
    "first_request_changed_tool_definitions": changed_tools,
    "comparison_scope": "Post-fix replay, not a one-variable instruction-assembly comparison: current repair also changes the teaching source-binding input contract.",
    "runs": runs,
    "completed_teaching_runs": sum(run["incomplete"] is False and run["persisted_moves"] > 0 and run["deliveries"] > 0 for run in runs),
    "total_runs": len(runs),
    "cumulative_tokens_spent": sum(run["cumulative_tokens_spent"] or 0 for run in runs),
    "content_review": "See content-findings.json and README.md; successful delivery is not a content-quality verdict.",
}
save(ROOT / "summary.json", summary)
print(json.dumps({key: value for key, value in summary.items() if key != "runs"}, ensure_ascii=False, indent=2))
for run in runs:
    print(json.dumps({key: value for key, value in run.items() if key not in ("trace", "initial_instruction_assets")}, ensure_ascii=False))
