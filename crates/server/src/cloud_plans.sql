-- C1. No production prices or quotas are seeded. Sold rights refer to the
-- existing allowance period and receipt; they never carry a second balance.
CREATE TABLE cloud_plan_products (
    product_id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    configuration_json TEXT NOT NULL CHECK(json_valid(configuration_json)),
    updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE cloud_plan_entitlements (
    period_id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    receipt_id TEXT NOT NULL UNIQUE,
    product_id TEXT NOT NULL,
    product_revision INTEGER NOT NULL CHECK(product_revision > 0),
    tier TEXT NOT NULL CHECK(tier IN ('basic','plus','pro')),
    duration TEXT NOT NULL CHECK(duration IN ('day1','day3','day7','month')),
    starts_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK(expires_at > starts_at),
    month_anchor_day INTEGER CHECK(month_anchor_day BETWEEN 1 AND 31),
    storage_bytes INTEGER NOT NULL CHECK(storage_bytes > 0),
    max_file_bytes INTEGER NOT NULL CHECK(max_file_bytes > 0 AND max_file_bytes <= storage_bytes),
    retention_seconds INTEGER NOT NULL CHECK(retention_seconds > 0),
    product_snapshot_json TEXT NOT NULL CHECK(json_valid(product_snapshot_json)),
    purchased_at INTEGER NOT NULL,
    FOREIGN KEY(period_id, user_id) REFERENCES allowance_periods(period_id, user_id),
    FOREIGN KEY(receipt_id, user_id) REFERENCES manual_receipts(receipt_id, user_id),
    CHECK((duration='month' AND month_anchor_day IS NOT NULL)
       OR (duration<>'month' AND month_anchor_day IS NULL))
) STRICT;
CREATE INDEX cloud_plan_entitlements_user_time
    ON cloud_plan_entitlements(user_id, starts_at, expires_at);

-- A purchased term is a receipt fact. Renewals create an adjacent new term;
-- the old management validity editor must not stretch a sold term.
CREATE TRIGGER cloud_plan_period_dates_immutable
BEFORE UPDATE OF starts_at, expires_at ON allowance_periods
WHEN EXISTS (SELECT 1 FROM cloud_plan_entitlements WHERE period_id=OLD.period_id)
 AND (NEW.starts_at<>OLD.starts_at OR NEW.expires_at<>OLD.expires_at)
BEGIN SELECT RAISE(ABORT, 'CLOUD_PLAN_PERIOD_IMMUTABLE'); END;
