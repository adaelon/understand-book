//! A byte-bounded observation buffer; socket writes never own the buffer or AppState lock.
use memory::boards::BoardDelivery;
use runtime::run_events::{RunActivity, RunEventSink, RuntimeEvent};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const EVENT_BYTES: usize = 256 * 1024;
#[derive(Debug, Clone, Serialize)]
pub struct RunDescriptor {
    pub book_id: String,
    pub session_id: String,
    pub turn_id: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct RunSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub music_command: Option<runtime::music::MusicCommand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_command: Option<runtime::presentation_interact::PresentationCommand>,
    pub descriptor: RunDescriptor,
    pub last_seq: u64,
    pub execution_state: String,
    pub persistence_state: String,
    pub activities: Vec<RunActivity>,
    pub reader_state: Option<Value>,
    pub effects: Vec<Value>,
    #[serde(default)]
    pub board_commits: Vec<BoardDelivery>,
    pub draft: Option<runtime::answer_stream::AnswerPatch>,
    pub final_view: Option<Value>,
    pub error: Option<Value>,
}
#[derive(Clone, Serialize)]
pub struct RunEvent {
    pub turn_id: String,
    pub seq: u64,
    pub elapsed_ms: f64,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: Value,
}
struct Buffer {
    bindings: Vec<runtime::orchestrator::SourceBinding>,
    snapshot: RunSnapshot,
    events: VecDeque<(RunEvent, usize)>,
    bytes: usize,
}
pub struct RunStream {
    pub(crate) music_commands: crate::music_interact::MusicCommands,
    pub(crate) presentation_commands: crate::presentation_interact::PresentationCommands,
    start: Instant,
    buffer: Mutex<Buffer>,
    changed: Condvar,
    byte_limit: usize,
    epoch: Option<String>,
}
impl RunStream {
    pub fn new(descriptor: RunDescriptor) -> Arc<Self> {
        let stream = Self::from_snapshot(RunSnapshot {
            music_command: None,
            presentation_command: None,
            descriptor,
            last_seq: 0,
            execution_state: "running".into(),
            persistence_state: "pending".into(),
            activities: Vec::new(),
            reader_state: None,
            effects: Vec::new(),
            board_commits: Vec::new(),
            draft: None,
            final_view: None,
            error: None,
        });
        stream.update("run.started", |_| (), |s| json!(s.descriptor));
        stream
    }
    pub fn from_snapshot(snapshot: RunSnapshot) -> Arc<Self> {
        Self::with_observation(snapshot, None, EVENT_BYTES)
    }
    pub fn with_observation(
        mut snapshot: RunSnapshot,
        epoch: Option<String>,
        byte_limit: usize,
    ) -> Arc<Self> {
        snapshot.error = snapshot.error.map(crate::agent_failure::value);
        snapshot.final_view = snapshot.final_view.map(crate::agent_failure::view);
        Arc::new(Self {
            music_commands: Default::default(),
            presentation_commands: Default::default(),
            start: Instant::now(),
            buffer: Mutex::new(Buffer {
                bindings: Vec::new(),
                snapshot,
                events: VecDeque::new(),
                bytes: 0,
            }),
            changed: Condvar::new(),
            byte_limit,
            epoch,
        })
    }
    pub fn queued(descriptor: RunDescriptor, boot: &str, byte_limit: usize) -> Arc<Self> {
        Self::with_observation(
            RunSnapshot {
                music_command: None,
                presentation_command: None,
                descriptor,
                last_seq: 0,
                execution_state: "queued".into(),
                persistence_state: "pending".into(),
                activities: vec![],
                reader_state: None,
                effects: vec![],
                board_commits: vec![],
                draft: None,
                final_view: None,
                error: None,
            },
            Some(format!("{boot}-{}", uuid::Uuid::now_v7())),
            byte_limit,
        )
    }
    pub fn dispatch_state(&self, state: &str) {
        self.update(
            "run.dispatch",
            |s| s.execution_state = state.into(),
            |s| json!({"execution_state":s.execution_state}),
        );
    }
    pub fn started(&self) {
        self.update(
            "run.started",
            |s| s.execution_state = "running".into(),
            |s| json!(s),
        );
    }
    pub fn resource_wait(&self, waiting: bool) {
        self.update(
            "run.resource",
            |s| {
                if matches!(s.execution_state.as_str(), "running" | "waiting_model") {
                    s.execution_state = if waiting { "waiting_model" } else { "running" }.into();
                }
            },
            |s| json!({"execution_state":s.execution_state}),
        );
    }
    pub fn cursor(&self, value: Option<&str>) -> Result<Option<u64>, read_tools::ToolError> {
        let Some(value) = value else {
            return Ok(None);
        };
        let (epoch, sequence) = value
            .rsplit_once(':')
            .map(|(e, s)| (Some(e), s))
            .unwrap_or((None, value));
        let sequence = sequence.parse::<u64>().map_err(|_| {
            crate::user_storage_paths::error(
                "INVALID_EVENT_CURSOR",
                "validation",
                "Invalid observation cursor",
            )
        })?;
        Ok(if epoch == self.epoch.as_deref() {
            Some(sequence)
        } else {
            None
        })
    }
    pub(crate) fn event_frame(&self, event: &RunEvent) -> String {
        let id = self
            .epoch
            .as_ref()
            .map(|e| format!("{e}:{}", event.seq))
            .unwrap_or_else(|| event.seq.to_string());
        let mut value = json!(event);
        if let Some(epoch) = &self.epoch {
            value["observation_epoch"] = json!(epoch);
            if event.event_type == "run.snapshot" {
                value["live_buffer_reset"] = json!(true);
            }
        }
        format!("id: {id}\nevent: {}\ndata: {value}\n\n", event.event_type)
    }
    pub fn source_binding(
        &self,
        book_id: &str,
        turn_id: &str,
        ref_id: &str,
    ) -> Option<runtime::orchestrator::SourceBinding> {
        let buffer = self.buffer.lock().unwrap();
        let s = &buffer.snapshot;
        if s.persistence_state != "pending"
            || s.descriptor.book_id != book_id
            || s.descriptor.turn_id != turn_id
        {
            return None;
        }
        let published = s
            .draft
            .as_ref()?
            .view
            .as_ref()?
            .sources
            .iter()
            .any(|source| source.source_ref_id == ref_id);
        published
            .then(|| {
                buffer
                    .bindings
                    .iter()
                    .find(|b| b.source_ref_id == ref_id)
                    .cloned()
            })
            .flatten()
    }
    pub fn snapshot(&self) -> RunSnapshot {
        self.buffer.lock().unwrap().snapshot.clone()
    }
    /// Seed a reconstructed stream before exposing it, without replaying old live events.
    pub fn restore_board_commits(&self, deliveries: &[BoardDelivery]) {
        let mut buffer = self.buffer.lock().unwrap();
        for delivery in deliveries {
            retain_board_commit(&mut buffer.snapshot, delivery);
        }
    }
    fn update(
        &self,
        kind: &str,
        apply: impl FnOnce(&mut RunSnapshot),
        payload: impl FnOnce(&RunSnapshot) -> Value,
    ) {
        self.update_if(kind, |snapshot| { apply(snapshot); true }, payload);
    }
    fn update_if(
        &self,
        kind: &str,
        apply: impl FnOnce(&mut RunSnapshot) -> bool,
        payload: impl FnOnce(&RunSnapshot) -> Value,
    ) {
        let mut buffer = self.buffer.lock().unwrap();
        if kind == "run.cancelling"
            && !matches!(
                buffer.snapshot.execution_state.as_str(),
                "running" | "waiting_model"
            )
        {
            return;
        }
        if !apply(&mut buffer.snapshot) {
            return;
        }
        buffer.snapshot.last_seq += 1;
        let event = RunEvent {
            turn_id: buffer.snapshot.descriptor.turn_id.clone(),
            seq: buffer.snapshot.last_seq,
            elapsed_ms: self.start.elapsed().as_secs_f64() * 1000.0,
            event_type: kind.into(),
            payload: payload(&buffer.snapshot),
        };
        let bytes = serde_json::to_vec(&event).unwrap().len();
        buffer.bytes += bytes;
        buffer.events.push_back((event, bytes));
        while buffer.bytes > self.byte_limit {
            if let Some((_, bytes)) = buffer.events.pop_front() {
                buffer.bytes -= bytes;
            }
        }
        self.changed.notify_all();
    }
    pub(crate) fn presentation_command(&self, command: Option<&runtime::presentation_interact::PresentationCommand>) {
        self.update("presentation.command", |s| s.presentation_command = command.cloned(), |s| json!(s.presentation_command));
    }

