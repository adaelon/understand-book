"""Bounded live comparison of Runtime-generated workspace request transcripts.

Examples (from the repository root):
  python docs/performance/workspace-input-cache-20261022/probe.py compare \
      --baseline-dir BEFORE --candidate-dir AFTER --indexes 0,1,2 --protocol native
  python docs/performance/workspace-input-cache-20261022/probe.py live-replay \
      --baseline-dir BEFORE --candidate-dir AFTER --indexes 0,1,2 --protocol native \
      --repetitions 2 --max-calls 12 --output-token-cap 2048 --rates-file RATES.json \
      --output-dir FRESH_DIRECTORY --sample-description "Workspace explanation and follow-ups"

Only live-replay calls the configured paid provider. It makes no retries or hidden
warm-up calls. Outputs are retained for semantic review but are NOT substituted
into later recorded requests. Tool calls returned by the model are never executed.
"""

import argparse
import copy
from datetime import datetime, timezone
from decimal import Decimal
import importlib.util
import json
from pathlib import Path
import statistics
import time
import urllib.error
import urllib.request
import uuid


ROOT = Path(__file__).resolve().parents[3]
_spec = importlib.util.spec_from_file_location(
    "existing_cache_probe", ROOT / "docs/performance/context-cache-cc-20261015/cache_probe.py")
legacy = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(legacy)

LIMITATIONS = [
    "Fixed Runtime transcripts: new model outputs are retained for review, not used as later inputs; this is not autonomous end-to-end evaluation.",
    "The output cap is identical between arms; length-limited responses and tool calls are reported and are not complete explanation samples.",
    "Each arm and repetition has its own leading marker; this does not guarantee a fully cold provider tool prefix. The provider controls cache placement and eviction.",
    "First text means the first nonempty assistant content delta at this client; reasoning and tool deltas have separate timing. This is not the Reader's first visible answer time.",
    "Cost is computed from provider-reported usage and the recorded operator price snapshot, not an upstream invoice.",
    "A short controlled sequence establishes only this workload's measurements, not application-wide cache savings or learning outcomes.",
]


def select_rate(value, config, now):
    rates = value.get("rates", [value])
    matches = [rate for rate in rates if rate.get("provider", "").rstrip("/") == config["OPENCODE_BASE_URL"].rstrip("/")
               and rate.get("model") == config["FLUID_LLM_MODEL"]
               and rate.get("starts_at", now + 1) <= now < rate.get("expires_at", now)]
    if len(matches) != 1:
        raise ValueError("rates-file must contain exactly one current rate for the configured provider and model")
    rate = matches[0]
    for key in ("input_micro_cny_per_million", "cached_input_micro_cny_per_million", "output_micro_cny_per_million"):
        if type(rate.get(key)) is not int or rate[key] < 0:
            raise ValueError("Invalid non-negative integer rate: " + key)
    if not rate.get("version") or not all(rate.get("source", {}).get(key) for key in
            ("currency", "original_prices", "cny_conversion", "checked_on", "official_url")):
        raise ValueError("Rate version and complete source attribution are required")
    return rate


def load_rate(path, config, now):
    return select_rate(json.loads(path.read_text(encoding="utf-8-sig")), config, now)


def measured_usage(result):
    usage = result.get("usage")
    if not isinstance(usage, dict):
        raise ValueError("Provider usage missing; no further calls sent")
    prompt = usage.get("prompt_tokens", usage.get("input_tokens"))
    output = usage.get("completion_tokens", usage.get("output_tokens"))
    cached = usage.get("prompt_cache_hit_tokens")
    if cached is None:
        cached = (usage.get("prompt_tokens_details") or {}).get("cached_tokens")
    if cached is None:
        cached = (usage.get("input_tokens_details") or {}).get("cached_tokens")
    if any(type(value) is not int or value < 0 for value in (prompt, output, cached)) or cached > prompt:
        raise ValueError("Provider input/output/cache usage missing or inconsistent; no further calls sent")
    creation = (usage.get("input_tokens_details") or {}).get("cache_creation_tokens", 0)
    if creation:
        raise ValueError("Separate cache-creation charges are unsupported by this price contract")
    return {"input_tokens": prompt, "cached_input_tokens": cached,
            "uncached_input_tokens": prompt - cached, "output_tokens": output}


