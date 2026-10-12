"""Compare frozen TO9 results and report the cost of candidate coverage fields."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def read(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


before, after = read("before.json"), read("after.json")
assert len(before) == len(after) == 5
rows = []
for old, new in zip(before, after):
    assert old["label"] == new["label"]
    result = new["result"]
    coverage_keys = {"total_candidates", "returned_count", "truncated"}
    assert {k: v for k, v in result.items() if k not in coverage_keys} == old["result"]
    assert old["structure"] == new["structure"]
    assert result["total_candidates"] == new["expected_total"]
    assert result["returned_count"] == new["expected_returned"] == len(result["items"])
    assert result["truncated"] == (result["total_candidates"] > result["returned_count"])
    rows.append({
        "label": new["label"], "total_candidates": result["total_candidates"],
        "returned_count": result["returned_count"], "truncated": result["truncated"],
        "before_bytes": size(old["result"]), "after_bytes": size(result),
        "added_bytes": size(result) - size(old["result"]),
    })

resident, local, remote, http = [read(name + ".json") for name in ["resident", "local", "remote", "http"]]
for r, l in zip(resident["samples"], local["samples"]):
    assert r["input"] == l["input"]
    for key in coverage_keys:
        assert r["model_body"][key] == l["result"][key]
    assert not r["delivery_truncated"]
assert not resident["structure"]["available"] and not local["structure"]["available"]
assert remote["missing_structure"]["error_code"] == "MCP_CAPABILITY_UNAVAILABLE"
for sample in remote["samples"]:
    first = sample["first"]
    assert first["data"]["returned_count"] == sample["reconstructed_count"]
    assert first["data"]["truncated"] == (sample["reconstructed_count"] < 60)
    assert first["complete"] == (sample["input"]["k"] <= 20)
assert http["charged_calls"] == 3
assert all(sample["result"]["complete"] for sample in http["samples"])

report = {
    "original_candidate_values_unchanged": True,
    "structure_availability_unchanged": True,
    "canonical_results": rows,
    "resident_and_local_cases": len(local["samples"]),
    "remote_pagination_cases": len(remote["samples"]),
    "http_content_calls": http["charged_calls"],
    "real_model_sampling": False,
}
(ROOT / "sizes-and-results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