    pub(crate) fn music_command(&self, command: Option<&runtime::music::MusicCommand>) {
        self.update("music.command", |s| s.music_command = command.cloned(), |s| json!(s.music_command));
    }
    pub fn reader_changed(&self, state: Value) {
        self.update(
            "reader.changed",
            |s| s.reader_state = Some(state.clone()),
            |_| state.clone(),
        );
    }
    pub fn cancelling(&self) {
        // The coordinator serializes stop requests. Finalizing/terminal runs keep their actual state.
        self.update(
            "run.cancelling",
            |s| s.execution_state = "cancelling".into(),
            |_| Value::Null,
        );
    }
    pub fn finalizing(&self) {
        self.update(
            "run.finalizing",
            |s| s.execution_state = "finalizing".into(),
            |_| Value::Null,
        );
    }
    pub fn finish(&self, view: Option<Value>, error: Option<Value>) {
        let view = view.map(crate::agent_failure::view);
        let error = error.or_else(|| view.as_ref()
            .and_then(|view| view.get("board_delivery_error"))
            .filter(|error| error.is_object()).cloned()).map(crate::agent_failure::value);
        let (kind, execution, persistence) = if error.is_some() {
            ("run.persistence_failed", "ended", "failed")
        } else {
            match view.as_ref().and_then(|v| v["status"].as_str()) {
                Some("cancelled") => ("run.cancelled", "cancelled", "saved"),
                Some("failed") => ("run.failed", "failed", "saved"),
                _ => ("run.completed", "completed", "saved"),
            }
        };
        // Called after the history commit: only this boundary can finalize a draft.
        let previous = self.snapshot().draft;
        let canonical = if error.is_none() {
            view.as_ref()
                .and_then(|v| serde_json::from_value(v["outcome"]["answer_view"].clone()).ok())
        } else {
            None
        };
        let patch = runtime::answer_stream::AnswerPatch {
            message_id: previous.as_ref().map_or(0, |p| p.message_id),
            revision: previous.as_ref().map_or(0, |p| p.revision),
            operation: if execution == "completed" {
                "finalize"
            } else {
                "discard"
            }
            .into(),
            view: canonical,
        };
        self.answer_patch(patch);
        let committed = view.as_ref().and_then(|v| v.get("board_commits"))
            .and_then(|value| serde_json::from_value::<Vec<BoardDelivery>>(value.clone()).ok())
            .unwrap_or_default();
        self.update(
            kind,
            |s| {
                for delivery in &committed {
                    retain_board_commit(s, delivery);
                }
                s.execution_state = execution.into();
                s.persistence_state = persistence.into();
                s.draft = None;
                s.final_view = view;
                s.error = error;
            },
            |s| json!(s),
        );
    }
    /// Selection of snapshot(N) and cursor N occurs under the same mutex as publication.
    pub fn read_after(&self, after: Option<u64>, wait: Duration) -> (Vec<RunEvent>, bool) {
        let mut buffer = self.buffer.lock().unwrap();
        if after == Some(buffer.snapshot.last_seq) && buffer.snapshot.persistence_state == "pending"
        {
            buffer = self.changed.wait_timeout(buffer, wait).unwrap().0;
        }
        let snapshot = &buffer.snapshot;
        let terminal = snapshot.persistence_state != "pending";
        let covered = after.is_some_and(|seq| {
            seq <= snapshot.last_seq
                && (seq == snapshot.last_seq
                    || buffer
                        .events
                        .front()
                        .is_some_and(|(first, _)| seq + 1 >= first.seq))
        });
        let events = if covered {
            buffer
                .events
                .iter()
                .filter(|(event, _)| event.seq > after.unwrap())
                .map(|(event, _)| event.clone())
                .collect()
        } else {
            vec![RunEvent {
                turn_id: snapshot.descriptor.turn_id.clone(),
                seq: snapshot.last_seq,
                elapsed_ms: self.start.elapsed().as_secs_f64() * 1000.0,
                event_type: "run.snapshot".into(),
                payload: json!(snapshot),
            }]
        };
        (events, terminal)
    }
}
impl RunEventSink for RunStream {
    fn board_committed(&self, delivery: &BoardDelivery) {
        self.update_if(
            "board.committed",
            |snapshot| retain_board_commit(snapshot, delivery),
            |_| json!({"delivery": delivery}),
        );
    }
    fn effect_created(&self, _step_id: u32, effect: &runtime::orchestrator::AgentEffect) {
        self.update(
            "effect.created",
            |s| {
                let id = runtime::orchestrator::effect_id(effect);
                if let Some(old) = s.effects.iter_mut().find(|e| e["effect_id"] == id) {
                    *old = json!({"effect_id":id, "effect":effect});
                } else {
                    s.effects.push(json!({"effect_id":id, "effect":effect}));
                }
            },
            |s| s.effects.iter().find(|e| e["effect_id"] == runtime::orchestrator::effect_id(effect)).cloned().unwrap_or(Value::Null),
        );
    }
    fn source_bindings(&self, bindings: &[runtime::orchestrator::SourceBinding]) {
        self.buffer.lock().unwrap().bindings = bindings.to_vec();
    }
    fn answer_patch(&self, patch: runtime::answer_stream::AnswerPatch) {
        self.update(
            "answer.patch",
            |s| {
                if s.persistence_state == "pending" {
                    s.draft = runtime::answer_stream::apply_patch(s.draft.as_ref(), &patch);
                }
            },
            |_| json!(patch),
        );
    }

