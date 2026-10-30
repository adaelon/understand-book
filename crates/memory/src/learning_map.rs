//! Private learning map (ADR-0178 §7–§8): relationships this user was taught,
//! across books. Derived from delivered teaching, append-only and rebuildable.
//! It never stores learner state and never moves evidence between books.
use crate::learning::{storage, LearningStore};
use crate::teaching::{invalid, TeachingEvent};
use read_tools::ToolError;
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS learning_map_nodes (learning_workspace_id TEXT NOT NULL, node_id TEXT NOT NULL, statement TEXT NOT NULL,
    created_seq INTEGER NOT NULL, PRIMARY KEY(learning_workspace_id, node_id));
CREATE TABLE IF NOT EXISTS learning_map_sources (learning_workspace_id TEXT NOT NULL, node_id TEXT NOT NULL, source_id TEXT NOT NULL,
    source_revision TEXT NOT NULL, start_lid TEXT NOT NULL, end_lid TEXT NOT NULL, quote TEXT NOT NULL, delivery_ref TEXT NOT NULL,
    turn_id TEXT NOT NULL, publication_id TEXT,
    PRIMARY KEY(learning_workspace_id, node_id, source_id, source_revision, start_lid, end_lid, delivery_ref));
CREATE TABLE IF NOT EXISTS learning_map_chains (learning_workspace_id TEXT NOT NULL, chain_id TEXT NOT NULL, premises TEXT NOT NULL,
    conclusion TEXT NOT NULL, delivery_ref TEXT NOT NULL, PRIMARY KEY(learning_workspace_id, chain_id));
CREATE TABLE IF NOT EXISTS learning_map_coverage (learning_workspace_id TEXT NOT NULL, delivery_ref TEXT NOT NULL, done INTEGER NOT NULL,
    attempts INTEGER NOT NULL, PRIMARY KEY(learning_workspace_id, delivery_ref));
CREATE TABLE IF NOT EXISTS learning_map_note_removals (learning_workspace_id TEXT NOT NULL, note_id TEXT NOT NULL, node_id TEXT NOT NULL,
    PRIMARY KEY(learning_workspace_id, note_id, node_id));
";

/// Failed builds after which a delivery stays uncovered until a rebuild.
pub const MAX_BUILD_ATTEMPTS: i64 = 3;

/// One cited passage. LIDs are only meaningful with their source and revision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MapSource {
    pub source_id: String,
    pub source_revision: String,
    pub start_lid: String,
    pub end_lid: String,
    pub quote: String,
    pub delivery_ref: String,
    /// The delivering turn: a note written against it touches this relationship.
    pub turn_id: String,
    /// The immutable publication taught from, when there was one; pins the revision of a note's LID anchor.
    pub publication_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapNode {
    pub node_id: String,
    pub statement: String,
    pub sources: Vec<MapSource>,
}

/// Premises that jointly lead to the conclusion, as one delivered explanation stated them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapChain {
    pub chain_id: String,
    pub premises: Vec<String>,
    pub conclusion: String,
    pub delivery_ref: String,
}

/// One delivery's verified contribution. Existing nodes are only referenced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapMerge {
    pub delivery_ref: String,
    pub new_nodes: Vec<(String, String)>,
    pub sources: Vec<(String, MapSource)>,
    /// (premise node ids, conclusion node id)
    pub chains: Vec<(Vec<String>, String)>,
}

/// A new node's identity: stable for one delivery's proposition, so a retried build reuses it.
pub fn node_id(delivery_ref: &str, key: &str) -> String {
    format!("node:{:016x}", crate::fnv1a(&format!("{delivery_ref}\n{key}")))
}

/// Delete the derived map and its build progress. User removals of note
/// attachments are kept: they are the user's decisions, not derived data.
pub(crate) fn clear(connection: &Connection, workspace: &str) -> Result<(), ToolError> {
    for table in ["learning_map_nodes", "learning_map_sources", "learning_map_chains", "learning_map_coverage"] {
        connection.execute(&format!("DELETE FROM {table} WHERE learning_workspace_id=?"), params![workspace]).map_err(storage)?;
    }
    Ok(())
}

