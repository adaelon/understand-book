-- ADM5: preserve transport receipts and append manual corrections to charge facts.
ALTER TABLE model_call_charges ADD COLUMN revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0);
ALTER TABLE model_call_charges ADD COLUMN needs_reconciliation INTEGER NOT NULL DEFAULT 0 CHECK(needs_reconciliation IN (0,1));
ALTER TABLE model_call_charges ADD COLUMN send_identity TEXT CHECK(send_identity IS NULL OR json_valid(send_identity));
ALTER TABLE model_call_charges ADD COLUMN send_outcome TEXT CHECK(send_outcome IS NULL OR json_valid(send_outcome));
CREATE TABLE charge_reports (
    call_id TEXT NOT NULL REFERENCES model_call_charges(call_id),
    outcome_json TEXT NOT NULL CHECK(json_valid(outcome_json)),
    disposition TEXT NOT NULL CHECK(disposition IN ('accepted','conflict')),
    received_at INTEGER NOT NULL,
    PRIMARY KEY(call_id, outcome_json)
) STRICT;
CREATE TABLE charge_reconciliations (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    call_id TEXT NOT NULL REFERENCES model_call_charges(call_id),
    before_json TEXT NOT NULL CHECK(json_valid(before_json)),
    after_json TEXT NOT NULL CHECK(json_valid(after_json)),
    account_delta_micro_cny INTEGER NOT NULL,
    provider_delta_micro_cny INTEGER,
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    evidence TEXT NOT NULL CHECK(length(trim(evidence)) > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id)
) STRICT;
CREATE INDEX charge_reconciliations_call ON charge_reconciliations(call_id, created_at);
