-- PN4: an admitted personal run has no material or Reader workspace.
CREATE TABLE personal_run_admissions (
 owner_user_id TEXT NOT NULL REFERENCES users(user_id), client_request_id TEXT NOT NULL,
 turn_id TEXT NOT NULL, chat_session_id TEXT NOT NULL, workspace_id TEXT,
 workspace_generation INTEGER NOT NULL, book_id TEXT, publication_id TEXT,
 dispatch_state TEXT NOT NULL CHECK(dispatch_state IN ('preparing','queued','claimed','settled','admission_failed')),
 boot_id TEXT, attempt INTEGER NOT NULL DEFAULT 0,
 cancel_requested INTEGER NOT NULL DEFAULT 0, key_closed INTEGER NOT NULL DEFAULT 0, unsaved INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(owner_user_id,client_request_id), UNIQUE(owner_user_id,turn_id),
 FOREIGN KEY(owner_user_id,workspace_id) REFERENCES reader_workspaces(owner_user_id,workspace_id),
 FOREIGN KEY(book_id,publication_id) REFERENCES book_publications(book_id,publication_id)
);
INSERT INTO personal_run_admissions SELECT * FROM run_admissions;
DROP TABLE run_admissions;
ALTER TABLE personal_run_admissions RENAME TO run_admissions;
CREATE INDEX admission_dispatch ON run_admissions(dispatch_state,owner_user_id);
