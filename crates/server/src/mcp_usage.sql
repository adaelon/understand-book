-- MCP counts are independent of model-cost allowances and cash receipts.
CREATE TABLE mcp_free_months (
    user_id TEXT NOT NULL REFERENCES users(user_id),
    month_hk TEXT NOT NULL,
    granted_units INTEGER NOT NULL CHECK(granted_units >= 0),
    used_units INTEGER NOT NULL DEFAULT 0 CHECK(used_units >= 0 AND used_units <= granted_units),
    PRIMARY KEY(user_id, month_hk)
) STRICT;

CREATE TABLE mcp_free_gifts (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    month_hk TEXT NOT NULL,
    units INTEGER NOT NULL CHECK(units > 0),
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id),
    FOREIGN KEY(user_id, month_hk) REFERENCES mcp_free_months(user_id, month_hk)
) STRICT;

-- Publication identifiers are durable history, independent of publication retention.
CREATE TABLE mcp_usage (
    call_id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    tool TEXT NOT NULL,
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    units INTEGER NOT NULL CHECK(units = 1),
    debit_source TEXT NOT NULL CHECK(debit_source = 'free_month'),
    debit_ref TEXT NOT NULL,
    completed_at INTEGER NOT NULL,
    FOREIGN KEY(user_id, debit_ref) REFERENCES mcp_free_months(user_id, month_hk)
) STRICT;
CREATE INDEX mcp_usage_user_time ON mcp_usage(user_id, completed_at);
