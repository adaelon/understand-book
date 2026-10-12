"""Validate exported calls against actual MCP schemas and report TO8 result cost."""
import copy
import json
import math
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def read(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def wire(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def size(value):
    return len(wire(value).encode("utf-8"))


def estimate(value):
    return math.ceil(sum(1 if ord(c) >= 0x2E80 else 0.25 for c in wire(value)))


def strip_calls(value):
    if isinstance(value, dict):
        return {k: strip_calls(v) for k, v in value.items() if k != "next_calls"}
    if isinstance(value, list):
        return [strip_calls(v) for v in value]
    return value


before, after, delivered = read("before.json"), read("after.json"), read("delivered.json")
assert strip_calls(after) == before, "candidate selection, order, positions or source data changed"
assert all(not sample["truncated"] for sample in delivered)
assert [sample["delivered"] for sample in delivered] == [sample["result"] for sample in after]
local, remote = read("local.json"), read("remote.json")
local_schemas = {tool["name"]: tool["inputSchema"] for tool in local["tools"]["tools"]}
remote_schemas = {tool["name"]: tool["inputSchema"] for tool in remote["tools"]}
for schema in [*local_schemas.values(), *remote_schemas.values()]:
    Draft7Validator.check_schema(schema)

counts = {"resident": 0, "local_mcp": 0, "remote_mcp": 0}
for sample in delivered:
    assert sample["candidate_source_evidence"] == []
    for read_call in sample["reads"]:
        Draft7Validator(local_schemas["book_text"]).validate(read_call["call"]["arguments"])
        counts["resident"] += 1
for handoff in local["handoffs"]:
    call = handoff["call"]
    Draft7Validator(local_schemas[call["name"]]).validate(call["arguments"])
    counts["local_mcp"] += 1
for sample in remote["samples"]:
    Draft7Validator(remote_schemas[sample["tool"]]).validate(sample["input"])
    assert sample["located"]["complete"] and sample["located"]["next_calls"] == []
    for handoff in sample["reads"]:
        call = handoff["call"]
        Draft7Validator(remote_schemas[call["name"]]).validate(call["arguments"])
        assert call["arguments"]["published_book_ref"] == sample["input"]["published_book_ref"]
        counts["remote_mcp"] += 1

report = {
    "candidate_values_unchanged": True,
    "fixed_results_delivered_in_full": len(delivered),
    "validated_read_calls": counts,
    "remote_charged_calls": remote["charged_calls"],
    "canonical_results": [
        {"label": a["label"], "before_bytes": size(b["result"]), "after_bytes": size(a["result"]),
         "before_estimated_tokens": estimate(b["result"]), "after_estimated_tokens": estimate(a["result"])}
        for b, a in zip(before, after)
    ],
    "remote_results": [],
    "resident_next_sampling": [
        {"label": sample["label"], "active_context": sample["active_context"]}
        for sample in read("requests.json")
    ],
    "budget_observation": {
        key: value for key, value in read("budget-observation.json").items()
        if key not in {"delivered", "continuation"}
    },
}
for sample in remote["samples"]:
    result = sample["located"]
    # Same current envelope, excluding only newly added candidate calls for the cost comparison.
    previous_shape = copy.deepcopy(result)
    previous_shape["data"] = strip_calls(previous_shape["data"])
    report["remote_results"].append({
        "tool": sample["tool"], "query": sample["input"].get("query"),
        "without_candidate_calls_bytes": size(previous_shape), "with_candidate_calls_bytes": size(result),
    })
(ROOT / "sizes-and-results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
