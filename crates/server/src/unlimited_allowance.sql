-- Beta registrations retain the usage ledger without a monetary allowance cap.
ALTER TABLE allowance_periods ADD COLUMN unlimited INTEGER NOT NULL DEFAULT 0 CHECK(unlimited IN (0,1));
