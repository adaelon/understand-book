-- C4. Processing time is a service quantity, never model tokens.
CREATE TABLE service_usage (
    usage_id TEXT PRIMARY KEY NOT NULL,
    logical_usage_id TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK(attempt > 0),
    task_ref TEXT REFERENCES build_task_budgets(task_ref),
    run_ref TEXT,
    parent_task_ref TEXT NOT NULL,
    cost_owner TEXT NOT NULL CHECK(cost_owner IN ('account','platform')),
    user_id TEXT REFERENCES users(user_id),
    period_id TEXT REFERENCES allowance_periods(period_id),
    learning_workspace_id TEXT,
    service_kind TEXT NOT NULL CHECK(service_kind = 'video_transcription'),
    admitted_quantity_ms INTEGER NOT NULL CHECK(admitted_quantity_ms > 0),
    quantity_ms INTEGER,
    rate_snapshot TEXT NOT NULL CHECK(json_valid(rate_snapshot)),
    admission_json TEXT NOT NULL CHECK(json_valid(admission_json)),
    state TEXT NOT NULL CHECK(state IN ('sent','pending','settled','released')),
    reserved_micro_cny INTEGER NOT NULL CHECK(reserved_micro_cny >= 0),
    service_charge_micro_cny INTEGER,
    account_debit_micro_cny INTEGER,
    provider_cost_micro_cny INTEGER,
    provider_cost_status TEXT NOT NULL CHECK(provider_cost_status IN ('confirmed','estimated','pending')),
    evidence_json TEXT CHECK(evidence_json IS NULL OR json_valid(evidence_json)),
    created_at INTEGER NOT NULL,
    settled_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0,
    UNIQUE(task_ref, logical_usage_id, attempt),
    CHECK((task_ref IS NOT NULL AND run_ref IS NULL) OR (task_ref IS NULL AND run_ref IS NOT NULL)),
    CHECK((cost_owner='account' AND user_id IS NOT NULL AND period_id IS NOT NULL)
       OR (cost_owner='platform' AND user_id IS NULL AND period_id IS NULL))
) STRICT;
CREATE INDEX service_usage_period ON service_usage(period_id);
CREATE INDEX service_usage_task ON service_usage(task_ref);
CREATE TABLE service_usage_receipts (
    usage_id TEXT NOT NULL REFERENCES service_usage(usage_id),
    report_json TEXT NOT NULL CHECK(json_valid(report_json)),
    received_at INTEGER NOT NULL,
    PRIMARY KEY(usage_id, report_json)
) STRICT;

-- Read-only reporting projection; each source retains its own usage contract.
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
FROM service_usage;