/// The same argument re-asserted elsewhere keeps one identity.
pub fn chain_id(premises: &[String], conclusion: &str) -> String {
    let premises: BTreeSet<&str> = premises.iter().map(String::as_str).collect();
    format!("chain:{:016x}", crate::fnv1a(&format!("{}=>{conclusion}", premises.into_iter().collect::<Vec<_>>().join("&"))))
}

impl LearningStore {
    pub fn learning_map_enabled(&self) -> Result<bool, ToolError> {
        Ok(self.state()?.learning_map_enabled)
    }

    /// Delivered explanations not yet built into the map, oldest first.
    /// Practice problems pose questions; they teach nothing to extract.
    pub fn uncovered_map_deliveries(&self, limit: u32) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT d.event FROM teaching_trace d WHERE d.learning_workspace_id=?1
            AND json_extract(d.event,'$.kind')='message_delivered'
            AND json_type(d.event,'$.payload.move.practice') IS NOT 'object'
            AND length(trim(coalesce(json_extract(d.event,'$.payload.answer'),'')))>0
            AND json_array_length(d.event,'$.payload.move.target.source_bindings')>0
            AND NOT EXISTS (SELECT 1 FROM learning_map_coverage c WHERE c.learning_workspace_id=?1 AND c.delivery_ref=d.event_id
                AND (c.done=1 OR c.attempts>=?2))
            ORDER BY d.seq LIMIT ?3").map_err(storage)?;
        let rows = query.query_map(params![self.workspace_scope, MAX_BUILD_ATTEMPTS, limit], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    pub fn record_map_failure(&mut self, delivery_ref: &str) -> Result<(), ToolError> {
        self.connection.execute("INSERT INTO learning_map_coverage(learning_workspace_id,delivery_ref,done,attempts) VALUES (?,?,0,1)
            ON CONFLICT(learning_workspace_id,delivery_ref) DO UPDATE SET attempts=attempts+1", params![self.workspace_scope, delivery_ref])
            .map_err(storage)?;
        Ok(())
    }

    pub fn map_nodes(&self) -> Result<Vec<MapNode>, ToolError> {
        let mut nodes: Vec<MapNode> = {
            let mut query = self.connection.prepare("SELECT node_id,statement FROM learning_map_nodes WHERE learning_workspace_id=? ORDER BY created_seq,node_id")
                .map_err(storage)?;
            let rows = query.query_map(params![self.workspace_scope], |r| Ok(MapNode { node_id: r.get(0)?, statement: r.get(1)?, sources: vec![] }))
                .map_err(storage)?;
            rows.collect::<Result<_, _>>().map_err(storage)?
        };
        let mut query = self.connection.prepare("SELECT node_id,source_id,source_revision,start_lid,end_lid,quote,delivery_ref,turn_id,publication_id FROM learning_map_sources
            WHERE learning_workspace_id=? ORDER BY node_id,source_id,start_lid").map_err(storage)?;
        let rows = query.query_map(params![self.workspace_scope], |r| Ok((r.get::<_, String>(0)?, MapSource {
            source_id: r.get(1)?, source_revision: r.get(2)?, start_lid: r.get(3)?, end_lid: r.get(4)?, quote: r.get(5)?, delivery_ref: r.get(6)?,
            turn_id: r.get(7)?, publication_id: r.get(8)? })))
            .map_err(storage)?;
        for row in rows {
            let (id, source) = row.map_err(storage)?;
            if let Some(node) = nodes.iter_mut().find(|n| n.node_id == id) { node.sources.push(source); }
        }
        Ok(nodes)
    }

    pub fn map_chains(&self) -> Result<Vec<MapChain>, ToolError> {
        let mut query = self.connection.prepare("SELECT chain_id,premises,conclusion,delivery_ref FROM learning_map_chains WHERE learning_workspace_id=? ORDER BY chain_id")
            .map_err(storage)?;
        let rows = query.query_map(params![self.workspace_scope], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))
            .map_err(storage)?;
        rows.map(|row| {
            let (chain_id, premises, conclusion, delivery_ref) = row.map_err(storage)?;
            Ok(MapChain { chain_id, premises: serde_json::from_str(&premises).map_err(storage)?, conclusion, delivery_ref })
        }).collect()
    }

    /// Append one delivery's contribution atomically. Re-merging is a no-op.
    pub fn merge_map(&mut self, merge: &MapMerge) -> Result<(), ToolError> {
        let known: BTreeSet<String> = self.map_nodes()?.into_iter().map(|n| n.node_id)
            .chain(merge.new_nodes.iter().map(|(id, _)| id.clone())).collect();
        if merge.sources.iter().any(|(id, _)| !known.contains(id))
            || merge.chains.iter().any(|(premises, conclusion)| premises.is_empty() || premises.contains(conclusion)
                || !known.contains(conclusion) || premises.iter().any(|p| !known.contains(p))) {
            return Err(invalid("学习脉络合并引用了不存在的关系，或推理链无效"));
        }
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let next: i64 = tx.query_row("SELECT coalesce(max(created_seq),0)+1 FROM learning_map_nodes WHERE learning_workspace_id=?",
            params![self.workspace_scope], |r| r.get(0)).map_err(storage)?;
        for (offset, (id, statement)) in merge.new_nodes.iter().enumerate() {
            tx.execute("INSERT OR IGNORE INTO learning_map_nodes(learning_workspace_id,node_id,statement,created_seq) VALUES (?,?,?,?)",
                params![self.workspace_scope, id, statement, next + offset as i64]).map_err(storage)?;
        }
        for (id, s) in &merge.sources {
            tx.execute("INSERT OR IGNORE INTO learning_map_sources(learning_workspace_id,node_id,source_id,source_revision,start_lid,end_lid,quote,delivery_ref,turn_id,publication_id)
                VALUES (?,?,?,?,?,?,?,?,?,?)", params![self.workspace_scope, id, s.source_id, s.source_revision, s.start_lid, s.end_lid, s.quote, s.delivery_ref,
                s.turn_id, s.publication_id])
                .map_err(storage)?;
        }
        for (premises, conclusion) in &merge.chains {
            let mut sorted = premises.clone();
            sorted.sort();
            sorted.dedup();
            tx.execute("INSERT OR IGNORE INTO learning_map_chains(learning_workspace_id,chain_id,premises,conclusion,delivery_ref) VALUES (?,?,?,?,?)",
                params![self.workspace_scope, chain_id(&sorted, conclusion), serde_json::to_string(&sorted).map_err(storage)?, conclusion, merge.delivery_ref])
                .map_err(storage)?;
        }
        tx.execute("INSERT INTO learning_map_coverage(learning_workspace_id,delivery_ref,done,attempts) VALUES (?,?,1,0)
            ON CONFLICT(learning_workspace_id,delivery_ref) DO UPDATE SET done=1", params![self.workspace_scope, merge.delivery_ref])
            .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// (deliveries built, deliveries given up after repeated failures)
    pub fn map_coverage(&self) -> Result<(u64, u64), ToolError> {
        self.connection.query_row("SELECT coalesce(sum(done=1),0), coalesce(sum(done=0 AND attempts>=?2),0) FROM learning_map_coverage
            WHERE learning_workspace_id=?1", params![self.workspace_scope, MAX_BUILD_ATTEMPTS], |r| Ok((r.get(0)?, r.get(1)?))).map_err(storage)
    }

    pub fn remove_note_attachment(&mut self, note_id: &str, node_id: &str) -> Result<(), ToolError> {
        self.connection.execute("INSERT OR IGNORE INTO learning_map_note_removals(learning_workspace_id,note_id,node_id) VALUES (?,?,?)",
            params![self.workspace_scope, note_id, node_id]).map_err(storage)?;
        Ok(())
    }

    pub fn removed_note_attachments(&self) -> Result<BTreeSet<(String, String)>, ToolError> {
        let mut query = self.connection.prepare("SELECT note_id,node_id FROM learning_map_note_removals WHERE learning_workspace_id=?").map_err(storage)?;
        let rows = query.query_map(params![self.workspace_scope], |r| Ok((r.get(0)?, r.get(1)?))).map_err(storage)?;
        rows.collect::<Result<_, _>>().map_err(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, LearningStore) {
        let root = tempfile::tempdir().unwrap();
        let store = LearningStore::open(&root.path().join("learning.sqlite"), false).unwrap();
        (root, store)
    }

    fn set(store: &mut LearningStore, operation: &str, enabled: bool) {
        let revision = store.state().unwrap().control.revision;
        store.mutate(&crate::learning::TutorMutation { operation_id: operation.into(), expected_revision: revision,
            action: crate::learning::TutorAction::SetLearningMap { enabled } }, "now").unwrap();
    }

    fn source(book: &str, lid: &str, delivery: &str) -> MapSource {
        MapSource { source_id: book.into(), source_revision: "r1".into(), start_lid: lid.into(), end_lid: lid.into(),
            quote: "quoted".into(), delivery_ref: delivery.into(), turn_id: "t".into(), publication_id: None }
    }

    #[test]
    fn merges_are_append_only_idempotent_and_rebuildable() {
        let (_root, mut store) = store();
        assert!(!store.learning_map_enabled().unwrap());
        set(&mut store, "map-on", true);
        assert!(store.learning_map_enabled().unwrap());
        let first = MapMerge { delivery_ref: "d1".into(),
            new_nodes: vec![("n1".into(), "Distance is rate times time.".into()), ("n2".into(), "Doubling time doubles distance at constant rate.".into())],
            sources: vec![("n1".into(), source("book-a", "1.1", "d1")), ("n2".into(), source("book-a", "1.2", "d1"))],
            chains: vec![(vec!["n1".into()], "n2".into())] };
        store.merge_map(&first).unwrap();
        store.merge_map(&first).unwrap();
        // Another book reaches the same relationship: a new source on the existing node, and the same argument keeps one id.
        store.merge_map(&MapMerge { delivery_ref: "d2".into(), new_nodes: vec![],
            sources: vec![("n2".into(), source("book-b", "1.1", "d2"))], chains: vec![(vec!["n1".into()], "n2".into())] }).unwrap();
        let nodes = store.map_nodes().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[1].sources.iter().map(|s| s.source_id.as_str()).collect::<Vec<_>>(), ["book-a", "book-b"]);
        assert_eq!(store.map_chains().unwrap().len(), 1);
        assert!(store.merge_map(&MapMerge { delivery_ref: "d3".into(), chains: vec![(vec!["missing".into()], "n2".into())], ..Default::default() }).is_err());
        assert!(store.merge_map(&MapMerge { delivery_ref: "d3".into(), chains: vec![(vec!["n2".into()], "n2".into())], ..Default::default() }).is_err());
        assert_eq!(store.map_coverage().unwrap(), (2, 0));
        store.remove_note_attachment("note-1", "n1").unwrap();
        set(&mut store, "map-off", false);
        assert!(!store.learning_map_enabled().unwrap());
        assert!(store.map_nodes().unwrap().is_empty() && store.map_chains().unwrap().is_empty());
        assert_eq!(store.map_coverage().unwrap(), (0, 0), "turning the map back on rebuilds every delivery");
        assert_eq!(store.removed_note_attachments().unwrap().len(), 1, "user decisions survive a rebuild");
    }

    #[test]
    fn chain_identity_ignores_premise_order() {
        assert_eq!(chain_id(&["b".into(), "a".into()], "c"), chain_id(&["a".into(), "b".into()], "c"));
        assert_ne!(chain_id(&["a".into()], "c"), chain_id(&["a".into()], "d"));
    }
}
