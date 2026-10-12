"""Check final discovery inputs/results and report serialized input cost."""
import json
import math
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def read(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def encode(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def size(value):
    return len(encode(value).encode("utf-8"))


def estimate(value):
    # Runtime model_runtime::estimate_text_tokens, not provider billed usage.
    return math.ceil(sum(1 if ord(c) >= 0x2e80 else .25 for c in encode(value)))


before, after = read("before.json"), read("after.json")
schema = after["spec"]["parameters"]
Draft7Validator.check_schema(schema)
validator = Draft7Validator(schema)
assert len(schema["examples"]) == 4
assert {k: v for k, v in schema.items() if k not in ("examples", "description")} == before["spec"]["parameters"]
for example in schema["examples"]:
    validator.validate(example)

rows = []
for name, sample in after["scenarios"].items():
    old = before["scenarios"][name]
    assert old["input"] == sample["input"], name
    validator.validate(sample["input"])
    result = sample["result"]
    assert sample["delivered"] == result, name
    for field in ("task_need", "capability_plan", "request_audit"):
        assert field not in result, name
    previous_shape = {k: v for k, v in result.items() if k not in ("blocked", "unmet_capabilities")}
    previous_shape["matches"] = [
        {k: v for k, v in match.items() if k not in ("use_when", "avoid_when")}
        for match in result["matches"]
    ]
    assert previous_shape == old["result"], name
    for block in result["blocked"]:
        assert "tool" not in block and "precondition" not in block
        for recovery in block.get("next_searches", []):
            validator.validate(recovery)
            assert recovery["effect_mode"] == "read_only"
    rows.append(dict(scenario=name, before_bytes=size(old["result"]),
                     after_bytes=size(result), before_estimated_tokens=estimate(old["result"]),
                     after_estimated_tokens=estimate(result)))

requests = []
for name in ("read-only", "note-request"):
    sample = read(name + ".json")
    assert sample["effects"] == []
    assert sample["reader_before"] == sample["reader_after"]
    assert sample["memory_before"] == sample["memory_after"]
    for protocol in ("native", "react"):
        serialized = encode(sample[protocol])
        assert "use_when" in serialized and "unmet_capabilities" in serialized
        if name == "read-only":
            assert "explicit_effect_intent_required" in serialized
    requests.append(dict(scenario=name, active_context=sample["active_context"]))

budget = read("budget-observation.json")
assert size(budget["delivered"]) <= budget["limit"]
legacy_budget_shape = {k: v for k, v in budget["raw"].items() if k not in ("blocked", "unmet_capabilities")}
legacy_budget_shape["matches"] = [
    {k: v for k, v in match.items() if k not in ("use_when", "avoid_when")}
    for match in budget["raw"]["matches"]
]
summary = dict(
    schemas_validated=1, published_examples=4, fixed_scenarios=len(rows),
    schema_before_bytes=size(before["spec"]["parameters"]), schema_after_bytes=size(schema),
    schema_before_estimated_tokens=estimate(before["spec"]["parameters"]),
    schema_after_estimated_tokens=estimate(schema), results=rows, requests=requests,
    budget_observation=dict(limit=budget["limit"], raw_bytes=size(budget["raw"]),
                            legacy_shape_bytes=size(legacy_budget_shape),
                            delivered_bytes=size(budget["delivered"]), truncated=budget["truncated"]),
)
(ROOT / "sizes-and-results.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(summary, ensure_ascii=False, indent=2))
