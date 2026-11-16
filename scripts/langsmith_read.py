"""SmithDB reads for local LangSmith diagnostics (Python standard library only).

The report adapter accepts existing snake_case report queries and returns their
``runs/cursors`` shape, but sends only v2 run requests. Run usage is always the
run's own usage: sum LLM spans for reports, never add parent and child totals.
"""

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import time
import urllib.error
import urllib.parse
import urllib.request


DEFAULT_FIELDS = [
    "id", "name", "run_type", "status", "start_time", "end_time", "error",
    "parent_run_id", "trace_id", "session_id", "extra", "tags", "app_path",
    "total_tokens", "prompt_tokens", "completion_tokens", "total_cost",
    "prompt_cost", "completion_cost", "prompt_token_details",
    "completion_token_details", "prompt_cost_details", "completion_cost_details",
    "price_model_id", "first_token_time",
]
FIELD_ALIASES = {"PARENT_RUN_ID": "PARENT_RUN_IDS", "SESSION_ID": "PROJECT_ID"}
QUERY_ALIASES = {
    "session": "project_ids", "start_time": "min_start_time", "trace": "trace_id",
    "id": "ids", "error": "has_error", "limit": "page_size", "select": "selects",
}
QUERY_FIELDS = {
    "project_ids", "reference_dataset_id", "min_start_time", "max_start_time",
    "trace_id", "ids", "has_error", "page_size", "selects", "run_type",
    "filter", "trace_filter", "tree_filter", "is_root", "cursor", "reference_examples",
}


def read_config(filename=None):
    values = {}
    if filename is not None:
        for line in Path(filename).read_text(encoding="utf-8-sig").splitlines():
            if "=" in line and not line.lstrip().startswith("#"):
                key, value = line.split("=", 1)
                values[key.strip()] = value.strip().strip('"').strip("'")
    values.update({key: value for key, value in os.environ.items()
                   if key.startswith("LANGSMITH_")})
    return values


def select_fields(fields):
    if not fields or isinstance(fields, str):
        raise ValueError("Select at least one run field")
    return list(dict.fromkeys(FIELD_ALIASES.get(field.upper(), field.upper())
                              for field in fields))


