ALTER TABLE tutor_events RENAME TO tutor_events_legacy;
CREATE TABLE tutor_events (
    revision INTEGER PRIMARY KEY, operation_id TEXT NOT NULL,
    request TEXT NOT NULL, occurred_at TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    UNIQUE(learning_workspace_id, operation_id));
INSERT INTO tutor_events(revision, operation_id, request, occurred_at)
    SELECT revision, operation_id, request, occurred_at FROM tutor_events_legacy;
DROP TABLE tutor_events_legacy;

ALTER TABLE tutor_projection RENAME TO tutor_projection_legacy;
CREATE TABLE tutor_projection (
    id INTEGER NOT NULL CHECK(id=1), state TEXT NOT NULL,
    event_watermark INTEGER NOT NULL DEFAULT 0,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(learning_workspace_id, id));
INSERT INTO tutor_projection(id, state, event_watermark)
    SELECT id, state, (SELECT coalesce(max(revision),0) FROM tutor_events) FROM tutor_projection_legacy;
DROP TABLE tutor_projection_legacy;

DROP INDEX teaching_trace_session;
ALTER TABLE teaching_trace RENAME TO teaching_trace_legacy;
CREATE TABLE teaching_trace (
    seq INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT NOT NULL,
    session_id TEXT NOT NULL, event TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    UNIQUE(learning_workspace_id, event_id));
INSERT INTO teaching_trace(seq, event_id, session_id, event)
    SELECT seq, event_id, session_id, event FROM teaching_trace_legacy;
DROP TABLE teaching_trace_legacy;
CREATE INDEX teaching_trace_session ON teaching_trace(learning_workspace_id, session_id, seq);

ALTER TABLE teaching_assessments RENAME TO teaching_assessments_legacy;
CREATE TABLE teaching_assessments (
    action_ref TEXT NOT NULL, assessment TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(learning_workspace_id, action_ref));
INSERT INTO teaching_assessments(action_ref, assessment)
    SELECT action_ref, assessment FROM teaching_assessments_legacy;
DROP TABLE teaching_assessments_legacy;

DROP INDEX learning_evidence_source;
ALTER TABLE learning_evidence RENAME TO learning_evidence_legacy;
CREATE TABLE learning_evidence (
    seq INTEGER PRIMARY KEY AUTOINCREMENT, evidence_id TEXT NOT NULL,
    source_id TEXT NOT NULL, evidence TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    UNIQUE(learning_workspace_id, evidence_id));
INSERT INTO learning_evidence(seq, evidence_id, source_id, evidence)
    SELECT seq, evidence_id, source_id, evidence FROM learning_evidence_legacy;
DROP TABLE learning_evidence_legacy;
CREATE INDEX learning_evidence_source ON learning_evidence(learning_workspace_id, source_id, seq);

ALTER TABLE learner_projection RENAME TO learner_projection_legacy;
CREATE TABLE learner_projection (
    source_id TEXT NOT NULL, projection TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(learning_workspace_id, source_id));
INSERT INTO learner_projection(source_id, projection)
    SELECT source_id, projection FROM learner_projection_legacy;
DROP TABLE learner_projection_legacy;
