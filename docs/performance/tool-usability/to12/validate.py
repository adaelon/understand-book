"""Validate actual delivered samples against their advertised output schemas.

Run after TO12_DIR exports from `cargo test -p runtime -p server --lib -- to12`.
Uses the existing Python jsonschema dependency; no model verdicts or API calls.
"""
import copy
import json
from pathlib import Path
from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def validator(schema):
    Draft7Validator.check_schema(schema)
    return Draft7Validator(schema)


def validate(v, value, label):
    errors = list(v.iter_errors(value))
    if errors:
        error = errors[0]
        # Keep failures bounded even for large nested union schemas.
        details = [(list(e.absolute_path), e.validator, str(e.message)[:200]) for e in error.context[:8]]
        raise AssertionError((label, list(error.absolute_path), error.validator, details))


counts = {}
for surface in ["local", "remote"]:
    capture = load(surface + ".json")
    tools = {t["name"]: t for t in capture["tools"] if "outputSchema" in t}
    validators = {name: validator(t["outputSchema"]) for name, t in tools.items()}
    passed, errors, paged = 0, 0, 0
    seen = set()
    for sample in capture["samples"]:
        name, reply = sample["tool"], sample["reply"]
        body = reply["structuredContent"]
        assert json.loads(reply["content"][0]["text"]) == body
        if reply["isError"]:
            assert body["error_code"]
            assert not validators[name].is_valid(body), (surface, name, "error passed success schema")
            errors += 1
        else:
            validate(validators[name], body, (surface, name, sample.get("input")))
            assert not validators[name].is_valid({}), (surface, name, "empty object accepted")
            seen.add(name)
            passed += 1
            paged += bool(body.get("data_path"))
            if surface == "remote" and "data" in body:
                bad = copy.deepcopy(body)
                bad["data"] = {"unexpected": True}
                assert not validators[name].is_valid(bad), (name, "untyped data accepted")
                if body.get("data_path"):
                    bad = copy.deepcopy(body)
                    bad["data_path"] = "/not-a-collection"
                    assert not validators[name].is_valid(bad)
    assert seen == set(tools), (surface, "missing tools", set(tools) - seen)
    counts[surface] = {"schemas": len(tools), "success": passed, "errors": errors, "collection_pages": paged}

capture = load("resident.json")
validators = {name: validator(t["outputSchema"]) for name, t in capture["tools"].items()}
canonical = {name: validator(t["canonical"]) for name, t in capture["tools"].items()}
statuses = {}
for i, sample in enumerate(capture["samples"]):
    name, envelope = sample["tool"], sample["envelope"]
    validate(validators[name], envelope, ("resident", name, sample.get("limit"), i))
    status = envelope["status"]
    statuses[status] = statuses.get(status, 0) + 1
    if "raw" in sample and not sample.get("error"):
        validate(canonical[name], sample["raw"], ("canonical", name))
    if status in ["partial", "error"] and (envelope["model_body"] is None or "delivery" in envelope["model_body"] or "projection" in envelope["model_body"]):
        assert not canonical[name].is_valid(envelope["model_body"]), (name, "projection accepted as canonical")
    bad = copy.deepcopy(envelope)
    bad["model_body"] = {"unexpected": True}
    assert not validators[name].is_valid(bad), (name, "arbitrary model body accepted")
counts["resident"] = {"schemas": len(validators), "samples": len(capture["samples"]), "statuses": statuses}
(ROOT / "validation.json").write_text(json.dumps(counts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(counts, ensure_ascii=False, indent=2))
