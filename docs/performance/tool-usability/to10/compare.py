"""Validate exported continuation arguments against advertised schemas and summarize delivery."""
import json
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def read(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


before, after = read("before.json"), read("after.json")
local, remote, resident = read("local.json"), read("http.json"), read("resident.json")
local_schemas = {tool["name"]: tool["inputSchema"] for tool in local["tools"]}
remote_schemas = {tool["name"]: tool["inputSchema"] for tool in remote["tools"]}
for schemas in [local_schemas, remote_schemas]:
    for schema in schemas.values():
        Draft7Validator.check_schema(schema)

validated = {"resident": 0, "local_mcp": 0, "remote_mcp": 0}
Draft7Validator(local_schemas["book_text"]).validate(local["input"])
validated["local_mcp"] += 1
for sample in after:
    continuation = sample["continuation"]
    if continuation:
        schema = local_schemas[continuation["tool"].replace(".", "_")]
        Draft7Validator(schema).validate(continuation["arguments"])
        validated["resident"] += 1
for sample in resident:
    if sample.get("continuation"):
        Draft7Validator(local_schemas["book_text"]).validate(sample["continuation"]["arguments"])
        validated["resident"] += 1
for sample in remote["samples"]:
    Draft7Validator(remote_schemas["book_text"]).validate(sample["input"])
    assert sample["result"]["usage"]["charged_units"] == 1
    validated["remote_mcp"] += 1
for sample in read("remote-search.json"):
    Draft7Validator(remote_schemas["book_search_text"]).validate(sample["call"]["arguments"])
    validated["remote_mcp"] += 1

assert remote["charged_calls"] == len(remote["samples"])
assert before[2] == after[2], "native search paging changed"
for old, new in zip(before, after):
    assert old["label"] == new["label"] and old["input"] == new["input"]
    assert size(new["body"]) <= new["limit"]

report = {
    "validated_inputs": validated,
    "native_search_sample_unchanged": True,
    "resident_text_pages": len(resident),
    "resident_utf16_units": resident[-1]["model_body"]["source_range"]["end"],
    "http_text_pages": len(remote["samples"]),
    "http_charged_calls": remote["charged_calls"],
    "remote_search_calls": len(read("remote-search.json")),
    "samples": [
        {
            "label": new["label"],
            "before_body_bytes": size(old["body"]),
            "after_body_bytes": size(new["body"]),
            "before_continuation": old["continuation"]["kind"],
            "after_continuation": new["continuation"]["kind"],
        }
        for old, new in zip(before, after)
    ],
    "real_model_sampling": False,
}
(ROOT / "sizes-and-results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