    fn emit(&self, event: RuntimeEvent) {
        let activity = event.activity;
        let phase = if activity.status == runtime::run_events::ActivityStatus::Running {
            "started"
        } else {
            "finished"
        };
        let kind = format!("{}.{}", activity.kind, phase);
        self.update(
            &kind,
            |s| {
                if let Some(old) = s
                    .activities
                    .iter_mut()
                    .find(|a| a.step_id == activity.step_id)
                {
                    *old = activity.clone();
                } else {
                    s.activities.push(activity.clone());
                }
            },
            |_| json!(activity),
        );
    }
}

fn retain_board_commit(snapshot: &mut RunSnapshot, delivery: &BoardDelivery) -> bool {
    if snapshot.board_commits.iter().any(|old| old.board_id == delivery.board_id
        && old.origin_session_id == delivery.origin_session_id
        && old.origin_turn_id == delivery.origin_turn_id
        && old.commit_id == delivery.commit_id) {
        return false;
    }
    snapshot.board_commits.push(delivery.clone());
    true
}

pub fn serve(request: tiny_http::Request, stream: Arc<RunStream>, after: Option<u64>) {
    serve_checked(request, stream, after, None);
}

/// A host revalidates permission while observing an ongoing stream.
pub trait ObservationAccess { fn allows(&self, descriptor: &RunDescriptor) -> bool; }

