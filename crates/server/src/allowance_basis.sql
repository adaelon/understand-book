-- Q1: commercial values are supplied explicitly; legacy periods remain unbound.
CREATE TABLE allowance_bases (
    version TEXT PRIMARY KEY NOT NULL CHECK(length(trim(version)) > 0),
    basis_micro_cny INTEGER NOT NULL CHECK(basis_micro_cny > 0)
) STRICT;
CREATE TABLE allowance_period_bases (
    period_id TEXT PRIMARY KEY NOT NULL REFERENCES allowance_periods(period_id),
    basis_version TEXT NOT NULL REFERENCES allowance_bases(version)
) STRICT;
CREATE TRIGGER allowance_basis_immutable BEFORE UPDATE ON allowance_bases
BEGIN SELECT RAISE(ABORT, 'ALLOWANCE_BASIS_IMMUTABLE'); END;
CREATE TRIGGER allowance_period_basis_immutable BEFORE UPDATE ON allowance_period_bases
BEGIN SELECT RAISE(ABORT, 'ALLOWANCE_BASIS_IMMUTABLE'); END;
