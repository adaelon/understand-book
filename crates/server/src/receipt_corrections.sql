-- Append-only corrections to the external receipt fact, independent of grants.
CREATE TABLE receipt_corrections (
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    receipt_id TEXT NOT NULL,
    delta_fen INTEGER NOT NULL CHECK(delta_fen <> 0),
    reason TEXT NOT NULL CHECK(length(trim(reason)) > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id),
    FOREIGN KEY(actor, operation_id, user_id) REFERENCES admin_operations(actor, operation_id, user_id),
    FOREIGN KEY(receipt_id, user_id) REFERENCES manual_receipts(receipt_id, user_id)
) STRICT;
CREATE INDEX receipt_corrections_receipt ON receipt_corrections(receipt_id);
