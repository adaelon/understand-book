-- Search and page reads are physical requests, not model tokens or video time.
CREATE TABLE web_usage (
    usage_id TEXT PRIMARY KEY NOT NULL,
    logical_usage_id TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK(attempt > 0),
    run_ref TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    period_id TEXT NOT NULL REFERENCES allowance_periods(period_id),
    learning_workspace_id TEXT,
    service_kind TEXT NOT NULL CHECK(service_kind IN ('web_search','web_fetch')),
    rate_snapshot TEXT NOT NULL CHECK(json_valid(rate_snapshot)),
    state TEXT NOT NULL CHECK(state IN ('sent','pending','settled','released')),
    reserved_micro_cny INTEGER NOT NULL CHECK(reserved_micro_cny >= 0),
    account_debit_micro_cny INTEGER CHECK(account_debit_micro_cny >= 0),
    provider_cost_micro_cny INTEGER CHECK(provider_cost_micro_cny >= 0),
    provider_cost_status TEXT NOT NULL CHECK(provider_cost_status IN ('confirmed','pending')),
    evidence_json TEXT CHECK(evidence_json IS NULL OR json_valid(evidence_json)),
    created_at INTEGER NOT NULL,
    settled_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0,
    needs_reconciliation INTEGER NOT NULL DEFAULT 0 CHECK(needs_reconciliation IN (0,1)),
    UNIQUE(run_ref, logical_usage_id, attempt)
) STRICT;
CREATE INDEX web_usage_period ON web_usage(period_id);
CREATE INDEX web_usage_run ON web_usage(run_ref);
CREATE TABLE web_usage_receipts (
    usage_id TEXT NOT NULL REFERENCES web_usage(usage_id),
    report_json TEXT NOT NULL CHECK(json_valid(report_json)),
    received_at INTEGER NOT NULL,
    PRIMARY KEY(usage_id, report_json)
) STRICT;
CREATE TABLE web_usage_reconciliations (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    usage_id TEXT NOT NULL REFERENCES web_usage(usage_id),
    before_json TEXT NOT NULL CHECK(json_valid(before_json)),
    after_json TEXT NOT NULL CHECK(json_valid(after_json)),
    account_delta_micro_cny INTEGER NOT NULL,
    provider_delta_micro_cny INTEGER,
    reason TEXT NOT NULL,
    evidence TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id)
) STRICT;

DROP VIEW billable_usage;
CREATE VIEW billable_usage AS
SELECT call_id,'model' AS entry_kind,cost_owner,user_id,period_id,run_ref,task_ref,parent_task_ref,
       learning_workspace_id,related_learning_workspace_ids,purpose,state,reserved_micro_cny,
       account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,needs_reconciliation,
       created_at,settled_at
FROM model_call_charges
UNION ALL
SELECT usage_id,'service',cost_owner,user_id,period_id,run_ref,task_ref,parent_task_ref,
       learning_workspace_id,'[]',CASE WHEN run_ref IS NOT NULL THEN 'video_audio_review' ELSE service_kind END,state,reserved_micro_cny,
       account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,0,created_at,settled_at
FROM service_usage
UNION ALL
SELECT usage_id,service_kind,'account',user_id,period_id,run_ref,NULL,run_ref,
       learning_workspace_id,'[]',service_kind,state,reserved_micro_cny,
       account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,needs_reconciliation,
       created_at,settled_at
FROM web_usage;
