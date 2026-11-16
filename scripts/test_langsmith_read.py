import copy
import io
import json
import unittest
import urllib.error
import urllib.parse
from unittest.mock import Mock, patch

from langsmith_read import LangSmithReader, report_run


class SmithDBReadTests(unittest.TestCase):
    def reader(self, responses, endpoint="https://api.smith.langchain.com"):
        calls = []

        def request(path, body=None):
            calls.append((path, copy.deepcopy(body)))
            return responses.pop(0)

        client = LangSmithReader({
            "LANGSMITH_API_KEY": "unit-test-key",
            "LANGSMITH_WORKSPACE_ID": "workspace",
            "LANGSMITH_PROJECT": "test-project",
            "LANGSMITH_ENDPOINT": endpoint,
        }, request_json=request)
        return client, calls

    def test_weekly_report_keeps_time_window_filters_and_all_pages(self):
        client, calls = self.reader([
            {"items": [{"id": "first", "run_type": "LLM", "status": "SUCCESS"}],
             "next_cursor": "second-page"},
            {"items": [{"id": "second", "total_tokens": 0}], "next_cursor": None},
        ])
        query = {"session": ["project"], "start_time": "2026-10-01T00:00:00Z",
                 "max_start_time": "2026-10-08T00:00:00Z", "run_type": "llm",
                 "select": ["id", "run_type", "status", "total_tokens"], "limit": 100,
                 "filter": 'eq(metadata_key, "user_id")'}
        original = copy.deepcopy(query)
        runs = list(client.iter_runs(query))
        self.assertEqual(query, original)
        self.assertEqual([run["id"] for run in runs], ["first", "second"])
        self.assertEqual(runs[0]["run_type"], "llm")
        self.assertEqual(runs[0]["status"], "success")
        self.assertEqual(runs[1]["total_tokens"], 0)
        self.assertEqual(calls[0], ("/api/v2/runs/query", {
            "project_ids": ["project"], "min_start_time": "2026-10-01T00:00:00Z",
            "max_start_time": "2026-10-08T00:00:00Z", "run_type": "LLM",
            "selects": ["ID", "RUN_TYPE", "STATUS", "TOTAL_TOKENS"], "page_size": 100,
            "filter": query["filter"],
        }))
        self.assertEqual(calls[1][1], {**calls[0][1], "cursor": "second-page"})

    def test_old_run_ids_are_project_scoped_without_a_24_hour_cutoff(self):
        client, calls = self.reader([
            [{"name": "test-project", "id": "project"}],
            {"items": [{"id": "old"}], "next_cursor": None},
            {"items": [], "next_cursor": None},
        ])
        client.query_runs({"id": ["old"], "start_time": "2026-10-01T00:00:00Z", "select": ["id", "outputs"]})
        client.query_runs({"id": ["other"], "start_time": "2026-10-01T00:00:00Z"})
        self.assertTrue(calls[0][0].startswith("/api/v1/sessions?"))
        self.assertEqual(calls[1][1]["project_ids"], ["project"])
        self.assertEqual(calls[1][1]["ids"], ["old"])
        self.assertEqual(calls[1][1]["min_start_time"], "2026-10-01T00:00:00Z")
        self.assertEqual(calls[1][1]["selects"], ["ID", "OUTPUTS"])
        self.assertEqual(len(calls), 3, "project resolution should be cached")

    def test_parent_and_project_aliases_preserve_the_immediate_parent(self):
        client, calls = self.reader([{"items": [{
            "id": "child", "project_id": "project", "parent_run_ids": ["root", "parent"],
            "total_tokens": 10, "prompt_token_details": {"raw": {"cache_read": 0}},
        }], "next_cursor": None}])
        row = client.query_runs({"session": ["project"], "start_time": "2026-10-01T00:00:00Z",
                                 "select": ["parent_run_id", "session_id", "total_tokens"]})["runs"][0]
        self.assertEqual(calls[0][1]["selects"], ["PARENT_RUN_IDS", "PROJECT_ID", "TOTAL_TOKENS"])
        self.assertEqual(row["parent_run_id"], "parent")
        self.assertEqual(row["session_id"], "project")
        self.assertEqual(row["prompt_token_details"], {"cache_read": 0})

    def test_run_usage_is_not_replaced_by_trace_totals_or_invented_zeros(self):
        root = report_run({"id": "root", "parent_run_ids": [], "total_tokens": None,
                           "total_cost": None, "trace_aggregates": {"total_tokens": 500}})
        child = report_run({"id": "llm", "parent_run_ids": ["root"], "total_tokens": 500})
        self.assertIsNone(root["parent_run_id"])
        self.assertIsNone(root["total_tokens"])
        self.assertIsNone(root["total_cost"])
        self.assertEqual(child["total_tokens"], 500)
        self.assertEqual(child["usage_scope"], "run")
        self.assertNotIn("total_tokens", report_run({"id": "unknown"}))
        self.assertNotIn("parent_run_id", report_run({"id": "unknown"}))

    def test_retrieve_uses_explicit_canary_project_and_repeated_selects(self):
        client, calls = self.reader([
            [{"name": "canary", "id": "canary-project"}],
            {"id": "run", "status": "SUCCESS"},
        ])
        run = client.read_run("run", project_name="canary", fields=["id", "status", "extra"],
                              start_time="2026-10-01T00:00:00Z")
        parsed = urllib.parse.urlsplit(calls[1][0])
        params = urllib.parse.parse_qs(parsed.query)
        self.assertEqual(parsed.path, "/api/v2/runs/run")
        self.assertEqual(params["project_id"], ["canary-project"])
        self.assertEqual(params["selects"], ["ID", "STATUS", "EXTRA"])
        self.assertEqual(run["status"], "success")

    def test_empty_page_with_cursor_does_not_end_pagination(self):
        client, calls = self.reader([
            {"items": [], "next_cursor": "next"},
            {"items": [{"id": "later"}], "next_cursor": None},
        ])
        self.assertEqual([r["id"] for r in client.iter_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"})], ["later"])
        self.assertEqual(calls[0][1]["max_start_time"], calls[1][1]["max_start_time"])

    def test_repeated_cursor_fails_instead_of_looping_forever(self):
        client, _ = self.reader([
            {"items": [], "next_cursor": "same"}, {"items": [], "next_cursor": "same"},
        ])
        with self.assertRaisesRegex(ValueError, "repeated"):
            list(client.iter_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"}))

    def test_old_or_malformed_response_is_not_mistaken_for_an_empty_report(self):
        for response in ({"runs": []}, {}, [], {"items": None}):
            client, _ = self.reader([response])
            with self.subTest(response=response), self.assertRaisesRegex(ValueError, "SmithDB response"):
                client.query_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"})

    def test_removed_or_conflicting_fields_fail_before_sending_a_request(self):
        for query in ({"parent_run": "p"}, {"query": "text"}, {"order": "asc"},
                      {"session": ["p"], "project_ids": ["p"]},
                      {"session": ["p"], "reference_dataset_id": "d"},
                      {"session": ["p"], "limit": 1001}):
            client, calls = self.reader([])
            with self.subTest(query=query), self.assertRaises(ValueError):
                client.query_runs({"start_time": "2026-10-01T00:00:00Z", **query})
            self.assertEqual(calls, [])

    def test_http_requests_use_v2_once_with_workspace_auth_and_no_content_by_default(self):
        client, _ = self.reader([], "https://example.test/prefix/api/v1/")
        client._request_json = client._http_json
        client._opener.open = Mock(return_value=io.BytesIO(json.dumps({"items": []}).encode()))
        client.query_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"})
        request = client._opener.open.call_args.args[0]
        self.assertEqual(request.full_url, "https://example.test/prefix/api/v2/runs/query")
        self.assertEqual(request.get_header("X-api-key"), "unit-test-key")
        self.assertEqual(request.get_header("X-tenant-id"), "workspace")
        body = json.loads(request.data)
        self.assertNotIn("INPUTS", body["selects"])
        self.assertNotIn("OUTPUTS", body["selects"])

    def test_auth_failure_is_not_retried_or_logged_with_private_body(self):
        client, _ = self.reader([])
        client._request_json = client._http_json
        client._opener.open = Mock(side_effect=urllib.error.HTTPError(
            client.base, 401, "Unauthorized", {}, io.BytesIO(b"private trace")))
        with self.assertRaisesRegex(RuntimeError, "^LangSmith HTTP 401$"):
            client.query_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"})
        self.assertEqual(client._opener.open.call_count, 1)

    def test_rate_limit_retries_same_read_with_a_bounded_delay(self):
        client, _ = self.reader([])
        client._request_json = client._http_json
        client._opener.open = Mock(side_effect=[
            urllib.error.HTTPError(client.base, 429, "Limited", {"Retry-After": "90"}, io.BytesIO()),
            io.BytesIO(b'{"items": [], "next_cursor": null}'),
        ])
        with patch("langsmith_read.time.sleep") as sleep:
            client.query_runs({"session": ["p"], "start_time": "2026-10-01T00:00:00Z"})
        sleep.assert_called_once_with(30)
        self.assertEqual(client._opener.open.call_count, 2)
        self.assertIs(client._opener.open.call_args_list[0].args[0],
                      client._opener.open.call_args_list[1].args[0])

    def test_missing_time_range_cannot_silently_drop_history_or_query_from_epoch(self):
        client, calls = self.reader([])
        with self.assertRaisesRegex(ValueError, "Supply start_time"):
            client.query_runs({"session": ["p"], "id": ["historical"]})
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
