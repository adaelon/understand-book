"""Summarize two saved live Server delivery runs; never sends model requests.

From the repository root:
  python docs/performance/workspace-input-cache-20261022/summarize_delivery.py \
      BASELINE_DIRECTORY CANDIDATE_DIRECTORY --output analysis.json

The default rates.json is beside this script. Costs are exact decimal CNY strings;
null plus unknown_fields denotes unavailable usage or pricing, never zero usage.
"""

import argparse
from collections import Counter
from decimal import Decimal
import json
from pathlib import Path


TOKEN_FIELDS = ("input_tokens", "cached_input_tokens", "uncached_input_tokens", "output_tokens")
COST_FIELDS = ("input_cny", "output_cny", "total_cny")
OFF_PEAK_RATE = {"input_micro_cny_per_million": 1_000_000,
                 "cached_input_micro_cny_per_million": 20_000,
                 "output_micro_cny_per_million": 4_000_000}


def read_json(path, optional=False):
    if optional and not path.exists():
        return None
    return json.loads(path.read_text(encoding="utf-8-sig"))


def token(value):
    return value if type(value) is int and value >= 0 else None


def price_usage(usage, rate, separate_creation):
    cost = dict.fromkeys(COST_FIELDS)
    if rate is None:
        return cost
    divisor = Decimal(1_000_000_000_000)  # micro-CNY per million tokens -> CNY
    if usage["uncached_input_tokens"] is not None and not separate_creation:
        numerator = (usage["uncached_input_tokens"] * rate["input_micro_cny_per_million"]
                     + usage["cached_input_tokens"] * rate["cached_input_micro_cny_per_million"])
        cost["input_cny"] = format(Decimal(numerator) / divisor, "f")
    if usage["output_tokens"] is not None:
        cost["output_cny"] = format(Decimal(usage["output_tokens"] * rate["output_micro_cny_per_million"]) / divisor, "f")
    if cost["input_cny"] is not None and cost["output_cny"] is not None:
        cost["total_cny"] = format(Decimal(cost["input_cny"]) + Decimal(cost["output_cny"]), "f")
    return cost


def summarize_attempt(attempt, rates):
    outcome = attempt.get("outcome") or {}
    raw = outcome.get("usage") or {}
    usage = {key: token(raw.get(key)) for key in TOKEN_FIELDS if key != "uncached_input_tokens"}
    input_tokens, cached = usage["input_tokens"], usage["cached_input_tokens"]
    notes = []
    if input_tokens is not None and cached is not None and cached > input_tokens:
        usage["cached_input_tokens"] = cached = None
        notes.append("cached_input_tokens exceeds input_tokens")
    usage["uncached_input_tokens"] = input_tokens - cached if input_tokens is not None and cached is not None else None
    started_ms = attempt.get("started_at_unix_ms")
    matches = [rate for rate in rates
               if rate.get("model") == attempt.get("model")
               and type(started_ms) is int
               and rate["starts_at"] * 1000 <= started_ms < rate["expires_at"] * 1000]
    rate = matches[0] if len(matches) == 1 else None
    if rate is None:
        notes.append("No unique model/time price match")
    separate_creation = raw.get("cache_creation_input_tokens") not in (None, 0)
    if separate_creation:
        notes.append("Separate cache-creation input has no price in this snapshot")
    cost = price_usage(usage, rate, separate_creation)
    normalized = price_usage(usage, OFF_PEAK_RATE, separate_creation)
    unknown = [key for key in TOKEN_FIELDS if usage[key] is None]
    unknown.extend(f"actual_cost.{key}" for key in COST_FIELDS if cost[key] is None)
    unknown.extend(f"normalized_off_peak_cost.{key}" for key in COST_FIELDS if normalized[key] is None)
    return {"call_id": attempt.get("call_id"), "purpose": attempt.get("purpose"),
            "started_at_unix_ms": started_ms, "evidence": outcome.get("evidence", "unknown"),
            "succeeded": outcome.get("succeeded"), "rate_version": rate.get("version") if rate else None,
            "usage": usage, "actual_cost": cost, "normalized_off_peak_cost": normalized,
            "unknown_fields": unknown, "notes": notes}


def aggregate(attempts):
    # A missing value in any HTTP attempt makes that total unknown. Do not hide
    # failed/partial requests by summing only successful provider responses.
    usage = {key: sum(item["usage"][key] for item in attempts)
             if all(item["usage"][key] is not None for item in attempts) else None
             for key in TOKEN_FIELDS}
    costs = {kind: {key: format(sum((Decimal(item[kind][key]) for item in attempts), Decimal(0)), "f")
                   if all(item[kind][key] is not None for item in attempts) else None
                   for key in COST_FIELDS}
             for kind in ("actual_cost", "normalized_off_peak_cost")}
    unknown = [key for key in TOKEN_FIELDS if usage[key] is None]
    unknown.extend(f"{kind}.{key}" for kind, values in costs.items() for key in COST_FIELDS if values[key] is None)
    return {"http_attempts": len(attempts), "usage": usage, **costs,
            "cache_hit_rate": usage["cached_input_tokens"] / usage["input_tokens"]
            if usage["input_tokens"] and usage["cached_input_tokens"] is not None else None,
            "usage_status": "complete" if all(value is not None for value in usage.values())
            else "unknown" if all(value is None for value in usage.values()) else "partial",
            "unknown_fields": unknown,
            "attempts_with_unknown_fields": sum(bool(item["unknown_fields"]) for item in attempts),
            "evidence_counts": dict(Counter(item["evidence"] for item in attempts)),
            "rate_versions": dict(Counter(item["rate_version"] for item in attempts if item["rate_version"] is not None))}