def report_run(run):
    """Retain report aliases without fabricating ancestor usage or unknown values."""
    result = dict(run)
    for key in ("run_type", "status"):
        if isinstance(result.get(key), str):
            result[key] = result[key].lower()
    if "parent_run_ids" in result and result["parent_run_ids"] is not None:
        parents = result["parent_run_ids"]
        result["parent_run_id"] = parents[-1] if parents else None
    elif result.get("is_root") is True:
        result["parent_run_id"] = None
    if "project_id" in result:
        result["session_id"] = result["project_id"]
    for key in ("prompt_token_details", "completion_token_details",
                "prompt_cost_details", "completion_cost_details"):
        value = result.get(key)
        if isinstance(value, dict) and set(value) == {"raw"}:
            result[key] = value["raw"]
    result["usage_scope"] = "run"
    return result


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class LangSmithReader:
    def __init__(self, config, *, timeout=30, request_json=None):
        self.config = dict(config)
        endpoint = self.config.get("LANGSMITH_ENDPOINT") or "https://api.smith.langchain.com"
        parsed = urllib.parse.urlsplit(endpoint)
        if (parsed.scheme != "https" or not parsed.hostname or parsed.username
                or parsed.password or parsed.query or parsed.fragment):
            raise ValueError("LANGSMITH_ENDPOINT must be an HTTPS URL without credentials or a query")
        prefix = parsed.path.rstrip("/")
        for version in ("/api/v1", "/api/v2"):
            if prefix.endswith(version):
                prefix = prefix[:-len(version)]
                break
        self.base = urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, prefix, "", ""))
        if not self.config.get("LANGSMITH_API_KEY"):
            raise ValueError("LANGSMITH_API_KEY is required")
        self.headers = {"x-api-key": self.config["LANGSMITH_API_KEY"],
                        "Content-Type": "application/json"}
        if self.config.get("LANGSMITH_WORKSPACE_ID"):
            self.headers["X-Tenant-Id"] = self.config["LANGSMITH_WORKSPACE_ID"]
        self.timeout = timeout
        self._opener = urllib.request.build_opener(_NoRedirect())
        self._request_json = request_json or self._http_json
        self._projects = {}

    def _http_json(self, path, body=None):
        request = urllib.request.Request(
            self.base + path, headers=self.headers,
            data=None if body is None else json.dumps(body).encode("utf-8"),
        )
        for attempt in range(4):
            try:
                with self._opener.open(request, timeout=self.timeout) as response:
                    return json.load(response)
            except urllib.error.HTTPError as error:
                status = error.code
                retry_after = error.headers.get("Retry-After", "")
                error.close()
                if status in (429, 500, 502, 503, 504) and attempt < 3:
                    try:
                        delay = max(0, min(float(retry_after), 30))
                    except ValueError:
                        delay = 2 ** attempt
                    time.sleep(delay)
                    continue
                # Do not print server bodies: they may contain private trace data.
                raise RuntimeError(f"LangSmith HTTP {status}") from None

    def project_id(self, name=None):
        name = name or self.config.get("LANGSMITH_PROJECT")
        if not name:
            raise ValueError("Supply a project UUID or LANGSMITH_PROJECT")
        if name not in self._projects:
            query = urllib.parse.urlencode({"name": name, "limit": 100})
            projects = self._request_json("/api/v1/sessions?" + query)
            matches = [project["id"] for project in projects if project.get("name") == name]
            if len(matches) != 1:
                raise ValueError("LangSmith project name did not resolve to exactly one project")
            self._projects[name] = matches[0]
        return self._projects[name]

    def query_runs(self, query):
        """Read one v2 page, with compatibility aliases for the local reports."""
        body = {}
        for key, value in query.items():
            target = QUERY_ALIASES.get(key, key)
            if target not in QUERY_FIELDS or target in body:
                raise ValueError(f"Unsupported or duplicate run query field: {key}")
            body[target] = value
        # v2 defaults to only 24h and also rejects excessively wide ranges. An
        # explicit lower bound prevents both silent truncation and epoch queries.
        if not body.get("min_start_time"):
            raise ValueError("Supply start_time or min_start_time; use read_run for IDs with unknown dates")
        if not body.get("project_ids") and not body.get("reference_dataset_id"):
            body["project_ids"] = [self.project_id()]
        if body.get("project_ids") and body.get("reference_dataset_id"):
            raise ValueError("Choose project_ids or reference_dataset_id")
        body.setdefault("max_start_time", dt.datetime.now(dt.timezone.utc).isoformat())
        body["selects"] = select_fields(body.get("selects", DEFAULT_FIELDS))
        if body.get("run_type"):
            body["run_type"] = body["run_type"].upper()
        page_size = body.setdefault("page_size", 100)
        if isinstance(page_size, bool) or not isinstance(page_size, int) or not 1 <= page_size <= 1000:
            raise ValueError("page_size must be an integer from 1 to 1000")
        response = self._request_json("/api/v2/runs/query", body)
        if not isinstance(response, dict) or not isinstance(response.get("items"), list):
            raise ValueError("Unexpected SmithDB response: expected items and next_cursor")
        return {"runs": [report_run(run) for run in response["items"]],
                "cursors": {"next": response.get("next_cursor")}, "usage_scope": "run"}

    def iter_runs(self, query):
        query = dict(query)
        query.setdefault("max_start_time", dt.datetime.now(dt.timezone.utc).isoformat())
        seen = set()
        if query.get("cursor"):
            seen.add(query["cursor"])
        while True:
            page = self.query_runs(query)
            yield from page["runs"]
            cursor = page["cursors"]["next"]
            if not cursor:
                return
            if cursor in seen:
                raise ValueError("LangSmith returned a repeated pagination cursor")
            seen.add(cursor)
            query["cursor"] = cursor

    def read_run(self, run_id, *, project_id=None, project_name=None, fields=None, start_time=None):
        params = [("project_id", project_id or self.project_id(project_name))]
        params.extend(("selects", field) for field in select_fields(
            DEFAULT_FIELDS if fields is None else fields))
        if start_time:
            params.append(("start_time", start_time))
        path = "/api/v2/runs/" + urllib.parse.quote(run_id, safe="")
        return report_run(self._request_json(path + "?" + urllib.parse.urlencode(params)))


def main():
    parser = argparse.ArgumentParser(description="Check LangSmith SmithDB reads without writing traces")
    parser.add_argument("--env-file", type=Path, help="Optional private dotenv file")
    parser.add_argument("--project-id")
    parser.add_argument("--project-name")
    args = parser.parse_args()
    config = read_config(args.env_file)
    if args.project_name:
        config["LANGSMITH_PROJECT"] = args.project_name
    reader = LangSmithReader(config)
    project = args.project_id or reader.project_id()
    now = dt.datetime.now(dt.timezone.utc)
    page = reader.query_runs({
        "project_ids": [project], "min_start_time": (now - dt.timedelta(days=7)).isoformat(),
        "max_start_time": now.isoformat(), "page_size": 1,
        "selects": ["ID", "PROJECT_ID", "START_TIME", "STATUS"],
    })
    if page["runs"]:
        run = page["runs"][0]
        detail = reader.read_run(run["id"], project_id=project, fields=["ID", "STATUS"],
                                 start_time=run.get("start_time"))
        if detail.get("id") != run["id"]:
            raise ValueError("LangSmith retrieved a different run")
    print(json.dumps({"query": "passed", "retrieve": "passed" if page["runs"] else "no_recent_runs",
                      "sampled_runs": len(page["runs"]), "api_version": 2,
                      "remote_writes": 0, "content_requested": False}))


if __name__ == "__main__":
    main()
