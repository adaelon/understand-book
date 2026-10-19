-- Q3: a parent ceiling is admission metadata, never an extra charge.
CREATE TABLE build_task_budgets (
    task_ref TEXT PRIMARY KEY NOT NULL,
    cost_owner TEXT NOT NULL CHECK(cost_owner IN ('account','platform')),
    user_id TEXT REFERENCES users(user_id),
    learning_workspace_id TEXT,
    max_micro_cny INTEGER NOT NULL CHECK(max_micro_cny > 0),
    confirmation TEXT NOT NULL CHECK(length(trim(confirmation)) > 0),
    request_json TEXT NOT NULL CHECK(json_valid(request_json)),
    created_by TEXT NOT NULL REFERENCES users(user_id),
    created_at INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    FOREIGN KEY(user_id,learning_workspace_id) REFERENCES learning_workspaces(owner_user_id,learning_workspace_id),
    CHECK((cost_owner='account' AND user_id IS NOT NULL AND learning_workspace_id IS NOT NULL)
       OR (cost_owner='platform' AND user_id IS NULL AND learning_workspace_id IS NULL))
) STRICT;
CREATE TABLE build_budget_confirmations (
    task_ref TEXT NOT NULL REFERENCES build_task_budgets(task_ref),
    confirmation TEXT NOT NULL,
    request_json TEXT NOT NULL CHECK(json_valid(request_json)),
    revision INTEGER NOT NULL,
    confirmed_by TEXT NOT NULL REFERENCES users(user_id),
    confirmed_at INTEGER NOT NULL,
    PRIMARY KEY(task_ref,confirmation)
) STRICT;
CREATE TABLE external_build_executions (
    execution_id TEXT PRIMARY KEY NOT NULL,
    task_ref TEXT NOT NULL REFERENCES build_task_budgets(task_ref),
    granularity TEXT NOT NULL CHECK(granularity IN ('execution_total','calls')),
    created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE external_build_calls (
    call_id TEXT PRIMARY KEY NOT NULL REFERENCES model_call_charges(call_id),
    execution_id TEXT NOT NULL REFERENCES external_build_executions(execution_id),
    admission_json TEXT NOT NULL CHECK(json_valid(admission_json))
) STRICT;
CREATE INDEX external_build_executions_task ON external_build_executions(task_ref);
CREATE INDEX external_build_calls_execution ON external_build_calls(execution_id);