def summarize_run(directory, rates):
    log = read_json(directory / "http-attempts.json")
    attempts = [summarize_attempt(attempt, rates) for attempt in log["attempts"]]
    complete = read_json(directory / "complete.json", optional=True)
    turns = []
    cursor = 0
    partition_available = True
    for number in (1, 2):
        record = read_json(directory / f"turn-{number:02}.json", optional=True)
        if record is None:
            turns.append({"turn": number, "record_status": "missing", "delivered": None,
                          "time_to_first_public_answer_patch_ms": None, "metrics": None})
            partition_available = False
            continue
        cumulative = record.get("http_attempts_total")
        metrics = None
        if partition_available and type(cumulative) is int:
            if not cursor <= cumulative <= len(attempts):
                raise ValueError(f"Invalid cumulative HTTP count in {directory}/turn-{number:02}.json")
            metrics = aggregate(attempts[cursor:cumulative])
            metrics["http_attempt_indexes"] = list(range(cursor + 1, cumulative + 1))
            cursor = cumulative
        else:
            partition_available = False
        turns.append({"turn": number, "record_status": "present", "delivered": record.get("delivered"),
                      "runtime_incomplete": record.get("runtime_incomplete"),
                      "admission_failed": record.get("admission_failed", False),
                      "http_status": record.get("http_status"),
                      "time_to_first_public_answer_patch_ms": record.get("time_to_first_public_answer_patch_ms"),
                      "elapsed_including_post_coach_ms": record.get("elapsed_including_post_coach_ms"),
                      "http_attempts_total": cumulative, "metrics": metrics})
    return {"directory": str(directory.resolve()), "complete": complete.get("complete") if complete else None,
            "successful_deliveries": complete.get("successful_deliveries") if complete else None,
            "logical_model_calls": complete.get("logical_model_calls") if complete else None,
            "budget_denials": complete.get("budget_denials") if complete else log.get("denied"),
            "totals": aggregate(attempts), "turns": turns, "attempts": attempts,
            "unassigned_http_attempts": len(attempts) - cursor}


def difference(before, after, money=False):
    if before is None or after is None:
        return None
    return format(Decimal(after) - Decimal(before), "f") if money else after - before


def comparison(baseline, candidate):
    before, after = baseline["totals"], candidate["totals"]
    return {"both_conversations_complete": baseline["complete"] is True and candidate["complete"] is True,
            "delta_direction": "candidate minus baseline; incomplete deliveries are retained, not equivalent successful work",
            "http_attempts_delta": after["http_attempts"] - before["http_attempts"],
            "usage_delta": {key: difference(before["usage"][key], after["usage"][key]) for key in TOKEN_FIELDS},
            "actual_cost_delta": {key: difference(before["actual_cost"][key], after["actual_cost"][key], money=True) for key in COST_FIELDS},
            "normalized_off_peak_cost_delta": {key: difference(before["normalized_off_peak_cost"][key], after["normalized_off_peak_cost"][key], money=True) for key in COST_FIELDS},
            "cache_hit_rate_percentage_points_delta": difference(
                before["cache_hit_rate"] * 100 if before["cache_hit_rate"] is not None else None,
                after["cache_hit_rate"] * 100 if after["cache_hit_rate"] is not None else None),
            "turns": [{"turn": old["turn"], "both_delivered": old["delivered"] is True and new["delivered"] is True,
                       "public_answer_first_patch_ms_delta": difference(old["time_to_first_public_answer_patch_ms"], new["time_to_first_public_answer_patch_ms"])}
                      for old, new in zip(baseline["turns"], candidate["turns"])]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("baseline_dir", type=Path)
    parser.add_argument("candidate_dir", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rates-file", type=Path, default=Path(__file__).with_name("rates.json"))
    args = parser.parse_args()
    rates = read_json(args.rates_file)["rates"]
    baseline = summarize_run(args.baseline_dir, rates)
    candidate = summarize_run(args.candidate_dir, rates)
    result = {"schema": "workspace_delivery_comparison.v1", "rates_file": str(args.rates_file.resolve()),
              "normalized_off_peak_rates_cny_per_million": {"uncached_input": "1", "cached_input": "0.02", "output": "4"},
              "notes": [
                  "HTTP attempts include retries and private coach/review calls. Evidence counts retain not_sent and unknown outcomes.",
                  "Usage is the final ModelUsage snapshot per HTTP attempt; reasoning output is already included in output_tokens.",
                  "Price intervals use started_at_unix_ms with an inclusive start and exclusive end. Costs are provider-usage estimates, not invoices.",
                  "actual_cost follows each request's time interval; normalized_off_peak_cost applies the same 1/0.02/4 CNY per million rates to both runs to remove tariff timing differences.",
                  "Public answer latency is the recorded nonempty Markdown answer.patch boundary, not provider text or browser paint.",
                  "Per-turn costs include all attempts through http_attempts_total, including post-turn coach; public first-patch latency excludes post-turn coach.",
                  "Null means unknown; a missing usage field is never counted as zero. Zero is used only for a known empty attempt slice.",
              ], "baseline": baseline, "candidate": candidate, "comparison": comparison(baseline, candidate)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(args.output.resolve())


if __name__ == "__main__":
    main()
