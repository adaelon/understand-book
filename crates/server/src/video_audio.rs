//! Cloud accounting adapter for the shared bounded audio-review pipeline.
pub use reader_service::video_audio::*;
use base_schema::PublishedBookRef;
use read_tools::{Book, ToolError};
use runtime::{run_context::CancellationToken, video_audio::{AudioReview, ReviewAudio}};
use std::path::Path;

struct RunAudioUsage<'a> {
    spend: &'a crate::model_spend_store::ModelSpendStore,
    scope: &'a crate::run_scope::RunScope,
}

impl AudioReviewUsagePort for RunAudioUsage<'_> {
    fn before_recognition(&self, quantity_ms: i64) -> Result<String, ToolError> {
        self.spend.service_admit_run(self.scope, quantity_ms, crate::multi_user_host::now())
    }
    fn after_recognition(&self, receipt: AudioReviewUsageReceipt) -> Result<(), ToolError> {
        let status = match receipt.status {
            AudioRecognitionStatus::Completed => "completed",
            AudioRecognitionStatus::Unknown => "unknown",
            AudioRecognitionStatus::NotStarted => "not_started",
        };
        self.spend.service_receipt(&self.scope.turn_id, crate::service_usage::ServiceReceipt {
            usage_id: receipt.usage_id, status: status.into(),
            quantity_ms: receipt.quantity_ms, evidence: receipt.evidence,
        }, crate::multi_user_host::now()).map(|_| ())
    }
}

pub(crate) fn review_for_run_metered(scope: &crate::run_scope::RunScope, request: ReviewAudio,
    cancel: &CancellationToken, spend: Option<&crate::model_spend_store::ModelSpendStore>) -> Result<AudioReview, ToolError> {
    let usage = spend.map(|spend| RunAudioUsage { spend, scope });
    review_for_run_with_usage(scope, request, cancel, usage.as_ref().map(|port| port as &dyn AudioReviewUsagePort))
}

pub(crate) fn review_audio_metered(book: &Book, directory: &Path, publication: &PublishedBookRef,
    run_id: &str, request: ReviewAudio, cancel: &CancellationToken, config: &AudioConfig,
    metering: Option<(&crate::model_spend_store::ModelSpendStore, &crate::run_scope::RunScope)>) -> Result<AudioReview, ToolError> {
    let usage = metering.map(|(spend, scope)| RunAudioUsage { spend, scope });
    review_audio_with_usage(book, directory, publication, run_id, request, cancel, config,
        usage.as_ref().map(|port| port as &dyn AudioReviewUsagePort))
}
