"""Validate final advertised examples and summarize fixed, unrepeated runs."""
import json
from fractions import Fraction
from pathlib import Path
from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def size(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


before, after = read(ROOT / "before.json"), read(ROOT / "after.json")
rows = []
for surface, tools in before.items():
    assert [t["name"] for t in tools] == [t["name"] for t in after[surface]]
    for old, new in zip(tools, after[surface]):
        if new["name"] != "tutor.step":
            assert old == new, (surface, new["name"], "unrelated contract changed")
        schema = new["inputSchema"]
        Draft7Validator.check_schema(schema)
        examples = schema.get("examples", [])
        for index, example in enumerate(examples):
            Draft7Validator(schema).validate(example)
            assert example not in examples[:index]
        rows.append(dict(surface=surface, tool=new["name"], examples=len(examples),
                         schema_before_bytes=size(old["inputSchema"]), schema_after_bytes=size(schema),
                         description_before_bytes=len(old["description"].encode()),
                         description_after_bytes=len(new["description"].encode())))

states = []
for state, sample in read(ROOT / "states.json").items():
    spec = sample["spec"]
    schema = spec["parameters"]
    Draft7Validator.check_schema(schema)
    assert sample["native"]["tools"][0]["function"]["parameters"] == schema
    for example in schema["examples"]:
        Draft7Validator(schema).validate(example)
        if example["operation"] == "select":
            broken = dict(example, move="explain speed")
            assert not Draft7Validator(schema).is_valid(broken)
        if example.get("nature") == "hypothesis":
            assert example["fact_refs"][0] in sample["context"]["learner_context"]["uninterpreted_fact_refs"]
        if example["operation"] == "assess":
            assert example["action_ref"] in sample["context"]["learner_context"]["pending_assessment_refs"]
    without = {k: v for k, v in schema.items() if k != "examples"}
    states.append(dict(state=state, examples=len(schema["examples"]), schema_bytes=size(schema),
                       example_bytes=size(schema)-size(without)))

summary = dict(tools=rows, states=states, final_schemas=len(rows),
               published_examples=sum(row["examples"] for row in rows))
if (ROOT / "after-runs" / "configuration.json").exists():
    baseline_config = read(ROOT / "baseline" / "configuration.json")
    assert baseline_config == read(ROOT / "after-runs" / "configuration.json")
    runs = []
    for group in ("baseline", "after-runs"):
        for directory in sorted((ROOT / group).glob("run-*")):
            result = read(directory / "result.json")
            requests = [read(p) for p in sorted(directory.glob("request-*.json"))]
            responses = [read(p) for p in sorted(directory.glob("response-*.json"))]
            reply = result["reply"]
            errors = [step for step in reply.get("trace", []) if '"error_code"' in step["result_digest"]]
            answer = reply.get("answer", "") or ""
            (directory / "answer.md").write_text(answer, encoding="utf-8")
            moves = [e["payload"]["move"] for e in result["events"] if e["kind"] == "move_selected"]
            (directory / "persisted-moves.json").write_text(json.dumps(moves, ensure_ascii=False, indent=2), encoding="utf-8")
            runs.append(dict(group=group, run=directory.name, requests=len(requests),
                             activity=[r["active_context"] for r in requests],
                             first_tutor_schema_bytes=size(next(t for t in requests[0]["tools"] if t["name"] == "tutor.step")["parameters"]),
                             tokens_spent=reply.get("tokens_spent"),
                             provider_usage=sum(r.get("usage_total_tokens") or 0 for r in responses),
                             http_status=result["http_status"], incomplete=reply.get("incomplete"),
                             selected=len(moves), deliveries=len(result["deliveries"]), errors=errors))
    summary["model_runs"] = runs
    # Human-located exact claims, checked against independent arithmetic cases.
    claims = [
        ("baseline", "run-1", "它只在「各段用时相等」时恰好成立", "equal_speed_unequal_time"),
        ("baseline", "run-2", "只有各段时间相等时，直接取算术平均才对", "equal_speed_unequal_time"),
        ("baseline", "run-3", "各段**路程**相等 → 不能直接平均", "equal_speed_equal_distance"),
        ("after-runs", "run-1", "等路程恰好不属于这个特殊情形", "equal_speed_equal_distance"),
        ("after-runs", "run-2", "两套规则只在一种情况下重合:**各段耗时相等**", "equal_speed_unequal_time"),
        ("after-runs", "run-3", "相等的是**路程**(题目里更常见) → 不能直接平均", "equal_speed_equal_distance"),
    ]
    cases = {}
    for name, v1, v2, t1, t2 in [("equal_speed_unequal_time", 10, 10, 1, 2), ("equal_speed_equal_distance", 10, 10, 1, 1),
                                 ("equal_time", 60, 30, 1, 1), ("original_unequal_time", 60, 30, 1, 2)]:
        weighted = Fraction(v1*t1+v2*t2, t1+t2)
        arithmetic = Fraction(v1+v2, 2)
        assert weighted-arithmetic == Fraction((v1-v2)*(t1-t2), 2*(t1+t2))
        cases[name] = dict(speeds=[v1,v2], times=[t1,t2], distances=[v1*t1,v2*t2], weighted=str(weighted), arithmetic=str(arithmetic))
    findings = []
    for group, run, quote, counterexample in claims:
        assert quote in (ROOT/group/run/"answer.md").read_text(encoding="utf-8")
        assert cases[counterexample]["weighted"] == cases[counterexample]["arithmetic"]
        findings.append(dict(group=group, run=run, quote=quote, counterexample=counterexample,
                             finding="The general summary omits the equal-speed case; specific 60/30 calculations remain correct."))
    after_first = read(ROOT/"after-runs"/"run-1"/"persisted-moves.json")[0]
    assert len(after_first["continuation"]["options"]) == 2
    assert "如果想继续，可以挑一个" in (ROOT/"after-runs"/"run-1"/"answer.md").read_text(encoding="utf-8")
    (ROOT/"content-findings.json").write_text(json.dumps(dict(counterexamples=cases, findings=findings,
        continuation_finding="after-runs/run-1 repeats its two structured continuation suggestions in the public answer."),ensure_ascii=False,indent=2),encoding="utf-8")
(ROOT / "sizes-and-results.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
print(json.dumps(summary, ensure_ascii=False, indent=2))
