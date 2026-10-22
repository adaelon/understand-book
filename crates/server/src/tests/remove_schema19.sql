-- Only used when a test reconstructs an older schema from a current fixture.
-- Remove dependent commercial objects before dropping the older parent tables.
DROP VIEW billable_usage;
DROP TABLE web_usage_reconciliations;
DROP TABLE web_usage_receipts;
DROP TABLE web_usage;
DROP TABLE service_usage_receipts;
DROP TABLE service_usage;
DROP TABLE course_release_access;
DROP TABLE workspace_exam_sprints;
DROP TABLE cloud_storage_objects;
DROP TABLE cloud_storage_adoptions;
DROP TRIGGER cloud_plan_period_dates_immutable;
DROP TABLE cloud_plan_entitlements;
DROP TABLE cloud_plan_products;
DROP TABLE desktop_authorizations;
