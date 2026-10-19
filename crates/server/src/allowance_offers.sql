-- Q4. Operators explicitly configure prices and percentage grants; no default price.
CREATE TABLE allowance_offers (
    offer_id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    amount_fen INTEGER NOT NULL CHECK(amount_fen > 0),
    delta_milli_percent INTEGER NOT NULL CHECK(delta_milli_percent > 0),
    basis_version TEXT NOT NULL REFERENCES allowance_bases(version),
    instructions TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    updated_at INTEGER NOT NULL
) STRICT;
