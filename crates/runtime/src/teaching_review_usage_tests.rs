fn unfinished_review() -> AdapterError {
    AdapterError { spend_stop: None, message: "Provider unfinished response: length".into() }
}

#[test]
fn teaching_review_usage_preserves_reported_failure_and_unknown_usage() {
    for reported_usage in [Some(58_016), None] {
        let mut adapter = ReviewedTeachingAdapter::new(vec![], vec![Err(unfinished_review())]);
        adapter.reported_review_usage = reported_usage;
        let failure = crate::tutor::review_delivery(&adapter, adapter.model_runtime_profile(),
            serde_json::json!({"original_request":"Explain the relation.",
                "proposed_delivery":{"text":"The catalog retains the card omitted from this route."}}), Vec::new()).unwrap_err();
        assert_eq!(failure.reported_usage, reported_usage, "unknown usage must remain unknown rather than become a measured zero");
        assert_eq!(failure.error.message, "Provider unfinished response: length");
        assert!(failure.error.spend_stop.is_none());
    }
}
