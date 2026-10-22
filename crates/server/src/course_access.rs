//! Current subscription licenses for exact course publications; explicit grants remain independent.
use crate::{published_library::PublishedBookRef, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};

fn storage() -> ToolError {
    error("COURSE_ACCESS_UNAVAILABLE", "unavailable", "Course access could not be read")
}

pub(crate) fn tiers(db: &Connection, id: &str, version: u32) -> Result<Vec<String>, ToolError> {
    let json: Option<String> = db.query_row(
        "SELECT allowed_tiers_json FROM course_release_access WHERE release_id=? AND version=?",
        params![id, version], |r| r.get(0),
    ).optional().map_err(|_| storage())?;
    json.map(|value| serde_json::from_str(&value).map_err(|_| storage())).transpose()
        .map(|value| value.unwrap_or_default())
}

pub(crate) fn current_tier(db: &Connection, owner: &str, now: i64) -> Result<Option<String>, ToolError> {
    Ok(crate::cloud_plans::current(db, owner, now)?.map(|entitlement| entitlement.tier))
}

pub(crate) fn release_allowed(db: &Connection, owner: &str, id: &str, version: u32, now: i64) -> Result<bool, ToolError> {
    let Some(tier) = current_tier(db, owner, now)? else { return Ok(false) };
    Ok(tiers(db, id, version)?.contains(&tier))
}

pub(crate) fn material_allowed(db: &Connection, owner: &str, reference: &PublishedBookRef, now: i64) -> Result<bool, ToolError> {
    let enabled: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)", [owner], |r| r.get(0)).map_err(|_| storage())?;
    if !enabled { return Ok(false); }
    let explicit: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM book_grants WHERE owner_user_id=? AND book_id=? AND publication_id=?)", params![owner, reference.book_id, reference.publication_id], |r| r.get(0)).map_err(|_| storage())?;
    if explicit { return Ok(true); }
    let Some(tier) = current_tier(db, owner, now)? else { return Ok(false) };
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM workspace_releases r JOIN course_release_access a USING(release_id,version)
         WHERE EXISTS(SELECT 1 FROM json_each(a.allowed_tiers_json) WHERE value=?1)
         AND (NOT EXISTS(SELECT 1 FROM workspace_release_archives x WHERE x.release_id=r.release_id AND x.archived=1)
              OR EXISTS(SELECT 1 FROM workspace_release_instance_history h JOIN workspace_release_instances i USING(owner_user_id,learning_workspace_id)
                        WHERE i.owner_user_id=?2 AND i.release_id=r.release_id AND h.version=r.version))
         AND EXISTS(SELECT 1 FROM json_each(r.manifest_json,'$.materials') m
                    WHERE json_extract(m.value,'$.published_book_ref.book_id')=?3
                    AND json_extract(m.value,'$.published_book_ref.publication_id')=?4))",
        params![tier, owner, reference.book_id, reference.publication_id], |r| r.get(0),
    ).map_err(|_| storage())
}

/// Compute the small exact material set once for catalog queries; no permanent grants are added.
pub(crate) fn material_refs(db: &Connection, owner: &str, now: i64) -> Result<String, ToolError> {
    let Some(tier) = current_tier(db, owner, now)? else { return Ok("[]".into()) };
    let mut query = db.prepare(
        "SELECT DISTINCT json_extract(m.value,'$.published_book_ref') FROM workspace_releases r
         JOIN course_release_access a USING(release_id,version), json_each(r.manifest_json,'$.materials') m
         WHERE EXISTS(SELECT 1 FROM json_each(a.allowed_tiers_json) WHERE value=?1)
         AND (NOT EXISTS(SELECT 1 FROM workspace_release_archives x WHERE x.release_id=r.release_id AND x.archived=1)
              OR EXISTS(SELECT 1 FROM workspace_release_instance_history h JOIN workspace_release_instances i USING(owner_user_id,learning_workspace_id)
                        WHERE i.owner_user_id=?2 AND i.release_id=r.release_id AND h.version=r.version))",
    ).map_err(|_| storage())?;
    let refs = query.query_map(params![tier, owner], |r| r.get::<_, String>(0)).map_err(|_| storage())?
        .map(|r| r.map_err(|_| storage()).and_then(|r| serde_json::from_str::<PublishedBookRef>(&r).map_err(|_| storage())))
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&refs).map_err(|_| storage())
}
