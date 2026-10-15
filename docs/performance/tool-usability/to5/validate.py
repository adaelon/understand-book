"""Check final advertised inputs and real calls against JSON Schema Draft 7."""
import copy
import json
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


before, after = load("before.json"), load("after.json")
assert before["tasks"] == after["tasks"]
validators = {}
sizes = {}
example_count = 0
invalid_count = 0
for i, (old, spec) in enumerate(zip(before["specs"], after["specs"], strict=True)):
    name = spec["name"]
    assert old["name"] == name
    schema = spec["parameters"]
    unchanged = copy.deepcopy(schema)
    examples = unchanged.pop("examples")
    assert unchanged == old["parameters"], f"Changed input constraints: {name}"
    Draft7Validator.check_schema(schema)
    validator = validators[name] = Draft7Validator(schema)
    assert len(examples) == [3, 1, 2, 4][i]
    assert len({json.dumps(e, sort_keys=True) for e in examples}) == len(examples)
    for example in examples:
        validator.validate(example)
        if name == "goal.update":
            # Operation-specific fields must never be silently ignored.
            for field, value in {
                "focus": "Read", "next_move": "Explain", "open_questions": [], "items": [],
                "interpretation": "Explain", "requirements": [],
                "basis_turn_id": "t1", "basis_quote": "quote",
            }.items():
                allowed = {
                    "working": {"focus", "next_move", "open_questions", "items"},
                    "refine": {"interpretation", "requirements"},
                    "revise": {"interpretation", "requirements", "basis_turn_id", "basis_quote"},
                }[example["operation"]]
                if field not in allowed:
                    assert not validator.is_valid({**example, field: value})
                    invalid_count += 1
            branch = next(b for b in schema["anyOf"] if b["properties"]["operation"]["enum"] == [example["operation"]])
            for field in branch["required"]:
                missing = copy.deepcopy(example)
                missing.pop(field)
                assert not validator.is_valid(missing)
                invalid_count += 1
    assert after["native"]["tools"][i]["function"]["parameters"] == schema
    example_count += len(examples)
    sizes[name] = {"before": size(old), "after": size(spec)}

assert not validators["memory.save"].is_valid({"type": "note", "anchor_lid": "1.1", "content": "note"})
invalid_count += 1
calls = []
for case in load("reader.json"):
    for _ in case["results"]:
        calls.append({"tool": case["tool"], "input": case["input"]})
calls.extend(load("memory.json")["calls"])
goal = load("goal.json")
calls.extend({"tool": "goal.update", **call} for call in goal["calls"])
for call in calls:
    validators[call["tool"]].validate(call["input"])
for rejection in goal["rejected"]:
    # Shape-valid calls still need an actual current-turn basis at execution.
    validators["goal.update"].validate(rejection["input"])
validators["goal.update"].validate(goal["unchanged_domain_update"]["input"])
assert goal["unchanged_domain_update"]["changed"] is False

report = {
    "tools": len(validators),
    "advertised_examples": example_count,
    "schema_rejected_invalid_inputs": invalid_count,
    "executed_successful_calls": len(calls),
    "goal_basis_rejections": len(goal["rejected"]),
    "unchanged_goal_domain_update": True,
    "input_constraints_unchanged": True,
    "tool_definition_bytes": sizes,
    "total_definition_bytes": {
        "before": sum(s["before"] for s in sizes.values()),
        "after": sum(s["after"] for s in sizes.values()),
    },
    "model_sampling": False,
}
(ROOT / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
