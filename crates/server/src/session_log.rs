//! Single-writer JSONL persistence and pure projection (ADR-0152).
use crate::session_event::*;
use crate::*;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

pub(crate) struct SessionLog {
    pub path: PathBuf,
    pub projection: SessionProjection,
    committed_len: u64,
    uncertain: Option<(SessionEvent, Vec<u8>)>,
    #[cfg(test)]
    pub(crate) fail_write: Option<bool>,
    #[cfg(test)]
    pub(crate) fail_terminal_only: bool,
}

fn failure(path: &Path, offset: u64, detail: impl std::fmt::Display) -> ToolError {
    agent_history_load_error(path, "session_log", format!("byte={offset}: {detail}"))
}

impl SessionLog {
    /// Read-only open. A torn final line is ignored until the next writer repairs it.
    pub fn open(path: PathBuf) -> Result<Self, ToolError> {
        let (projection, committed_len) = read_prefix(&path, None)?;
        Ok(Self {
            path,
            projection,
            committed_len,
            uncertain: None,
            #[cfg(test)]
            fail_write: None,
            #[cfg(test)]
            fail_terminal_only: false,
        })
    }

    pub fn next_event(&self, at: &str, turn: Option<&str>, body: EventBody) -> SessionEvent {
        SessionEvent::new(self.projection.through_seq + 1, at, turn, body)
    }

    pub fn pending_event(&self) -> Option<SessionEvent> {
        self.uncertain.as_ref().map(|(event, _)| event.clone())
    }

    pub fn append(&mut self, event: SessionEvent) -> Result<(), ToolError> {
        #[cfg(test)]
        if self.fail_write.is_some()
            && (!self.fail_terminal_only || matches!(&event.body, EventBody::TurnFinished(_))) {
            let full = self.fail_write.take().unwrap();
            return self.append_with(event, |file, bytes| {
                file.write_all(if full { bytes } else { &bytes[..bytes.len()/2] })?;
                Err(std::io::Error::other("injected session append failure"))
            });
        }
        self.append_with(event, |file, bytes| {
            file.write_all(bytes)?;
            // File is unbuffered; flush documents the boundary and sync_all is
            // the durable acknowledgment before publishing the projection.
            file.flush()?;
            file.sync_all()
        })
    }

    fn append_with(
        &mut self,
        mut event: SessionEvent,
        write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    ) -> Result<(), ToolError> {
        sanitize(&mut event);
        let mut bytes =
            serde_json::to_vec(&event).map_err(|e| failure(&self.path, self.committed_len, e))?;
        bytes.push(b'\n');
        // Exact seq + payload permits a caller to retry an acknowledged event
        // after reopening. No tool is executed, no second record is written.
        if event.seq <= self.projection.through_seq {
            if read_event(&self.path, event.seq)?
                .as_ref()
                .is_some_and(|e| serde_json::to_value(e).ok() == serde_json::to_value(&event).ok())
            {
                return Ok(());
            }
            return Err(failure(
                &self.path,
                self.committed_len,
                "retry does not match committed record",
            ));
        }
        self.projection.validate(&event)?;
        if let Some((pending_event, pending_bytes)) = &self.uncertain {
            if serde_json::to_value(pending_event).ok() != serde_json::to_value(&event).ok() {
                return Err(failure(&self.path, self.committed_len,
                    "resolve the uncertain append before a different event"));
            }
            // Frozen scene maps can serialize in another field order after
            // decoding. Retry the original bytes, not a newly encoded line.
            event = pending_event.clone();
            bytes = pending_bytes.clone();
        }
        memory::ReaderPrivateStorageGate::enforce(&self.path)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|e| failure(&self.path, self.committed_len, e))?;
        file.seek(SeekFrom::Start(self.committed_len))
            .map_err(|e| failure(&self.path, self.committed_len, e))?;
        let mut tail = Vec::new();
        file.read_to_end(&mut tail)
            .map_err(|e| failure(&self.path, self.committed_len, e))?;
        self.uncertain = Some((event.clone(), bytes.clone()));
        if tail == bytes {
            // A complete write followed by a failed flush/sync: sync and adopt it.
            file.sync_all()
                .map_err(|e| failure(&self.path, self.committed_len, e))?;
        } else {
            if tail.contains(&b'\n') {
                return Err(failure(
                    &self.path,
                    self.committed_len,
                    "unexpected complete record after committed position",
                ));
            }
            file.set_len(self.committed_len)
                .map_err(|e| failure(&self.path, self.committed_len, e))?;
            file.seek(SeekFrom::Start(self.committed_len))
                .map_err(|e| failure(&self.path, self.committed_len, e))?;
            write(&mut file, &bytes).map_err(|e| failure(&self.path, self.committed_len, e))?;
        }
        self.committed_len += bytes.len() as u64;
        self.projection.apply(event);
        self.uncertain = None;
        Ok(())
    }

    pub fn at(&self, through_seq: u64) -> Result<SessionProjection, ToolError> {
        if through_seq > self.projection.through_seq {
            return Err(failure(
                &self.path,
                self.committed_len,
                "requested history position is not committed",
            ));
        }
        Ok(read_prefix(&self.path, Some(through_seq))?.0)
    }

    #[cfg(test)]
    pub fn frozen_messages(&self, position: &HistoryPosition) -> Result<Vec<Message>, ToolError> {
        match position {
            HistoryPosition::Committed {
                history_through_seq,
            } => {
                let s = self
                    .at(*history_through_seq)?
                    .session
                    .ok_or_else(|| failure(&self.path, 0, "missing history baseline"))?;
                if let Some(checkpoint) = &s.compaction_checkpoint {
                    runtime::project_compaction_checkpoint_messages(
                        &s.messages,
                        checkpoint,
                        &[],
                        runtime::compaction::COMPACTION_CONSUMPTION_WRAPPER,
                    )
                    .map_err(|e| failure(&self.path, 0, e.message))
                } else {
                    Ok(s.messages)
                }
            }

        }
    }
}

