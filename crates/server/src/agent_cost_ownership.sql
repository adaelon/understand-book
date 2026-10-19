-- Q2: one cost ledger for account and platform tasks. Existing identities,
-- amounts, receipts and manual corrections survive the atomic table rebuild.
CREATE TABLE model_call_charges_next (
    call_id TEXT PRIMARY KEY NOT NULL,
    logical_call_id TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK(attempt >= 1),
    user_id TEXT,
    period_id TEXT,
    run_ref TEXT,
    task_ref TEXT,
    purpose TEXT NOT NULL,
    model TEXT NOT NULL,
    rate_snapshot TEXT NOT NULL CHECK(json_valid(rate_snapshot)),
    reservation_estimate TEXT NOT NULL CHECK(json_valid(reservation_estimate)),
    state TEXT NOT NULL CHECK(state IN ('reserved','sent','pending','settled','released')),
    reserved_micro_cny INTEGER NOT NULL CHECK(reserved_micro_cny >= 0),
    provider_usage TEXT CHECK(provider_usage IS NULL OR json_valid(provider_usage)),
    provider_cost_micro_cny INTEGER CHECK(provider_cost_micro_cny >= 0),
    provider_cost_status TEXT NOT NULL CHECK(provider_cost_status IN ('pending','confirmed')),
    account_debit_micro_cny INTEGER CHECK(account_debit_micro_cny >= 0),
    evidence_kind TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    settled_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    needs_reconciliation INTEGER NOT NULL DEFAULT 0 CHECK(needs_reconciliation IN (0,1)),
    send_identity TEXT CHECK(send_identity IS NULL OR json_valid(send_identity)),
    send_outcome TEXT CHECK(send_outcome IS NULL OR json_valid(send_outcome)),
    cost_owner TEXT NOT NULL DEFAULT 'account' CHECK(cost_owner IN ('account','platform')),
    parent_task_ref TEXT,
    learning_workspace_id TEXT,
    related_learning_workspace_ids TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(related_learning_workspace_ids) AND json_type(related_learning_workspace_ids)='array'),
    UNIQUE(logical_call_id, attempt),
    FOREIGN KEY(period_id, user_id) REFERENCES allowance_periods(period_id, user_id),
    FOREIGN KEY(user_id, learning_workspace_id) REFERENCES learning_workspaces(owner_user_id, learning_workspace_id),
    CHECK(run_ref IS NOT NULL OR task_ref IS NOT NULL),
    CHECK(parent_task_ref IS NULL OR length(trim(parent_task_ref)) > 0),
    CHECK((cost_owner='account' AND user_id IS NOT NULL AND period_id IS NOT NULL)
       OR (cost_owner='platform' AND user_id IS NULL AND period_id IS NULL AND learning_workspace_id IS NULL AND related_learning_workspace_ids='[]' AND (account_debit_micro_cny IS NULL OR account_debit_micro_cny=0))),
    CHECK((provider_cost_status='pending' AND provider_cost_micro_cny IS NULL)
       OR (provider_cost_status='confirmed' AND provider_cost_micro_cny IS NOT NULL)),
    CHECK((state IN ('reserved','sent','pending') AND account_debit_micro_cny IS NULL AND settled_at IS NULL)
       OR (state IN ('settled','released') AND account_debit_micro_cny IS NOT NULL AND settled_at IS NOT NULL)),
    CHECK(state <> 'settled' OR provider_cost_status='confirmed'),
    CHECK(state <> 'released' OR account_debit_micro_cny=0)
) STRICT;
INSERT INTO model_call_charges_next (
    rowid,call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at,revision,needs_reconciliation,send_identity,send_outcome,cost_owner,parent_task_ref
) SELECT rowid,call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at,revision,needs_reconciliation,send_identity,send_outcome,'account',coalesce(run_ref,task_ref) FROM model_call_charges;
-- Empty the referencing tables only within this transaction; preserve their
-- schemas and restore their rows after replacing the parent with the same IDs.
CREATE TEMP TABLE q2_charge_reports AS SELECT rowid AS original_rowid,* FROM charge_reports;
CREATE TEMP TABLE q2_charge_reconciliations AS SELECT rowid AS original_rowid,* FROM charge_reconciliations;
DELETE FROM charge_reports;
DELETE FROM charge_reconciliations;
DROP TABLE model_call_charges;
ALTER TABLE model_call_charges_next RENAME TO model_call_charges;
INSERT INTO charge_reports(rowid,call_id,outcome_json,disposition,received_at) SELECT * FROM q2_charge_reports;
INSERT INTO charge_reconciliations(rowid,actor,operation_id,user_id,call_id,before_json,after_json,account_delta_micro_cny,provider_delta_micro_cny,reason,evidence,created_at) SELECT * FROM q2_charge_reconciliations;
DROP TABLE q2_charge_reports;
DROP TABLE q2_charge_reconciliations;
CREATE INDEX model_call_charges_user_time ON model_call_charges(user_id, created_at);
CREATE INDEX model_call_charges_period_time ON model_call_charges(period_id, created_at);
CREATE INDEX model_call_charges_pending ON model_call_charges(state, provider_cost_status);
CREATE INDEX model_call_charges_parent_task ON model_call_charges(cost_owner, user_id, parent_task_ref, created_at);
CREATE INDEX model_call_charges_learning_workspace ON model_call_charges(learning_workspace_id, created_at);
