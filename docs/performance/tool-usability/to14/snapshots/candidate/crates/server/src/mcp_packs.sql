-- One purchased batch has one receipt and the terms fixed at opening.
CREATE TABLE mcp_call_packs (
    pack_seq INTEGER PRIMARY KEY AUTOINCREMENT,
    pack_id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    receipt_id TEXT NOT NULL UNIQUE,
    pack_count INTEGER NOT NULL CHECK(pack_count > 0),
    unit_price_fen INTEGER NOT NULL CHECK(unit_price_fen > 0),
    units_per_pack INTEGER NOT NULL CHECK(units_per_pack > 0),
    price_fen INTEGER NOT NULL CHECK(price_fen > 0),
    total_units INTEGER NOT NULL CHECK(total_units > 0),
    adjusted_units INTEGER NOT NULL DEFAULT 0,
    used_units INTEGER NOT NULL DEFAULT 0 CHECK(used_units >= 0 AND used_units <= total_units + adjusted_units),
    opened_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL CHECK(expires_at > opened_at),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    UNIQUE(user_id, pack_id),
    FOREIGN KEY(receipt_id, user_id) REFERENCES manual_receipts(receipt_id, user_id)
) STRICT;
CREATE INDEX mcp_call_packs_debit ON mcp_call_packs(user_id, expires_at, opened_at, pack_seq);

CREATE TABLE mcp_pack_adjustments (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    pack_id TEXT NOT NULL,
    delta_units INTEGER NOT NULL CHECK(delta_units <> 0),
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id),
    FOREIGN KEY(user_id, pack_id) REFERENCES mcp_call_packs(user_id, pack_id)
) STRICT;

-- Preserve free consumption history while allowing either exact debit source.
CREATE TABLE mcp_usage_next (
    call_id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(user_id),
    tool TEXT NOT NULL,
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    units INTEGER NOT NULL CHECK(units = 1),
    debit_source TEXT NOT NULL CHECK(debit_source IN ('free_month','call_pack')),
    debit_ref TEXT NOT NULL,
    completed_at INTEGER NOT NULL,
    free_month_ref TEXT GENERATED ALWAYS AS (CASE WHEN debit_source='free_month' THEN debit_ref END) VIRTUAL,
    call_pack_ref TEXT GENERATED ALWAYS AS (CASE WHEN debit_source='call_pack' THEN debit_ref END) VIRTUAL,
    FOREIGN KEY(user_id, free_month_ref) REFERENCES mcp_free_months(user_id, month_hk),
    FOREIGN KEY(user_id, call_pack_ref) REFERENCES mcp_call_packs(user_id, pack_id)
) STRICT;
INSERT INTO mcp_usage_next(call_id,user_id,tool,book_id,publication_id,units,debit_source,debit_ref,completed_at)
    SELECT call_id,user_id,tool,book_id,publication_id,units,debit_source,debit_ref,completed_at FROM mcp_usage;
DROP TABLE mcp_usage;
ALTER TABLE mcp_usage_next RENAME TO mcp_usage;
CREATE INDEX mcp_usage_user_time ON mcp_usage(user_id, completed_at);
