# LangSmith SmithDB migration audit — 2026-11-12

The deprecated calls found in this working directory were Python operations and
analysis scripts under `tmp/`. The production trace exporter does not query runs.
The notification alone does not identify which process triggered it.

## Maintained integration

| Component | Finding / action |
| --- | --- |
| `crates/server/src/observability/langsmith.rs` | Uses `POST /runs` and `PATCH /runs/{id}`. These writes are outside this migration. |
| `packages/observability/src/langsmith-client.ts` | Uses only `createRun` / `updateRun`. Installed and locked TypeScript `langsmith` is `0.10.4`, above the documented minimum `0.8.9`. |
| `packages/observability/src/eval-import.ts` | Uses `/api/v1/datasets/upload-experiment`, not the deprecated dataset-run query endpoint. |
| `scripts/langsmith_read.py` | Shared standard-library Python reader for SmithDB run queries and retrieval. |

The reader sends `POST /api/v2/runs/query` and `GET /api/v2/runs/{id}`.
Project-name resolution still uses the supported `/api/v1/sessions` endpoint.
It preserves existing local report aliases (`runs/cursors`, lowercase status and
run type, `parent_run_id`, `session_id`) without sending the old wire contract.

| Old report convention | SmithDB wire contract |
| --- | --- |
| `session` | `project_ids`; required even for ID queries |
| `start_time` | `min_start_time`; explicitly required by the reader |
| `limit` | `page_size`; a per-page size, not a total-result limit |
| `select` / lowercase fields | `selects` / uppercase fields |
| `trace`, `id`, `error` | `trace_id`, `ids`, `has_error` |
| `runs`, `cursors.next` | `items`, `next_cursor` |
| Single-run GET without context | Explicit `project_id` and repeated `selects` parameters |

`iter_runs` freezes the upper time bound, follows every cursor (including empty
pages with a next cursor), and rejects a repeating cursor or malformed response.
There is no fallback to deprecated endpoints.

Queries must specify a lower time bound. SmithDB otherwise defaults to the last
24 hours. A live check also established that an epoch-to-present query is rejected
with HTTP 400 because the allowed range is at most 401 days. Use bounded windows;
use `read_run` with a project UUID when an individual run's date is unknown.

Run token and cost fields now describe **only that run's own usage**. The reader
marks normalized records with `usage_scope: "run"` and does not copy trace totals
onto parents or invent zero usage. Existing cost reports sum LLM spans. The old
root-versus-descendant-total diagnostic now excludes records with this new scope.
Use the trace API's `trace_aggregates` for whole-trace totals when needed.

## Local operations scripts migrated

The following existing local scripts were adjusted. They are in the repository's
ignored `tmp/` directory; these edits alone will not travel with a Git checkout.
Future diagnostics should import the maintained reader above. Historical data
files and vendored research/release snapshots were not regenerated.

- `langsmith-analysis-20261025/{fetch_snapshot,enrich_roots,analyze}.py`
- `langsmith-cache-20261025/fetch_pairs.py`
- `langsmith-cost-20261029/fetch_costs.py`
- `langsmith-cost-20261031-7days/fetch_recent.py`
- `langsmith-week-20261102/fetch.py`
- `langsmith-review-20261029/read_langsmith.py`
- `langsmith-repair-20261025/{backfill_evaluation,new_record_canary}.py` (read paths)
- `turn-failure-20261019/fetch-traces.py`
- `turn-failure-repair-20261011/trace-summary.py`
- `cache-observation-20261003/read_canary_remote.py`
- `cache-repair-20261003/read_remote_canary.py`
- `langsmith-full-20261009/read_cloud.py`
- `teaching-invalid-20261017/read-reviews.py`
- `visitor-rust-review-20261028/read-artifacts.py`
- `recent-user-questions-20261028.py`

## Verification

```sh
python -m unittest discover -s scripts -p test_langsmith_read.py -v
pnpm --filter @understand-book/observability typecheck
python scripts/langsmith_read.py --env-file .env
```

The last command performs a read-only cloud check: one recent metadata query and
one record retrieval, without requesting inputs/outputs or creating traces.
It prints a small status summary rather than credentials or trace content.

Results: all 13 reader tests and the observability TypeScript typecheck passed.
Live reads also verified the default 25 selected fields, two records older than
24 hours, a second non-overlapping page, historical ID filtering, root-parent
aliases, and single-run retrieval. No remote writes were made. Python syntax
checks covered the local diagnostic scripts and their embedded remote programs.

The existing observability Vitest suite was attempted with both fork and thread
pools. This environment blocks required child-process creation (`spawn EPERM`),
so neither attempt executed its tests; this is not recorded as a passing suite.

## IP-address migration

The project configuration uses LangSmith DNS hostnames. No LangSmith destination
IP allowlist was found in versioned deployment scripts. The live server firewall
and cloud-network rules were not inspected or modified during this local audit.

Only networks restricting outbound destinations by IP need the additions below,
before **2027-02-10**, according to the account's 2026-11-11 (Hong Kong time) email.
Keep current addresses until the transition; from 2027-02-11, remove an old
address only after its hostname no longer resolves to it. Keep the application
endpoint as a hostname rather than replacing it with an IP.

| GCP region | New destination IP |
| --- | --- |
| US | `34.56.47.75` |
| EU | `34.32.254.168` |
| APAC | `34.151.160.163` |

## References

- [Migration overview and SDK versions](https://docs.langchain.com/langsmith/smithdb-sdk-migration)
- [Query fields, paging, time windows and usage semantics](https://docs.langchain.com/langsmith/smithdb-sdk-migration-query-runs)
- [Retrieval project context and field selection](https://docs.langchain.com/langsmith/smithdb-sdk-migration-runs)

Deprecated Cloud read endpoints are scheduled for removal on **2027-01-31**.