pub fn serve_authorized(
    request: tiny_http::Request,
    stream: Arc<RunStream>,
    after: Option<u64>,
    authorization: impl ObservationAccess + Send + 'static,
) {
    serve_checked(request, stream, after, Some(Box::new(authorization)));
}

fn serve_checked(
    request: tiny_http::Request,
    stream: Arc<RunStream>,
    mut after: Option<u64>,
    authorization: Option<Box<dyn ObservationAccess + Send>>,
) {
    std::thread::spawn(move || {
        let descriptor = stream.snapshot().descriptor;
        let allowed = || {
            authorization
                .as_ref()
                .is_none_or(|permit| permit.allows(&descriptor))
        };
        if !allowed() {
            let _ = request.respond(
                tiny_http::Response::from_string(
                    json!({"error_code":"OBJECT_NOT_FOUND","message":"Object is unavailable"})
                        .to_string(),
                )
                .with_status_code(404)
                .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap())
                .with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            );
            return;
        }
        let mut writer = request.into_writer();
        let result = (|| -> std::io::Result<()> {
            let chunked = authorization.is_some();
            writer.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Accel-Buffering: no\r\nConnection: close\r\n")?;
            if chunked {
                writer.write_all(b"Transfer-Encoding: chunked\r\n")?;
            }
            writer.write_all(b"\r\n")?;
            writer.flush()?;
            let mut last_write = Instant::now();
            loop {
                let (events, terminal) = stream.read_after(
                    after,
                    if authorization.is_some() {
                        Duration::from_secs(1)
                    } else {
                        Duration::from_secs(10)
                    },
                );
                if !allowed() {
                    break;
                }
                // Check access once per batch, including idle ticks, while keeping
                // network heartbeats independent of the revocation polling rate.
                let wrote = !events.is_empty() || last_write.elapsed() >= Duration::from_secs(10);
                if events.is_empty() && wrote {
                    write_observation(&mut writer, b": keepalive\n\n", chunked)?;
                }
                for event in events {
                    let frame = stream.event_frame(&event);
                    write_observation(&mut writer, frame.as_bytes(), chunked)?;
                    after = Some(event.seq);
                }
                if wrote {
                    writer.flush()?;
                    last_write = Instant::now();
                }
                if terminal {
                    break;
                }
            }
            if chunked {
                writer.write_all(b"0\r\n\r\n")?;
                writer.flush()?;
            }
            Ok(())
        })();
        let _ = result; // A disconnected observer never cancels or restarts execution.
    });
}

