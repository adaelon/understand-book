-- ADM1. UTC timestamps are Unix seconds. Money is stored as integers:
-- receipts in fen, allowances and model costs in micro_cny (1/1,000,000 yuan).
-- Operations store normalized business fields and receipts, never credentials.
CREATE TABLE admin_operations (
    actor TEXT NOT NULL REFERENCES users(user_id),
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    kind TEXT NOT NULL,
    parameters_json TEXT NOT NULL CHECK(json_valid(parameters_json)),
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    UNIQUE(actor, operation_id, user_id)
) STRICT;
CREATE INDEX admin_operations_user_time ON admin_operations(user_id, created_at);

CREATE TABLE allowance_periods (
    period_id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    starts_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK(expires_at > starts_at),
    closed_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    UNIQUE(period_id, user_id)
) STRICT;
CREATE INDEX allowance_periods_user_time ON allowance_periods(user_id, starts_at, expires_at);
-- Adjacent [start, end) periods are valid. Closing a period ends its eligibility;
-- its charges and adjustments remain attached for later settlement.
CREATE TRIGGER allowance_period_no_overlap_insert BEFORE INSERT ON allowance_periods
WHEN NEW.closed_at IS NULL AND EXISTS (
    SELECT 1 FROM allowance_periods p WHERE p.user_id=NEW.user_id AND p.closed_at IS NULL
    AND p.starts_at < NEW.expires_at AND NEW.starts_at < p.expires_at
)
BEGIN SELECT RAISE(ABORT, 'ALLOWANCE_PERIOD_OVERLAP'); END;
CREATE TRIGGER allowance_period_no_overlap_update
BEFORE UPDATE OF user_id, starts_at, expires_at, closed_at ON allowance_periods
WHEN NEW.closed_at IS NULL AND EXISTS (
    SELECT 1 FROM allowance_periods p WHERE p.user_id=NEW.user_id AND p.closed_at IS NULL
    AND p.period_id <> OLD.period_id
    AND p.starts_at < NEW.expires_at AND NEW.starts_at < p.expires_at
)
BEGIN SELECT RAISE(ABORT, 'ALLOWANCE_PERIOD_OVERLAP'); END;

CREATE TABLE manual_receipts (
    receipt_id TEXT PRIMARY KEY NOT NULL,
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    amount_fen INTEGER NOT NULL CHECK(amount_fen > 0),
    paid_at INTEGER NOT NULL,
    channel TEXT NOT NULL DEFAULT 'wechat' CHECK(length(trim(channel)) > 0),
    external_ref TEXT CHECK(external_ref IS NULL OR length(trim(external_ref)) > 0),
    note TEXT NOT NULL,
    UNIQUE(actor, operation_id),
    UNIQUE(channel, external_ref),
    UNIQUE(receipt_id, user_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id)
) STRICT;
CREATE INDEX manual_receipts_user_time ON manual_receipts(user_id, paid_at);

CREATE TABLE allowance_adjustments (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    period_id TEXT NOT NULL,
    delta_micro_cny INTEGER NOT NULL CHECK(delta_micro_cny <> 0),
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    receipt_id TEXT,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id),
    FOREIGN KEY(period_id, user_id) REFERENCES allowance_periods(period_id, user_id),
    FOREIGN KEY(receipt_id, user_id) REFERENCES manual_receipts(receipt_id, user_id)
) STRICT;
CREATE INDEX allowance_adjustments_period_time ON allowance_adjustments(period_id, created_at);

-- run_ref is the existing turn_id; task_ref identifies an account-owned background
-- task. These are durable references, not cascading foreign keys to chat content.
CREATE TABLE model_call_charges (
    call_id TEXT PRIMARY KEY NOT NULL,
    logical_call_id TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK(attempt >= 1),
    user_id TEXT NOT NULL,
    period_id TEXT NOT NULL,
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
    UNIQUE(logical_call_id, attempt),
    FOREIGN KEY(period_id, user_id) REFERENCES allowance_periods(period_id, user_id),
    CHECK(run_ref IS NOT NULL OR task_ref IS NOT NULL),
    CHECK((provider_cost_status='pending' AND provider_cost_micro_cny IS NULL)
       OR (provider_cost_status='confirmed' AND provider_cost_micro_cny IS NOT NULL)),
    CHECK((state IN ('reserved','sent','pending') AND account_debit_micro_cny IS NULL AND settled_at IS NULL)
       OR (state IN ('settled','released') AND account_debit_micro_cny IS NOT NULL AND settled_at IS NOT NULL)),
    CHECK(state <> 'settled' OR provider_cost_status='confirmed'),
    CHECK(state <> 'released' OR account_debit_micro_cny=0)
) STRICT;
CREATE INDEX model_call_charges_user_time ON model_call_charges(user_id, created_at);
CREATE INDEX model_call_charges_period_time ON model_call_charges(period_id, created_at);
CREATE INDEX model_call_charges_pending ON model_call_charges(state, provider_cost_status);
