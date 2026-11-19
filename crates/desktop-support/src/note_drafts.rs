use serde_json::Value;
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, sync::Mutex};

pub type DraftMap = BTreeMap<String, Value>;

/// Device drafts belong to the local data directory, not the server's ephemeral origin.
pub struct NoteDraftStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl NoteDraftStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { path: data_dir.join("personal-note-drafts.json"), lock: Mutex::new(()) }
    }

    pub fn load_or_import(&self, initial: DraftMap) -> Result<DraftMap, String> {
        let _guard = self.lock.lock().map_err(|error| error.to_string())?;
        if let Some(saved) = self.read()? { return Ok(saved); }
        self.write(&initial)?;
        Ok(initial)
    }

    pub fn update(&self, draft_id: String, draft: Option<Value>) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|error| error.to_string())?;
        let mut saved = self.read()?.unwrap_or_default();
        if let Some(draft) = draft {
            if draft["draft_id"].as_str() != Some(draft_id.as_str())
                || !draft["owner"].is_string() || !draft["value"]["content"].is_string() {
                return Err("Invalid personal note draft".into());
            }
            saved.insert(draft_id, draft);
        } else {
            saved.remove(&draft_id);
        }
        self.write(&saved)
    }

    fn read(&self) -> Result<Option<DraftMap>, String> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| format!("Cannot read personal note drafts: {error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Cannot read personal note drafts: {error}")),
        }
    }

    fn write(&self, drafts: &DraftMap) -> Result<(), String> {
        let temporary = self.path.with_extension(format!("json.tmp-{}", std::process::id()));
        let result = (|| -> Result<(), String> {
            fs::create_dir_all(self.path.parent().unwrap()).map_err(|error| error.to_string())?;
            let bytes = serde_json::to_vec(drafts).map_err(|error| error.to_string())?;
            let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
            file.write_all(&bytes).map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
            drop(file);
            fs::rename(&temporary, &self.path).map_err(|error| error.to_string())
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result.map_err(|error| format!("Cannot save personal note drafts: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ub-note-drafts-{name}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }

    fn draft(id: &str, text: &str) -> Value {
        json!({"draft_id":id,"owner":"local","value":{"content":text},
            "pending":{"operation_id":"pending-save","value":{"content":text},"base_revision":3},
            "destination":{"kind":"attachment","label":"原文"},"saved_note":{"note_id":"saved"}})
    }

    #[test]
    fn native_drafts_survive_restart_and_empty_store_does_not_reimport_old_origin() {
        let root = directory("restart");
        let original = DraftMap::from([("draft".into(), draft("draft", "还没有写完的想法"))]);
        let first = NoteDraftStore::new(root.clone());
        assert_eq!(first.load_or_import(original.clone()).unwrap(), original);
        let restarted = NoteDraftStore::new(root.clone());
        assert_eq!(restarted.load_or_import(DraftMap::new()).unwrap(), original);
        restarted.update("draft".into(), None).unwrap();
        assert!(NoteDraftStore::new(root.clone()).load_or_import(original).unwrap().is_empty());
        assert_eq!(fs::read_to_string(root.join("personal-note-drafts.json")).unwrap(), "{}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn individual_updates_preserve_other_drafts_and_all_retry_fields() {
        let root = directory("update");
        let store = NoteDraftStore::new(root.clone());
        store.load_or_import(DraftMap::new()).unwrap();
        let first = draft("first", "第一条");
        let second = draft("second", "第二条");
        store.update("first".into(), Some(first.clone())).unwrap();
        store.update("second".into(), Some(second)).unwrap();
        store.update("second".into(), None).unwrap();
        assert_eq!(NoteDraftStore::new(root.clone()).load_or_import(DraftMap::new()).unwrap(),
            DraftMap::from([("first".into(), first)]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_or_failed_write_never_replaces_existing_drafts() {
        let root = directory("failed-write");
        let store = NoteDraftStore::new(root.clone());
        let original = DraftMap::from([("draft".into(), draft("draft", "保留原文"))]);
        store.load_or_import(original.clone()).unwrap();
        let temporary = store.path.with_extension(format!("json.tmp-{}", std::process::id()));
        fs::create_dir(&temporary).unwrap();
        assert!(store.update("draft".into(), None).is_err());
        assert_eq!(store.load_or_import(DraftMap::new()).unwrap(), original);
        fs::remove_dir(&temporary).unwrap();
        fs::write(&store.path, b"interrupted old file").unwrap();
        assert!(store.load_or_import(DraftMap::new()).is_err());
        assert!(store.update("draft".into(), Some(draft("draft", "new"))).is_err());
        assert_eq!(fs::read(&store.path).unwrap(), b"interrupted old file");
        fs::remove_dir_all(root).unwrap();
    }
}