fn write_observation(writer: &mut impl Write, bytes: &[u8], chunked: bool) -> std::io::Result<()> {
    if chunked {
        write!(writer, "{:x}\r\n", bytes.len())?;
    }
    writer.write_all(bytes)?;
    if chunked {
        writer.write_all(b"\r\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn board_delivery(commit_id: &str, before_revision: u32) -> BoardDelivery {
        BoardDelivery {
            board_id: "board".into(),
            commit_id: Some(commit_id.into()),
            before_revision,
            after_revision: before_revision + 1,
            item_ids: vec![format!("item-{commit_id}")],
            block_ids: vec![format!("block-{commit_id}")],
            origin_session_id: "session".into(),
            origin_turn_id: "turn".into(),
            parent_item_id: Some("parent".into()),
        }
    }
    #[test]
    fn wb4_board_commits_survive_answer_repair_and_cancellation_without_duplicates() {
        let stream = RunStream::new(RunDescriptor {
            book_id: "book".into(), session_id: "session".into(), turn_id: "turn".into(),
        });
        let events = runtime::run_events::RunEvents::new(Some(stream.clone()));
        let first = board_delivery("part-one", 1);
        let second = board_delivery("part-two", 2);
        let cursor = stream.snapshot().last_seq;
        events.board_committed(&first);
        events.board_committed(&first);
        let (committed, terminal) = stream.read_after(Some(cursor), Duration::ZERO);
        assert!(!terminal);
        assert_eq!(committed.len(), 1);
        assert_eq!(committed[0].event_type, "board.committed");
        assert_eq!(committed[0].payload, json!({"delivery":first}));
        assert_eq!(stream.snapshot().last_seq, cursor + 1);

        stream.answer_patch(runtime::answer_stream::AnswerPatch {
            message_id: 1, revision: 0, operation: "replace".into(),
            view: Some(runtime::orchestrator::AgentAnswerView {
                parts: vec![runtime::orchestrator::AgentAnswerPart::Markdown { text: "下一段草稿".into() }],
                sources: vec![],
            }),
        });
        events.board_committed(&second);
        stream.answer_patch(runtime::answer_stream::AnswerPatch {
            message_id: 1, revision: 1, operation: "replace".into(), view: None,
        });
        stream.finish(Some(json!({"status":"cancelled"})), None);
        let snapshot = stream.snapshot();
        assert_eq!(snapshot.execution_state, "cancelled");
        assert!(snapshot.draft.is_none());
        assert_eq!(snapshot.board_commits, vec![first, second]);
    }
    #[test]
    fn wb4_restored_board_commits_survive_observation_overflow() {
        let first = board_delivery("part-one", 1);
        let second = board_delivery("part-two", 2);
        let initial = RunStream::new(RunDescriptor {
            book_id: "book".into(), session_id: "session".into(), turn_id: "turn".into(),
        });
        initial.board_committed(&first);
        let stream = RunStream::with_observation(initial.snapshot(), Some("restarted".into()), 512);
        let cursor = stream.snapshot().last_seq;
        stream.restore_board_commits(&[first.clone(), second.clone()]);
        stream.restore_board_commits(std::slice::from_ref(&second));
        assert_eq!(stream.snapshot().last_seq, cursor);
        assert!(stream.read_after(Some(cursor), Duration::ZERO).0.is_empty());
        stream.reader_changed(json!({"text":"x".repeat(1024)}));
        let (frames, _) = stream.read_after(Some(cursor), Duration::ZERO);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_type, "run.snapshot");
        assert_eq!(frames[0].payload["board_commits"], json!([first, second]));
        assert!(stream.buffer.lock().unwrap().bytes <= 512);
    }
    #[test]
    fn wb4_terminal_view_recovers_unobserved_board_commits_and_keeps_existing_parts() {
        let stream = RunStream::new(RunDescriptor {
            book_id: "book".into(), session_id: "session".into(), turn_id: "turn".into(),
        });
        let first = board_delivery("part-one", 1);
        assert!(stream.snapshot().board_commits.is_empty());
        stream.finish(Some(json!({"status":"failed", "board_commits":[first]})), None);
        let (frames, terminal) = stream.read_after(None, Duration::ZERO);
        assert!(terminal);
        assert_eq!(frames[0].payload["board_commits"], json!([first]));
        assert_eq!(stream.snapshot().board_commits, vec![first.clone()]);

        let second = board_delivery("part-two", 2);
        stream.board_committed(&second);
        stream.finish(Some(json!({"status":"completed", "board_commits":[first]})), None);
        assert_eq!(stream.snapshot().board_commits, vec![first.clone(), second.clone()]);
        stream.finish(None, Some(json!({"error_code":"STORE_ERROR"})));
        assert_eq!(stream.snapshot().board_commits, vec![first, second]);
    }
    #[test]
    fn board_delivery_error_cannot_publish_a_completed_saved_run() {
        let stream = RunStream::new(RunDescriptor {
            book_id: "book".into(), session_id: "session".into(), turn_id: "turn".into(),
        });
        let cursor = stream.snapshot().last_seq;
        let failure = json!({"error_code":"INTERNAL_ERROR", "category":"storage", "message":"Board write failed"});
        stream.finish(Some(json!({"status":"completed", "board_delivery_error":failure})), None);
        let (events, terminal) = stream.read_after(Some(cursor), Duration::ZERO);
        assert!(terminal);
        assert_eq!(stream.snapshot().execution_state, "ended");
        assert_eq!(stream.snapshot().persistence_state, "failed");
        assert_eq!(stream.snapshot().error, Some(json!({"error_code":"INTERNAL_ERROR", "category":"storage",
            "message":"结果保存失败，请重试保存。"})));
        assert!(!serde_json::to_string(&events.iter().map(|event| &event.payload).collect::<Vec<_>>()).unwrap().contains("Board write failed"));
        assert_eq!(events.last().unwrap().event_type, "run.persistence_failed");
        assert!(!events.iter().any(|event| event.event_type == "run.completed"));
        stream.finish(Some(json!({"status":"completed", "board_delivery":{"board_id":"board"}})), None);
        assert_eq!(stream.snapshot().persistence_state, "saved");
        assert_eq!(stream.snapshot().execution_state, "completed");
        assert!(stream.snapshot().error.is_none());
    }
    #[test]
    fn draft_snapshot_repair_and_commit_boundary_are_atomic() {
        use runtime::answer_stream::AnswerPatch;
        use runtime::orchestrator::{AgentAnswerPart, AgentAnswerView};
        let stream = RunStream::new(RunDescriptor {
            book_id: "book".into(),
            session_id: "session".into(),
            turn_id: "turn".into(),
        });
        let view = AgentAnswerView {
            parts: vec![AgentAnswerPart::Markdown {
                text: "草稿。".into(),
            }],
            sources: vec![],
        };
        stream.answer_patch(AnswerPatch {
            message_id: 1,
            revision: 0,
            operation: "replace".into(),
            view: Some(view.clone()),
        });
        stream.answer_patch(AnswerPatch {
            message_id: 1,
            revision: 0,
            operation: "append".into(),
            view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Markdown {
                    text: "补充。".into(),
                }],
                sources: vec![],
            }),
        });
        assert_eq!(
            stream
                .snapshot()
                .draft
                .as_ref()
                .unwrap()
                .view
                .as_ref()
                .unwrap()
                .parts
                .len(),
            1
        );
        stream.answer_patch(AnswerPatch {
            message_id: 1,
            revision: 0,
            operation: "replace".into(),
            view: Some(view.clone()),
        });
        let (snapshot, _) = stream.read_after(None, Duration::ZERO);
        assert_eq!(snapshot[0].payload["draft"]["view"], json!(view));
        stream.answer_patch(AnswerPatch {
            message_id: 1,
            revision: 1,
            operation: "replace".into(),
            view: None,
        });
        assert!(stream.snapshot().draft.unwrap().view.is_none());
        let cursor = stream.snapshot().last_seq;
        stream.finalizing();
        let (before_commit, _) = stream.read_after(Some(cursor), Duration::ZERO);
        assert!(before_commit
            .iter()
            .all(|e| e.payload["operation"] != "finalize"));
        let cursor = stream.snapshot().last_seq;
        stream.finish(
            Some(json!({"status":"completed", "outcome":{"answer_view":view}})),
            None,
        );
        let (after, terminal) = stream.read_after(Some(cursor), Duration::ZERO);
        assert!(terminal);
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].payload["operation"], "finalize");
        assert_eq!(after[0].payload["view"], json!(view));
        assert!(stream.snapshot().draft.is_none());
        let failed = RunStream::new(RunDescriptor {
            book_id: "book".into(),
            session_id: "session".into(),
            turn_id: "failed".into(),
        });
        let cursor = failed.snapshot().last_seq;
        failed.finish(None, Some(json!({"error_code":"STORE_ERROR"})));
        let (after, _) = failed.read_after(Some(cursor), Duration::ZERO);
        assert_eq!(after[0].payload["operation"], "discard");
        assert_eq!(after[1].event_type, "run.persistence_failed");
    }
    #[test]
    fn evicted_cursor_gets_atomic_snapshot_then_only_new_events() {
        let stream = RunStream::new(RunDescriptor {
            book_id: "book".into(),
            session_id: "session".into(),
            turn_id: "turn".into(),
        });
        let events = runtime::run_events::RunEvents::new(Some(stream.clone()));
        for i in 0..800 {
            let activity = events.begin("tool", "book.text", &format!("read {i}"), true);
            events.finish(
                activity,
                runtime::run_events::ActivityStatus::Succeeded,
                None,
                None,
                None,
            );
        }
        let (frames, _) = stream.read_after(Some(1), Duration::ZERO);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_type, "run.snapshot");
        assert_eq!(frames[0].payload["activities"], json!(events.activities()));
        let cursor = frames[0].seq;
        events.begin("model", "outer", "生成回答", true);
        let (next, _) = stream.read_after(Some(cursor), Duration::ZERO);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].seq, cursor + 1);
        assert!(stream.buffer.lock().unwrap().bytes <= EVENT_BYTES);
    }
    #[test]
    fn mu6d_epoch_and_overflow_reset_without_mixing_sequence_spaces() {
        let stream = RunStream::queued(
            RunDescriptor {
                book_id: "book".into(),
                session_id: "chat".into(),
                turn_id: "turn".into(),
            },
            "boot-one",
            512,
        );
        let initial = stream.read_after(None, Duration::ZERO).0.remove(0);
        let frame = stream.event_frame(&initial);
        let cursor = frame.lines().next().unwrap().strip_prefix("id: ").unwrap();
        assert_eq!(stream.cursor(Some(cursor)).unwrap(), Some(0));
        assert_eq!(stream.cursor(Some("old-boot:0")).unwrap(), None);
        assert_eq!(stream.cursor(Some("0")).unwrap(), None);
        assert!(stream.cursor(Some("bad:abc")).is_err());
        for n in 0..30 {
            stream.reader_changed(json!({"revision":n,"text":"x".repeat(100)}));
        }
        let (events, _) = stream.read_after(Some(0), Duration::ZERO);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "run.snapshot");
        assert!(stream
            .event_frame(&events[0])
            .contains("\"live_buffer_reset\":true"));
        let sequence = events[0].seq;
        stream.reader_changed(json!({"revision":31}));
        let (next, _) = stream.read_after(Some(sequence), Duration::ZERO);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].seq, sequence + 1);
        assert!(stream.buffer.lock().unwrap().bytes <= 512);
    }
}
