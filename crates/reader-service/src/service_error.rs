//! Stable errors and timestamps shared by both hosts.
use read_tools::ToolError;
pub fn missing() -> ToolError {
    crate::user_storage_paths::error("OBJECT_NOT_FOUND", "not_found", "Object is unavailable")
}
pub fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub fn forbidden() -> ToolError {
    crate::user_storage_paths::error(
        "CAPABILITY_FORBIDDEN",
        "permission",
        "This operation is not available to readers",
    )
}
