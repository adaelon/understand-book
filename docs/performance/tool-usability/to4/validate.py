"""Validate captured advertised examples and executed calls with JSON Schema Draft 7."""
import copy
import json
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


before, after = load("before.json"), load("after.json")
schema = after["spec"]["parameters"]
Draft7Validator.check_schema(schema)
validator = Draft7Validator(schema)
examples = schema["examples"]
assert len(examples) == 14
assert len({json.dumps(e, sort_keys=True) for e in examples}) == len(examples)
assert {e["operation"] for e in examples} == {
    "prepare", "render_plot", "render_animation", "read", "search", "patch", "write", "preview", "deliver"
}
for example in examples:
    validator.validate(example)
assert after["native"]["tools"][0]["function"]["parameters"] == schema
unchanged = copy.deepcopy(schema)
del unchanged["examples"]
assert unchanged == before["spec"]["parameters"], "TO4 must preserve the existing input constraints"

invalid = []
for operation in ["read", "search", "patch"]:
    example = copy.deepcopy(next(e for e in examples if e["operation"] == operation and "reference" in e))
    example["candidate_id"] = "a-current-candidate"
    invalid.append(example)
    example = copy.deepcopy(example)
    del example["candidate_id"], example["reference"]
    invalid.append(example)
write = next(e for e in examples if e["operation"] == "write" and "based_on" in e)
missing_readable = copy.deepcopy(write)
del missing_readable["readable_content"]
invalid.extend([missing_readable, {**write, "new_object": True}, {**write, "based_on": "candidate-id"}])
preview = next(e for e in examples if e["operation"] == "preview" and "viewport" in e)
invalid.append({**preview, "width": 960})
patch = next(e for e in examples if e["operation"] == "patch")
invalid.append({**patch, "source_ref_ids": ["observed-source"]})
for example in invalid:
    assert not validator.is_valid(example), example

chain = load("chain.json")
animation = load("animation.json")
assert chain["spec"] == after["spec"]
calls = chain["calls"] + animation
for call in calls:
    validator.validate(call["input"])
assert {c["input"]["operation"] for c in calls} == {
    "render_plot", "render_animation", "read", "search", "patch", "write", "preview", "deliver"
}


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


report = {
    "operations": 9,
    "advertised_examples": len(examples),
    "rejected_invalid_inputs": len(invalid),
    "executed_host_calls": len(calls),
    "prepare": "real Runtime loop; presentation_to4_prepare_example_applies_at_next_sampling",
    "input_constraints_unchanged": True,
    "parameters_bytes": {"before": size(before["spec"]["parameters"]), "after": size(schema)},
    "tool_definition_bytes": {"before": size(before["spec"]), "after": size(after["spec"])},
    "model_sampling": False,
}
(ROOT / "validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
