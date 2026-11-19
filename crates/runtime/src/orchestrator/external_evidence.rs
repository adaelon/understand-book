//! Source evidence and answer projection for one externally executed reading turn.
use super::*;
use crate::answer_stream::AnswerProjector;
use crate::provider_stream::{ModelDelta, ModelObserver};
use crate::run_events::RunEvents;
use serde_json::{json, Value};

pub struct ExternalEvidence {
    ledger: TurnEvidenceLedger,
    provenance: AnswerProvenanceLedger,
    preview: Option<AnswerProjector>,
    trace: Vec<TraceStep>,
}

impl ExternalEvidence {
    pub fn new(book: &Book, seed: Vec<EvidenceRange>, question: &str) -> Result<Self, ToolError> {
        let ledger = TurnEvidenceLedger::from_seed(book, seed)?;
        let mut provenance = AnswerProvenanceLedger::from_messages(&[Message::user(question)]);
        for entry in &ledger.evidence {
            let source = book.resolve_source(&entry.range, SOURCE_PRESENTATION_LOCALE, None)?;
            provenance.observe_public_text(
                &source.highlighted_quote,
                AnswerProvenanceChannel::SelectionEvidence,
            );
            for lid in [&entry.range.start_lid, &entry.range.end_lid] {
                provenance.observe_internal_locator(lid, AnswerProvenanceChannel::SelectionLocator);
            }
        }
        Ok(Self {
            ledger,
            provenance,
            preview: None,
            trace: Vec::new(),
        })
    }

    pub fn start_preview(&mut self, events: RunEvents) {
        self.preview = Some(AnswerProjector::new(
            events,
            false,
            &self.ledger.bindings(),
            &self.provenance,
            false,
        ));
    }

    pub fn push_text(&mut self, delta: &str) {
        if let Some(preview) = &mut self.preview {
            preview.observe(ModelDelta::Text(delta.into()));
        }
    }

    /// Only the host supplying the actual tool result calls this entry.
    pub fn observe(&mut self, book: &Book, name: &str, arguments: &Value, result: &Value) {
        let args = arguments.to_string();
        let output = result.to_string();
        observe_tool_evidence(&mut self.ledger, name, &args, &output, book);
        self.provenance.observe_tool_arguments(name, &args);
        self.provenance.observe_tool_result(name, &output);
        self.trace.push(TraceStep {
            model_tool_loop: None,
            tool: name.into(),
            args,
            result_digest: digest(&output),
            query_audit: None,
        });
        if let Some(preview) = &mut self.preview {
            preview.update_evidence(self.ledger.bindings(), &self.provenance);
        }
    }

    pub fn present(&mut self, book: &Book, arguments: &Value) -> Result<Value, ToolError> {
        let result = self.ledger.present(book, &arguments.to_string())?;
        let mut data = json!(result);
        data["marker"] = json!(format!("[[source:{}]]", result.source_ref_id));
        self.observe(book, "source.present", arguments, &data);
        Ok(data)
    }

    pub fn source_range(&self, id: &str) -> Result<&EvidenceRange, ToolError> {
        self.ledger
            .presented
            .iter()
            .find(|source| source.binding.source_ref_id == id)
            .and_then(|source| source.binding.evidence_range.as_ref())
            .ok_or_else(|| {
                source_presentation_error(
                    "SOURCE_NOT_OBSERVED",
                    "Use a source_ref_id returned by source_present in this turn.",
                )
            })
    }

    pub fn finish(&mut self, raw: &str) -> Result<OuterOutcome, ToolError> {
        let compiled = compile_agent_answer(raw, &self.ledger.bindings(), &self.provenance)
            .map_err(|error| {
                let code = error
                    .issues
                    .first()
                    .map(|issue| issue.error_code.as_str())
                    .unwrap_or("INVALID_AGENT_ANSWER");
                source_presentation_error(
                    code,
                    "本机 Agent 的回答或引用未能验证，请重新查证并回答。",
                )
            })?;
        self.preview.take();
        Ok(OuterOutcome {
            answer: Some(compiled.answer),
            answer_view: Some(compiled.view),
            source_bindings: compiled.bindings,
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: Vec::new(),
            trace: std::mem::take(&mut self.trace),
            profile_usage: Default::default(),
            memory_updates: Vec::new(),
            teaching_delivery_move_refs: Vec::new(),
            delivery_diagnostics: None,
            request_audit: Default::default(),
        })
    }
}

impl Drop for ExternalEvidence {
    fn drop(&mut self) {
        if let Some(preview) = &mut self.preview {
            preview.discard();
        }
    }
}