def priced_usage(usage, rate):
    input_numerator = (usage["uncached_input_tokens"] * rate["input_micro_cny_per_million"]
                       + usage["cached_input_tokens"] * rate["cached_input_micro_cny_per_million"])
    output_numerator = usage["output_tokens"] * rate["output_micro_cny_per_million"]
    # Prices are micro-CNY per million tokens; preserve exact component costs.
    money = lambda numerator: format(Decimal(numerator) / Decimal(1_000_000_000_000), "f")
    return {"input_cny": money(input_numerator), "output_cny": money(output_numerator),
            "total_cny": money(input_numerator + output_numerator),
            "total_rounded_micro_cny": (input_numerator + output_numerator + 999_999) // 1_000_000}


def prepare_requests(arms, config, experiment, repetitions, output_cap, max_calls):
    if not 1 <= repetitions <= 4 or not 1 <= output_cap <= 2048 or not 1 <= max_calls <= 32:
        raise ValueError("Bounds: repetitions 1..4, output-token-cap 1..2048, max-calls 1..32")
    if any(len(samples) < 2 for samples in arms.values()):
        raise ValueError("At least two requests per arm are needed to observe reuse")
    if sum(len(samples) for samples in arms.values()) * repetitions > max_calls:
        raise ValueError("Selected requests exceed max-calls; no request sent")
    prepared = []
    for repetition in range(1, repetitions + 1):
        order = ("baseline", "candidate") if repetition % 2 else ("candidate", "baseline")
        for arm in order:
            marker = f"Workspace cache comparison {experiment} repetition {repetition} arm {arm}.\n"
            for position, sample in enumerate(arms[arm]):
                body = copy.deepcopy(sample["body"])
                if legacy.contains_image(body["messages"]):
                    raise ValueError("Select text-only requests for this controlled comparison")
                first = body["messages"][0]
                if first.get("role") != "system" or not isinstance(first.get("content"), str):
                    raise ValueError("Requests must begin with a text system message")
                first["content"] = marker + first["content"]
                body["model"] = config["FLUID_LLM_MODEL"]
                body["max_tokens"] = output_cap
                body.pop("max_completion_tokens", None)
                body["stream"] = True
                body["stream_options"] = {**body.get("stream_options", {}), "include_usage": True}
                if len(legacy.encoded(body)) > 1_000_000:
                    raise ValueError("Selected request exceeds 1,000,000 bytes")
                prepared.append({"arm": arm, "repetition": repetition, "position": position,
                                 "index": sample["index"], "source": sample["path"], "body": body})
    return prepared


def apply_answer_only(prepared, protocol):
    if protocol != "native":
        raise ValueError("answer-only requires native tool calling")
    for item in prepared:
        item["body"]["tool_choice"] = "none"


def send_request(endpoint, key, body, timeout_seconds=120):
    request = urllib.request.Request(endpoint, data=legacy.encoded(body), headers={
        "Content-Type": "application/json", "Authorization": "Bearer " + key})
    start = time.monotonic()
    result = {"usage": None, "response_id": None, "model": None, "choices": [],
              "first_delta_ms": None, "first_text_ms": None, "first_reasoning_ms": None,
              "first_tool_ms": None, "reasoning_characters": 0, "stream_done": False}
    choices = {}

    def mark(field, observed_ms):
        if result[field] is None:
            result[field] = observed_ms

    def event(value):
        observed_ms = round((time.monotonic() - start) * 1000, 3)
        if value.get("error"):
            raise ValueError("Provider returned a streaming error")
        result["response_id"] = value.get("id", result["response_id"])
        result["model"] = value.get("model", result["model"])
        if value.get("usage") is not None:
            result["usage"] = value["usage"]
        for part in value.get("choices", []):
            index = part.get("index", 0)
            if index not in choices:
                choice = {"index": index, "finish_reason": None, "content": "", "tool_calls": []}
                choices[index] = (choice, {})
                result["choices"].append(choice)
            choice, tools = choices[index]
            if part.get("finish_reason") is not None:
                choice["finish_reason"] = part["finish_reason"]
            delta = part.get("delta", part.get("message", {}))
            if isinstance(delta.get("content"), str) and delta["content"]:
                if result["streaming"]:
                    mark("first_delta_ms", observed_ms)
                    mark("first_text_ms", observed_ms)
                choice["content"] += delta["content"]
            reasoning = delta.get("reasoning_content", delta.get("reasoning", ""))
            if isinstance(reasoning, str) and reasoning:
                if result["streaming"]:
                    mark("first_delta_ms", observed_ms)
                    mark("first_reasoning_ms", observed_ms)
                result["reasoning_characters"] += len(reasoning)
            for fragment in delta.get("tool_calls", []):
                if result["streaming"]:
                    mark("first_delta_ms", observed_ms)
                    mark("first_tool_ms", observed_ms)
                tool_index = fragment.get("index", len(tools) if not result["streaming"] else 0)
                if tool_index not in tools:
                    tool = {"id": "", "type": "function", "function": {"name": "", "arguments": ""}}
                    tools[tool_index] = tool
                    choice["tool_calls"].append(tool)
                tool = tools[tool_index]
                if fragment.get("id"):
                    tool["id"] = fragment["id"]
                if fragment.get("type"):
                    tool["type"] = fragment["type"]
                for name in ("name", "arguments"):
                    tool["function"][name] += fragment.get("function", {}).get(name, "")

    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        result["http_status"] = response.status
        result["response_headers_ms"] = round((time.monotonic() - start) * 1000, 3)
        result["streaming"] = "text/event-stream" in response.headers.get("Content-Type", "")
        if result["streaming"]:
            data = []

            def flush():
                if not data:
                    return
                payload = b"\n".join(data).strip()
                data.clear()
                if payload == b"[DONE]":
                    result["stream_done"] = True
                else:
                    event(json.loads(payload))

            for line in response:
                line = line.rstrip(b"\r\n")
                if not line:
                    flush()
                elif line.startswith(b"data:"):
                    data.append(line[5:].lstrip())
            flush()
        else:
            event(json.load(response))
    result["duration_ms"] = round((time.monotonic() - start) * 1000, 3)
    result["response_complete"] = bool(choices) and all(c[0]["finish_reason"] is not None for c in choices.values())
    result["output_truncated"] = any(c["finish_reason"] == "length" for c in result["choices"])
    result["complete_text_sample"] = (result["response_complete"] and not result["output_truncated"]
        and any(c["content"].strip() for c in result["choices"])
        and all(not c["tool_calls"] for c in result["choices"]))
    return result


def summarize(rows):
    measured = [row for row in rows if row.get("measured_usage")]
    tokens = {field: sum(row["measured_usage"][field] for row in measured) for field in
              ("input_tokens", "cached_input_tokens", "uncached_input_tokens", "output_tokens")}
    timings = {}
    for field in ("first_text_ms", "first_reasoning_ms", "first_tool_ms", "first_delta_ms", "duration_ms"):
        values = [row[field] for row in measured if row.get(field) is not None]
        timings[field] = {"samples": len(values), "median": statistics.median(values) if values else None,
                          "min": min(values) if values else None, "max": max(values) if values else None}
    costs = {field: format(sum((Decimal(row["cost"][field]) for row in measured), Decimal(0)), "f")
             for field in ("input_cny", "output_cny", "total_cny")}
    costs["total_rounded_micro_cny"] = sum(row["cost"]["total_rounded_micro_cny"] for row in measured)
    return {"calls": len(rows), "calls_with_usage": len(measured), **tokens,
            "cache_hit_rate": tokens["cached_input_tokens"] / tokens["input_tokens"] if tokens["input_tokens"] else None,
            "cost": costs, "timings": timings,
            "complete_text_samples": sum(bool(row.get("complete_text_sample")) for row in rows),
            "truncated_samples": sum(bool(row.get("output_truncated")) for row in rows),
            "failed_samples": sum(bool(row.get("error")) for row in rows)}


def aggregate(rows):
    arms = {}
    for arm in ("baseline", "candidate"):
        selected = [row for row in rows if row["arm"] == arm]
        arms[arm] = {"all": summarize(selected),
                     "first_requests": summarize([row for row in selected if row["position"] == 0]),
                     "continuation_requests": summarize([row for row in selected if row["position"] > 0]),
                     "by_repetition": {str(repetition): summarize([row for row in selected if row["repetition"] == repetition])
                                       for repetition in sorted({row["repetition"] for row in selected})}}
    return arms


def run_live(args, sender=send_request):
    config = legacy.provider_config(args.env_file)
    arms = {name: legacy.load_samples(getattr(args, name + "_dir"), args.protocol, args.indexes)
            for name in ("baseline", "candidate")}
    rate_catalog = json.loads(args.rates_file.read_text(encoding="utf-8-sig"))
    initial_rate = select_rate(rate_catalog, config, int(time.time()))
    experiment = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + str(uuid.uuid4())
    prepared = prepare_requests(arms, config, experiment, args.repetitions, args.output_token_cap, args.max_calls)
    answer_only = getattr(args, "answer_only", False)
    if answer_only:
        apply_answer_only(prepared, args.protocol)
    if not 0 <= args.delay_seconds <= 10 or not 1 <= args.timeout_seconds <= 300:
        raise ValueError("delay-seconds must be 0..10 and timeout-seconds 1..300")
    if args.disable_thinking:
        for item in prepared:
            item["body"]["thinking"] = {"type": "disabled"}
    endpoint = config["OPENCODE_BASE_URL"].rstrip("/") + "/chat/completions"
    key = config["OPENCODE_API_KEY"]
    args.output_dir.mkdir(parents=True, exist_ok=False)
    report = {"kind": "workspace_answer_only_quality_probe.v1" if answer_only else "workspace_fixed_transcript_cache_comparison.v1", "experiment": experiment,
              "sample_description": args.sample_description, "endpoint": endpoint, "model": config["FLUID_LLM_MODEL"],
              "protocol": args.protocol, "indexes": args.indexes, "repetitions": args.repetitions,
              "planned_calls": len(prepared), "maximum_calls": args.max_calls, "output_token_cap": args.output_token_cap,
              "order": "AB in odd repetitions, BA in even repetitions; chronological requests within each arm",
              "delay_seconds": args.delay_seconds, "price_snapshots": {initial_rate["version"]: initial_rate},
              "price_selection": "Frozen file contents; select the applicable interval at each HTTP request's start time",
              "limitations": list(LIMITATIONS),
              "request_adjustments": ["unique leading system marker per arm and repetition", "configured provider model",
                  "identical max_tokens output cap", "stream=true and include_usage=true"],
              "results": [], "stop_reason": None}
    if answer_only:
        report["request_adjustments"].append("native tool_choice=none; tool definitions, source evidence and recorded messages preserved")
        report["limitations"].append("Answer-only quality probe: native tool calls are disabled to obtain text from already-read evidence; results do not replace the automatic-tool cache comparison or prove autonomous tool-use quality.")
    if args.disable_thinking:
        report["request_adjustments"].append("thinking disabled in both arms; recorded transcript lacks provider reasoning continuation")
    legacy.save(args.output_dir / "comparison.json", legacy.compare_samples(arms["baseline"], arms["candidate"], args.protocol), key)
    legacy.save(args.output_dir / "results.json", report, key)
    for ordinal, item in enumerate(prepared, 1):
        if ordinal > 1 and args.delay_seconds:
            time.sleep(args.delay_seconds)
        name = f"{ordinal:02}-r{item['repetition']}-{item['arm']}-{item['index']:02}"
        legacy.save(args.output_dir / f"{name}-request.json", item["body"], key)
        record = {field: item[field] for field in ("arm", "repetition", "position", "index", "source")}
        record["ordinal"] = ordinal
        start = time.monotonic()
        try:
            record["started_utc"] = datetime.now(timezone.utc).isoformat()
            rate = select_rate(rate_catalog, config, int(time.time()))
            record["price_version"] = rate["version"]
            report["price_snapshots"][rate["version"]] = rate
            record.update(sender(endpoint, key, item["body"], args.timeout_seconds))
            record["measured_usage"] = measured_usage(record)
            record["cost"] = priced_usage(record["measured_usage"], rate)
            if not record.get("response_complete"):
                raise ValueError("Incomplete provider response; no further calls sent")
        except urllib.error.HTTPError as error:
            report["stop_reason"] = f"HTTP {error.code}; no retry"
            record["error"] = report["stop_reason"]
            record["provider_error"] = error.read(8192).decode("utf-8", "replace")
        except (OSError, ValueError, TimeoutError) as error:
            report["stop_reason"] = f"{type(error).__name__}: {error}"
            record["error"] = report["stop_reason"]
        record.setdefault("duration_ms", round((time.monotonic() - start) * 1000, 3))
        report["results"].append(record)
        report["totals"] = aggregate(report["results"])
        report["finished_utc"] = datetime.now(timezone.utc).isoformat()
        legacy.save(args.output_dir / f"{name}-response.json", record, key)
        legacy.save(args.output_dir / "results.json", report, key)
        print(json.dumps({"ordinal": ordinal, "arm": item["arm"], "repetition": item["repetition"],
                          "index": item["index"], "usage": record.get("measured_usage"), "cost": record.get("cost"),
                          "first_text_ms": record.get("first_text_ms"), "complete_text_sample": record.get("complete_text_sample"),
                          "stopped": report["stop_reason"] is not None}, ensure_ascii=False), flush=True)
        if report["stop_reason"]:
            break
    print(json.dumps({"output_dir": str(args.output_dir.resolve()), "calls": len(report["results"]),
                      "totals": report.get("totals"), "stopped": report["stop_reason"] is not None}, ensure_ascii=False))
    return 1 if report["stop_reason"] else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    compare = sub.add_parser("compare", help="Offline request comparison; no provider calls")
    live = sub.add_parser("live-replay", help="Paid bounded request comparison with complete output capture")
    for command in (compare, live):
        command.add_argument("--baseline-dir", type=Path, required=True)
        command.add_argument("--candidate-dir", type=Path, required=True)
        command.add_argument("--indexes", type=legacy.indexes, required=True)
        command.add_argument("--protocol", choices=("native", "react"), default="native")
    compare.add_argument("--output", type=Path)
    live.add_argument("--env-file", type=Path, default=ROOT / ".env")
    live.add_argument("--rates-file", type=Path, required=True)
    live.add_argument("--output-dir", type=Path, required=True)
    live.add_argument("--sample-description", required=True)
    live.add_argument("--repetitions", type=int, default=2)
    live.add_argument("--max-calls", type=int, default=32)
    live.add_argument("--output-token-cap", type=int, default=2048)
    live.add_argument("--delay-seconds", type=float, default=2)
    live.add_argument("--timeout-seconds", type=float, default=120)
    live.add_argument("--disable-thinking", action="store_true")
    live.add_argument("--answer-only", action="store_true", help="Separate Native quality probe: force tool_choice=none while preserving tool definitions and evidence")
    args = parser.parse_args()
    try:
        if args.command == "live-replay":
            return run_live(args)
        report = legacy.compare_samples(legacy.load_samples(args.baseline_dir, args.protocol, args.indexes),
            legacy.load_samples(args.candidate_dir, args.protocol, args.indexes), args.protocol)
        if args.output:
            legacy.save(args.output, report)
        else:
            print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    except (OSError, ValueError) as error:
        parser.exit(1, f"Workspace cache probe: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
