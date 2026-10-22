//! Explicit public compositions over immutable material publications, without private learning state.
use crate::{admin_store::AdminStore, published_library::{PublicationManifest, PublishedBookRef}, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

fn invalid() -> ToolError { error("WORKSPACE_RELEASE_INVALID", "validation", "Provide an explicit public composition, exact materials and visible readers") }
fn storage() -> ToolError { error("WORKSPACE_RELEASE_STORAGE_FAILED", "unavailable", "Workspace release could not be read or published") }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Material {
    pub published_book_ref: PublishedBookRef,
    pub required_capabilities: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Composition {
    pub title: String,
    pub description: String,
    pub materials: Vec<PublishedBookRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalResource {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub name: String,
    pub publisher: String,
    pub public_goal: String,
    pub applicability: String,
    pub prerequisites: String,
    pub materials: Vec<Material>,
    pub topic_links: Vec<Composition>,
    pub suggested_route: Vec<Composition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_resources: Vec<ExternalResource>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Publish {
    pub release_id: String,
    pub expected_version: u32,
    pub visible_to_users: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tiers: Vec<String>,
    pub manifest: Manifest,
}
fn text(value: &mut String) -> Result<(), ToolError> {
    *value = value.trim().to_owned();
    if value.is_empty() { Err(invalid()) } else { Ok(()) }
}
impl Publish {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        crate::admin_store::operation_id(&self.release_id).map_err(|_| invalid())?;
        if self.expected_version == u32::MAX || (self.visible_to_users.is_empty() && self.allowed_tiers.is_empty()) || self.manifest.materials.is_empty() { return Err(invalid()); }
        self.allowed_tiers.sort();
        self.allowed_tiers.dedup();
        if self.allowed_tiers.iter().any(|tier| !matches!(tier.as_str(), "basic" | "plus" | "pro")) { return Err(invalid()); }
        self.visible_to_users.sort();
        self.visible_to_users.dedup();
        for user in &self.visible_to_users { crate::user_storage_paths::validate_user_id(user)?; }
        let manifest = &mut self.manifest;
        for value in [&mut manifest.name, &mut manifest.publisher, &mut manifest.public_goal, &mut manifest.applicability, &mut manifest.prerequisites] { text(value)?; }
        let mut references = BTreeSet::new();
        for material in &mut manifest.materials {
            for part in [&material.published_book_ref.book_id, &material.published_book_ref.publication_id] {
                if part.is_empty() || part.contains(['/', '\\', '%', ':']) { return Err(invalid()); }
            }
            if !references.insert(material.published_book_ref.clone()) || material.required_capabilities.is_empty() { return Err(invalid()); }
            for capability in &mut material.required_capabilities { text(capability)?; }
            material.required_capabilities.sort();
            material.required_capabilities.dedup();
        }
        for composition in manifest.topic_links.iter_mut().chain(manifest.suggested_route.iter_mut()) {
            text(&mut composition.title)?; text(&mut composition.description)?;
            if composition.materials.is_empty() || composition.materials.iter().any(|reference| !references.contains(reference)) { return Err(invalid()); }
        }
        for external in &mut manifest.external_resources {
            text(&mut external.title)?;
            text(&mut external.url)?;
            external.description = external.description.trim().to_owned();
            let url = url::Url::parse(&external.url).map_err(|_| invalid())?;
            if !matches!(url.scheme(), "http" | "https") { return Err(invalid()); }
        }
        Ok(())
    }
    pub(crate) fn apply(&self, db: &Connection, actor: &str, operation_id: &str, now: i64) -> Result<Value, ToolError> {
        let latest: u32 = db.query_row("SELECT COALESCE(MAX(version),0) FROM workspace_releases WHERE release_id=?", [&self.release_id], |r| r.get(0)).map_err(|_| storage())?;
        if latest != self.expected_version { return Err(error("WORKSPACE_RELEASE_VERSION_CONFLICT", "conflict", "The release has a newer version; review it before publishing")); }
        for user in &self.visible_to_users {
            let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)", [user], |r| r.get(0)).map_err(|_| storage())?;
            if !exists { return Err(error("WORKSPACE_RELEASE_READER_UNAVAILABLE", "validation", "A selected reader is unavailable")); }
        }
        let readiness = readiness(db, &self.manifest, &self.visible_to_users, &self.allowed_tiers)?;
        let version = latest + 1;
        db.execute("INSERT INTO workspace_releases(release_id,version,manifest_json,visible_to_users_json,readiness_json,published_by,operation_id,created_at) VALUES(?,?,?,?,?,?,?,?)", params![self.release_id,version,json!(self.manifest).to_string(),json!(self.visible_to_users).to_string(),readiness.to_string(),actor,operation_id,now]).map_err(|_| storage())?;
        db.execute("INSERT INTO course_release_access(release_id,version,allowed_tiers_json) VALUES(?,?,?)", params![self.release_id,version,json!(self.allowed_tiers).to_string()]).map_err(|_|storage())?;
        Ok(json!({"workspace_release":detail(db,None,&self.release_id,version,false)?}))
    }
}

// Publication import has already validated these immutable artifacts. Recheck the runtime
// products for each capability; an unrelated optional image cannot block text reading.
pub(crate) fn material_readiness(db: &Connection, material: &Material) -> Result<Value, ToolError> {
    let reference = &material.published_book_ref;
    let row: Option<(String,String)> = db.query_row("SELECT manifest,directory FROM book_publications WHERE book_id=? AND publication_id=?", params![reference.book_id,reference.publication_id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_| storage())?;
    let mut available = BTreeSet::new();
    let mut publication_available = false;
    if let Some((manifest,directory)) = row {
        let manifest: PublicationManifest = serde_json::from_str(&manifest).map_err(|_| storage())?;
        let directory = Path::new(&directory);
        let present = |file:&str| manifest.files.contains_key(file) && directory.join(file).is_file();
        publication_available = directory.is_dir() && present("base.json") && present("source.txt");
        if publication_available {
            available.insert("text".to_owned());
            if manifest.capabilities["music"] == true {
                if let Ok(music) = crate::music_media::load(directory) {
                    if crate::music_media::files(&music).iter().all(|file| present(file)) {
                        let capabilities = crate::music_media::capabilities(&music);
                        for name in ["music", "music_score", "music_events", "music_note_alignment"] {
                            if capabilities[name] == true { available.insert(name.into()); }
                        }
                    }
                }
            }
            let is_video = manifest.capabilities["video"] == true;
            let book = if is_video {
                // Catalog admission needs canonical text and transcript metadata, not the
                // graph, discourse contributions or their image contents.
                (|| {
                    let base = serde_json::from_slice(&std::fs::read(directory.join("base.json")).ok()?).ok()?;
                    let source = std::fs::read_to_string(directory.join("source.txt")).ok()?;
                    Some(read_tools::Book::new(base, &source))
                })()
            } else {
                directory.to_str().and_then(|path|read_tools::Book::load(path).ok())
            };
            if let Some((video, _)) = book.as_ref().filter(|_|is_video).and_then(|book|book.load_video_source(directory).ok()) {
                available.insert("video".to_owned());
                let catalog = std::fs::read(directory.join("video_enrichments.json")).ok()
                    .and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok());
                for coverage in &video.coverage {
                    let capability = serde_json::to_value(coverage.capability).map_err(|_|storage())?;
                    let name = capability.as_str().ok_or_else(storage)?;
                    let batches:Vec<_> = catalog.as_ref().and_then(|v|v["batches"].as_array()).into_iter().flatten()
                        .filter(|batch|batch["coverage"].as_array().is_some_and(|items|items.iter().any(|c|c["capability"]==capability))).collect();
                    if coverage.gaps(&video.source).is_ok_and(|g|g.is_empty())
                        && (name=="transcript" || (!batches.is_empty() && batches.iter().all(|batch|batch["files"].as_array()
                            .is_some_and(|files|files.iter().all(|file|file.as_str().is_some_and(present)))))) {
                        available.insert(name.to_owned());
                    }
                }
            }
            let tutor_ready = !is_video && book.as_ref()
                .is_some_and(|book|crate::tutor_api::tutor_source_readiness(book,directory)["status"]=="ready");
            if tutor_ready { available.insert("tutor".to_owned()); }
            let teaching_map = manifest.teaching_readiness["teaching_map_revision"].as_str().map(|revision|format!("teaching/versions/{revision}/map.json"));
            if manifest.teaching_readiness["status"] == "ready" && present("teaching_readiness.json") && teaching_map.is_some_and(|map|present(&map)) { available.insert("teaching_assets".to_owned()); }
            if let Some(capabilities) = manifest.capabilities.as_object() {
                for (name,capability) in capabilities {
                    if capability["status"] == "available" && ["artifact_path","report_path"].iter().all(|key|capability[*key].as_str().is_none_or(|file|present(file))) { available.insert(name.clone()); }
                }
            }
        }
    }
    let missing: Vec<_> = material.required_capabilities.iter().filter(|capability| !available.contains(*capability)).cloned().collect();
    Ok(json!({"published_book_ref":reference,"publication_available":publication_available,"available_capabilities":available,"missing_capabilities":missing}))
}
fn readiness(db: &Connection, manifest: &Manifest, readers: &[String], publishing_tiers: &[String]) -> Result<Value, ToolError> {
    let materials = manifest.materials.iter().map(|material| material_readiness(db,material)).collect::<Result<Vec<_>,_>>()?;
    let materials_ready = materials.iter().all(|material| material["publication_available"] == true && material["missing_capabilities"].as_array().is_some_and(Vec::is_empty));
    let mut audience = Vec::new();
    for user in readers {
        let course_allowed = !publishing_tiers.is_empty() && crate::course_access::current_tier(db,user,crate::multi_user_host::now())?.is_some_and(|tier|publishing_tiers.contains(&tier));
        let mut missing = Vec::new();
        for material in &manifest.materials {
            let reference = &material.published_book_ref;
            let allowed = course_allowed || crate::course_access::material_allowed(db,user,reference,crate::multi_user_host::now())?;
            if !allowed { missing.push(reference); }
        }
        audience.push(json!({"user_id":user,"ready":materials_ready && missing.is_empty(),"missing_grants":missing}));
    }
    let ready = materials_ready && audience.iter().all(|user|user["ready"] == true);
    Ok(json!({"status":if ready {"ready"} else {"incomplete"},"materials":materials,"audience":audience}))
}
pub(crate) fn detail(db:&Connection, owner:Option<&str>, id:&str, version:u32, export:bool)->Result<Value,ToolError> {
    let row: Option<(String,String,String,String,i64)> = db.query_row("SELECT manifest_json,visible_to_users_json,readiness_json,published_by,created_at FROM workspace_releases WHERE release_id=? AND version=?",params![id,version],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(|_|storage())?;
    let Some((manifest,readers,initial,publisher,created_at)) = row else {return Err(crate::authorization::missing())};
    let readers:Vec<String> = serde_json::from_str(&readers).map_err(|_|storage())?;
    if let Some(owner) = owner {
        let retained: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM workspace_release_instance_history h JOIN workspace_release_instances i USING(owner_user_id,learning_workspace_id) WHERE i.owner_user_id=? AND i.release_id=? AND h.version=?)",params![owner,id,version],|r|r.get(0)).map_err(|_|storage())?;
        if !readers.iter().any(|reader|reader==owner) && !retained && !crate::course_access::release_allowed(db,owner,id,version,crate::multi_user_host::now())? { return Err(crate::authorization::missing()); }
    }
    let manifest:Manifest = serde_json::from_str(&manifest).map_err(|_|storage())?;
    if export { return Ok(json!({"schema":"workspace_release.v1","release_id":id,"version":version,"manifest":manifest})); }
    let mut initial:Value = serde_json::from_str(&initial).map_err(|_|storage())?;
    let current_readers = owner.map(|owner|vec![owner.to_owned()]).unwrap_or_else(||readers.clone());
    if let Some(owner) = owner {
        if let Some(audience) = initial["audience"].as_array_mut() { audience.retain(|reader|reader["user_id"]==owner); }
        initial["status"] = json!(if initial["materials"].as_array().is_some_and(|materials| materials.iter().all(|material|material["publication_available"]==true && material["missing_capabilities"].as_array().is_some_and(Vec::is_empty))) && initial["audience"].as_array().is_some_and(|users|users.iter().all(|user|user["ready"]==true)) {"ready"} else {"incomplete"});
    }
    let availability = readiness(db,&manifest,&current_readers,&[])?;
    let latest_version:u32 = db.query_row("SELECT MAX(version) FROM workspace_releases WHERE release_id=?",[id],|r|r.get(0)).map_err(|_|storage())?;
    let archived = archived(db,id)?;
    let mut result = json!({"release_id":id,"version":version,"latest_version":latest_version,"manifest":manifest,"created_at":created_at,"publisher_user_id":publisher,"readiness_at_publication":initial,"availability":availability,"archived":archived});
    result["allowed_tiers"] = json!(crate::course_access::tiers(db,id,version)?);
    if owner.is_none() { result["visible_to_users"] = json!(readers); }
    Ok(result)
}
pub(crate) fn archived(db:&Connection, id:&str)->Result<bool,ToolError> {
    Ok(db.query_row("SELECT archived FROM workspace_release_archives WHERE release_id=?",[id],|r|r.get(0)).optional().map_err(|_|storage())?.unwrap_or(false))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Archive {
    pub release_id: String,
    pub archived: bool,
}
impl Archive {
    pub(crate) fn apply(&self,db:&Connection,actor:&str,operation_id:&str,now:i64)->Result<Value,ToolError> {
        let exists:bool = db.query_row("SELECT EXISTS(SELECT 1 FROM workspace_releases WHERE release_id=?)",[&self.release_id],|r|r.get(0)).map_err(|_|storage())?;
        if !exists { return Err(crate::authorization::missing()); }
        db.execute("INSERT INTO workspace_release_archives(release_id,archived,updated_at,actor,operation_id) VALUES(?,?,?,?,?) ON CONFLICT(release_id) DO UPDATE SET archived=excluded.archived,updated_at=excluded.updated_at,actor=excluded.actor,operation_id=excluded.operation_id",params![self.release_id,self.archived,now,actor,operation_id]).map_err(|_|storage())?;
        Ok(json!({"release_id":self.release_id,"archived":self.archived}))
    }
}
impl AdminStore {
    pub(crate) fn workspace_release(&self, owner:Option<&str>, id:&str, version:u32, export:bool)->Result<Value,ToolError> {
        let control = self.control.lock().unwrap();
        detail(&control.connection,owner,id,version,export)
    }
    pub(crate) fn workspace_releases(&self, owner:Option<&str>, limit:u32, offset:u32)->Result<Value,ToolError> {
        let control = self.control.lock().unwrap();
        let db = &control.connection;
        let tier = owner.map(|owner|crate::course_access::current_tier(db,owner,crate::multi_user_host::now())).transpose()?.flatten();
        let filter = "(?1 IS NULL OR EXISTS(SELECT 1 FROM json_each(visible_to_users_json) WHERE value=?1) OR EXISTS(SELECT 1 FROM course_release_access a,json_each(a.allowed_tiers_json) t WHERE a.release_id=workspace_releases.release_id AND a.version=workspace_releases.version AND t.value=?2))";
        let total:i64 = db.query_row(&format!("SELECT count(*) FROM workspace_releases WHERE {filter}"),params![owner,tier],|r|r.get(0)).map_err(|_|storage())?;
        let mut statement = db.prepare(&format!("SELECT release_id,version FROM workspace_releases WHERE {filter} ORDER BY created_at DESC,release_id,version DESC LIMIT ?3 OFFSET ?4")).map_err(|_|storage())?;
        let rows = statement.query_map(params![owner,tier,limit,offset],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?))).map_err(|_|storage())?.collect::<Result<Vec<_>,_>>().map_err(|_|storage())?;
        let items = rows.into_iter().map(|(id,version)|detail(db,owner,&id,version,false)).collect::<Result<Vec<_>,_>>()?;
        Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}))
    }
}
