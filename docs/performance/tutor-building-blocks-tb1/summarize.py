"""Summarize captured facts; teaching quality remains a cited content judgment."""
import json
import pathlib
import sys


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def main():
    root = pathlib.Path(sys.argv[1]).resolve()
    config = read(root / "configuration.json")
    rows = []
    total = 0
    http_calls = 0
    for case in config["cases"]["cases"]:
        directory = root / case["id"]
        result = read(directory / "result.json")
        request = read(directory / "input.json")
        context = read(directory / "learner-context.json")
        assert request["message"] == case["background"] + "\n" + case["request"]
        assert context["learner_context"]["current_request"] == request["message"]
        requests = sorted(directory.glob("request-*.json"))
        assert len(requests) == result["provider_logical_calls"]
        plans = [read(path) for path in requests]
        for path in requests:
            assert (directory / path.name.replace("request-", "response-")).exists()
        transports = [read(path) for path in sorted(directory.glob("transport-*.json"))]
        usages = [t["outcome"]["usage"] for t in transports]
        known_tokens = sum(u["total_tokens"] for u in usages if u and u["total_tokens"] is not None)
        total += known_tokens
        http_calls += len(transports)
        outcome = result["turn"].get("outcome") or {}
        trace = outcome.get("trace") or result["turn"].get("run_summary", {}).get("trace", [])
        errors = []
        for step in trace:
            digest = step.get("result_digest", "")
            try:
                body = json.loads(digest)
            except (ValueError, TypeError):
                continue
            if "error_code" in body or "error" in body:
                errors.append({"tool":step["tool"],"arguments":step.get("args"),"result":body})
        wire = [read(path) for path in sorted(directory.glob("wire-request-*.json"))]
        assert all(w["model"] == config["model"] and w["temperature"] == config["temperature"] for w in wire)
        answer = outcome.get("answer")
        if answer:
            assert (directory / "answer.md").read_text(encoding="utf-8") == answer
        deliveries = [f for _, f in result["facts"] if f["kind"] == "message_delivered"]
        assert all(p["runtime_profile"]["profile_id"] == "resident-agent-deepseek-flash-v1" for p in plans)
        rows.append({
            "case":case["id"],"status":result["turn"]["status"],"incomplete":outcome.get("incomplete"),"failure":result["turn"].get("error"),
            "elapsed_ms":result["elapsed_ms"],"http_calls":len(transports),"resident_chat_calls":len(plans),"wire_logical_calls":len(wire),
            "known_total_tokens":known_tokens,"usage_complete":bool(transports) and all(u and u["total_tokens"] is not None for u in usages),
            "active_input_estimated_tokens":[p["active_context"]["estimated_input_tokens"] for p in plans],
            "actual_input_tokens":[u.get("input_tokens") if u else None for u in usages],
            "actual_output_tokens":[u.get("output_tokens") if u else None for u in usages],
            "wire_bytes_without_private_reasoning":[len(json.dumps(w,ensure_ascii=False,separators=(",",":")).encode("utf-8")) for w in wire],
            "tools":[s["tool"] for s in trace],"tool_errors":errors,
            "delivery_count":len(deliveries),"moves":[d["payload"].get("move") for d in deliveries],
            "hypotheses":[{"interpretation":e["interpretation"],"teaching_implication":e["teaching_implication"],"learner_quote":e["learner_quote"]} for _, e in result["evidence"]],
            "learner_response_after_delivery":False,
        })
    budget = read(root / "budget.json")
    prior = config.get("prior_budget") or {"http_calls":0,"tokens_or_reserved":0}
    assert http_calls + prior["http_calls"] == budget["http_calls"]
    if not budget["stopped"]:
        assert total + prior["tokens_or_reserved"] == budget["tokens_or_reserved"]
    summary = {"common_code_baseline":config["common_code_baseline"],"model":config["model"],
        "known_total_tokens":total,"http_calls":http_calls,"cumulative_budget":budget,"monetary_cost":None,"cases":rows}
    (root / "summary.json").write_text(json.dumps(summary,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"captured_cases":len(rows),"http_calls":http_calls,"known_total_tokens":total},ensure_ascii=False))


if __name__ == "__main__":
    main()
