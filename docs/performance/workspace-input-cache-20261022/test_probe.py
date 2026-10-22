"""Local HTTP checks for timing, complete output, isolation, usage and prices."""

import argparse
import copy
import io
import json
from pathlib import Path
import tempfile
import threading
import time
import unittest
from contextlib import redirect_stdout
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import patch

import probe


def sample(index):
    return {"index": index, "path": f"fixture-{index}", "body": {"model": "fixture", "messages": [
        {"role": "system", "content": "Stable instruction"}, {"role": "user", "content": "Explain this."}]}}


def rate():
    return {"version": "local-test", "provider": "http://localhost", "model": "fixture", "starts_at": 0,
            "expires_at": 9999999999, "input_micro_cny_per_million": 2_000_000,
            "cached_input_micro_cny_per_million": 200_000, "output_micro_cny_per_million": 3_000_000,
            "source": {"currency": "CNY", "original_prices": "test only", "cny_conversion": "1",
                       "checked_on": "2026-10-22", "official_url": "https://example.invalid/test"}}


class ProbeTests(unittest.TestCase):
    def test_answer_only_changes_only_native_tool_choice(self):
        arms = {arm: [sample(0), sample(1)] for arm in ("baseline", "candidate")}
        for rows in arms.values():
            for row in rows:
                row["body"]["tool_choice"] = "auto"
                row["body"]["tools"] = [{"type": "function", "function": {"name": "source_present", "parameters": {"type": "object"}}}]
                row["body"]["messages"].append({"role": "tool", "content": "Previously read source evidence."})
        prepared = probe.prepare_requests(arms, {"FLUID_LLM_MODEL": "configured"}, "test", 1, 2048, 4)
        unchanged = copy.deepcopy(prepared)
        self.assertTrue(all(row["body"]["tool_choice"] == "auto" for row in prepared))
        probe.apply_answer_only(prepared, "native")
        for before, after in zip(unchanged, prepared):
            before["body"]["tool_choice"] = "none"
            self.assertEqual(before, after)
        with self.assertRaisesRegex(ValueError, "requires native"):
            probe.apply_answer_only(prepared, "react")

    def test_balanced_order_independent_prefixes_and_call_cap(self):
        arms = {arm: [sample(0), sample(1)] for arm in ("baseline", "candidate")}
        original = copy.deepcopy(arms)
        rows = probe.prepare_requests(arms, {"FLUID_LLM_MODEL": "configured"}, "test", 2, 2048, 8)
        self.assertEqual(arms, original)
        self.assertEqual([(r["repetition"], r["arm"], r["index"]) for r in rows], [
            (1, "baseline", 0), (1, "baseline", 1), (1, "candidate", 0), (1, "candidate", 1),
            (2, "candidate", 0), (2, "candidate", 1), (2, "baseline", 0), (2, "baseline", 1)])
        prefixes = [r["body"]["messages"][0]["content"] for r in rows]
        self.assertEqual(prefixes[0], prefixes[1])
        self.assertEqual(len(set(prefixes)), 4)
        self.assertEqual(rows[0]["body"]["max_tokens"], 2048)
        with self.assertRaisesRegex(ValueError, "exceed max-calls"):
            probe.prepare_requests(arms, {"FLUID_LLM_MODEL": "configured"}, "test", 2, 2048, 7)

    def test_stream_timing_is_first_content_not_reasoning_and_preserves_tools(self):
        captured = []

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                captured.append((self.headers["Authorization"], json.loads(self.rfile.read(int(self.headers["Content-Length"])))))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                events = [
                    {"choices": [{"index": 0, "delta": {"role": "assistant"}}]},
                    {"choices": [{"index": 0, "delta": {"reasoning_content": "private deliberation"}}]},
                    {"choices": [{"index": 0, "delta": {"content": "梯度"}}]},
                    {"choices": [{"index": 0, "delta": {"content": "通过链式法则相乘。"}}]},
                    {"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0, "id": "call-1", "type": "function", "function": {"name": "source_present", "arguments": '{"id":'}}]}}]},
                    {"choices": [{"index": 0, "delta": {"tool_calls": [{"index": 0, "function": {"arguments": '"source-1"}'}}]}, "finish_reason": "tool_calls"}]},
                    {"id": "response-1", "model": "fixture", "choices": [], "usage": {"prompt_tokens": 1000, "prompt_cache_hit_tokens": 800, "completion_tokens": 20}},
                ]
                for event in events:
                    self.wfile.write(b"data: " + json.dumps(event, ensure_ascii=False).encode() + b"\n\n")
                    self.wfile.flush()
                    time.sleep(0.012)
                self.wfile.write(b"data: [DONE]\n\n")

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            result = probe.send_request(f"http://127.0.0.1:{server.server_port}", "test-secret", sample(0)["body"])
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
        self.assertEqual(result["choices"][0]["content"], "梯度通过链式法则相乘。")
        self.assertLess(result["first_reasoning_ms"], result["first_text_ms"])
        self.assertLess(result["first_text_ms"], result["first_tool_ms"])
        self.assertLess(result["first_tool_ms"], result["duration_ms"])
        self.assertEqual(result["first_delta_ms"], result["first_reasoning_ms"])
        self.assertEqual(json.loads(result["choices"][0]["tool_calls"][0]["function"]["arguments"]), {"id": "source-1"})
        self.assertTrue(result["response_complete"])
        self.assertFalse(result["complete_text_sample"])
        self.assertNotIn("private deliberation", json.dumps(result))
        self.assertEqual(captured[0][0], "Bearer test-secret")
        self.assertNotIn("test-secret", json.dumps(captured[0][1]))

    def test_price_components_do_not_double_count_cached_input(self):
        usage = probe.measured_usage({"usage": {"prompt_tokens": 1000, "prompt_cache_hit_tokens": 800, "completion_tokens": 20}})
        self.assertEqual(usage["uncached_input_tokens"], 200)
        self.assertEqual(probe.priced_usage(usage, rate()), {"input_cny": "0.00056", "output_cny": "0.00006", "total_cny": "0.00062", "total_rounded_micro_cny": 620})
        with self.assertRaisesRegex(ValueError, "inconsistent"):
            probe.measured_usage({"usage": {"prompt_tokens": 10, "prompt_cache_hit_tokens": 11, "completion_tokens": 1}})

    def test_rates_must_match_provider_model_and_valid_time(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rates.json"
            probe.legacy.save(path, {"rates": [rate()]})
            config = {"OPENCODE_BASE_URL": "http://localhost/", "FLUID_LLM_MODEL": "fixture"}
            self.assertEqual(probe.load_rate(path, config, 1), rate())
            with self.assertRaisesRegex(ValueError, "exactly one current"):
                probe.load_rate(path, config, 9999999999)
            config["FLUID_LLM_MODEL"] = "other"
            with self.assertRaisesRegex(ValueError, "exactly one current"):
                probe.load_rate(path, config, 1)

    def test_missing_usage_stops_without_retry_and_redacts_secret(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            for index in (0, 1):
                probe.legacy.save(source / f"native-{index:02}.json", sample(index)["body"])
            probe.legacy.save(root / "rates.json", rate())
            args = argparse.Namespace(env_file=root / "none", baseline_dir=source, candidate_dir=source,
                indexes=[0, 1], protocol="native", rates_file=root / "rates.json", repetitions=2,
                output_token_cap=2048, max_calls=8, delay_seconds=0, timeout_seconds=10,
                disable_thinking=False, output_dir=root / "run", sample_description="local test")
            calls = []

            def sender(endpoint, key, body, timeout):
                calls.append(body)
                return {"usage": None, "response_id": key, "response_complete": True}

            with patch.dict(probe.legacy.os.environ, {"OPENCODE_API_KEY": "test-secret", "OPENCODE_BASE_URL": "http://localhost", "FLUID_LLM_MODEL": "fixture"}), redirect_stdout(io.StringIO()):
                self.assertEqual(probe.run_live(args, sender), 1)
            self.assertEqual(len(calls), 1)
            report = json.loads((args.output_dir / "results.json").read_text(encoding="utf-8"))
            self.assertIn("usage missing", report["stop_reason"])
            self.assertEqual(report["results"][0]["response_id"], "[REDACTED]")
            self.assertEqual(report["totals"]["candidate"]["all"]["calls_with_usage"], 0)
            self.assertTrue(all("test-secret" not in p.read_text(encoding="utf-8") for p in args.output_dir.glob("*.json")))

    def test_each_request_selects_its_rate_and_keeps_first_continuation_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            for index in (0, 1):
                probe.legacy.save(source / f"native-{index:02}.json", sample(index)["body"])
            early, later = rate(), rate()
            early["expires_at"] = 5
            later.update(version="later", starts_at=5, input_micro_cny_per_million=4_000_000)
            probe.legacy.save(root / "rates.json", {"rates": [early, later]})
            args = argparse.Namespace(env_file=root / "none", baseline_dir=source, candidate_dir=source,
                indexes=[0, 1], protocol="native", rates_file=root / "rates.json", repetitions=2,
                output_token_cap=2048, max_calls=8, delay_seconds=0, timeout_seconds=10,
                disable_thinking=False, output_dir=root / "run", sample_description="local test")

            def sender(*_):
                return {"usage": {"prompt_tokens": 1000, "prompt_cache_hit_tokens": 800, "completion_tokens": 20},
                        "response_complete": True, "complete_text_sample": True, "first_text_ms": 12, "duration_ms": 30}

            with patch.dict(probe.legacy.os.environ, {"OPENCODE_API_KEY": "test-secret", "OPENCODE_BASE_URL": "http://localhost", "FLUID_LLM_MODEL": "fixture"}), \
                    patch.object(probe.time, "time", side_effect=[1, 1, 2, 3, 4, 5, 6, 7, 8]), redirect_stdout(io.StringIO()):
                self.assertEqual(probe.run_live(args, sender), 0)
            report = json.loads((args.output_dir / "results.json").read_text(encoding="utf-8"))
            self.assertEqual([r["price_version"] for r in report["results"]], ["local-test"] * 4 + ["later"] * 4)
            self.assertEqual(len(report["price_snapshots"]), 2)
            for arm in ("baseline", "candidate"):
                self.assertEqual(report["totals"][arm]["first_requests"]["calls"], 2)
                self.assertEqual(report["totals"][arm]["continuation_requests"]["calls"], 2)
                self.assertEqual(report["totals"][arm]["all"]["timings"]["first_text_ms"]["samples"], 4)
                self.assertEqual(report["totals"][arm]["all"]["input_tokens"], 4000)
                self.assertEqual(report["totals"][arm]["all"]["cost"]["total_cny"], "0.00328")


if __name__ == "__main__":
    unittest.main()
