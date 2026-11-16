-- Only operational metadata. Questions, answers and source excerpts stay in private History.
CREATE TABLE operational_outcomes (
    user_id TEXT NOT NULL REFERENCES users(user_id),
    request_id TEXT NOT NULL,
    turn_id TEXT,
    book_id TEXT,
    accepted_at INTEGER NOT NULL,
    finished_at INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    stage TEXT NOT NULL,
    error_code TEXT,
    is_probe INTEGER NOT NULL CHECK(is_probe IN (0,1)),
    PRIMARY KEY(user_id,request_id)
) STRICT;
CREATE INDEX operational_outcomes_time ON operational_outcomes(finished_at,is_probe);
CREATE INDEX operational_outcomes_turn ON operational_outcomes(user_id,turn_id);