fn read_prefix(path: &Path, through: Option<u64>) -> Result<(SessionProjection, u64), ToolError> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((SessionProjection::default(), 0))
        }
        Err(e) => return Err(failure(path, 0, e)),
    };
    let mut reader = BufReader::new(file);
    let mut p = SessionProjection::default();
    let mut offset = 0;
    let mut bytes = Vec::new();
    while through.is_none_or(|n| p.through_seq < n) {
        bytes.clear();
        let n = reader
            .read_until(b'\n', &mut bytes)
            .map_err(|e| failure(path, offset, e))?;
        if n == 0 || bytes.last() != Some(&b'\n') {
            break;
        }
        let event = serde_json::from_slice(&bytes).map_err(|e| failure(path, offset, e))?;
        p.fold(event)
            .map_err(|e| failure(path, offset, e.message))?;
        offset += n as u64;
    }
    if through.is_some_and(|n| p.through_seq != n) {
        return Err(failure(path, offset, "history position is absent"));
    }
    Ok((p, offset))
}

fn read_event(path: &Path, seq: u64) -> Result<Option<SessionEvent>, ToolError> {
    let file = File::open(path).map_err(|e| failure(path, 0, e))?;
    let mut reader = BufReader::new(file);
    let mut bytes = Vec::new();
    let mut offset = 0;
    loop {
        bytes.clear();
        let n = reader
            .read_until(b'\n', &mut bytes)
            .map_err(|e| failure(path, offset, e))?;
        if n == 0 || bytes.last() != Some(&b'\n') {
            return Ok(None);
        }
        let e: SessionEvent =
            serde_json::from_slice(&bytes).map_err(|e| failure(path, offset, e))?;
        if e.seq == seq {
            return Ok(Some(e));
        }
        offset += n as u64;
    }
}

fn clean(messages: &mut [Message]) {
    runtime::presentation_author::redact_history(messages);
    runtime::tool_exposure::redact_history(messages);
}

pub(crate) fn sanitize(e: &mut SessionEvent) {
    match &mut e.body {
        EventBody::SessionCreated(c) => clean(&mut c.messages),
        EventBody::MessageAppended { messages } => clean(messages),
        EventBody::HistoryRevised(r) => clean(&mut r.suffix),
        EventBody::TurnFinished(f) => {
            if let Some(r) = &mut f.history_revision {
                clean(&mut r.suffix);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
