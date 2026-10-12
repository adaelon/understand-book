"""Validate advertised TO6 examples, captured calls and unchanged input constraints."""
import copy
import json
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def constraints(value):
    if isinstance(value, dict):
        return {k: constraints(v) for k, v in value.items() if k not in {"description", "examples"}}
    if isinstance(value, list):
        return [constraints(v) for v in value]
    return value


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


before = load("before/normal.json")
after = load("after/normal.json")
baseline_calls = [c for c in before["calls"] if not c["result"]["isError"]]
baseline_calls += load("before/dsh.json")["calls"]
validators, sizes = {}, {}
examples = rejected = successful = semantic_rejections = protocol_rejections = 0
for old, spec, count in zip(before["specs"], after["specs"], [1, 2, 1, 1], strict=True):
    name = spec["name"]
    assert old["name"] == name
    expected_annotations = {**old["annotations"], "readOnlyHint": False}
    assert spec["annotations"] == expected_annotations
    assert constraints(old["inputSchema"]) == constraints(spec["inputSchema"]), name
    schema = spec["inputSchema"]
    Draft7Validator.check_schema(schema)
    validator = validators[name] = Draft7Validator(schema)
    assert len(schema["examples"]) == count
    assert len({json.dumps(e, sort_keys=True) for e in schema["examples"]}) == count
    for example in schema["examples"]:
        validator.validate(example)
        assert any(c["name"] == name and c["arguments"] == example for c in baseline_calls), name
        examples += 1
        invalid = [{**example, "version": "wrong.v0"}, {**example, "candidate_path": "candidate.json"}]
        for field in schema["required"]:
            missing = copy.deepcopy(example)
            del missing[field]
            invalid.append(missing)
        for field in example:
            if field.endswith("_ref"):
                invalid.append({**example, field: "wrong-reference"})
            if field.endswith("_ordinal"):
                invalid.extend([{**example, field: -1}, {**example, field: 0.5}])
        for value in invalid:
            assert not validator.is_valid(value), (name, value)
            rejected += 1
    sizes[name] = {"before": size(old), "after": size(spec)}

for scenario in ["normal", "recovery", "dsh"]:
    capture = load(f"after/{scenario}.json")
    assert capture["fixture"] == load(f"before/{scenario}.json")["fixture"]
    assert capture["specs"] == after["specs"]
    for call in capture["calls"]:
        validator = validators[call["name"]]
        validator.validate(call["arguments"])
        if call.get("result", {}).get("isError"):
            semantic_rejections += 1
        else:
            successful += 1

stdio = load("after/stdio.json")["exchanges"]
assert next(e["response"]["result"]["tools"] for e in stdio if e["request"]["method"] == "tools/list") == after["specs"]
for exchange in stdio:
    request, result = exchange["request"], exchange["response"]["result"]
    if request["method"] != "tools/call":
        continue
    params = request["params"]
    if result["isError"]:
        assert not validators[params["name"]].is_valid(params["arguments"])
        protocol_rejections += 1
    else:
        validators[params["name"]].validate(params["arguments"])
        successful += 1

report = {
    "tools": len(validators), "advertised_examples": examples,
    "example_provenance": "exact successful before/normal.json and before/dsh.json requests",
    "schema_rejected_invalid_inputs": rejected, "executed_successful_calls": successful,
    "session_rejections_with_valid_shape": semantic_rejections,
    "stdio_protocol_rejections": protocol_rejections,
    "input_constraints_unchanged": True,
    "annotation_correction": {"executor.input.next.readOnlyHint": {"before": True, "after": False}},
    "tool_definition_bytes": sizes,
    "total_definition_bytes": {phase: sum(s[phase] for s in sizes.values()) for phase in ["before", "after"]},
    "model_sampling": False,
}
(ROOT / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
